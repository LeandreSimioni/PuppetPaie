

//! L'engagement : un salarie, un spectacle (ou un contrat paye au mois), des lignes de paie datees. Il est au centre :
//! il porte la saisie, ses paies se calculent ici, une par mois qu'il couvre, et c'est d'elles que tout document se
//! fabrique. Le droit (`droit`) juge ce qu'on lui passe ; l'engagement l'appelle, jamais l'inverse.
//!
//! SEUL L'ENGAGEMENT CALCULE. Le calcul (`calcul`, `charger_donnees`), les cumuls, les conges, les minima, le solde et
//! les periodes sont prives a la crate. Ce qui est public, ce sont les types qu'on lit (`Paie`, `Engagement`, le
//! compteur de conges) et les gestes : valider, modifier et relire (`etats`, `stock`, `relire`, `departs`, `numeros`),
//! et, pour l'ecran seulement (la fonctionnalite Cargo `ecran`, que seul `moteur` demande), le calcul d'une saisie
//! (`pour_l_ecran`), viser un montant (`viser`), la saisie lue sans paie et le preavis. Un document ou une declaration
//! ne lit que les paies ecrites d'un engagement valide.

pub mod cachet;
pub mod calcul;
pub mod carte;
pub mod conges;
pub mod cumul;
pub mod duree;
pub mod departs;
pub mod ecarts;
pub mod empreinte;
pub mod heures_en_plus;
pub mod etats;
pub mod importe;
pub mod ligne;
pub mod imprevus;
pub mod maintien;
pub mod minima;
pub mod periodes;
pub mod relire;
pub mod saisie;
pub mod solde;
pub mod numeros;
pub mod paie_ecrite;
pub mod paies;
pub mod stock;
pub mod versions;
#[cfg(feature = "ecran")]
pub mod viser;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use socle::forme::NombreLu;

use droit::catalogue::rubrique;
use droit::contexte::{Contexte, IdSalarie, IdSpectacle};
use droit::natures::{self, Nature};
pub use ligne::{Chiffre, DateDeLigne, LignePaie};
use socle::date::Jour;
use socle::mois::Mois;
use droit::emplois;

use socle::num::{dec_f64, Decimal, Resultat};

#[derive(Debug, Clone)]
pub struct Engagement {
    /// La clé de la fiche du salarié, telle que la carte l'écrit.
    pub salarie: IdSalarie,
    /// Le salarié tel qu'une phrase d'écran le nomme (« NOM Prénom »).
    pub nom: String,
    /// La clé de la fiche du spectacle, telle que la carte l'écrit ; `None` : un contrat payé au mois n'en a pas.
    pub spectacle: Option<IdSpectacle>,
    pub dates_travail: Vec<Jour>,
    pub lignes: Vec<LignePaie>,
    pub debut: Jour,
    /// `None` seulement pour un contrat permanent ouvert.
    pub fin: Option<Jour>,
    /// Le numero du contrat (S21.G00.40.009) : `numeros::former` le donne a la validation ; `None` avant.
    pub numero: Option<numeros::NumeroContrat>,
    pub mois: String,
    pub planning: Vec<Jour>,
    pub planning_lieux: BTreeMap<Jour, String>,
    pub jours: i64,
    pub jours_contrat: i64,
    /// Le jour du versement que la saisie dit pour CET engagement (« Payé le » : un paiement fait un autre jour), ou
    /// `None` : celui que la compagnie a choisi (`versement`).
    pub date_paiement: Option<String>,
    /// Le jour du versement que la compagnie a choisi dans sa configuration (`droit::versement`), lu au dernier jour du
    /// mois de la paie (`engagements`) ; `Engagement::nouveau` seul : le dernier jour du mois.
    pub versement: droit::versement::Versement,
    /// Le taux de la premiere ligne de cachet ; sans cachet, celui que l'engagement ou son mois ecrit ; sinon `None`
    /// (regle 1 : aucun taux par defaut, aucun texte n'en fixe).
    pub taux_cachet: Option<f64>,
    pub cachets: i64,
    pub heures: Option<f64>,
    pub lieu: String,
    pub lieux: BTreeMap<Jour, String>,
    pub emploi: Option<String>,
    /// Le libelle lisible du metier de CET engagement (`emplois::resoudre`, au
    /// feminin si la fiche le dit) ; `None` si l'engagement ne choisit pas de metier.
    pub emploi_affichage: Option<String>,
    pub pcs_ese: Option<String>,
    pub code_emploi_ft: Option<String>,
    pub statut: Option<String>,
    pub statut_conventionnel: Option<String>,
    pub statut_categoriel_rc: Option<String>,
    /// L'etablissement (NIC) ou le salarie est rattache pour CE contrat, quand la compagnie en a plusieurs ; `None` : le
    /// principal. La fiche du salarie le porte pour la paie et les declarations du contrat (`calcul::fiche_de_l_engagement`).
    pub etablissement: Option<String>,
    /// La cle de sa nature de contrat (`droit::natures`), toujours dite (regle 6).
    pub nature: String,
    pub quotite_contrat: Option<f64>,
    pub quotite_reference_saisie: Option<f64>,
    pub modalite_temps_travail_saisie: Option<String>,
    pub motif_recours: Option<String>,
    pub motif_rupture: Option<String>,
    pub notification_rupture: Option<String>,
    pub preavis: Option<saisie::Preavis>,
    /// Le dernier jour travaille et paye au salaire habituel que la saisie dit (`dernier_jour_paye`, AAAA-MM-JJ) ;
    /// ce que la DSN en declare se lit par `fctu::dernier_jour_paye`.
    pub dernier_jour_paye: Option<String>,
    /// Le jour ou prend effet ce que la saisie de CE mois change au contrat (avenant : nature, motif, duree du
    /// travail, emploi, date de debut ; AAAA-MM-JJ) : la « Date de la modification » du bloc 41 (`dsn::changements_contrat`).
    /// Il ne se recopie pas d'un mois a l'autre (`moteur::routes::mois::DEFINIT_LE_MENSUEL`) : le mois suivant declare deja
    /// la nouvelle valeur.
    pub date_modification: Option<String>,
    pub transaction: Option<String>,
    pub suspensions: Vec<imprevus::Suspension>,
    /// Les termes du contrat AVANT l'avenant que `date_modification` date en cours de mois (`salaire_mensuel`,
    /// `quotite_contrat`), quand la saisie les dit : la paie du mois se decoupe alors au jour de l'avenant
    /// (`periodes::bornes`). Comme `date_modification`, il ne se recopie pas d'un mois a l'autre.
    pub avant: Option<saisie::Avant>,
    /// Une periode elementaire de paie : les bornes de CE calcul quand la paie du mois est decoupee
    /// (`periodes`) ; `None` : la periode de paie entiere (le contrat borne au mois).
    pub periode: Option<(Jour, Jour)>,
    /// Ce que la saisie dit du contrat ECRIT et que rien d'autre ne porte (`ContratEcrit`) : la periode
    /// d'essai (L1242-12, 6° ; R1221-34, 7°), la personne remplacee (L1242-12, 1°), la repartition des horaires d'un
    /// temps partiel (L3123-6, 1°), les clauses particulieres de ce contrat. Vide : rien n'est dit.
    pub ecrit: ContratEcrit,
}

/// CE QUE LA SAISIE DIT DU CONTRAT ECRIT et que seul le contrat ecrit lit (`Engagement::ecrit`), chaque champ seulement
/// quand il est dit.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContratEcrit {
    pub periode_essai_jours: Option<Chiffre>,
    pub personne_remplacee: Option<String>,
    pub repartition_horaire: Option<String>,
    pub clauses_particulieres: Option<String>,
}

impl ContratEcrit {
    /// Rien n'est dit.
    pub fn est_vide(&self) -> bool {
        *self == ContratEcrit::default()
    }
}

/// CE DONT UN ENGAGEMENT SE FABRIQUE (`Engagement::nouveau`) : la carte du mois telle que `engagements` l'assemble (le
/// metier resolu, les jours tires des lignes, les lignes deduites d'un contrat paye au mois), ou telle qu'un oracle la
/// donne. Chaque cle s'ecrit ; `null` : rien n'est dit.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assemble {
    pub salarie: Option<IdSalarie>,
    pub spectacle: Option<IdSpectacle>,
    pub dates_travail: Vec<DateDeLigne>,
    pub planning: Vec<DateDeLigne>,
    pub lignes: Option<Vec<LignePaie>>,
    pub mois: Option<String>,
    pub debut: Option<String>,
    pub fin: Option<String>,
    pub numero: Option<String>,
    pub nature: Option<String>,
    pub taux_cachet: Option<Chiffre>,
    pub jours: Option<Chiffre>,
    pub jours_contrat: Option<Chiffre>,
    pub date_paiement: Option<String>,
    pub lieu: Option<String>,
    pub emploi: Option<String>,
    pub emploi_affichage: Option<String>,
    pub pcs_ese: Option<String>,
    pub code_emploi_ft: Option<String>,
    pub statut: Option<String>,
    pub statut_conventionnel: Option<String>,
    pub statut_categoriel_rc: Option<String>,
    pub etablissement: Option<String>,
    pub quotite_contrat: Option<Chiffre>,
    pub quotite_reference_saisie: Option<Chiffre>,
    pub modalite_temps_travail_saisie: Option<String>,
    pub motif_recours: Option<String>,
    pub motif_rupture: Option<String>,
    pub notification_rupture: Option<String>,
    pub preavis: Option<saisie::Preavis>,
    pub dernier_jour_paye: Option<String>,
    pub date_modification: Option<String>,
    pub transaction: Option<String>,
    pub avant: Option<saisie::Avant>,
    pub periode_essai_jours: Option<Chiffre>,
    pub personne_remplacee: Option<String>,
    pub repartition_horaire: Option<String>,
    pub clauses_particulieres: Option<String>,
}

/// Un texte dit et non vide.
fn dit(x: &Option<String>) -> bool {
    x.as_deref().is_some_and(|s| !s.is_empty())
}

/// Un entier dit (un nombre sans decimale) ; un autre nombre, ou un texte, se refuse.
fn entier_dit(v: &Option<Chiffre>, nom: &str) -> Resultat<Option<i64>> {
    match v {
        None => Ok(None),
        Some(Chiffre::Nombre(n)) => n
            .as_i64()
            .or_else(|| n.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64))
            .map(Some)
            .ok_or_else(|| socle::echec::a_la_cle("Un nombre entier est attendu.", nom, n)),
        Some(autre) => Err(socle::echec::a_la_cle("Un nombre entier est attendu.", nom, socle::date::repr_texte(&autre.en_texte()))),
    }
}

/// Les jours d'une liste de dates, et les lieux qu'elles disent (sans blancs aux bords).
fn jours_et_lieux(liste: &[DateDeLigne]) -> Resultat<(Vec<Jour>, BTreeMap<Jour, String>)> {
    let (mut jours, mut lieux) = (Vec::new(), BTreeMap::new());
    for x in liste {
        let jour = x.lu()?;
        if let Some(l) = x.lieu().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()) {
            lieux.insert(jour, l);
        }
        jours.push(jour);
    }
    Ok((jours, lieux))
}

impl Engagement {
    /// « NOM Prénom, contrat N » : qui une phrase de refus désigne.
    pub fn qui(&self) -> String {
        match self.numero.as_deref() {
            Some(n) => format!("{}, contrat {}", self.nom, n),
            None => format!("{}, contrat sans numero", self.nom),
        }
    }

    /// L'engagement que `v` décrit (`Assemble`). `racine` est celle des donnees de la compagnie, pour le catalogue des
    /// rubriques. `qui` : le salarie tel qu'un refus le nomme (`salarie_nomme`).
    pub fn nouveau(racine: &Path, v: &Assemble, qui: &str) -> Resultat<Engagement> {
        let (mut jours_joues, mut lieux) = jours_et_lieux(&v.dates_travail)?;
        jours_joues.sort();
        let presents: BTreeSet<Jour> = jours_joues.iter().copied().collect();
        lieux.retain(|j, _| presents.contains(j));

        let Some(lignes_brutes) = &v.lignes else {
            return Err(socle::echec::cle("Un engagement se saisit en lignes de paie, et celui-ci n'en porte pas.", "lignes"));
        };
        for l in lignes_brutes {
            l.verifier_pourcentage()?;
        }
        let lignes: Vec<LignePaie> = lignes_brutes.clone();

        // Un refus nomme le salarie : c'est lui que la personne voit sur la carte.
        let qui = qui.to_string();
        // Le mois dit ; vide, il n'est pas dit.
        let mois_saisi: String = v.mois.iter().cloned().collect();
        let debut_saisi = dit(&v.debut);
        let fin_saisie = dit(&v.fin);
        if lignes.is_empty() && !(debut_saisi && fin_saisie) {
            return Err(socle::echec::cle(
                "Un engagement sans aucune ligne de paie est un mois du contrat sans cachet : il dit le début et la fin du contrat, rien ne permet de les déduire.",
                "debut",
            ));
        }
        if lignes.is_empty() && mois_saisi.is_empty() {
            return Err("Un engagement sans aucune ligne de paie doit dire son mois.".to_string());
        }

        // Un contrat permanent ne se deduit pas de ses dates.
        // Une nature se dit, elle ne se devine pas (regle 6).
        let Some(nature) = v.nature.clone().filter(|n| !n.trim().is_empty()) else {
            return Err(socle::echec::cle(format!("{}{}", qui, PAS_DE_NATURE), "nature"));
        };
        let nat = natures::nature(&nature)?;
        let mensuel = nat.mensuelle();
        let planning_brut = &v.planning;
        if mensuel {
            if !jours_joues.is_empty() || !planning_brut.is_empty() {
                return Err(format!(
                    "{} : un contrat payé au mois ({}) ne porte pas de jours joués ; il dit sa durée du travail, en \
                     heures par mois, et ses dates de début et de fin. Les jours d'absence se saisissent en lignes \
                     d'absence.",
                    qui,
                    nat.libelle
                ));
            }
            if !debut_saisi {
                return Err(format!(
                    "{} : saisissez la date de début de son {}, celle du contrat signé, sous « Contrat ».\n\
                     Elle identifie le contrat dans la déclaration (S21.G00.40.001) : elle ne se déduit de rien.",
                    qui,
                    nat.libelle
                ));
            }
            if mois_saisi.is_empty() {
                return Err(format!(
                    "{} ({}) : le mois de paie n'est pas dit. Un contrat payé au mois n'a pas de jour joué d'où le \
                     tirer.",
                    qui,
                    nat.libelle
                ));
            }
            if !fin_saisie && !nat.duree_indeterminee {
                return Err(format!(
                    "{} : saisissez la date de fin de son {}, sous « Contrat ».\n\
                     Un contrat à durée déterminée dit sa fin : la déclaration l'exige (S21.G00.40.010, CCH-12).",
                    qui,
                    nat.libelle
                ));
            }
        }

        // Ce que la declaration compte : cachets et heures, derives des lignes.
        let mut cachets_somme = 0.0;
        let mut autres = 0.0;
        let mut premier_cachet: Option<&LignePaie> = None;
        // Les rubriques se lisent a la fin du mois paye (le mois dit, sinon celui de la premiere date jouee). Un mois
        // illisible se refuse plus loin, avec ce qui ne va pas : d'ici la, les rubriques se lisent au jour du moteur.
        let jour_lu = match (Mois::du_texte(&mois_saisi), jours_joues.first()) {
            (Some(m), _) => m.dernier_jour(),
            (None, Some(j)) => j.mois_civil().dernier_jour(),
            (None, None) => droit::contexte::jour_reel(),
        };
        for l in &lignes {
            let r = rubrique(racine, l.rubrique(), &jour_lu)?;
            let q = socle::num::flottant(l.quantite().ok_or_else(|| socle::fichiers::absent("quantite"))?);
            if r.unite == "cachet" {
                cachets_somme += q;
                premier_cachet.get_or_insert(l);
            } else {
                // Le signe porte aussi sur les heures, comme au calcul
                // (`calcul::lignes_brut`) : une absence en retire. Un acompte,
                // au forfait et sans heures, n'en retire aucune.
                let signe = if r.sens == "-" { -1.0 } else { 1.0 };
                autres += signe * r.heures(q, l.heures_lues());
            }
        }
        let cachets = cachets_somme as i64;
        // Les heures de travail remunere d'un contrat paye au mois : la quotite payee moins les absences, comptees au reel
        // (`imprevus::lignes_du_mois`). Au reel, un mois entier d'absence (154 h en septembre 2026) depasse la quotite
        // lissee (151,67 h) : l'activite remuneree est alors 0.00, jamais negative -- « Pour un individu entierement
        // absent sur le mois, avec une valeur a « 0.00 » », et « Si une DSN contient une activite negative sur la periode
        // courante, elle sera rejetee par l'assurance chomage » (fiche DSN 2640, `atelier/reference/sources/suspension-
        // non-remuneree/net-entreprises-fiche2640-volumes-activite-s21g00-53-20260929.txt`, « Regles generales »).
        let autres = if mensuel { autres.max(0.0) } else { autres };
        let heures = if autres != 0.0 { Some(autres) } else { None };
        // Le taux du cachet : celui de la premiere ligne de cachet ; sans cachet, celui qui est ecrit ; sinon aucun. Un
        // contrat qui ne paie aucun cachet n'a pas de taux de cachet, et le moteur n'en invente pas (regle 1).
        let mut taux_cachet = v.taux_cachet.as_ref().map(Chiffre::flottant).transpose()?;
        if let Some(l) = premier_cachet {
            taux_cachet = Some(socle::num::flottant(l.taux().ok_or_else(|| socle::fichiers::absent("taux"))?));
        }

        // Les jours travailles : un fait qui se saisit, sinon les dates distinctes.
        let jours = match entier_dit(&v.jours, "jours")? {
            Some(j) => j,
            None => presents.len() as i64,
        };
        if jours < 0 || (!lignes.is_empty() && !mensuel && jours == 0) {
            return Err(format!(
                "{} : {} jour(s) travaillé(s) pour {} ligne(s) de paie. Cochez les jours de ses lignes.\n\
                 Les jours partent dans la déclaration (bloc 53, unité 12) et dans l'AEM, et ce sont eux qui ouvrent \
                 les droits : un mois payé en compte au moins un.",
                qui,
                jours,
                lignes.len()
            ));
        }

        // Le planning se lit comme les dates jouees, et se range a part.
        let (brut, planning_lieux) = jours_et_lieux(planning_brut)?;
        let mut planning: Vec<Jour> = Vec::new();
        for j in brut {
            if !planning.contains(&j) {
                planning.push(j);
            }
        }
        planning.sort();
        let debut = if let (true, Some(t)) = (debut_saisi, &v.debut) {
            Jour::lire(t)?
        } else {
            *jours_joues.first().ok_or_else(|| socle::fichiers::trop_courte("les jours joués"))?
        };
        let fin = if let (true, Some(t)) = (fin_saisie, &v.fin) {
            Some(Jour::lire(t)?)
        } else if mensuel {
            None
        } else {
            let derniere = *jours_joues.last().ok_or_else(|| socle::fichiers::trop_courte("les jours joués"))?;
            Some(planning.iter().copied().fold(derniere, Jour::max))
        };
        // LA FIN D'UN CONTRAT SANS TERME est sa rupture, et elle ne vaut qu'a sa date : le mois paye avant celui ou elle
        // tombe ne la connait pas. Un CDI n'a pas de date de fin previsionnelle (S21.G00.40.010 n'est exigee que d'un
        // contrat a duree determinee, CCH-12, `atelier/reference/norme-dsn/texte/bloc-S21.G00.40.txt`, l. 517-527) ; sa
        // fin se declare dans la DSN du mois ou il finit (bloc S21.G00.62). La paie d'un mois d'avant ne change donc pas
        // quand la rupture se saisit.
        let fin = match fin {
            Some(f) if nat.duree_indeterminee && f > Mois::lire(&mois_saisi)?.dernier_jour() => None,
            autre => autre,
        };

        let debut_mois_de = |m: &str| Mois::lire(m).map(Mois::premier_jour);
        let fin_mois_de = |m: &str| Mois::lire(m).map(Mois::dernier_jour);
        if !mensuel && lignes.is_empty() {
            let fin = fin.unwrap();
            if !(debut < debut_mois_de(&mois_saisi)? || fin > fin_mois_de(&mois_saisi)?) {
                return Err(format!(
                    "{} : son contrat du {} au {} n'a aucune ligne de paie. Ajoutez-en une, ou retirez cet \
                     engagement : un mois sans cachet n'existe que pour un contrat qui déborde du mois.",
                    qui,
                    debut.jj_mm_aaaa(),
                    fin.jj_mm_aaaa()
                ));
            }
        }
        if let Some(f) = fin {
            if debut > f {
                return Err(format!(
                    "{} : la fin de son contrat ({}) précède son début ({}) : corrigez l'une des deux dates, sous \
                     « Contrat ».",
                    qui,
                    f.jj_mm_aaaa(),
                    debut.jj_mm_aaaa()
                ));
            }
        }

        // Les dates jouees tiennent dans le mois, quand le mois est dit.
        let hors: Vec<Jour> = jours_joues
            .iter()
            .filter(|j| !mois_saisi.is_empty() && j.mois_civil().to_string() != mois_saisi)
            .copied()
            .collect();
        let mois = if mois_saisi.is_empty() {
            jours_joues.first().ok_or_else(|| socle::fichiers::trop_courte("les jours joués"))?.mois_civil().to_string()
        } else {
            mois_saisi
        };
        let mut dates_contrat: Vec<Jour> = jours_joues.clone();
        dates_contrat.extend(&planning);
        let jours_contrat = match entier_dit(&v.jours_contrat, "jours_contrat")? {
            Some(n) => n,
            None => dates_contrat.iter().collect::<BTreeSet<_>>().len() as i64,
        };
        if let Some(j) = hors.first() {
            return Err(format!(
                "Le {} n'est pas dans le mois {}. Les jours joués d'un \
                 engagement sont ceux du mois de paie ; un contrat qui se poursuit le mois suivant y porte ses jours suivants.",
                j.jj_mm_aaaa(),
                mois
            ));
        }
        // Un jour de planning est d'un mois SUIVANT.
        let fin_du_mois = fin_mois_de(&mois)?;
        if let Some(j) = planning.iter().find(|j| **j <= fin_du_mois) {
            return Err(format!(
                "Le {} est dans le mois {} : c'est un jour joue de ce mois, \
                 a cocher comme tel, pas un jour de planning.",
                j.jj_mm_aaaa(),
                mois
            ));
        }
        if let (Some(f), Some(dernier)) = (fin, planning.last()) {
            if *dernier > f {
                return Err(format!(
                    "Contrat du {} au {}, mais un jour de planning le {} : le \
                     planning ne depasse pas la fin du contrat.",
                    debut.jj_mm_aaaa(),
                    f.jj_mm_aaaa(),
                    dernier.jj_mm_aaaa()
                ));
            }
        }
        // Un contrat contient ses jours joues.
        if let (Some(f), Some(premier), Some(dernier)) = (fin, jours_joues.first(), jours_joues.last()) {
            if *premier < debut || *dernier > f {
                return Err(format!(
                    "Contrat du {} au {}, mais des jours joues le {} et le {} : \
                     un jour joue hors du contrat n'est couvert par aucune DPAE.",
                    debut.jj_mm_aaaa(),
                    f.jj_mm_aaaa(),
                    premier.jj_mm_aaaa(),
                    dernier.jj_mm_aaaa()
                ));
            }
        }

        Ok(Engagement {
            salarie: v.salarie.clone().ok_or_else(|| socle::fichiers::absent("salarie"))?,
            nom: qui.to_string(),
            spectacle: v.spectacle.clone(),
            dates_travail: jours_joues,
            lignes,
            debut,
            fin,
            numero: numeros::NumeroContrat::du_texte(v.numero.as_deref())?,
            mois,
            planning,
            planning_lieux,
            jours,
            jours_contrat,
            date_paiement: v.date_paiement.clone(),
            versement: Default::default(),
            taux_cachet,
            cachets,
            heures,
            // Le lieu dit ; vide, il n'est pas dit.
            lieu: v.lieu.iter().cloned().collect(),
            lieux,
            emploi: v.emploi.clone(),
            emploi_affichage: v.emploi_affichage.clone(),
            pcs_ese: v.pcs_ese.clone(),
            code_emploi_ft: v.code_emploi_ft.clone(),
            statut: v.statut.clone(),
            statut_conventionnel: v.statut_conventionnel.clone(),
            statut_categoriel_rc: v.statut_categoriel_rc.clone(),
            etablissement: v.etablissement.clone().filter(|t| !t.trim().is_empty()),
            nature,
            quotite_contrat: v.quotite_contrat.as_ref().map(Chiffre::flottant).transpose()?,
            quotite_reference_saisie: v.quotite_reference_saisie.as_ref().map(Chiffre::flottant).transpose()?,
            modalite_temps_travail_saisie: v.modalite_temps_travail_saisie.clone(),
            motif_recours: v.motif_recours.clone(),
            motif_rupture: v.motif_rupture.clone(),
            notification_rupture: v.notification_rupture.clone(),
            preavis: v.preavis.clone(),
            dernier_jour_paye: v.dernier_jour_paye.clone().filter(|t| !t.is_empty()),
            date_modification: v.date_modification.clone().filter(|t| !t.trim().is_empty()),
            transaction: v.transaction.clone(),
            suspensions: Vec::new(),
            avant: v.avant.clone(),
            periode: None,
            ecrit: ContratEcrit {
                periode_essai_jours: v.periode_essai_jours.clone().filter(Chiffre::vrai),
                personne_remplacee: v.personne_remplacee.clone().filter(|t| !t.is_empty()),
                repartition_horaire: v.repartition_horaire.clone().filter(|t| !t.is_empty()),
                clauses_particulieres: v.clauses_particulieres.clone().filter(|t| !t.is_empty()),
            },
        })
    }

    // ------------------------------------------------ le contrat et le mois

    /// La nature de contrat, relue a chaque acces.
    pub fn nat(&self) -> Resultat<Arc<Nature>> {
        natures::nature(&self.nature)
    }

    pub fn debut_mois(&self) -> Resultat<Jour> {
        Mois::lire(&self.mois).map(Mois::premier_jour)
    }

    pub fn fin_mois(&self) -> Resultat<Jour> {
        Mois::lire(&self.mois).map(Mois::dernier_jour)
    }

    /// Un mois du contrat sans aucun cachet : DSN oui, bulletin non.
    pub fn sans_paie(&self) -> bool {
        self.lignes.is_empty()
    }

    /// S21.G00.40.013 : les jours du contrat (CDDU), les heures du contrat signe (mensuel).
    pub fn quotite(&self) -> Resultat<Decimal> {
        let nat = self.nat()?;
        if !nat.mensuelle() {
            return Ok(Decimal::from(self.jours_contrat));
        }
        match self.quotite_contrat {
            Some(q) => dec_f64(q),
            None => Err(format!(
                "{} : combien d'heures par mois ? La durée du travail se lit sur le contrat signé \
                 (151,67 h pour un temps plein à 35 h) et la DSN la déclare. À taper dans la case « Heures par \
                 mois » de l'engagement.",
                nat.libelle
            )),
        }
    }

    /// S21.G00.40.012 : la quotite de reference de l'entreprise.
    pub fn quotite_reference(&self) -> Resultat<Option<f64>> {
        let nat = self.nat()?;
        if !nat.mensuelle() {
            return Ok(Some(self.jours_contrat as f64));
        }
        Ok(match self.quotite_reference_saisie {
            Some(q) if q != 0.0 => Some(q),
            _ => nat.quotite_reference_defaut(),
        })
    }

    /// S21.G00.40.014 : temps plein (10) ou temps partiel (20). C. trav. L. 3123-1, 1° et 2° : est a temps partiel le
    /// salarie dont la duree du travail est inferieure « a la duree legale du travail ou, lorsque cette duree est
    /// inferieure a la duree legale, a la duree du travail fixee conventionnellement [...] ou applicable dans
    /// l'etablissement » (`atelier/reference/sources/code-du-travail/articles-20261010.md`). La reference est donc la plus
    /// courte de la duree legale (`quotite_reference_defaut`, 151,67 h) et de la quotite de reference (S21.G00.40.012) :
    /// a 35 h dans une compagnie a 39 h (40.012 = 169, admis en P26V01), un salarie est a temps plein. Sans quoi la
    /// caisse Agirc-Arrco, qui proratise elle-meme le plafond d'un « 20 » sur 40.013 / 40.012 (cahier CT2026, l. 3325-3343,
    /// `atelier/reference/sources/agirc-arrco/`), le proratiserait a tort, alors que la paie ne le fait pas
    /// (`calcul::duree_du_temps_partiel`, la meme plus courte des durees).
    pub fn modalite_temps_travail(&self) -> Resultat<String> {
        if let Some(m) = self.modalite_temps_travail_saisie.as_ref().filter(|m| !m.is_empty()) {
            return Ok(m.clone());
        }
        let nat = self.nat()?;
        let reference = match (self.quotite_reference()?, nat.quotite_reference_defaut().filter(|_| nat.mensuelle())) {
            (Some(r), Some(legale)) => Some(r.min(legale)),
            (r, _) => r,
        };
        let contrat = self.quotite()?;
        Ok(match reference {
            None => "10".into(),
            Some(r) => if contrat >= dec_f64(r)? { "10" } else { "20" }.into(),
        })
    }

    /// Le contrat commence ce mois-ci : contrat a imprimer, DPAE a faire.
    pub fn premier_mois(&self) -> Resultat<bool> {
        Ok(self.debut >= self.debut_mois()?)
    }

    /// Le contrat finit ce mois-ci : bloc 62 en DSN, certificat Conges Spectacles.
    pub fn dernier_mois(&self) -> Resultat<bool> {
        match self.fin {
            None => Ok(false),
            Some(f) => Ok(f <= self.fin_mois()?),
        }
    }

    pub fn a_cheval(&self) -> Resultat<bool> {
        Ok(!(self.premier_mois()? && self.dernier_mois()?))
    }

    /// Le contrat borne au mois : le debut de la periode de paie (de la periode elementaire, quand ce calcul en est
    /// une : `periode`).
    pub fn debut_periode(&self) -> Resultat<Jour> {
        if let Some((d, _)) = self.periode {
            return Ok(d);
        }
        Ok(self.debut.max(self.debut_mois()?))
    }

    pub fn fin_periode(&self) -> Resultat<Jour> {
        if let Some((_, f)) = self.periode {
            return Ok(f);
        }
        let fin_mois = self.fin_mois()?;
        Ok(self.fin.into_iter().fold(fin_mois, Jour::min))
    }

    /// Le jour du versement de ce que l'engagement paie (S21.G00.50.001, la « date de paiement » du bulletin) : celui que
    /// la saisie dit pour lui, sinon celui que la compagnie a choisi (`droit::versement`). C'est le jour ou se fait la
    /// retenue a la source (CGI, art. 1671, 1, `atelier/reference/sources/code-general-des-impots/articles-20261021.md`),
    /// donc celui ou se lit le taux de PAS du salarie.
    pub fn date_du_versement(&self) -> Resultat<Jour> {
        match self.date_paiement.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
            Some(d) => Jour::lire(d.chars().take(10).collect::<String>().as_str()),
            None => self.versement.jour(&self.mois, self.fin_periode()?),
        }
    }

    /// Les mois AAAA-MM que couvre le contrat, du premier au dernier.
    pub fn mois_du_contrat(&self) -> Resultat<Vec<String>> {
        let fin = match self.fin {
            Some(f) => f,
            None => self.fin_mois()?,
        };
        Ok(Mois::entre(&self.debut, &fin).iter().map(Mois::to_string).collect())
    }

    // ------------------------------------------------------------ les lieux

    /// Ou se joue CE jour-la : la date, puis le planning, puis l'engagement.
    pub fn lieu_de(&self, jour: &Jour) -> String {
        match [self.lieux.get(jour), self.planning_lieux.get(jour)].into_iter().flatten().find(|l| !l.is_empty()) {
            Some(l) => l.clone(),
            None => self.lieu.clone(),
        }
    }

    /// Tous les jours connus du contrat : ceux du mois, puis le planning.
    pub fn dates_du_contrat(&self) -> Vec<Jour> {
        let mut v = self.dates_travail.clone();
        v.extend(&self.planning);
        v
    }

    pub fn planning_connu(&self) -> bool {
        !self.planning.is_empty()
    }

    /// Les lieux du contrat, dans l'ordre des dates, sans repetition.
    pub fn lieux_joues(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for j in self.dates_du_contrat() {
            let ou = self.lieu_de(&j);
            if !ou.is_empty() && !out.contains(&ou) {
                out.push(ou);
            }
        }
        out
    }

    /// [(lieu, [dates])] dans l'ordre des dates, sans retrier.
    pub fn dates_par_lieu(&self, planning: bool) -> Vec<(String, Vec<Jour>)> {
        let mut out: Vec<(String, Vec<Jour>)> = Vec::new();
        let jours = if planning { self.dates_du_contrat() } else { self.dates_travail.clone() };
        for j in jours {
            let ou = self.lieu_de(&j);
            match out.last_mut() {
                Some((dernier, dates)) if *dernier == ou => dates.push(j),
                _ => out.push((ou, vec![j])),
            }
        }
        out
    }

    // -------------------------------------------------------- forme canonique

    /// La forme canonique d'un engagement : une ecriture stable, pour les empreintes et les oracles.
    pub fn canon(&self) -> Resultat<paie_ecrite::FormeDEngagement> {
        let nombre_f = |x: Option<f64>| x.map(dec_f64).transpose();
        Ok(paie_ecrite::FormeDEngagement {
            salarie: self.salarie.clone(),
            spectacle: self.spectacle.clone(),
            dates_travail: self.dates_travail.clone(),
            lignes: self.lignes.iter().map(LignePaie::canon).collect::<Resultat<Vec<_>>>()?,
            debut: self.debut,
            fin: self.fin,
            numero: self.numero.clone(),
            mois: self.mois.clone(),
            planning: self.planning.clone(),
            planning_lieux: self.planning_lieux.clone(),
            jours: self.jours,
            jours_contrat: self.jours_contrat,
            date_paiement: self.date_paiement.clone(),
            taux_cachet: nombre_f(self.taux_cachet)?,
            cachets: self.cachets,
            heures: nombre_f(self.heures)?,
            lieu: self.lieu.clone(),
            lieux: self.lieux.clone(),
            emploi: self.emploi.clone(),
            // Ecrit seulement quand il est dit.
            emploi_affichage: self.emploi_affichage.clone(),
            pcs_ese: self.pcs_ese.clone(),
            code_emploi_ft: self.code_emploi_ft.clone(),
            statut: self.statut.clone(),
            statut_conventionnel: self.statut_conventionnel.clone(),
            statut_categoriel_rc: self.statut_categoriel_rc.clone(),
            // Ecrit seulement quand il est dit.
            etablissement: self.etablissement.clone(),
            nature: self.nat()?.cle.clone(),
            quotite_contrat: nombre_f(self.quotite_contrat)?,
            quotite_reference_saisie: nombre_f(self.quotite_reference_saisie)?,
            modalite_temps_travail_saisie: self.modalite_temps_travail_saisie.clone(),
            motif_recours: self.motif_recours.clone(),
            motif_rupture: self.motif_rupture.clone(),
            notification_rupture: self.notification_rupture.clone(),
            preavis: self.preavis.clone(),
            transaction: self.transaction.clone(),
            suspensions: self.suspensions.clone(),
            // Ecrit seulement quand la saisie le dit.
            contrat_ecrit: if self.ecrit.est_vide() {
                None
            } else {
                Some(ContratEcrit { periode_essai_jours: self.ecrit.periode_essai_jours.as_ref().map(Chiffre::canon).transpose()?, ..self.ecrit.clone() })
            },
            // Ce qui se deduit de l'engagement : sa periode, sa quotite, ses mois.
            debut_periode: self.debut_periode()?,
            fin_periode: self.fin_periode()?,
            quotite: self.quotite()?,
            sans_paie: self.sans_paie(),
            premier_mois: self.premier_mois()?,
            dernier_mois: self.dernier_mois()?,
            mois_du_contrat: self.mois_du_contrat()?,
        })
    }
}

// ------------------------------------------------------- depuis un fichier

/// Un avertissement pose sur la paie ; `info` : renseigne sans rien demander. Il s'ecrit `{texte, info}` dans la paie
/// ecrite et son compteur de conges.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Avis {
    pub texte: String,
    pub info: bool,
}

/// UNE LIGNE DE FRAIS NE FAIT PAS LES JOURS DE L'ENGAGEMENT (`ligne::Frais`, rangee comme telle a la porte : sa rubrique
/// est de nature `frais`). Un remboursement de frais professionnels n'est pas un revenu d'activite
/// (CSS L136-1-1, `atelier/reference/sources/code-de-la-securite-sociale/articles-20260923.md`, l. 47) : ce qui fait le
/// debut, la fin et les jours d'un engagement, ce sont ses lignes de travail. Le frais d'un voyage pour rejoindre le
/// lieu de travail, ou en revenir, est a la charge de l'employeur (CCN 1285, art. VIII.5.1, KALIARTI000005808081,
/// `atelier/reference/sources/kali-entrepot/articles.jsonl.gz`) : il se paie avec l'engagement qui le porte, au mois de
/// celui-ci, quel que soit son jour. Le temps du trajet, lui, est du travail ou une servitude qui se compte (art.
/// VIII.1, KALIARTI000023306777) : il se saisit sur une ligne de travail, qui fait son jour.
///
/// Les jours des lignes de travail, ceux du mois (`planning` faux) ou ceux du planning : leur reunion, dedoublonnee,
/// chacun nu ou detaille `{jour, lieu}` quand une ligne dit ou elle se joue.
pub(crate) fn jours_des_lignes(lignes: &[LignePaie], planning: bool) -> Vec<DateDeLigne> {
    let (mut out, mut vus) = (Vec::new(), BTreeSet::new());
    for l in lignes.iter().filter(|l| l.frais().is_none()) {
        for j in if planning { l.planning() } else { l.dates() } {
            if !vus.insert(j.jour().to_string()) {
                continue;
            }
            // Le lieu de la date, sinon celui de la ligne ; ni l'un ni l'autre : la date est nue.
            let ou: String = j.lieu().or_else(|| l.lieu().map(String::from)).into_iter().collect();
            out.push(DateDeLigne::au(j.jour().to_string(), ou));
        }
    }
    out
}

/// LES LIGNES D'UN MOIS SANS FAITS d'un contrat d'usage, tirees des lignes `source` du dernier mois qui en dit
/// (`stock::Objet::lignes_du_planning`) : une ligne par ligne qui a des jours au planning de `mois` ou apres, a la meme
/// rubrique, au meme taux et au meme lieu, ses jours de `mois` en dates et ceux d'apres au planning ; un cachet, une
/// journee ou un service par date (une autre unite, des heures, ne se deduit pas d'un nombre de jours : quantite 0, a
/// completer).
pub(crate) fn lignes_du_planning(racine: &Path, source: &[LignePaie], mois: &str) -> Resultat<Vec<LignePaie>> {
    let mut lignes = Vec::new();
    for l in source {
        let dedans: Vec<DateDeLigne> = l.planning().iter().filter(|x| x.jour().starts_with(mois)).cloned().collect();
        let apres: Vec<DateDeLigne> = l.planning().iter().filter(|x| x.jour().get(..7).is_some_and(|m| m > mois)).cloned().collect();
        if dedans.is_empty() && apres.is_empty() {
            continue;
        }
        let unite = rubrique(racine, l.rubrique(), &Mois::lire(mois)?.dernier_jour())?.unite;
        let par_date = matches!(unite.as_str(), "cachet" | "jour" | "service");
        let quantite = Some(Decimal::from(if par_date { dedans.len() as i64 } else { 0 }));
        lignes.push(match l.avec_les_jours(dedans, apres) {
            LignePaie::Unites(x) => LignePaie::Unites(ligne::Unites { quantite, ..x }),
            LignePaie::UnitesEtHeures(x) => {
                let heures = x.heures.filter(|h| par_date && !h.is_zero());
                LignePaie::UnitesEtHeures(ligne::UnitesEtHeures { quantite, heures, ..x })
            }
            LignePaie::Frais(x) => LignePaie::Frais(ligne::Frais { quantite, ..x }),
            autre => return Err(format!("la ligne {} n'a pas de planning : le signaler au développeur", autre.rubrique())),
        });
    }
    Ok(lignes)
}

/// Les jours, sans doublon, que des lignes jouent ou planifient.
pub(crate) fn jours_de_lignes(lignes: &[LignePaie]) -> usize {
    let mut vus: BTreeSet<String> = jours_des_lignes(lignes, false).iter().map(|j| j.jour().to_string()).collect();
    vus.extend(jours_des_lignes(lignes, true).iter().map(|j| j.jour().to_string()));
    vus.len()
}

/// Le salarie qu'un refus nomme en tete, tel que l'ecran le nomme partout (`Contexte::nomme` : « NOM Prénom ») ;
/// « Cet engagement » quand la carte n'a pas encore de salarie.
pub(crate) fn salarie_nomme(ctx: &Contexte, salarie: Option<&str>) -> String {
    match salarie.filter(|s| !s.trim().is_empty()) {
        Some(c) => ctx.nomme(c),
        None => "Cet engagement".to_string(),
    }
}

/// CE QUE LA NATURE DECIDE D'UN ENGAGEMENT, lu de la variante de sa carte valide par un `match` exhaustif : un CDDU joue
/// un spectacle et se paie au cachet, au taux du mois quand sa carte n'en dit pas ; un contrat paye au mois dit ses heures
/// (et un CDD son motif). Ce qu'une nature n'a pas n'est rien.
struct Termes {
    spectacle: Option<IdSpectacle>,
    taux_cachet: Option<Chiffre>,
    quotite_contrat: Option<Chiffre>,
    quotite_reference: Option<Chiffre>,
    modalite_temps_travail: Option<String>,
    motif_recours: Option<String>,
    personne_remplacee: Option<String>,
    repartition_horaire: Option<String>,
}

impl Termes {
    fn de(v: &carte::CarteValide, cachet_du_mois: &Option<Chiffre>) -> Termes {
        let n = |x: &NombreLu| Chiffre::Nombre(x.clone());
        match v {
            carte::CarteValide::CdduSpectacle(c) => Termes {
                spectacle: Some(c.spectacle.clone()),
                taux_cachet: match &c.taux_cachet {
                    Some(t) => Some(n(t)),
                    None => cachet_du_mois.clone(),
                },
                quotite_contrat: None,
                quotite_reference: None,
                modalite_temps_travail: None,
                motif_recours: None,
                personne_remplacee: None,
                repartition_horaire: None,
            },
            // Un contrat payé au mois ne prend pas le cachet par défaut du mois : son calcul ne le lit pas, et sa paie ne
            // se recalcule pas quand il change.
            carte::CarteValide::Cdi(c) => Termes {
                spectacle: None,
                taux_cachet: None,
                quotite_contrat: Some(n(&c.quotite_contrat)),
                quotite_reference: c.quotite_reference.as_ref().map(n),
                modalite_temps_travail: c.modalite_temps_travail.clone(),
                motif_recours: None,
                personne_remplacee: None,
                repartition_horaire: c.repartition_horaire.clone(),
            },
            carte::CarteValide::Cdd(c) => Termes {
                spectacle: None,
                taux_cachet: None,
                quotite_contrat: Some(n(&c.quotite_contrat)),
                quotite_reference: c.quotite_reference.as_ref().map(n),
                modalite_temps_travail: c.modalite_temps_travail.clone(),
                motif_recours: Some(c.motif_recours.code().to_string()),
                personne_remplacee: c.motif_recours.personne_remplacee().cloned(),
                repartition_horaire: c.repartition_horaire.clone(),
            },
        }
    }
}

/// Ce que `conges::lignes_du_mois` rend pour un engagement paye
/// au mois : les lignes de paie du mois, ce qu'il y a a en dire, et ses jours travailles (`None` : la saisie les dit).
pub type LignesPermanent<'a> = &'a dyn Fn(&Contexte, &saisie::Carte, &str) -> Resultat<(Vec<LignePaie>, Vec<Avis>, Option<i64>)>;

/// Les engagements d'un fichier de mois,
/// construits avant tout calcul. `lignes_permanent` fournit les lignes d'un
/// engagement paye au mois ; les avertissements rendus vont avec, un jeu par
/// engagement (vide hors des permanents).
pub(crate) fn engagements(
    ctx: &Contexte,
    data: &saisie::VueDuMois,
    lignes_permanent: LignesPermanent,
) -> Resultat<(Vec<Engagement>, Vec<Vec<Avis>>)> {
    let mois_dit = data.mois.to_string();
    // Le taux du mois, s'il en ecrit un ; sinon rien (regle 1).
    let defaut_taux = data.taux_cachet.clone().map(Chiffre::Nombre);
    let (mut out, mut dits) = (Vec::new(), Vec::new());
    for carte in &data.engagements {
        let qui = salarie_nomme(ctx, carte.salarie.as_deref());
        // Seule une carte valide entre dans le calcul : sa nature dite, ses cases obligatoires remplies, aucune case que
        // sa nature n'a pas (`carte::CarteValide::depuis`). Ce qui depend de la nature se lit de sa variante.
        let valide = carte::CarteValide::depuis(carte, &qui)?;
        let nat = droit::natures::nature(valide.nature()).map_err(|m| format!("{} : {}", qui, m))?;
        let termes = Termes::de(&valide, &defaut_taux);
        let tarifee;
        let carte = match if nat.mensuelle() { None } else { lignes_au_tarif(ctx, carte.lignes(), &mois_dit)? } {
            Some(lignes) => {
                tarifee = saisie::Carte { lignes: Some(lignes), ..carte.clone() };
                &tarifee
            }
            None => carte,
        };
        if nat.mensuelle() {
            // Employer des permanents se declare.
            if !ctx.emploie_des_permanents() {
                return Err(format!(
                    "{} : cet engagement est un contrat payé au mois ({}), et la compagnie ne dit pas qu'elle emploie \
                     des salariés au mois.\n\
                     \x20 À faire, dans cet ordre : cocher « La compagnie emploie aussi des salariés au mois » dans \
                     Configuration › Entreprise (bloc « Vos salariés »), puis remplir ce que la case ouvre : la \
                     complémentaire santé de chaque permanent (obligatoire, art. L911-7, le logiciel ne paie pas sans \
                     elle) et, pour un non-cadre de moins d'un an dans l'entreprise, son entrée dans la branche.\n\
                     \x20 Si ce n'était pas voulu : la nature se change sur la carte ; un intermittent au cachet est \
                     un « CDDU ».",
                    qui, nat.libelle
                ));
            }
        }
        let jours = if nat.mensuelle() { Vec::new() } else { carte.dates() };
        // `lignes: []` ecrit est un mois du contrat sans cachet ; une ligne sans date est incomplete.
        if jours.is_empty() && !nat.mensuelle() && !carte.lignes().is_empty() {
            return Err(format!("{}{} Sans eux il n'y a ni début, ni fin, ni contrat.", qui, PAS_DE_JOUR));
        }
        if nat.mensuelle() {
            frais_d_un_mois_paye(ctx, carte.lignes(), &qui)?;
        }
        lignes_dites(ctx, carte.lignes(), &qui)?;
        // Le metier de ce contrat : six champs declares, que l'engagement
        // ecrit par-dessus, et son libelle lisible (`emploi_affichage`).
        let declare = match carte.metier.as_deref().filter(|m| !m.is_empty()) {
            Some(code) => {
                let salarie = fiche_du_salarie(ctx, carte.salarie.as_ref().ok_or_else(|| socle::fichiers::absent("salarie"))?)?;
                let feminin = sexe(&salarie).as_deref() == Some("2");
                Some(emplois::resoudre(code, carte.cadre, feminin, ctx.entreprise.idcc.texte()).map_err(|m| format!("{} : {}", qui, m))?)
            }
            None => None,
        };
        // Les jours travailles d'un contrat paye au mois : ceux que le moteur tire du mois (`conges::lignes_du_mois`).
        let mut jours_du_mois: Option<Chiffre> = None;
        let lignes = if nat.mensuelle() {
            let (lignes, avis, jours_travailles) = lignes_permanent(ctx, carte, &mois_dit)?;
            dits.push(avis);
            jours_du_mois = jours_travailles.map(Chiffre::entier);
            Some(lignes)
        } else {
            match &carte.lignes {
                Some(l) => {
                    let (lignes, avis) = frais_au_dela(ctx, carte.salarie.as_ref(), l.clone(), &mois_dit, &qui)?;
                    dits.push(avis);
                    Some(lignes)
                }
                None => {
                    dits.push(Vec::new());
                    None
                }
            }
        };
        let nombre = |x: &Option<NombreLu>| x.clone().map(Chiffre::Nombre);
        // Un contrat d'usage commence dans l'ancien logiciel et continue ici : les jours du contrat entier comptent ceux
        // que les pieces importees attestent (`importe::Importe::jours_du_contrat_continue`).
        let continue_ = match &carte.jours_contrat {
            None if !nat.mensuelle() && carte.debut_lu().get(..7).is_some_and(|m| m < mois_dit.as_str()) => {
                match crate::importe::continue_par(&ctx.racine, carte)? {
                    Some(i) => i.jours_du_contrat_continue(carte.jours_du_contrat())?.map(Chiffre::entier),
                    None => None,
                }
            }
            _ => None,
        };
        // Ce que la carte ecrit du metier passe avant ce que le catalogue en dit.
        let d = declare.as_ref();
        let ecrit = |x: &Option<String>| x.clone().filter(|s| !s.is_empty());
        let assemble = Assemble {
            salarie: carte.salarie.clone(),
            spectacle: termes.spectacle,
            dates_travail: jours,
            taux_cachet: termes.taux_cachet,
            debut: carte.debut.clone(),
            fin: carte.fin.clone(),
            numero: carte.numero.clone(),
            mois: Some(mois_dit.clone()),
            jours: nombre(&carte.jours).or(jours_du_mois),
            jours_contrat: nombre(&carte.jours_contrat).or(continue_),
            planning: carte.planning(),
            lignes,
            date_paiement: carte.date_paiement.clone(),
            lieu: carte.lieu.clone(),
            emploi: ecrit(&carte.emploi).or_else(|| d.map(|d| d.emploi.clone())),
            pcs_ese: ecrit(&carte.pcs_ese).or_else(|| d.and_then(|d| d.pcs_ese.clone())),
            code_emploi_ft: ecrit(&carte.code_emploi_ft).or_else(|| d.and_then(|d| d.code_emploi_ft.clone())),
            statut: ecrit(&carte.statut).or_else(|| d.map(|d| d.statut.clone())),
            statut_conventionnel: ecrit(&carte.statut_conventionnel).or_else(|| d.and_then(|d| d.statut_conventionnel.clone())),
            statut_categoriel_rc: ecrit(&carte.statut_categoriel_rc).or_else(|| d.map(|d| d.statut_categoriel_rc.to_string())),
            // Le libelle que `resoudre` a donne a CE metier, feminin compris ; un `emploi` ecrit a la main dirait un autre
            // metier que lui.
            emploi_affichage: if ecrit(&carte.emploi).is_none() { d.map(|d| d.emploi_affichage.clone()) } else { None },
            nature: Some(valide.nature().to_string()),
            quotite_contrat: termes.quotite_contrat,
            quotite_reference_saisie: termes.quotite_reference.filter(Chiffre::vrai),
            modalite_temps_travail_saisie: termes.modalite_temps_travail,
            motif_recours: termes.motif_recours,
            etablissement: carte.etablissement.clone(),
            motif_rupture: carte.motif_rupture.clone(),
            // La fin du contrat telle que la saisie la dit : la mensuelle et le FCTU en tirent le meme S21.G00.62.006
            // (`fctu::dernier_jour_paye`).
            preavis: carte.preavis.clone(),
            dernier_jour_paye: carte.dernier_jour_paye.clone(),
            date_modification: carte.date_modification.clone(),
            avant: carte.avant.clone(),
            periode_essai_jours: nombre(&carte.periode_essai_jours),
            personne_remplacee: termes.personne_remplacee,
            repartition_horaire: termes.repartition_horaire,
            clauses_particulieres: carte.clauses_particulieres.clone(),
            notification_rupture: None,
            transaction: None,
        };
        let mut eng = Engagement::nouveau(&ctx.racine, &assemble, &qui)?;
        eng.versement = droit::versement::Versement::de_la_fiche(&ctx.entreprise_au(&Mois::lire(&eng.mois)?.dernier_jour()), &*eng.nat()?)?;
        // Ce que l'ancien logiciel a verse est un fait : seul ce que la compagnie va verser se juge.
        if carte.importe.is_none() {
            if let Some(refus) = versement_refuse(ctx, &eng, &qui)? {
                return Err(refus);
            }
        }
        notifiee_pendant_le_contrat(carte, &eng)?;
        // Les lignes que la saisie ecrit disent leurs heures. Celles qu'un mois de permanent deduit, quand sa
        // saisie n'en ecrit pas (`conges::lignes_du_mois`), portent la quotite du contrat en heures : rien a controler.
        if !nat.mensuelle() || carte.lignes.is_some() {
            heures_dites(ctx, &eng, &qui)?;
        }
        // Ce que l'ancien logiciel a paye est un fait : seul un engagement neuf se juge.
        if carte.importe.is_none() {
            unites_du_statut(ctx, &eng, &qui)?;
        }
        out.push(eng);
    }
    Ok((out, dits))
}

/// La rupture se notifie pendant le contrat : « La date de notification de la rupture de contrat doit être supérieure
/// ou égale à la date de début de contrat de travail et inférieure ou égale à la date de fin de contrat de travail »
/// (CCH-11 de S21.G00.62.003, `atelier/reference/norme-dsn/texte/bloc-S21.G00.62.txt`). La saisie `e` qui la dit
/// (`notification_rupture`) se refuse, en disant les bornes du contrat `eng` ; un contrat sans fin ne borne que le debut.
/// Le refus du jour du versement de `eng` que les textes interdisent (`droit::versement::refus`) ; `None` : il passe.
fn versement_refuse(ctx: &Contexte, eng: &Engagement, qui: &str) -> Resultat<Option<String>> {
    let verse = eng.date_du_versement()?;
    // L'artiste de cirque en contrat a duree determinee de la 1285 dont le contrat finit dans ce mois : son metier est
    // range « cirque » dans la grille de la 1285 (`emplois-base.json`, `minima`), la grille de X.3.5.
    let fin_mois = eng.fin_mois()?;
    let fin_cirque = match eng.fin.filter(|f| *f <= fin_mois) {
        Some(f) if !eng.nat()?.duree_indeterminee && droit::versement::au_plus_tard_cirque_1285(&f)? < verse => {
            let ent = ctx.entreprise_au(&fin_mois);
            let fiche = crate::calcul::fiche_de_l_engagement(ctx, eng)?;
            let code = fiche.code_emploi_ft.texte().to_string();
            let metier = emplois::metier(&code)?;
            let cirque = metier.as_ref().and_then(|m| m.au_catalogue()).and_then(|f| f.minima.ccn_1285.as_deref()) == Some("cirque");
            (ent.idcc.texte() == "1285" && cirque).then_some(f)
        }
        _ => None,
    };
    droit::versement::refus(qui, &verse, &eng.mois, fin_cirque.as_ref())
}

fn notifiee_pendant_le_contrat(e: &saisie::Carte, eng: &Engagement) -> Resultat<()> {
    let Some(t) = e.notification_rupture.as_deref().filter(|t| !t.trim().is_empty()) else { return Ok(()) };
    let n = Jour::lire(t).map_err(|_| format!("{} : « Notifiée le » {} n'est pas une date.", eng.nom, t))?;
    if n >= eng.debut && eng.fin.is_none_or(|f| n <= f) {
        return Ok(());
    }
    let bornes = match eng.fin {
        Some(f) => format!("entre son premier jour, le {}, et sa fin, le {}", eng.debut.jj_mm_aaaa(), f.jj_mm_aaaa()),
        None => format!("au plus tôt son premier jour, le {}", eng.debut.jj_mm_aaaa()),
    };
    Err(format!(
        "{} : notifiée le {}, la rupture doit l'être pendant le contrat, {}. La date se corrige dans sa carte, sous « Contrat ».",
        eng.nom,
        n.jj_mm_aaaa(),
        bornes
    ))
}

/// Ce que disent les refus d'une saisie pas encore finie, et non un defaut du logiciel : un spectacle pas encore
/// choisi, des jours pas encore coches, une ligne sans quantite, sans montant ou sans heures, une quantite qui ne compte
/// rien. L'ecran enregistre a chaque frappe, et ces refus montent sous `SaisieIncomplete`
/// (`moteur::routes::etapes::classer_saisie`, `moteur::routes::mois::refus_du_calcul`) ; leur message s'ouvre sur le
/// salarie, puis dit quoi faire.
pub(crate) const PAS_DE_NATURE: &str = " : choisissez la nature du contrat, sur sa carte.";
const PAS_DE_JOUR: &str = " : cochez les jours joués sur sa ligne de paie.";
const LIGNE_A_DIRE: &str = " : saisissez ";
const QUANTITE_SANS_COMPTE: &str = " : saisissez un nombre plus grand que zéro";
/// Une journee de plus de 24 heures (`duree::plus_d_une_journee`) : une faute de frappe, a corriger comme une saisie pas finie.
pub(crate) const JOURNEE_DE_PLUS_DE_24: &str = ", et une journée n'a que 24 heures";
const SAISIE_INCOMPLETE: [&str; 7] =
    [carte::A_REMPLIR, PAS_DE_NATURE, PAS_DE_JOUR, LIGNE_A_DIRE, QUANTITE_SANS_COMPTE, JOURNEE_DE_PLUS_DE_24, crate::maintien::IJ_A_DIRE];

/// LE TARIF DE LA CONVENTION d'une rubrique a ce jour (`tarifs_conventionnels.<code>` de la couche de la convention du
/// bareme, liste datee : l'avenant qui le fixe et sa date d'effet y sont cites). `None` : la convention n'en fixe pas, ou
/// pas encore a ce jour ; le taux se saisit.
pub fn tarif_conventionnel(ctx: &Contexte, code: &str, jour: &Jour) -> Resultat<Option<Decimal>> {
    let bar = ctx.bareme(jour)?;
    Ok(bar.tarifs_conventionnels.as_ref().and_then(|t| match code {
        "repas" => t.repas.and_then(|n| n.0),
        "nuitee" => t.nuitee.and_then(|n| n.0),
        _ => None,
    }))
}

/// Les lignes d'un engagement au cachet dont la rubrique a un tarif de la convention (`tarif_conventionnel`) et qui ne
/// disent pas leur taux le prennent, au premier jour de la ligne (a defaut, au premier jour du mois saisi). Le
/// bareme ne se lit que pour une telle ligne.
fn lignes_au_tarif(ctx: &Contexte, lignes: &[LignePaie], mois: &str) -> Resultat<Option<Vec<LignePaie>>> {
    let mut neuves = lignes.to_vec();
    let mut change = false;
    for l in neuves.iter_mut() {
        // Une ligne dont le taux est une case (`Unites`, `UnitesEtHeures`, `Frais`), et vide.
        if !matches!(l, LignePaie::Unites(_) | LignePaie::UnitesEtHeures(_) | LignePaie::Frais(_)) || l.taux().is_some() {
            continue;
        }
        let jour = match l.dates().first() {
            Some(j) => j.lu()?,
            None => Mois::lire(mois)?.premier_jour(),
        };
        if let Some(t) = tarif_conventionnel(ctx, l.rubrique(), &jour)? {
            l.poser_taux(t)?;
            change = true;
        }
    }
    Ok(change.then_some(neuves))
}

/// Les engagements au cachet d'une saisie de mois, leurs lignes au tarif de la convention (`lignes_au_tarif`).
#[cfg(feature = "ecran")]
pub(crate) fn saisie_au_tarif(ctx: &Contexte, data: &mut saisie::VueDuMois) -> Resultat<()> {
    let mois = data.mois.to_string();
    for e in data.engagements.iter_mut() {
        if e.nature_lue()?.mensuelle() {
            continue;
        }
        if let Some(neuves) = lignes_au_tarif(ctx, e.lignes(), &mois)? {
            e.lignes = Some(neuves);
        }
    }
    Ok(())
}


/// Les rubriques des frais de deplacement rembourses au forfait, et celles de leur part au-dela de la limite.
pub const NUITEE: &str = "nuitee";
pub const NUITEE_AU_DELA: &str = "nuitee_au_dela";
pub const REPAS: &str = "repas";
pub const REPAS_AU_DELA: &str = "repas_au_dela";
use ligne::CaseDeFrais::{Justificatif as JUSTIFICATIF, Loge as LOGE, Paris as PARIS, Restaurant as RESTAURANT};
pub use ligne::CaseDeFrais;

/// Le lieu d'une ligne de nuitee ou de repas (`LignePaie::territoire`, le choix « Lieu », sous « Avancé ») : absent, la
/// metropole ; sinon un territoire de `frais_hors_metropole` du bareme (`dom`, `pacifique`, `monaco`), les Terres
/// australes et antarctiques (`taaf`), ou un lieu a l'etranger, par son code de la table des missions (`droit::missions`).
/// Les territoires hors metropole que le choix « Lieu » propose, avant les lieux a l'etranger : la cle du bareme
/// (`frais_hors_metropole`), et ce que le choix dit.
pub const TERRITOIRES: [(&str, &str); 4] = [
    ("dom", "Outre-mer : Antilles, Guyane, Réunion, Mayotte, Saint-Pierre-et-Miquelon, Saint-Barthélemy, Saint-Martin"),
    ("pacifique", "Outre-mer : Nouvelle-Calédonie, Wallis-et-Futuna, Polynésie française"),
    ("taaf", "Outre-mer : Terres australes et antarctiques françaises"),
    ("monaco", "Monaco"),
];

/// Les deux limites d'une rubrique de frais au barème, et la case de la ligne qui choisit la plus haute.
struct Limites {
    rubrique: &'static str,
    au_dela: &'static str,
    cle: &'static str,
    case: CaseDeFrais,
    /// Les limites de la metropole au bareme (la haute, la basse).
    metropole: fn(&droit::forme_bareme::Pile) -> Option<(Decimal, Decimal)>,
    /// Sa limite hors de la metropole (`frais_hors_metropole`), et son nom.
    hors_metropole: fn(&droit::forme_bareme::LogementEtRepas) -> Decimal,
    nom_hors_metropole: &'static str,
}

const FRAIS_LIMITES: [Limites; 2] = [
    Limites {
        rubrique: NUITEE, au_dela: NUITEE_AU_DELA, cle: "logement_grand_deplacement", case: PARIS,
        metropole: |b| b.logement_grand_deplacement.as_ref().map(|f| (f.paris, f.autres)),
        hors_metropole: |z| z.logement, nom_hors_metropole: "logement",
    },
    Limites {
        rubrique: REPAS, au_dela: REPAS_AU_DELA, cle: "repas_deplacement", case: RESTAURANT,
        metropole: |b| b.repas_deplacement.as_ref().map(|f| (f.restaurant, f.hors_restaurant)),
        hors_metropole: |z| z.repas, nom_hors_metropole: "repas",
    },
];

/// Une limite d'exoneration (`FRAIS_LIMITES`) est-elle attachee a cette rubrique ? Seuls ses repas et ses nuitees disent
/// un lieu et des cases, et se saisissent sur la carte d'un contrat paye au mois, aux memes limites que ceux de tout
/// salarie (le reste de sa paie se compose de ses faits).
pub fn a_une_limite(code: &str) -> bool {
    FRAIS_LIMITES.iter().any(|x| x.rubrique == code)
}

/// La reduction de l'indemnite de mission a l'etranger quand le salarie est loge gratuitement (arrete du 3 juillet
/// 2006, art. 2 ; BOSS § 1390).
const REDUCTION_LOGE: Decimal = Decimal::from_parts(65, 0, 0, false, 2);

/// L'aide d'une case, avec les limites de l'annee (`haute`, `basse`).
fn aide_de_case(case: CaseDeFrais, rubrique: &str, limites: Option<(Decimal, Decimal)>) -> Option<String> {
    let eur = socle::texte::euros;
    match (case, limites) {
        (PARIS, Some((h, b))) if rubrique == NUITEE => Some(format!("Paris ou petite couronne : la nuitée est exonérée jusqu'à {} au lieu de {}.", eur(h), eur(b))),
        (RESTAURANT, Some((h, b))) => Some(format!(
            "Le salarié ne pouvait pas rentrer et a dû manger au restaurant : le repas est exonéré jusqu'à {} au lieu de {}.",
            eur(h),
            eur(b)
        )),
        (JUSTIFICATIF, _) => Some("La compagnie garde la facture : tout le montant est exonéré.".to_string()),
        (LOGE, _) => Some("La compagnie a payé l'hôtel : la limite du jour est réduite de 65 %.".to_string()),
        _ => None,
    }
}


/// LES FRAIS DE DEPLACEMENT AU-DELA DE LEUR LIMITE. Au-dela, la fraction excedentaire entre dans l'assiette, sauf si
/// l'employeur prouve qu'elle a ete utilisee conformement a son objet (BOSS § 270 ; § 1280) : une ligne soumise
/// (`au_dela`), sauf quand la ligne dit que la compagnie garde le justificatif (`JUSTIFICATIF`). La ligne de frais garde
/// la limite, hors assiette. Le lieu de la ligne (`LignePaie::territoire`) dit la limite :
/// - EN METROPOLE (arrete du 4 septembre 2025,
///   `atelier/reference/sources/dfs/arrete-2025-frais-professionnels/articles-20261103.md`, montants revalorises chaque
///   1er janvier, art. 10 ; BOSS, Frais professionnels, `atelier/reference/sources/boss/boss-frais-professionnels-20261013.txt` ;
///   bareme `repas_deplacement` et `logement_grand_deplacement`) :
///   - le repas (art. 3) : la limite du salarie en deplacement « empêché de regagner sa résidence » (1°), quand il est
///     contraint de prendre son repas au restaurant ou que les usages de la profession l'y conduisent (BOSS § 240-260) ;
///     sinon celle du repas hors des locaux (3°). En grand deplacement, la limite du 1° (art. 5, I, 1°) : un repas d'un
///     jour dont la nuit, ou celle de la veille, est une nuitee de l'engagement. Hors de lui, la case `RESTAURANT` de la
///     ligne dit le fait ; elle n'a lieu d'etre qu'au-dessus de la plus basse ;
///   - la nuitee (art. 5, I, 1°) : le logement et le petit-dejeuner, par jour, a Paris et dans les Hauts-de-Seine, la
///     Seine-Saint-Denis et le Val-de-Marne, ou dans les autres departements de metropole (BOSS § 1310) : la case `PARIS`.
/// - EN OUTRE-MER ET A MONACO : les taux des indemnites de mission de l'Etat (arrete du 3 juillet 2006, art. 1, a) ;
///   BOSS § 1340-1360 et 1400 ; bareme `frais_hors_metropole`), par nuit et par repas ; dans les Terres australes, aucun
///   forfait (§ 1370) : seule la depense justifiee s'exonere.
/// - A L'ETRANGER : une seule limite par jour pour le logement et les repas, l'indemnite journaliere de mission du lieu
///   (§ 1380-1390, `droit::missions`), reduite de 65 % quand la compagnie paie l'hotel (`LOGE`, art. 2 de l'arrete de
///   2006) ; ce qui la depasse s'impute d'abord sur la nuitee du jour, puis sur ses repas.
/// - Sous deduction forfaitaire specifique, les frais que l'annexe 2 de l'arrete de 2025 met hors assiette pour la
///   profession (I, 2° et 5°, `atelier/reference/sources/dfs/annexes-2025/legi-arrete-20250904-frais-professionnels-annexe2-frais-hors-assiette-20250907.txt`
///   l. 8 et 11 ; bareme, `frais_hors_assiette`) le sont entiers : « en cas de déduction forfaitaire spécifique » (l. 5).
///   Tout autre salarie suit les limites.
pub(crate) fn frais_au_dela(ctx: &Contexte, salarie: Option<&IdSalarie>, lignes: Vec<LignePaie>, mois: &str, qui: &str) -> Resultat<(Vec<LignePaie>, Vec<Avis>)> {
    let limites_de = |l: &ligne::Frais| FRAIS_LIMITES.iter().find(|x| l.rubrique == x.rubrique);
    if !lignes.iter().any(|l| l.frais().and_then(limites_de).is_some()) {
        return Ok((lignes, Vec::new()));
    }
    let sal = fiche_du_salarie(ctx, salarie.ok_or_else(|| socle::fichiers::absent("salarie"))?)?;
    let cochee = |l: &ligne::Frais, k: CaseDeFrais| l.cochee(k);
    let territoire = |l: &ligne::Frais| l.territoire.clone();
    let a_l_etranger = |l: &ligne::Frais| l.territoire.as_deref().is_some_and(|t| !TERRITOIRES.iter().any(|(k, _)| *k == t));
    // Les nuits payees en nuitee : le salarie y est en grand deplacement.
    let mut nuits: Vec<Jour> = Vec::new();
    for l in lignes.iter().filter(|l| l.rubrique() == NUITEE) {
        nuits.extend(l.jours()?);
    }
    let (mut out, mut au_dela, mut avis) = (Vec::new(), Vec::new(), Vec::new());
    // Les lignes a l'etranger se comptent ensemble, jour par jour, apres les autres : leur rang dans `out`.
    let mut etrangeres: Vec<usize> = Vec::new();
    for (rang, l) in lignes.into_iter().enumerate() {
        let (mut l, lim) = match l {
            LignePaie::Frais(f) => match limites_de(&f) {
                Some(lim) => (f, lim),
                None => {
                    out.push(LignePaie::Frais(f));
                    continue;
                }
            },
            autre => {
                out.push(autre);
                continue;
            }
        };
        l.marques.rang = Some(rang);
        let mut cases: Vec<CaseDeFrais> = Vec::new();
        let taux = l.taux;
        let dates: Vec<DateDeLigne> = l.dates.clone();
        let jour = match dates.first() {
            Some(x) => x.lu()?,
            None => Mois::lire(mois)?.premier_jour(),
        };
        let bar = ctx.bareme(&jour)?;
        let ent = ctx.entreprise_au(&jour);
        let de_l_annexe = calcul::deduction_forfaitaire(&bar, &sal, &ent)?.is_some_and(|o| o.frais_hors_assiette.iter().any(|f| f == lim.rubrique));
        let ou = territoire(&l);
        // Les deux limites de la metropole, quand le bareme les porte : les cases et leurs aides les disent.
        let metropole = (lim.metropole)(&bar);
        let mut part = None;
        if a_l_etranger(&l) && !de_l_annexe {
            etrangeres.push(out.len());
        } else if let (Some(taux), false) = (taux, de_l_annexe) {
            let erreur_bareme = |cle: &str| {
                format!(
                    "{} : le barème {} ne porte pas la limite d'exonération de la ligne à {} (`{}`). Rien n'est calculé.",
                    qui,
                    jour.an,
                    socle::texte::euros(taux),
                    cle
                )
            };
            let limite = match ou.as_deref() {
                None => {
                    let (haute, basse) = metropole.ok_or_else(|| erreur_bareme(lim.cle))?;
                    // Un repas d'un jour dont la nuit, ou celle de la veille, est payee en nuitee : le grand deplacement
                    // (art. 5, I, 1°), a la limite haute sans case.
                    let en_grand_deplacement = lim.rubrique == REPAS
                        && !dates.is_empty()
                        && dates.iter().try_fold(true, |tout, x| -> Resultat<bool> {
                            let j = x.lu()?;
                            Ok(tout && (nuits.contains(&j) || nuits.contains(&j.plus_jours(-1)?)))
                        })?;
                    if taux > basse && !en_grand_deplacement {
                        cases.push(lim.case);
                    }
                    if en_grand_deplacement || cochee(&l, lim.case) { haute } else { basse }
                }
                Some("taaf") => Decimal::ZERO,
                Some(t) => {
                    let cle = format!("frais_hors_metropole.{}.{}", t, lim.nom_hors_metropole);
                    let zones = bar.frais_hors_metropole.as_ref();
                    let zone = match t {
                        "dom" => zones.map(|z| &z.dom),
                        "pacifique" => zones.map(|z| &z.pacifique),
                        "monaco" => zones.map(|z| &z.monaco),
                        _ => None,
                    };
                    zone.map(lim.hors_metropole).ok_or_else(|| erreur_bareme(&cle))?
                }
            };
            if taux > limite {
                cases.push(JUSTIFICATIF);
                if !cochee(&l, JUSTIFICATIF) {
                    part = Some((taux, limite));
                }
            }
        }
        for k in [lim.case, JUSTIFICATIF] {
            if cochee(&l, k) && !cases.contains(&k) && (k == JUSTIFICATIF || ou.is_none()) {
                cases.push(k);
            }
        }
        marquer_les_cases(&mut l, &cases, lim.rubrique, metropole);
        if let Some((taux, limite)) = part {
            let quantite = l.quantite.ok_or_else(|| socle::fichiers::absent("quantite"))?;
            au_dela.push(LignePaie::Composee(ligne::Composee {
                rubrique: lim.au_dela.into(),
                quantite,
                taux: (taux - limite).normalize(),
                heures: None,
                precision: None,
                dates,
            }));
            // LA LIGNE COUPEE SE DIT : la limite, ce qui entre dans le brut, et la case qui l'eviterait.
            let haute = metropole.map(|m| m.0);
            let (quoi, unite, case) = if lim.rubrique == NUITEE {
                ("nuitée", "nuit", if ou.is_some() || cochee(&l, PARIS) || Some(limite) == haute { "" } else { "« Paris ou petite couronne » si c'est le lieu, ou " })
            } else {
                ("repas", "repas", if ou.is_some() || cochee(&l, RESTAURANT) || Some(limite) == haute { "" } else { "« au restaurant » s'il y est contraint, ou " })
            };
            avis.push(Avis {
                info: true,
                texte: format!(
                    "{} : {} de {} au-dessus de la limite de {} : {} par {} entrent dans le brut. Cocher {}« justificatif conservé » si la compagnie garde la facture.",
                    qui,
                    quoi,
                    socle::texte::euros(taux),
                    socle::texte::euros(limite),
                    socle::texte::euros(taux - limite),
                    unite,
                    case
                ),
            });
            l.marques.taux_saisi = Some(taux.normalize());
            l.taux = Some(limite.normalize());
        }
        out.push(LignePaie::Frais(l));
    }
    let (parts, dits) = frais_a_l_etranger(&mut out, &etrangeres, &nuits, qui)?;
    avis.extend(dits);
    // Les parts au-dela viennent apres les lignes saisies : une ligne garde son rang (la case de l'ecran le lit).
    out.extend(au_dela);
    out.extend(parts);
    Ok((out, avis))
}

/// Pose sur la ligne `l` les cases qui ont lieu d'etre et leurs aides (`ligne::Marques`).
fn marquer_les_cases(l: &mut ligne::Frais, cases: &[CaseDeFrais], rubrique: &str, metropole: Option<(Decimal, Decimal)>) {
    if cases.is_empty() {
        return;
    }
    l.marques.cases = cases.to_vec();
    l.marques.aides = cases.iter().filter_map(|k| aide_de_case(*k, rubrique, metropole).map(|a| (*k, a))).collect();
}

/// LES LIGNES A L'ETRANGER (`etrangeres`, leurs rangs dans `lignes`), jour par jour : le montant du jour de chaque ligne
/// (son taux par sa quantite d'un jour), leur somme, et l'indemnite de mission du lieu du jour (celui de sa nuitee, sinon
/// du premier repas), reduite de 65 % un jour sans nuitee dont un repas dit que la compagnie a paye l'hotel (`LOGE`).
/// Ce qui depasse s'impute sur la nuitee du jour, puis sur les repas, hors des lignes au justificatif conserve : une
/// part soumise par ecart d'un jour, et la ligne garde ce qui reste exonere sur toute sa quantite. Rend les parts
/// soumises et les avis.
fn frais_a_l_etranger(lignes: &mut [LignePaie], etrangeres: &[usize], nuits: &[Jour], qui: &str) -> Resultat<(Vec<LignePaie>, Vec<Avis>)> {
    let cochee = |l: &LignePaie, k: CaseDeFrais| l.cochee(k);
    // Une ligne a l'etranger est une ligne de frais (`frais_au_dela`).
    fn frais(l: &mut LignePaie) -> Resultat<&mut ligne::Frais> {
        l.frais_mut().ok_or_else(|| "une ligne à l'étranger qui n'est pas une ligne de frais : le signaler au développeur".to_string())
    }
    if etrangeres.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    // Pour chaque ligne : ses jours, et ce qu'elle paie un jour (taux x quantite d'un jour).
    struct Etrangere {
        rang: usize,
        jours: Vec<(Jour, DateDeLigne)>,
        par_jour: Decimal,
        unites: Decimal,
        taux: Decimal,
        nuitee: bool,
    }
    let mut es: Vec<Etrangere> = Vec::new();
    for &i in etrangeres {
        let l = &lignes[i];
        let Some(taux) = l.taux() else { continue };
        // La quantite d'une ligne saisie est dite (`lignes_dites`).
        let quantite = l.quantite().ok_or_else(|| socle::fichiers::absent("quantite"))?;
        if l.dates().is_empty() {
            return Err(format!("{} : une ligne de frais à l'étranger dit ses jours : l'indemnité de mission est celle d'un jour. Rien n'est calculé.", qui));
        }
        let mut jours = Vec::new();
        for x in l.dates() {
            jours.push((x.lu()?, x.clone()));
        }
        let unites = quantite / Decimal::from(jours.len() as u64);
        es.push(Etrangere { rang: i, jours, par_jour: taux * unites, unites, taux, nuitee: l.rubrique() == NUITEE });
    }
    // Les cases d'une ligne a l'etranger : le justificatif quand elle depasse, l'hotel paye d'un repas un jour sans nuitee.
    let mut tous_les_jours: Vec<Jour> = es.iter().flat_map(|e| e.jours.iter().map(|(j, _)| *j)).collect();
    tous_les_jours.sort();
    tous_les_jours.dedup();
    // L'ecart de chaque ligne, jour par jour.
    let mut ecarts: Vec<Vec<Decimal>> = es.iter().map(|e| vec![Decimal::ZERO; e.jours.len()]).collect();
    let mut limites_du_jour: Vec<(Jour, Decimal, String)> = Vec::new();
    for j in &tous_les_jours {
        let du_jour: Vec<(usize, usize)> = es.iter().enumerate().filter_map(|(k, e)| e.jours.iter().position(|(x, _)| x == j).map(|p| (k, p))).collect();
        let nuitee = du_jour.iter().find(|(k, _)| es[*k].nuitee);
        // Une ligne a l'etranger dit son territoire (`a_l_etranger`) ; un jour de `tous_les_jours` a au moins une ligne.
        let (k, _) = nuitee.or(du_jour.first()).ok_or("un jour à l'étranger sans ligne : le signaler au développeur")?;
        let lieu = lignes[es[*k].rang].territoire_dit().map(String::from).ok_or_else(|| socle::fichiers::absent("territoire"))?;
        let indemnite = droit::missions::du_jour(&lieu, j).map_err(|m| format!("{} : {}", qui, m))?;
        socle::fichiers::noter_valeur(&format!("mission {} {}", lieu, j.iso()), indemnite.empreinte());
        let loge = nuitee.is_none() && !nuits.contains(j) && du_jour.iter().any(|(k, _)| cochee(&lignes[es[*k].rang], LOGE));
        let limite = if loge { socle::num::eur2(indemnite.euros * (Decimal::ONE - REDUCTION_LOGE)) } else { indemnite.euros };
        let total: Decimal = du_jour.iter().map(|(k, _)| es[*k].par_jour).sum();
        let mut reste = (total - limite).max(Decimal::ZERO);
        let mut ordre = du_jour.clone();
        ordre.sort_by_key(|(k, _)| !es[*k].nuitee);
        for (k, p) in ordre {
            if reste <= Decimal::ZERO || cochee(&lignes[es[k].rang], JUSTIFICATIF) {
                continue;
            }
            let pris = reste.min(es[k].par_jour);
            ecarts[k][p] = pris;
            reste -= pris;
        }
        // Le lieu se dit par son nom quand la table des lieux le nomme (`droit::missions::lieux`), sinon par son code.
        let dit = match droit::missions::nom(&lieu)? {
            Some(nom) => nom,
            None => lieu,
        };
        limites_du_jour.push((*j, limite, dit));
    }
    let (mut parts, mut avis) = (Vec::new(), Vec::new());
    for (k, e) in es.iter().enumerate() {
        let rubrique = if e.nuitee { NUITEE } else { REPAS };
        let au_dela = if e.nuitee { NUITEE_AU_DELA } else { REPAS_AU_DELA };
        let mut cases: Vec<CaseDeFrais> = Vec::new();
        // Le justificatif, sur la ligne qui porte un ecart (ou qui l'a deja coche).
        let depasse_un_jour = ecarts[k].iter().any(|x| *x > Decimal::ZERO);
        if !e.nuitee && e.jours.iter().any(|(j, _)| !nuits.contains(j)) || cochee(&lignes[e.rang], LOGE) {
            cases.push(LOGE);
        }
        if depasse_un_jour || cochee(&lignes[e.rang], JUSTIFICATIF) {
            cases.push(JUSTIFICATIF);
        }
        marquer_les_cases(frais(&mut lignes[e.rang])?, &cases, rubrique, None);
        // Les jours groupes par ecart : une part soumise par groupe ; la ligne garde ce qui reste exonere, sur toute sa
        // quantite.
        let mut groupes: Vec<(Decimal, Vec<DateDeLigne>)> = Vec::new();
        for ((_, x), ecart) in e.jours.iter().zip(&ecarts[k]) {
            match groupes.iter_mut().find(|(g, _)| g == ecart) {
                Some((_, v)) => v.push(x.clone()),
                None => groupes.push((*ecart, vec![x.clone()])),
            }
        }
        let soumis: Decimal = ecarts[k].iter().sum();
        if soumis > Decimal::ZERO {
            let quantite_ligne = e.unites * Decimal::from(e.jours.len() as u64);
            let l = frais(&mut lignes[e.rang])?;
            l.marques.taux_saisi = Some(e.taux.normalize());
            l.taux = Some((e.taux - soumis / quantite_ligne).normalize());
        }
        for (ecart, dates) in groupes.into_iter().filter(|(g, _)| *g > Decimal::ZERO) {
            let quantite = e.unites * Decimal::from(dates.len() as u64);
            let premier = dates.first().map(DateDeLigne::lu).transpose()?;
            parts.push(LignePaie::Composee(ligne::Composee {
                rubrique: au_dela.into(),
                quantite: quantite.normalize(),
                taux: (ecart / e.unites).normalize(),
                heures: None,
                precision: None,
                dates,
            }));
            if let Some((j0, lim, lieu)) = limites_du_jour.iter().find(|(j, _, _)| Some(*j) == premier) {
                avis.push(Avis {
                    info: true,
                    texte: format!(
                        "{} : à l'étranger ({}), la limite du {} est de {} pour la nuitée et les repas du jour : {} par jour entrent dans le brut. Cocher « justificatif conservé » si la compagnie garde la facture.",
                        qui,
                        lieu,
                        j0.jj_mm_aaaa(),
                        socle::texte::euros(*lim),
                        socle::texte::euros(ecart)
                    ),
                });
            }
        }
    }
    Ok((parts, avis))
}

/// LES LIGNES DE LA CARTE D'UN CONTRAT PAYE AU MOIS : ses repas et ses nuitees (`FRAIS_LIMITES`). Le reste de sa paie se
/// compose de ses faits (`conges::lignes_du_mois`) : une autre ligne se refuse.
fn frais_d_un_mois_paye(ctx: &Contexte, lignes: &[LignePaie], qui: &str) -> Resultat<()> {
    for (i, l) in lignes.iter().enumerate() {
        let code = l.rubrique();
        if a_une_limite(code) {
            continue;
        }
        let libelle = rubrique(&ctx.racine, code, &ctx.lue_au)?.libelle;
        return Err(format!(
            "{} : retirez sa ligne {} (« {} ») : un contrat payé au mois ne porte en ligne qu'un repas ou une nuitée ; son \
             salaire, ses congés et ses primes se disent dans ses cases.",
            qui,
            i + 1,
            libelle
        ));
    }
    Ok(())
}

/// Chaque ligne de paie saisie dit sa quantite et son taux (ou sa base et son pourcentage) : la saisie s'enregistre
/// sans eux, en saisie, et ce refus dit ce qui manque, ligne par ligne, avec le salarie (`qui`).
///
/// Une quantite (ou une base) dit combien la ligne paie : nulle, elle ne paie rien ; negative, elle retirerait ce
/// qu'elle dit payer, et ses jours partiraient en DSN sans un euro (une absence se saisit en ligne d'absence, de
/// quantite positive : c'est sa rubrique qui retire). L'une et l'autre se refusent, en disant la ligne. Seule une ligne
/// qui ne porte que des jours a venir (`planning`, sans jour de ce mois) a une quantite nulle : elle ne paie rien ce
/// mois-ci.
fn lignes_dites(ctx: &Contexte, lignes: &[LignePaie], qui: &str) -> Resultat<()> {
    for (i, l) in lignes.iter().enumerate() {
        let r = rubrique(&ctx.racine, l.rubrique(), &ctx.lue_au)?;
        let ligne = format!("sa ligne {} (« {} »)", i + 1, r.libelle);
        // Ce que la ligne dit qu'elle paie (une quantite, une base, ou rien a compter : un montant), et son prix.
        let (quantite, taux, quoi, prix) = match l {
            LignePaie::Unites(x) => (Some(x.quantite), x.taux, "la quantité", "le montant"),
            LignePaie::UnitesEtHeures(x) => (Some(x.quantite), x.taux, "la quantité", "le montant"),
            LignePaie::Frais(x) => (Some(x.quantite), x.taux, "la quantité", "le montant"),
            LignePaie::Pourcentage(x) => (Some(x.base), x.taux_majoration, "la base", "le pourcentage"),
            LignePaie::Montant(x) => (None, x.montant, "", "le montant"),
            LignePaie::AvantageEnNature(x) => (None, x.montant, "", "le montant"),
            // UNE LIGNE QUE LE MOTEUR POSE ne se saisit pas.
            LignePaie::Composee(_) | LignePaie::RetenueArret(_) | LignePaie::IndemniteJournaliere(_) | LignePaie::Maintien(_) | LignePaie::Importee(_) => {
                return Err(format!("{} : {} ne se saisit pas : le moteur la pose lui-même. Retirez-la.", qui, ligne));
            }
        };
        if let Some(quantite) = quantite {
            let Some(dite) = quantite else {
                return Err(format!("{}{}{} de {}.", qui, LIGNE_A_DIRE, quoi, ligne));
            };
            let a_venir_seulement = l.dates().is_empty() && !l.planning().is_empty();
            if dite < Decimal::ZERO || (dite.is_zero() && !a_venir_seulement) {
                // La ligne dit ses jours et le mois ou elle se saisit : la ligne d'un mois suivant d'un contrat a cheval
                // (`stock::Objet::vue`) porte les jours de son planning, et ses heures se disent sur la carte de ce mois.
                let jours: Vec<Jour> = l.jours()?;
                let (ou, carte) = match jours.first() {
                    Some(j) => (
                        format!(", {} {}", if jours.len() > 1 { "les" } else { "le" }, jours.iter().map(Jour::jj_mm_aaaa).collect::<Vec<_>>().join(", ")),
                        format!(" sur sa carte {}", socle::texte::de(&j.mois_civil().libelle(socle::mois::Graphie::Accentuee))),
                    ),
                    None => (String::new(), String::new()),
                };
                return Err(format!(
                    "{} : {} de {}{} est {}{}{}, ou retirez la ligne (×).",
                    qui,
                    quoi,
                    ligne.trim_end_matches(')'),
                    if ou.is_empty() { ")".to_string() } else { format!("{})", ou) },
                    dite,
                    QUANTITE_SANS_COMPTE,
                    carte
                ));
            }
        }
        if taux.is_none() {
            return Err(format!("{}{}{} de {}.", qui, LIGNE_A_DIRE, prix, ligne));
        }
    }
    match duree::plus_d_une_journee(ctx, lignes, qui)? {
        Some(refus) => Err(refus),
        None => Ok(()),
    }
}

/// Ce refus de `engagements` dit-il une saisie pas encore finie ([`SAISIE_INCOMPLETE`]) ? Sa premiere ligne le dit.
pub fn saisie_incomplete(message: &str) -> bool {
    message.lines().next().is_some_and(|tete| SAISIE_INCOMPLETE.iter().any(|p| tete.contains(p)))
}

/// REGLE 1 : une ligne dont la rubrique demande ses heures (`heures` dans sa `saisie`, aucun forfait --
/// `Rubrique::heures_a_saisir` : le service de repetition, la journee, la semaine, le mois, l'absence a la journee)
/// les dit, sinon l'engagement se refuse. Sans elles, la ligne paierait et declarerait zero heure en DSN, en AEM, aux
/// Conges Spectacles et a l'assurance chomage, sans un mot : `Rubrique::heures` n'a rien d'autre a compter. Une ligne
/// de quantite nulle ne paie rien et ne compte rien : elle n'a pas d'heures a dire.
fn heures_dites(ctx: &Contexte, eng: &Engagement, qui: &str) -> Resultat<()> {
    for (i, l) in eng.lignes.iter().enumerate() {
        let r = rubrique(&ctx.racine, l.rubrique(), &ctx.lue_au)?;
        if !r.heures_a_saisir() {
            continue;
        }
        let quantite = l.quantite().ok_or_else(|| socle::fichiers::absent("quantite"))?;
        if quantite.is_zero() || l.heures().is_some_and(|h| !h.is_zero()) {
            continue;
        }
        return Err(format!(
            "{}{}ses heures par {} sur sa ligne {} (« {} »).\nCette rubrique n'a pas de forfait d'heures, comme le \
             cachet en a un : ses heures sont un fait, que la DSN, l'AEM, les Congés Spectacles et l'assurance chômage \
             comptent, et le logiciel ne les devine pas.",
            qui,
            LIGNE_A_DIRE,
            r.unite,
            i + 1,
            r.libelle
        ));
    }
    Ok(())
}

/// Une ligne dont la rubrique est reservee a un statut (`Rubrique::reservee_au_statut` : le service et le cachet, unites
/// de paie des seuls artistes) ne paie qu'un salarie de ce statut, lu comme la paie le lit
/// (`calcul::fiche_de_l_engagement`, `calcul::statut_de`). Sinon l'engagement se refuse, en disant comment le payer.
fn unites_du_statut(ctx: &Contexte, eng: &Engagement, qui: &str) -> Resultat<()> {
    let mut statut: Option<&str> = None;
    for (i, l) in eng.lignes.iter().enumerate() {
        let r = rubrique(&ctx.racine, l.rubrique(), &ctx.lue_au)?;
        let Some(voulu) = r.reservee_au_statut() else { continue };
        if statut.is_none() {
            let fiche = crate::calcul::fiche_de_l_engagement(ctx, eng)?;
            statut = Some(crate::calcul::statut_de(&fiche)?);
        }
        if statut != Some(voulu) {
            // Les heures d'un service se lisent sur lui ; celles d'un cachet sont celles du travail fait.
            let combien = if r.unite == "service" { " (4 pour un service de 4 heures)" } else { "" };
            return Err(format!(
                "{} : la ligne {} (« {} ») paie au {}, et seul un artiste se paie au {}. Payez-la à l'heure : une ligne « Heure \
                 de travail », ses heures pour quantité{}.",
                qui,
                i + 1,
                r.libelle,
                r.unite,
                r.unite,
                combien
            ));
        }
    }
    Ok(())
}

/// "1", "2", ou `None` si la fiche ne le dit pas (le NIR d'abord).
fn sexe(fiche: &droit::fiche::ChampsSalarie) -> Option<String> {
    let nir = fiche.nir.texte().trim();
    if nir.starts_with('1') || nir.starts_with('2') {
        return Some(nir[..1].to_string());
    }
    fiche.sexe.dite().map(|s| s.as_str().to_string())
}

// ------------------------------------------- le contrat qui deborde du mois

/// Le debut de l'engagement ecrit `e`, comme `Engagement` le deduirait : `debut` s'il le dit, sinon son premier jour
/// joue (`Carte::debut_lu`).
pub fn debut_brut(e: &stock::Ecrit) -> String {
    e.saisie.carte.debut_lu()
}

/// Le debut dit, sinon le premier jour joue des lignes ; vide quand ni l'un ni l'autre n'est dit (une carte neuve).
pub(crate) fn debut_de(dit: Option<String>, lignes: &[LignePaie]) -> String {
    if let Some(d) = dit.filter(|d| !d.is_empty()) {
        return d;
    }
    jours_des_lignes(lignes, false).iter().map(|j| j.jour().to_string()).min().into_iter().collect()
}

/// LA FICHE DU SALARIE de cle `cle`, par sa cle exacte : celle que la carte et l'engagement portent. Le calcul ne cherche
/// jamais un salarie par un nom (c'est l'affaire des portes : `Contexte::cle_du_salarie`) ; une cle sans fiche se refuse
/// en la nommant.
pub(crate) fn fiche_du_salarie(ctx: &Contexte, cle: &IdSalarie) -> Resultat<droit::fiche::FicheSalarie> {
    ctx.salaries.get(cle).cloned().ok_or_else(|| format!("Salarie inconnu : {}.", socle::fichiers::repr(cle)))
}

/// LA FICHE DU SPECTACLE de cle `cle`, par sa cle exacte (`fiche_du_salarie`) ; une cle sans fiche se refuse en la nommant.
pub(crate) fn fiche_du_spectacle(ctx: &Contexte, cle: &IdSpectacle) -> Resultat<droit::fiche::Spectacle> {
    let champs = ctx.spectacles.get(cle).ok_or_else(|| format!("Spectacle inconnu : {}.", socle::fichiers::repr(cle)))?;
    Ok(droit::fiche::Spectacle { cle: cle.clone(), champs: champs.clone() })
}

/// Le salarie d'une fiche, tel que l'ecran le nomme (`socle::texte::salarie_nomme`).
pub(crate) fn qui_de(fiche: &droit::fiche::ChampsSalarie) -> Resultat<String> {
    fiche.nomme().ok_or_else(|| socle::fichiers::absent("nom"))
}

/// « (n° FA1.26.13) » quand le contrat porte son numero ; rien sinon.
fn numero_dit(eng: &Engagement) -> String {
    eng.numero.iter().map(|n| format!(" (n° {})", n)).collect()
}

/// LE CONTRAT DE CET ENGAGEMENT SE FAIT-IL SIGNER PAR LE LOGICIEL ? Oui quand sa nature a un contrat ecrit (`contrat_ecrit`
/// de `contrats-base.json` : le CDD « est etabli par ecrit », L1242-12 ; tout salarie recoit l'ecrit de L1221-5-1) et
/// qu'il n'a pas ete signe ailleurs (`signe_ailleurs`).
pub fn contrat_a_signer(racine: &Path, eng: &Engagement) -> Resultat<bool> {
    Ok(eng.nat()?.modele_contrat().is_some() && !signe_ailleurs(racine, eng)?)
}

/// LE CONTRAT ECRIT DE CET ENGAGEMENT A-T-IL ETE SIGNE HORS DU LOGICIEL ? Un contrat repris d'un ancien logiciel -- celui
/// d'un permanent, ou le contrat d'usage qui continue ici un contrat importe -- a ete signe la-bas
/// (`importe::vient_d_un_ancien_logiciel`) ; un contrat commence avant le logiciel (`stock::commence_avant_le_logiciel`),
/// avant. Il ne se refait pas signer ici ; un terme qui change s'y dit par un avenant (code du travail R. 1221-40 : « un
/// document indiquant ces modifications », `atelier/reference/sources/code-du-travail/xml/R1221-40.xml`), qui ne connait
/// pas son texte et dit en clair ce qui change (`documents::avenant::changements_en_clair`).
pub fn signe_ailleurs(racine: &Path, eng: &Engagement) -> Resultat<bool> {
    signe_ailleurs_de(racine, &*eng.nat()?, &eng.salarie, &eng.debut.iso(), eng.numero.as_deref(), &eng.mois)
}

/// `signe_ailleurs`, de ce que l'engagement dit : sa nature, son salarie, son debut, son numero, et le mois d'ou on le
/// regarde.
pub fn signe_ailleurs_de(racine: &Path, nat: &Nature, salarie: &str, debut: &str, numero: Option<&str>, mois: &str) -> Resultat<bool> {
    if nat.modele_contrat().is_none() {
        return Ok(false);
    }
    Ok(stock::commence_avant_le_logiciel(racine, debut, mois)? || importe::vient_d_un_ancien_logiciel(racine, salarie, debut, numero)?)
}

/// Un contrat permanent court d'un bout a l'autre, son numero est celui de l'engagement, et le dernier mois le dit.
fn suivre_permanent(ctx: &Contexte, eng: &mut Engagement) -> Resultat<Vec<Avis>> {
    let mut avis = Vec::new();
    let fiche = fiche_du_salarie(ctx, &eng.salarie)?;
    let qui = qui_de(&fiche)?;
    let libelle = eng.nat()?.libelle.clone();
    if eng.dernier_mois()? {
        let fin = eng.fin.ok_or("fin")?;
        avis.push(Avis {
            info: true,
            texte: format!(
                "{} : {}{}, dernier mois (fin le {}). Le bulletin porte le \
                 solde de tout compte ; ensuite, signaler la fin de contrat dans \
                 les cinq jours et remettre au salarié son certificat de travail \
                 et son reçu pour solde de tout compte (Fin de contrat).",
                qui,
                libelle,
                numero_dit(eng),
                fin.jj_mm_aaaa()
            ),
        });
    }
    Ok(avis)
}

/// Ce que devient un contrat a cheval sur plusieurs mois : sur un mois qui n'est pas le premier, il porte le numero de
/// l'engagement s'il en a un et les jours du contrat entier (`stock`, `jours_contrat`), et les
/// avertissements le disent. Un contrat paye au mois a sa propre suite (`suivre_permanent`).
pub fn suivre_contrat(ctx: &Contexte, eng: &mut Engagement) -> Resultat<Vec<Avis>> {
    let mut avis = Vec::new();
    if eng.nat()?.mensuelle() {
        return suivre_permanent(ctx, eng);
    }
    if !eng.a_cheval()? {
        return Ok(avis);
    }
    let fiche = fiche_du_salarie(ctx, &eng.salarie)?;
    let qui = qui_de(&fiche)?;
    let fin = eng.fin.ok_or("contrat a cheval sans fin")?;
    let bornes = format!("du {} au {}", eng.debut.jj_mm_aaaa(), fin.jj_mm_aaaa());
    let mois_du_contrat = eng.mois_du_contrat()?;
    if !eng.premier_mois()? {
        avis.push(Avis {
            info: true,
            texte: format!(
                "{} : contrat {} en cours depuis {}{} ; le contrat et \
                 la DPAE sont faits avec son premier mois, ce mois-ci n'a que son bulletin.",
                qui,
                bornes,
                Mois::en_clair(&mois_du_contrat[0]),
                numero_dit(eng)
            ),
        });
    }
    if !eng.dernier_mois()? {
        let place = mois_du_contrat.iter().position(|m| *m == eng.mois).ok_or("mois hors du contrat")?;
        let suivant = mois_du_contrat.get(place + 1).ok_or_else(|| socle::fichiers::trop_courte("les mois du contrat"))?;
        let fin_de_phrase = if eng.premier_mois()? {
            if eng.planning_connu() {
                " Le contrat imprimé couvre toutes les dates."
            } else {
                " Le contrat imprimé chiffre ce mois-ci et renvoie au planning pour la suite."
            }
        } else {
            ""
        };
        avis.push(Avis {
            info: true,
            texte: format!(
                "{} : contrat {}, se poursuit en {} ; ce mois-ci est payé \
                 maintenant, la suite le sera avec {}. Le certificat Congés \
                 Spectacles partira à la fin du contrat, pour le contrat \
                 entier.{}",
                qui,
                bornes,
                Mois::en_clair(suivant),
                Mois::en_clair(suivant),
                fin_de_phrase
            ),
        });
    }
    Ok(avis)
}

// ------------------------------------------------------------------ charger

/// Une saisie chargee : le mois et ses paies calculees.
pub struct Saisie {
    pub mois: String,
    pub paies: Vec<crate::calcul::Paie>,
}

/// La saisie de `mois` telle que l'ecran la montre (`stock::vue_du_mois`) ; refus quand aucun engagement ne le touche.
pub fn vue_requise(racine: &Path, mois: &str) -> Resultat<saisie::VueDuMois> {
    stock::vue_du_mois(racine, mois)?.ok_or_else(|| socle::echec::cle(format!("Aucun engagement ne touche le mois {}.", mois), stock::DOSSIER))
}

/// UN NUMERO DE CONTRAT EST CELUI D'UN SEUL SALARIE (regle 4) : il se lit, il ne se partage pas. Un salarie
/// coche en plus dans un bloc deja numerote, ou un numero recopie a la main, le porterait sur deux contrats : la DSN
/// declarerait deux salaries sous le meme 40.009. Le refus nomme le numero et les deux salaries, quelle que soit la
/// face qui a ecrit la saisie.
fn un_numero_un_salarie(paies: &[crate::calcul::Paie], mois: &str) -> Resultat<()> {
    let mut vus: BTreeMap<String, String> = BTreeMap::new();
    for p in paies {
        let Some(n) = p.engagement.numero.as_deref() else { continue };
        let cle = p.salarie.id.to_string();
        match vus.get(n) {
            Some(autre) if *autre != cle => {
                return Err(format!(
                    "Saisie de {} : le numero de contrat {} est porte par deux salaries, {} et {}. Un numero est celui \
                     d'un seul contrat (il part en DSN, S21.G00.40.009) : retirez-le de l'engagement auquel il \
                     n'appartient pas ; un contrat neuf recoit le sien a la production.",
                    mois, n, autre, cle
                ));
            }
            Some(_) => {}
            None => {
                vus.insert(n.to_string(), cle);
            }
        }
    }
    Ok(())
}

/// Les engagements de la saisie `data` lus et ses paies calculees, sans numeroter les contrats ; les minima et le SMIC
/// y portent leurs avertissements. La validation (`etats`) et l'ecran (`pour_l_ecran`) passent par lui.
pub(crate) fn charger_donnees(ctx: &Contexte, data: &saisie::VueDuMois) -> Resultat<Saisie> {
    // Une derivation pour tout le mois : le brut d'un mois anterieur se
    // recalcule une fois, pas une fois par salarie (meme resultat).
    let _derivation = crate::conges::Derivation::ouvrir();
    // Un engagement en saisie se calcule comme sa validation le calculera : ses propres mois d'avant comptent.
    let mois_vu = data.mois.to_string();
    let _comme_a_la_validation = crate::etats::CommeALaValidation::ouvrir(ctx, &mois_vu)?;
    let lignes: LignesPermanent = &crate::conges::lignes_du_mois;
    let (engs, dits) = engagements(ctx, data, lignes)?;
    let mois = mois_vu.clone();
    let mut paies = Vec::new();
    for e in &engs {
        let p = crate::calcul::calculer_apres(ctx, e, &paies)?;
        paies.push(p);
    }
    un_numero_un_salarie(&paies, &mois)?;
    // Ce que la deduction des lignes d'un permanent a eu a dire, ses suspensions (bloc 65), tirees du magasin des
    // imprevus pour le contrat de chaque paie, et les jours payes pendant un imprevu (`imprevus`).
    let jours_payes = crate::imprevus::avertir_les_jours_payes(ctx, data)?;
    for ((p, dit), payes) in paies.iter_mut().zip(dits).zip(jours_payes) {
        let suspensions = crate::imprevus::suspensions_du_mois(ctx, p, &mois)?;
        let p = p.ecrire();
        p.avertissements.extend(dit);
        p.avertissements.extend(payes);
        p.engagement.suspensions = suspensions;
    }
    // Les minima conventionnels, puis le SMIC : des avertissements, jamais un refus.
    for p in paies.iter_mut() {
        let mut avis = crate::minima::avertissements(ctx, p)?;
        avis.extend(crate::minima::sous_le_smic(ctx, p)?);
        p.ecrire().avertissements.extend(avis);
    }
    // La case cadre decochee sur un metier habituellement cadre : on le dit.
    let tous = &data.engagements;
    for (p, e) in paies.iter_mut().zip(tous) {
        let m = match &e.metier {
            Some(metier) => emplois::metier(metier)?.map(|m| (metier, m)),
            None => None,
        };
        if let Some((metier, m)) = m.filter(|(_, m)| e.cadre == Some(false) && m.cadre_par_defaut(ctx.entreprise.idcc.texte()) == Some(true)) {
            fn texte<T: AsRef<str>>(c: &droit::fiche::Case<T>) -> Option<String> {
                c.remplie().map(str::to_string)
            }
            // Le metier se nomme par son libelle, ou par son code quand il n'en a pas.
            let libelle = match Some(m.libelle()).filter(|l| !l.is_empty()) {
                Some(l) => l,
                None => metier.clone(),
            };
            let msg = format!(
                "{} {}, {} : case cadre decochee, alors que ce metier est \
                 habituellement cadre. Si c'est voulu, rien a faire ; sinon, \
                 la cocher -- la retraite complementaire en depend.",
                texte(&p.salarie.nom).ok_or_else(|| socle::fichiers::absent("nom"))?,
                texte(&p.salarie.prenom_affichage).or_else(|| texte(&p.salarie.prenom)).ok_or_else(|| socle::fichiers::absent("prenom"))?,
                libelle
            );
            p.ecrire().avertissements.push(Avis { info: false, texte: msg });
        }
        // Un technicien en CDD que la liste de l'annexe 8 de la convention ne nomme pas : on dit ce qui en suit.
        let nat = p.engagement.nat()?;
        if let Some((metier, idcc)) = droit::caisse_conges::hors_de_l_annexe8(&nat, &p.salarie)? {
            let texte = format!(
                "{} : {} n'est pas sur la liste de l'annexe 8 de la convention {} ; ce CDD ne relève pas de \
                 l'intermittence (ni Congés Spectacles ni AER spectacle) ; ses congés sont payés par la compagnie.",
                qui_de(&p.salarie)?,
                metier,
                idcc
            );
            p.ecrire().avertissements.push(Avis { info: true, texte });
        }
        // Une journee au-dela de la duree quotidienne maximale : on le dit, l'employeur en repond.
        if let Some(texte) = duree::au_dela_du_maximum(ctx, e.lignes(), &qui_de(&p.salarie)?)? {
            p.ecrire().avertissements.push(Avis { info: false, texte });
        }
    }
    // Les contrats qui debordent du mois.
    for p in paies.iter_mut() {
        let p = p.ecrire();
        let avis = suivre_contrat(ctx, &mut p.engagement)?;
        p.avertissements.extend(avis);
    }
    Ok(Saisie { mois, paies })
}

// ------------------------------------------------------------------ les gestes de l'ecran

/// LE CALCUL D'UNE SAISIE POUR L'ECRAN : les paies de la saisie `data` (un formulaire, enregistre ou non) telles que
/// la validation les calculera, avec leurs avertissements (minima, SMIC, contrat qui deborde du mois), et chacune son
/// prelevement a la source (`etats::prelevement`). Rien ne s'ecrit et rien ne part d'elles : un document ne lit que les
/// paies ecrites d'un engagement valide (`etats`), sauf le bulletin modele de l'ecran, qui imprime celles-ci.
#[cfg(feature = "ecran")]
pub fn pour_l_ecran(ctx: &Contexte, data: &saisie::VueDuMois) -> Resultat<Saisie> {
    let mut s = charger_donnees(ctx, data)?;
    for p in s.paies.iter_mut() {
        let pas = etats::prelevement(ctx, p)?;
        p.ecrire().pas = pas;
    }
    Ok(s)
}

/// Les engagements de la saisie `data`, lus tels que l'ecran les montre (les lignes d'un permanent deduites de ses
/// conges et de sa fin de contrat), sans paie.
pub fn engagements_de_la_saisie(ctx: &Contexte, data: &saisie::VueDuMois) -> Resultat<Vec<Engagement>> {
    let _derivation = conges::Derivation::ouvrir();
    Ok(engagements(ctx, data, &conges::lignes_du_mois)?.0)
}

/// Le refus que donne la lecture de la saisie `data` quand elle est encore incomplete (`saisie_incomplete`), lignes
/// d'un permanent mises a part ; `None` si elle se lit, ou si elle refuse pour une autre raison.
pub fn refus_de_saisie_incomplete(ctx: &Contexte, data: &saisie::VueDuMois) -> Option<String> {
    let vide: LignesPermanent = &|_, _, _| Ok((Vec::new(), Vec::new(), None));
    match engagements(ctx, data, vide) {
        Err(m) if saisie_incomplete(&m) => Some(m),
        _ => None,
    }
}

/// UNE EMBAUCHE : un engagement et la fiche du salarie telle que sa paie la lit (`calcul::fiche_de_l_engagement`),
/// sans paie ; les montants de la `Paie` rendue sont a zero. C'est ce que la DPAE declare et ce que lit un signalement.
pub fn embauche(ctx: &Contexte, eng: &Engagement) -> Resultat<calcul::Paie> {
    let nat = eng.nat()?;
    let sal = calcul::fiche_de_l_engagement(ctx, eng)?;
    let spec = if nat.mensuelle() {
        None
    } else {
        Some(fiche_du_spectacle(ctx, eng.spectacle.as_ref().ok_or_else(|| socle::fichiers::absent("spectacle"))?)?)
    };
    let zero = Decimal::new(0, 2);
    let tp = calcul::plafond_temps_partiel(&ctx.entreprise, eng, &sal)?;
    Ok(calcul::Paie::nouveau(eng.clone(), sal, spec, zero, zero, zero, 0.0, zero, tp))
}

/// LA SAISIE LUE SANS PAIE (l'ecran, le panneau, les DPAE a faire) : l'embauche de chaque engagement de la saisie `data`
/// (`embauche`), dans l'ordre de l'ecran ; chaque contrat qui deborde du mois suivi (`suivre_contrat` : il reprend le
/// numero de son premier mois) ; et les lignes de remuneration de chaque engagement paye au cachet telles que le calcul
/// les comprend (elles ne dependent que de l'engagement). Rien ne se calcule au-dela.
#[cfg(feature = "ecran")]
pub fn lue_sans_paie(ctx: &Contexte, data: &saisie::VueDuMois) -> Resultat<Vec<calcul::Paie>> {
    let mut paies = Vec::new();
    for e in &engagements_de_la_saisie(ctx, data)? {
        paies.push(embauche(ctx, e)?);
    }
    for p in paies.iter_mut() {
        let p = p.ecrire();
        suivre_contrat(ctx, &mut p.engagement)?;
        if !p.engagement.nat()?.mensuelle() {
            p.remuneration = calcul::lignes_brut(&p.engagement, ctx)?;
        }
    }
    Ok(paies)
}

/// Le preavis de l'engagement `e` (sa saisie), pour une anciennete et un motif de rupture : la loi (L1234-1) et la
/// convention, et laquelle s'applique (`solde::duree_du_preavis`).
#[cfg(feature = "ecran")]
pub fn preavis(ctx: &Contexte, e: &saisie::Carte, anciennete: Decimal, motif: Option<&str>) -> Resultat<solde::Preavis> {
    solde::duree_du_preavis(ctx, e, anciennete, motif)
}

/// LES LIGNES `presentes` SONT-ELLES CELLES DE `partie` ARRETEES A LA `fin` (AAAA-MM-JJ) d'un contrat rompu
/// (`etats::seule_la_rupture`) : une ligne garde ses jours jusqu'a la fin, celle qui n'en a plus disparait, et la quantite
/// d'une ligne coupee ne croit pas.
pub(crate) fn lignes_coupees_a_la_fin(partie: &[LignePaie], presentes: &[LignePaie], fin: &str) -> bool {
    let mut attendues: Vec<(LignePaie, bool)> = Vec::new();
    for l in partie {
        let gardes: Vec<DateDeLigne> = l.dates().iter().filter(|j| j.jour() <= fin).cloned().collect();
        if !l.dates().is_empty() && gardes.is_empty() {
            continue;
        }
        let coupee = gardes.len() != l.dates().len();
        attendues.push((l.avec_les_jours(gardes, l.planning().to_vec()), coupee));
    }
    presentes.len() == attendues.len()
        && presentes.iter().zip(&attendues).all(|(p, (a, coupee))| if *coupee { p.sans_quantite() == a.sans_quantite() && p.quantite() <= a.quantite() } else { p == a })
}

/// CE QU'UN REFUS DU CALCUL PORTE A PART (`socle::echec::avec`) : le champ a dire, quand il y en a un, et le detail, pour
/// le developpeur.
#[derive(serde::Serialize)]
pub(crate) struct Detail {
    #[serde(skip_serializing_if = "Option::is_none")]
    cle: Option<&'static str>,
    detail: String,
}

impl Detail {
    pub(crate) fn du(cle: Option<&'static str>, detail: String) -> Detail {
        Detail { cle, detail }
    }
}
