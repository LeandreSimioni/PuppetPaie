

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
pub mod conges;
pub mod cumul;
pub mod duree;
pub mod departs;
pub mod ecarts;
pub mod empreinte;
pub mod heures_en_plus;
pub mod etats;
pub mod importe;
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

use serde_json::{Map, Value};

use droit::catalogue::rubrique;
use droit::contexte::Contexte;
use droit::natures::{self, Nature};
use socle::date::{d, en_texte, forme_ecrite, Jour};
use socle::mois::Mois;
use droit::emplois;
use socle::fichiers::vrai;
use socle::num::{dec, dec_f64, dec_avec_echelle, div, flottant, nombre, Decimal, Resultat};

#[derive(Debug, Clone)]
pub struct Engagement {
    pub salarie: String,
    /// Le salarié tel qu'une phrase d'écran le nomme (« NOM Prénom »).
    pub nom: String,
    pub spectacle: Option<String>,
    pub dates_travail: Vec<Jour>,
    pub lignes: Vec<Value>,
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
    pub nature: Option<String>,
    pub quotite_contrat: Option<f64>,
    pub quotite_reference_saisie: Option<f64>,
    pub modalite_temps_travail_saisie: Option<String>,
    pub motif_recours: Option<String>,
    pub motif_rupture: Option<String>,
    pub notification_rupture: Option<String>,
    pub preavis: Option<Value>,
    /// Le dernier jour travaille et paye au salaire habituel que la saisie dit (`dernier_jour_paye`, AAAA-MM-JJ) ;
    /// ce que la DSN en declare se lit par `fctu::dernier_jour_paye`.
    pub dernier_jour_paye: Option<String>,
    /// Le jour ou prend effet ce que la saisie de CE mois change au contrat (avenant : nature, motif, duree du
    /// travail, emploi, date de debut ; AAAA-MM-JJ) : la « Date de la modification » du bloc 41 (`dsn::changements_contrat`).
    /// Il ne se recopie pas d'un mois a l'autre (`moteur::routes::mois::DEFINIT_LE_MENSUEL`) : le mois suivant declare deja
    /// la nouvelle valeur.
    pub date_modification: Option<String>,
    pub transaction: Option<String>,
    pub suspensions: Vec<Value>,
    /// Les termes du contrat AVANT l'avenant que `date_modification` date en cours de mois (`salaire_mensuel`,
    /// `quotite_contrat`), quand la saisie les dit : la paie du mois se decoupe alors au jour de l'avenant
    /// (`periodes::bornes`). Comme `date_modification`, il ne se recopie pas d'un mois a l'autre.
    pub avant: Option<Value>,
    /// Une periode elementaire de paie : les bornes de CE calcul quand la paie du mois est decoupee
    /// (`periodes`) ; `None` : la periode de paie entiere (le contrat borne au mois).
    pub periode: Option<(Jour, Jour)>,
    /// Ce que la saisie dit du contrat ECRIT et que rien d'autre ne porte (`CHAMPS_DU_CONTRAT_ECRIT`) : la periode
    /// d'essai (L1242-12, 6° ; R1221-34, 7°), la personne remplacee (L1242-12, 1°), la repartition des horaires d'un
    /// temps partiel (L3123-6, 1°), les clauses particulieres de ce contrat. Vide : rien n'est dit.
    pub ecrit: Map<String, Value>,
}

/// Les champs de la saisie que seul le contrat ecrit lit (`Engagement::ecrit`).
pub const CHAMPS_DU_CONTRAT_ECRIT: [&str; 4] = ["periode_essai_jours", "personne_remplacee", "repartition_horaire", "clauses_particulieres"];

// ------------------------------------------------------------------ lecture

/// Une valeur facultative lue comme texte (`None` et absent se confondent).
fn chaine(v: Option<&Value>) -> Option<String> {
    match v {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(autre) => Some(en_texte(autre)),
    }
}

fn flottant_opt(v: Option<&Value>) -> Resultat<Option<f64>> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(x) => flottant(x).map(Some),
    }
}

fn entier_opt(v: Option<&Value>, nom: &str) -> Resultat<Option<i64>> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n
            .as_i64()
            .or_else(|| n.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64))
            .map(Some)
            .ok_or_else(|| socle::echec::a_la_cle("Un nombre entier est attendu.", nom, n)),
        Some(autre) => Err(socle::echec::a_la_cle("Un nombre entier est attendu.", nom, forme_ecrite(autre))),
    }
}

fn truthy(v: Option<&Value>) -> bool {
    v.map_or(false, vrai)
}

/// Le lieu en texte, sans blancs aux bords ; vide s'il manque.
fn lieu_texte(v: Option<&Value>) -> String {
    match v {
        Some(x) if vrai(x) => en_texte(x).trim().to_string(),
        _ => String::new(),
    }
}

/// `x or y` sur deux valeurs facultatives, rendu comme texte.
fn ou(a: Option<&Value>, b: Option<&Value>) -> Option<String> {
    if truthy(a) {
        chaine(a)
    } else {
        chaine(b)
    }
}

/// Une ligne saisie en pourcentage (`base` x `taux_majoration` %) rendue en `quantite` x `taux`.
pub(crate) fn ligne_en_pourcentage(l: &Value) -> Resultat<Value> {
    let o = match l {
        Value::Object(o) if o.contains_key("base") || o.contains_key("taux_majoration") => o,
        _ => return Ok(l.clone()),
    };
    let rub = o.get("rubrique").map_or("None".to_string(), en_texte);
    let (base, pct) = match (o.get("base"), o.get("taux_majoration")) {
        (Some(b), Some(p)) => (dec_avec_echelle(b)?, dec_avec_echelle(p)?),
        _ => {
            return Err(socle::echec::a_la_cle(
                &format!("La ligne « {} » est en pourcentage : elle dit sa base (en euros) et son pourcentage.", rub),
                "taux_majoration",
                forme_ecrite(l),
            ))
        }
    };
    if base.is_sign_negative() && !base.is_zero() || pct.is_sign_negative() && !pct.is_zero() {
        return Err(format!("Ligne << {} >> : base et pourcentage sont positifs.", rub));
    }
    // Decimal(pct) / Decimal(100) : le quotient exact, a l'exposant ideal.
    let mut taux = div(pct, Decimal::ONE_HUNDRED).normalize();
    if taux.scale() < pct.scale() {
        taux.rescale(pct.scale());
    }
    for (champ, attendu) in [("quantite", base), ("taux", taux)] {
        if let Some(x) = o.get(champ) {
            let vide = x.is_null() || x.as_str() == Some("");
            if !vide && dec_avec_echelle(x)? != attendu {
                return Err(socle::echec::cle(
                    format!(
                        "La ligne « {} » : sa quantité ou son taux ({}) contredit sa base et son pourcentage ({} x {} %). N'écrire que la base et le pourcentage.",
                        rub,
                        en_texte(x),
                        base,
                        pct
                    ),
                    champ,
                ));
            }
        }
    }
    let mut sortie = o.clone();
    sortie.insert("quantite".into(), Value::String(base.to_string()));
    sortie.insert("taux".into(), Value::String(taux.to_string()));
    Ok(Value::Object(sortie))
}

/// Les jours d'une liste (dates jouees ou planning), avec leurs lieux.
/// `strict` : l'ecriture detaillee sans `jour` est refusee avec son message
/// (dates jouees) ; sinon c'est le `KeyError` de `x["jour"]` (planning).
fn jours_et_lieux(liste: &[Value], strict: bool) -> Resultat<(Vec<Jour>, BTreeMap<Jour, String>)> {
    let (mut jours, mut lieux) = (Vec::new(), BTreeMap::new());
    for x in liste {
        let jour = match x {
            Value::Object(m) => {
                let Some(j) = m.get("jour") else {
                    return Err(if strict {
                        format!(
                            "Une date ecrite en detail doit dire son `jour` : \
                             {{\"jour\": \"2026-10-02\", \"lieu\": \"Berlin\"}}. Recu : {}",
                            forme_ecrite(x)
                        )
                    } else {
                        socle::fichiers::absent("jour")
                    });
                };
                let jour = d(j)?;
                let l = lieu_texte(m.get("lieu"));
                if !l.is_empty() {
                    lieux.insert(jour, l);
                }
                jour
            }
            autre => d(autre)?,
        };
        jours.push(jour);
    }
    Ok((jours, lieux))
}

fn liste<'a>(v: Option<&'a Value>, nom: &str) -> Resultat<&'a [Value]> {
    match v {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(a)) => Ok(a),
        Some(autre) => Err(socle::echec::a_la_cle("Une liste est attendue.", nom, forme_ecrite(autre))),
    }
}

impl Engagement {
    /// « NOM Prénom, contrat N » : qui une phrase de refus désigne.
    pub fn qui(&self) -> String {
        format!("{}, contrat {}", self.nom, self.numero.as_deref().unwrap_or("sans numero"))
    }

    /// L'engagement que `v` décrit (`salarie`,
    /// `spectacle`, `dates_travail`, `lignes`, `debut`, ...). `racine` est
    /// celle des donnees de la compagnie, pour le catalogue des rubriques. `qui` : le salarie tel qu'un refus le nomme
    /// (`salarie_nomme`).
    pub fn nouveau(racine: &Path, v: &Value, qui: &str) -> Resultat<Engagement> {
        let o = v.as_object().ok_or("un engagement est un objet")?;
        let get = |k: &str| o.get(k);

        // Les deux ecritures d'une date se rejoignent ici.
        let mut lieux: BTreeMap<Jour, String> = BTreeMap::new();
        if let Some(Value::Object(m)) = get("lieux") {
            for (k, x) in m {
                lieux.insert(d(&Value::String(k.clone()))?, en_texte(x));
            }
        }
        let (mut jours_joues, lieux_dates) = jours_et_lieux(liste(get("dates_travail"), "dates_travail")?, true)?;
        lieux.extend(lieux_dates);
        jours_joues.sort();
        let presents: BTreeSet<Jour> = jours_joues.iter().copied().collect();
        lieux.retain(|j, _| presents.contains(j));

        let Some(Value::Array(lignes_brutes)) = get("lignes").filter(|x| !x.is_null()) else {
            return Err(socle::echec::cle("Un engagement se saisit en lignes de paie, et celui-ci n'en porte pas.", "lignes"));
        };
        let lignes: Vec<Value> = lignes_brutes.iter().map(ligne_en_pourcentage).collect::<Resultat<_>>()?;

        // Un refus nomme le salarie : c'est lui que la personne voit sur la carte.
        let qui = qui.to_string();
        let mois_saisi = ou(get("mois"), None).unwrap_or_default();
        let debut_saisi = truthy(get("debut"));
        let fin_saisie = truthy(get("fin"));
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
        let nature = chaine(get("nature"));
        let nat = natures::nature(nature.as_deref())?;
        let mensuel = nat.mensuelle();
        let planning_brut = liste(get("planning"), "planning")?;
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
        let mut premier_cachet: Option<&Value> = None;
        // Les rubriques se lisent a la fin du mois paye (le mois dit, sinon celui de la premiere date jouee). Un mois
        // illisible se refuse plus loin, avec ce qui ne va pas : d'ici la, les rubriques se lisent au jour du moteur.
        let jour_lu = Mois::lire(&mois_saisi).map(Mois::dernier_jour)
            .ok()
            .or_else(|| jours_joues.first().and_then(|j| Some(j.mois_civil().dernier_jour())))
            .unwrap_or_else(droit::contexte::jour_reel);
        for l in &lignes {
            let code = l.get("rubrique").ok_or_else(|| socle::fichiers::absent("rubrique"))?;
            let code = code.as_str().map_or_else(|| en_texte(code), String::from);
            let r = rubrique(racine, &code, &jour_lu)?;
            let q = flottant(l.get("quantite").ok_or_else(|| socle::fichiers::absent("quantite"))?)?;
            if r.unite == "cachet" {
                cachets_somme += q;
                premier_cachet.get_or_insert(l);
            } else {
                // Le signe porte aussi sur les heures, comme au calcul
                // (`calcul::lignes_brut`) : une absence en retire. Un acompte,
                // au forfait et sans heures, n'en retire aucune.
                let signe = if r.sens == "-" { -1.0 } else { 1.0 };
                autres += signe * r.heures(q, flottant_opt(l.get("heures"))?);
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
        let mut taux_cachet = match get("taux_cachet") {
            None | Some(Value::Null) => None,
            Some(x) => Some(flottant(x)?),
        };
        if let Some(l) = premier_cachet {
            taux_cachet = Some(flottant(l.get("taux").ok_or_else(|| socle::fichiers::absent("taux"))?)?);
        }

        // Les jours travailles : un fait qui se saisit, sinon les dates distinctes.
        let jours = entier_opt(get("jours"), "jours")?.unwrap_or(presents.len() as i64);
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
        let (brut, planning_lieux) = jours_et_lieux(planning_brut, false)?;
        let mut planning: Vec<Jour> = Vec::new();
        for j in brut {
            if !planning.contains(&j) {
                planning.push(j);
            }
        }
        planning.sort();
        let debut = if debut_saisi {
            d(get("debut").unwrap())?
        } else {
            *jours_joues.first().ok_or_else(|| socle::fichiers::trop_courte("les jours joués"))?
        };
        let fin = if fin_saisie {
            Some(d(get("fin").unwrap())?)
        } else if mensuel {
            None
        } else {
            let derniere = *jours_joues.last().ok_or_else(|| socle::fichiers::trop_courte("les jours joués"))?;
            Some(derniere.max(planning.last().copied().unwrap_or(derniere)))
        };
        // LA FIN D'UN CONTRAT SANS TERME est sa rupture, et elle ne vaut qu'a sa date : le mois paye avant celui ou elle
        // tombe ne la connait pas. Un CDI n'a pas de date de fin previsionnelle (S21.G00.40.010 n'est exigee que d'un
        // contrat a duree determinee, CCH-12, `atelier/reference/norme-dsn/texte/bloc-S21.G00.40.txt`, l. 517-527) ; sa
        // fin se declare dans la DSN du mois ou il finit (bloc S21.G00.62). La paie d'un mois d'avant ne change donc pas
        // quand la rupture se saisit.
        let fin = match fin {
            Some(f) if nat.duree_indeterminee && Mois::lire(&mois_saisi).map(Mois::dernier_jour).is_ok_and(|m| f > m) => None,
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
        let jours_contrat = match entier_opt(get("jours_contrat"), "jours_contrat")? {
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
            salarie: chaine(get("salarie")).unwrap_or_default(),
            nom: qui.to_string(),
            spectacle: chaine(get("spectacle")),
            dates_travail: jours_joues,
            lignes,
            debut,
            fin,
            numero: numeros::NumeroContrat::de_la_valeur(get("numero"))?,
            mois,
            planning,
            planning_lieux,
            jours,
            jours_contrat,
            date_paiement: chaine(get("date_paiement")),
            versement: Default::default(),
            taux_cachet,
            cachets,
            heures,
            lieu: chaine(get("lieu")).unwrap_or_default(),
            lieux,
            emploi: chaine(get("emploi")),
            emploi_affichage: chaine(get("emploi_affichage")),
            pcs_ese: chaine(get("pcs_ese")),
            code_emploi_ft: chaine(get("code_emploi_ft")),
            statut: chaine(get("statut")),
            statut_conventionnel: chaine(get("statut_conventionnel")),
            statut_categoriel_rc: chaine(get("statut_categoriel_rc")),
            etablissement: chaine(get("etablissement")).filter(|t| !t.trim().is_empty()),
            nature,
            quotite_contrat: flottant_opt(get("quotite_contrat"))?,
            quotite_reference_saisie: flottant_opt(get("quotite_reference_saisie"))?,
            modalite_temps_travail_saisie: chaine(get("modalite_temps_travail_saisie")),
            motif_recours: chaine(get("motif_recours")),
            motif_rupture: chaine(get("motif_rupture")),
            notification_rupture: chaine(get("notification_rupture")),
            preavis: get("preavis").filter(|x| !x.is_null()).cloned(),
            dernier_jour_paye: chaine(get("dernier_jour_paye")).filter(|t| !t.is_empty()),
            date_modification: chaine(get("date_modification")).filter(|t| !t.trim().is_empty()),
            transaction: chaine(get("transaction")),
            suspensions: liste(get("suspensions"), "suspensions")?.to_vec(),
            avant: get("avant").filter(|x| !x.is_null()).cloned(),
            periode: None,
            ecrit: CHAMPS_DU_CONTRAT_ECRIT
                .iter()
                .filter_map(|k| get(k).filter(|x| truthy(Some(x))).map(|x| (k.to_string(), x.clone())))
                .collect(),
        })
    }

    // ------------------------------------------------ le contrat et le mois

    /// La nature de contrat, relue a chaque acces.
    pub fn nat(&self) -> Resultat<Arc<Nature>> {
        natures::nature(self.nature.as_deref())
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
        Ok(self.fin.map_or(fin_mois, |f| f.min(fin_mois)))
    }

    /// Le jour du versement de ce que l'engagement paie (S21.G00.50.001, la « date de paiement » du bulletin) : celui que
    /// la saisie dit pour lui, sinon celui que la compagnie a choisi (`droit::versement`). C'est le jour ou se fait la
    /// retenue a la source (CGI, art. 1671, 1, `atelier/reference/sources/code-general-des-impots/articles-20261021.md`),
    /// donc celui ou se lit le taux de PAS du salarie.
    pub fn date_du_versement(&self) -> Resultat<Jour> {
        match self.date_paiement.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
            Some(d) => Jour::lire(d.get(..10).unwrap_or(d)),
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
        [self.lieux.get(jour), self.planning_lieux.get(jour)]
            .into_iter()
            .flatten()
            .find(|l| !l.is_empty())
            .cloned()
            .unwrap_or_else(|| self.lieu.clone())
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
    pub fn canon(&self) -> Resultat<Value> {
        let iso = |j: &Jour| Value::String(j.iso());
        let jours = |v: &[Jour]| Value::Array(v.iter().map(iso).collect());
        let lieux = |m: &BTreeMap<Jour, String>| {
            Value::Object(m.iter().map(|(j, l)| (j.iso(), Value::String(l.clone()))).collect::<Map<_, _>>())
        };
        let texte = |o: &Option<String>| o.clone().map_or(Value::Null, Value::String);
        let nombre_f = |x: f64| dec_f64(x).map(|v| Value::String(nombre(v)));
        let optionnel = |o: Option<f64>| o.map_or(Ok(Value::Null), nombre_f);
        let entier = |n: i64| Value::String(n.to_string());
        let mut m = Map::new();
        m.insert("salarie".into(), Value::String(self.salarie.clone()));
        m.insert("spectacle".into(), texte(&self.spectacle));
        m.insert("dates_travail".into(), jours(&self.dates_travail));
        m.insert("lignes".into(), canon_valeur(&Value::Array(self.lignes.clone()))?);
        m.insert("debut".into(), iso(&self.debut));
        m.insert("fin".into(), self.fin.as_ref().map_or(Value::Null, iso));
        m.insert("numero".into(), self.numero.clone().map_or(Value::Null, Value::from));
        m.insert("mois".into(), Value::String(self.mois.clone()));
        m.insert("planning".into(), jours(&self.planning));
        m.insert("planning_lieux".into(), lieux(&self.planning_lieux));
        m.insert("jours".into(), entier(self.jours));
        m.insert("jours_contrat".into(), entier(self.jours_contrat));
        m.insert("date_paiement".into(), texte(&self.date_paiement));
        m.insert("taux_cachet".into(), optionnel(self.taux_cachet)?);
        m.insert("cachets".into(), entier(self.cachets));
        m.insert("heures".into(), optionnel(self.heures)?);
        m.insert("lieu".into(), Value::String(self.lieu.clone()));
        m.insert("lieux".into(), lieux(&self.lieux));
        m.insert("emploi".into(), texte(&self.emploi));
        // Ecrit seulement quand il est dit.
        if let Some(a) = &self.emploi_affichage {
            m.insert("emploi_affichage".into(), Value::String(a.clone()));
        }
        m.insert("pcs_ese".into(), texte(&self.pcs_ese));
        m.insert("code_emploi_ft".into(), texte(&self.code_emploi_ft));
        m.insert("statut".into(), texte(&self.statut));
        m.insert("statut_conventionnel".into(), texte(&self.statut_conventionnel));
        m.insert("statut_categoriel_rc".into(), texte(&self.statut_categoriel_rc));
        // Ecrit seulement quand il est dit.
        if let Some(n) = &self.etablissement {
            m.insert("etablissement".into(), Value::String(n.clone()));
        }
        m.insert("nature".into(), Value::String(self.nat()?.cle.clone()));
        m.insert("quotite_contrat".into(), optionnel(self.quotite_contrat)?);
        m.insert("quotite_reference_saisie".into(), optionnel(self.quotite_reference_saisie)?);
        m.insert("modalite_temps_travail_saisie".into(), texte(&self.modalite_temps_travail_saisie));
        m.insert("motif_recours".into(), texte(&self.motif_recours));
        m.insert("motif_rupture".into(), texte(&self.motif_rupture));
        m.insert("notification_rupture".into(), texte(&self.notification_rupture));
        m.insert(
            "preavis".into(),
            match &self.preavis {
                Some(p) => canon_valeur(p)?,
                None => Value::Null,
            },
        );
        m.insert("transaction".into(), texte(&self.transaction));
        m.insert("suspensions".into(), canon_valeur(&Value::Array(self.suspensions.clone()))?);
        // Ecrit seulement quand la saisie le dit.
        if !self.ecrit.is_empty() {
            m.insert("contrat_ecrit".into(), canon_valeur(&Value::Object(self.ecrit.clone()))?);
        }
        // Ce qui se deduit de l'engagement : sa periode, sa quotite, ses mois.
        m.insert("debut_periode".into(), iso(&self.debut_periode()?));
        m.insert("fin_periode".into(), iso(&self.fin_periode()?));
        m.insert("quotite".into(), Value::String(nombre(self.quotite()?)));
        m.insert("sans_paie".into(), Value::Bool(self.sans_paie()));
        m.insert("premier_mois".into(), Value::Bool(self.premier_mois()?));
        m.insert("dernier_mois".into(), Value::Bool(self.dernier_mois()?));
        m.insert(
            "mois_du_contrat".into(),
            Value::Array(self.mois_du_contrat()?.into_iter().map(Value::String).collect()),
        );
        Ok(Value::Object(m))
    }
}

/// La forme canonique d'une valeur JSON : nombres en chaines normalisees, cles
/// en `_` retirees (sauf `_cle`).
pub fn canon_valeur(v: &Value) -> Resultat<Value> {
    Ok(match v {
        Value::Number(_) => Value::String(nombre(dec(v)?)),
        Value::Array(a) => Value::Array(a.iter().map(canon_valeur).collect::<Resultat<_>>()?),
        Value::Object(o) => Value::Object(
            o.iter()
                .filter(|(k, _)| !k.starts_with('_') || *k == "_cle")
                .map(|(k, x)| canon_valeur(x).map(|c| (k.clone(), c)))
                .collect::<Resultat<Map<_, _>>>()?,
        ),
        autre => autre.clone(),
    })
}

// ------------------------------------------------------- depuis un fichier

/// Un avertissement pose sur la paie ; `info` : renseigne sans rien demander.
#[derive(Debug, Clone, PartialEq)]
pub struct Avis {
    pub info: bool,
    pub texte: String,
}

/// UNE LIGNE DE FRAIS NE FAIT PAS LES JOURS DE L'ENGAGEMENT : sa rubrique est de nature `frais` au catalogue de la
/// compagnie (lu au premier jour de la ligne). Un remboursement de frais professionnels n'est pas un revenu d'activite
/// (CSS L136-1-1, `atelier/reference/sources/code-de-la-securite-sociale/articles-20260923.md`, l. 47) : ce qui fait le
/// debut, la fin et les jours d'un engagement, ce sont ses lignes de travail. Le frais d'un voyage pour rejoindre le
/// lieu de travail, ou en revenir, est a la charge de l'employeur (CCN 1285, art. VIII.5.1, KALIARTI000005808081,
/// `atelier/reference/sources/kali-entrepot/articles.jsonl.gz`) : il se paie avec l'engagement qui le porte, au mois de
/// celui-ci, quel que soit son jour. Le temps du trajet, lui, est du travail ou une servitude qui se compte (art.
/// VIII.1, KALIARTI000023306777) : il se saisit sur une ligne de travail, qui fait son jour. Une rubrique que le catalogue
/// ne connait pas compte comme du travail : le calcul la refuse en la nommant.
pub fn ligne_de_frais(racine: &Path, l: &Value) -> bool {
    let Some(code) = l.get("rubrique").filter(|c| vrai(c)).map(en_texte) else { return false };
    let premier = ["dates", "planning"].iter().find_map(|k| l.get(*k).and_then(Value::as_array).and_then(|a| a.first()).cloned());
    let Some(jour) = premier.and_then(|x| d(x.get("jour").unwrap_or(&x)).ok()) else { return false };
    rubrique(racine, &code, &jour).is_ok_and(|r| r.nature == "frais")
}

/// `dates_engagement` (`cle` = "dates") et `planning_engagement` (`cle` =
/// "planning") : la reunion, dedoublonnee, des jours des lignes de travail (`ligne_de_frais`), chacun nu ou
/// detaille `{jour, lieu}` quand une ligne dit ou elle se joue.
pub(crate) fn jours_des_lignes(racine: &Path, lignes: &[Value], cle: &str) -> Vec<Value> {
    let (mut out, mut vus) = (Vec::new(), BTreeSet::new());
    for l in lignes.iter().filter(|l| !ligne_de_frais(racine, l)) {
        let Some(Value::Array(js)) = l.get(cle) else { continue };
        for j in js {
            let jour = match j {
                Value::Object(m) => m.get("jour").cloned().unwrap_or(Value::Null),
                autre => autre.clone(),
            };
            if !vus.insert(jour.to_string()) {
                continue;
            }
            let lieu_ligne = || l.get("lieu").filter(|x| vrai(x)).map(en_texte).unwrap_or_default();
            let ou = match j {
                Value::Object(m) => m.get("lieu").filter(|x| vrai(x)).map(en_texte).unwrap_or_else(lieu_ligne),
                _ => lieu_ligne(),
            };
            out.push(if ou.is_empty() {
                jour
            } else {
                let mut m = Map::new();
                m.insert("jour".into(), jour);
                m.insert("lieu".into(), Value::String(ou));
                Value::Object(m)
            });
        }
    }
    out
}

/// Le jour d'une date de ligne (`"AAAA-MM-JJ"` ou `{"jour": …}`), en texte.
pub fn jour_ecrit(x: &Value) -> String {
    match x {
        Value::Object(m) => m.get("jour").map(en_texte).unwrap_or_default(),
        autre => en_texte(autre),
    }
}

/// LES LIGNES D'UN MOIS SANS FAITS d'un contrat d'usage, tirees des lignes `source` du dernier mois qui en dit
/// (`stock::Objet::lignes_du_planning`) : une ligne par ligne qui a des jours au planning de `mois` ou apres, a la meme
/// rubrique, au meme taux et au meme lieu, ses jours de `mois` en dates et ceux d'apres au planning ; un cachet, une
/// journee ou un service par date (une autre unite, des heures, ne se deduit pas d'un nombre de jours : quantite 0, a
/// completer).
pub(crate) fn lignes_du_planning(racine: &Path, source: &[Value], mois: &str) -> Resultat<Vec<Value>> {
    let mut lignes = Vec::new();
    for l in source {
        let plan: Vec<Value> = l.get("planning").and_then(Value::as_array).cloned().unwrap_or_default();
        let dedans: Vec<Value> = plan.iter().filter(|x| jour_ecrit(x).starts_with(mois)).cloned().collect();
        let apres: Vec<Value> = plan.iter().filter(|x| jour_ecrit(x).get(..7).is_some_and(|m| m > mois)).cloned().collect();
        if dedans.is_empty() && apres.is_empty() {
            continue;
        }
        let code = l.get("rubrique").map(en_texte).unwrap_or_else(|| "cachet".into());
        let unite = Mois::lire(mois).map(Mois::dernier_jour).and_then(|j| rubrique(racine, &code, &j)).map(|r| r.unite).unwrap_or_default();
        let par_date = matches!(unite.as_str(), "cachet" | "jour" | "service");
        let mut n = Map::new();
        n.insert("rubrique".into(), Value::String(code));
        n.insert("taux".into(), l.get("taux").cloned().unwrap_or(Value::Null));
        if let Some(lieu) = l.get("lieu").filter(|x| vrai(x)) {
            n.insert("lieu".into(), lieu.clone());
        }
        n.insert("quantite".into(), if par_date { Value::from(dedans.len()) } else { Value::from(0) });
        if let Some(h) = l.get("heures").filter(|x| par_date && vrai(x)) {
            n.insert("heures".into(), h.clone());
        }
        n.insert("dates".into(), Value::Array(dedans));
        n.insert("planning".into(), Value::Array(apres));
        lignes.push(Value::Object(n));
    }
    Ok(lignes)
}

/// Les jours, sans doublon, que des lignes jouent ou planifient.
pub(crate) fn jours_de_lignes(racine: &Path, lignes: &[Value]) -> usize {
    let cle = |j: &Value| match j {
        Value::Object(m) => m.get("jour").map_or(String::new(), |x| x.to_string()),
        autre => autre.to_string(),
    };
    let mut vus: BTreeSet<String> = jours_des_lignes(racine, lignes, "dates").iter().map(cle).collect();
    vus.extend(jours_des_lignes(racine, lignes, "planning").iter().map(cle));
    vus.len()
}

/// Le nom d'un salarie tel qu'une donnee le dit. TRAVAIL A FAIRE (chaine A) : `conges.rs` (l. 43) le passe encore en
/// valeur ; il ne reste alors que le texte.
pub(crate) trait NomDit {
    fn nom_dit(&self) -> Option<String>;
}

impl NomDit for &str {
    fn nom_dit(&self) -> Option<String> {
        Some(self.to_string())
    }
}

impl NomDit for &Value {
    fn nom_dit(&self) -> Option<String> {
        chaine(Some(self))
    }
}

/// Le salarie qu'un refus nomme en tete, tel que l'ecran le nomme partout (`Contexte::nomme` : « NOM Prénom ») ;
/// « Cet engagement » quand la carte n'a pas encore de salarie.
pub(crate) fn salarie_nomme<N: NomDit>(ctx: &Contexte, salarie: Option<N>) -> String {
    salarie.and_then(|s| s.nom_dit()).filter(|s| !s.trim().is_empty()).map(|c| ctx.nomme(&c)).unwrap_or_else(|| "Cet engagement".to_string())
}

/// Ce que `conges::lignes_du_mois` rend pour un engagement paye
/// au mois : les lignes de paie du mois, ce qu'il y a a en dire, et ses jours travailles (`None` : la saisie les dit).
pub type LignesPermanent<'a> = &'a dyn Fn(&Contexte, &Value, &str) -> Resultat<(Vec<Value>, Vec<Avis>, Option<i64>)>;

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
    let defaut_taux = data.taux_cachet.clone().map_or(Value::Null, Value::Number);
    let sans_taux = Value::Null;
    let (mut out, mut dits) = (Vec::new(), Vec::new());
    for carte in &data.engagements {
        let qui = carte.salarie_dit().map(|c| ctx.nomme(c)).unwrap_or_else(|| "Cet engagement".to_string());
        let nat = carte.nature_lue().map_err(|m| format!("{} : {}", qui, m))?;
        let tarifee;
        let carte = match if nat.mensuelle() { None } else { lignes_au_tarif(ctx, carte.lignes(), &mois_dit)? } {
            Some(lignes) => {
                tarifee = saisie::Carte { lignes: Some(lignes), ..carte.clone() };
                &tarifee
            }
            None => carte,
        };
        // TRAVAIL A FAIRE (chaine A) : la carte se lit ici par ses cles, comme ce qu'elle passe a ceux qui la lisent
        // encore ainsi (`frais_au_dela`, `frais_d_un_mois_paye`, `lignes_dites`, `conges::lignes_du_mois`,
        // `importe::continue_par`, `notifiee_pendant_le_contrat`) et a `Engagement::nouveau`.
        let brut = carte.en_valeur();
        let e = &brut;
        let mut spectacle = carte.spectacle.clone().filter(|s| !s.is_empty());
        // Les heures en plus sont des faits d'un contrat paye au mois (`heures_en_plus`) : ceux d'un contrat d'usage ne
        // seraient payes par rien.
        if !nat.mensuelle() {
            let dites: Vec<&str> = [heures_en_plus::HEURES_SUPPLEMENTAIRES, heures_en_plus::HEURES_SUPPLEMENTAIRES_A_50, heures_en_plus::HEURES_COMPLEMENTAIRES]
                .into_iter()
                .filter(|k| truthy(e.get(*k)))
                .collect();
            if !dites.is_empty() {
                return Err(format!(
                    "{} : {} est un fait d'un contrat payé au mois ; un {} paie ses heures par ses lignes (une ligne « Heure de \
                     travail », ou « Majoration » pour leur majoration). Retirer {}. Rien n'est écrit.",
                    qui,
                    dites.join(", "),
                    nat.libelle,
                    if dites.len() > 1 { "ces champs" } else { "ce champ" }
                ));
            }
        }
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
                     un « CDD d'usage du spectacle », et c'est ce que vaut un engagement qui ne dit pas sa nature.",
                    qui, nat.libelle
                ));
            }
            // Un permanent ne joue pas un spectacle (S21.G00.40.050, CCH-11).
            if spectacle.as_deref().map_or(false, |s| !s.is_empty()) && truthy(e.get("spectacle")) {
                return Err(format!(
                    "{} : retirez le spectacle ({}) de cet engagement, ou choisissez le CDD d'usage si ce contrat est \
                     celui d'un spectacle.\n\
                     Le numéro d'objet spectacle est interdit sur un {} (S21.G00.40.050, CCH-11).",
                    qui,
                    spectacle.as_deref().unwrap_or(""),
                    nat.libelle
                ));
            }
            spectacle = None;
        } else if spectacle.as_deref().map_or(true, str::is_empty) {
            // Un salaire au mois sans nature : c'est la nature qui manque, pas le spectacle (un engagement muet est un
            // contrat d'usage aux cachets, qui n'a pas de salaire au mois).
            if !truthy(e.get("nature")) && ["salaire_mensuel", "quotite_contrat"].iter().any(|k| truthy(e.get(*k))) {
                return Err(format!(
                    "{}{}\nUn engagement qui ne la dit pas est un {}, payé aux cachets ; un salaire au mois est celui d'un \
                     CDI ou d'un CDD.",
                    qui, PAS_DE_NATURE, nat.libelle
                ));
            }
            return Err(format!(
                "{}{}\n\
                 Le spectacle porte le numéro d'objet, qui part en DSN, en AEM et sur le bulletin : il se choisit, il ne \
                 se devine pas.",
                qui, PAS_DE_SPECTACLE
            ));
        }
        let jours = if nat.mensuelle() { Vec::new() } else { carte.dates(&ctx.racine) };
        // `lignes: []` ecrit est un mois du contrat sans cachet ; une ligne sans date est incomplete.
        if jours.is_empty() && !nat.mensuelle() && carte.lignes.as_ref().is_none_or(|l| !l.is_empty()) {
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
                let salarie = ctx.salarie(carte.salarie.as_deref().ok_or_else(|| socle::fichiers::absent("salarie"))?)?;
                let feminin = sexe(&salarie) == "2";
                Some(emplois::resoudre(code, carte.cadre, feminin, ctx.entreprise.idcc.texte()).map_err(|m| format!("{} : {}", qui, m))?)
            }
            None => None,
        };
        // Les jours travailles d'un contrat paye au mois : ceux que le moteur tire du mois (`conges::lignes_du_mois`).
        let mut jours_du_mois: Option<Value> = None;
        let lignes = if nat.mensuelle() {
            let (lignes, avis, jours_travailles) =
                lignes_permanent(ctx, e, &mois_dit)?;
            dits.push(avis);
            jours_du_mois = jours_travailles.map(Value::from);
            Some(Value::Array(lignes))
        } else {
            match e.get("lignes") {
                Some(Value::Array(l)) => {
                    let (lignes, avis) = frais_au_dela(ctx, carte.salarie.as_deref(), l.clone(), &mois_dit, &qui)?;
                    dits.push(avis);
                    Some(Value::Array(lignes))
                }
                autre => {
                    dits.push(Vec::new());
                    autre.cloned()
                }
            }
        };
        let mut champs = Map::new();
        let mut poser = |k: &str, v: Option<&Value>| {
            if let Some(v) = v {
                champs.insert(k.to_string(), v.clone());
            }
        };
        poser("salarie", e.get("salarie"));
        poser("spectacle", spectacle.as_ref().map(|s| Value::String(s.clone())).as_ref());
        poser("dates_travail", Some(&Value::Array(jours)));
        // Un contrat payé au mois ne prend pas le cachet par défaut du mois : son calcul ne le lit pas, et sa paie ne se
        // recalcule pas quand il change.
        poser("taux_cachet", Some(e.get("taux_cachet").unwrap_or(if nat.mensuelle() { &sans_taux } else { &defaut_taux })));
        poser("debut", e.get("debut"));
        poser("fin", e.get("fin"));
        poser("numero", e.get("numero"));
        poser("mois", Some(&Value::String(mois_dit.clone())));
        poser("jours", e.get("jours").or(jours_du_mois.as_ref()));
        // Un contrat d'usage commence dans l'ancien logiciel et continue ici : les jours du contrat entier comptent ceux
        // que les pieces importees attestent (`importe::Importe::jours_du_contrat_continue`).
        let continue_ = match &carte.jours_contrat {
            None if !nat.mensuelle() && carte.debut_lu(&ctx.racine).get(..7).is_some_and(|m| m < mois_dit.as_str()) => {
                crate::importe::continue_par(&ctx.racine, carte)?.and_then(|i| i.jours_du_contrat_continue(carte.jours_du_contrat(&ctx.racine))).map(Value::from)
            }
            _ => None,
        };
        poser("jours_contrat", e.get("jours_contrat").or(continue_.as_ref()));
        poser("planning", Some(&Value::Array(carte.planning(&ctx.racine))));
        poser("lignes", lignes.as_ref());
        poser("date_paiement", e.get("date_paiement"));
        poser("lieu", e.get("lieu"));
        // Ce que la carte ecrit du metier passe avant ce que le catalogue en dit.
        let d = declare.as_ref();
        let dit = |x: &Option<String>| x.clone().filter(|s| !s.is_empty());
        for (champ, v) in [
            ("emploi", dit(&carte.emploi).or_else(|| d.map(|d| d.emploi.clone()))),
            ("pcs_ese", dit(&carte.pcs_ese).or_else(|| d.and_then(|d| d.pcs_ese.clone()))),
            ("code_emploi_ft", dit(&carte.code_emploi_ft).or_else(|| d.and_then(|d| d.code_emploi_ft.clone()))),
            ("statut", dit(&carte.statut).or_else(|| d.map(|d| d.statut.clone()))),
            ("statut_conventionnel", dit(&carte.statut_conventionnel).or_else(|| d.and_then(|d| d.statut_conventionnel.clone()))),
            ("statut_categoriel_rc", dit(&carte.statut_categoriel_rc).or_else(|| d.map(|d| d.statut_categoriel_rc.to_string()))),
        ] {
            poser(champ, v.map(Value::String).as_ref());
        }
        // Le libelle que `resoudre` a donne a CE metier, feminin compris ; un
        // `emploi` ecrit a la main dirait un autre metier que lui.
        if dit(&carte.emploi).is_none() {
            poser("emploi_affichage", d.map(|d| Value::String(d.emploi_affichage.clone())).as_ref());
        }
        poser("nature", e.get("nature"));
        poser("quotite_contrat", e.get("quotite_contrat"));
        poser("quotite_reference_saisie", e.get("quotite_reference").filter(|q| truthy(Some(q))));
        poser("modalite_temps_travail_saisie", e.get("modalite_temps_travail"));
        poser("motif_recours", e.get("motif_recours"));
        poser("etablissement", e.get("etablissement"));
        poser("motif_rupture", e.get("motif_rupture"));
        // La fin du contrat telle que la saisie la dit : la mensuelle et le FCTU en tirent le meme S21.G00.62.006
        // (`fctu::dernier_jour_paye`).
        poser("preavis", e.get("preavis"));
        poser("dernier_jour_paye", e.get("dernier_jour_paye"));
        poser("date_modification", e.get("date_modification"));
        poser("avant", e.get("avant"));
        for k in CHAMPS_DU_CONTRAT_ECRIT {
            poser(k, e.get(k));
        }
        let mut eng = Engagement::nouveau(&ctx.racine, &Value::Object(champs), &qui)?;
        eng.versement = droit::versement::Versement::de_la_fiche(&ctx.entreprise_au(&Mois::lire(&eng.mois)?.dernier_jour()), &*eng.nat()?)?;
        // Ce que l'ancien logiciel a verse est un fait : seul ce que la compagnie va verser se juge.
        if carte.importe.is_none() {
            if let Some(refus) = versement_refuse(ctx, &eng, &qui)? {
                return Err(refus);
            }
        }
        notifiee_pendant_le_contrat(e, &eng)?;
        // Les lignes que la saisie ecrit disent leurs heures. Celles qu'un mois de permanent deduit, quand sa
        // saisie n'en ecrit pas (`conges::lignes_du_mois`), portent la quotite du contrat en heures : rien a controler.
        if !nat.mensuelle() || e.get("lignes").map_or(false, |l| !l.is_null()) {
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

fn notifiee_pendant_le_contrat(e: &Value, eng: &Engagement) -> Resultat<()> {
    let Some(t) = e.get("notification_rupture").and_then(Value::as_str).filter(|t| !t.trim().is_empty()) else { return Ok(()) };
    let n = Jour::lire(t).map_err(|_| format!("{} : « Notifiée le » {} n'est pas une date.", eng.nom, t))?;
    if n >= eng.debut && eng.fin.map_or(true, |f| n <= f) {
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
const PAS_DE_SPECTACLE: &str = " : choisissez le spectacle que joue cet engagement, sur sa carte.";
const PAS_DE_NATURE: &str = " : choisissez la nature du contrat, sur sa carte (`nature`).";
const PAS_DE_JOUR: &str = " : cochez les jours joués sur sa ligne de paie.";
const LIGNE_A_DIRE: &str = " : saisissez ";
const QUANTITE_SANS_COMPTE: &str = " : saisissez un nombre plus grand que zéro";
/// Une journee de plus de 24 heures (`duree::plus_d_une_journee`) : une faute de frappe, a corriger comme une saisie pas finie.
pub(crate) const JOURNEE_DE_PLUS_DE_24: &str = ", et une journée n'a que 24 heures";
const SAISIE_INCOMPLETE: [&str; 7] =
    [PAS_DE_SPECTACLE, PAS_DE_NATURE, PAS_DE_JOUR, LIGNE_A_DIRE, QUANTITE_SANS_COMPTE, JOURNEE_DE_PLUS_DE_24, crate::maintien::IJ_A_DIRE];

/// LE TARIF DE LA CONVENTION d'une rubrique a ce jour (`tarifs_conventionnels.<code>` de la couche de la convention du
/// bareme, liste datee : l'avenant qui le fixe et sa date d'effet y sont cites). `None` : la convention n'en fixe pas, ou
/// pas encore a ce jour ; le taux se saisit.
pub fn tarif_conventionnel(ctx: &Contexte, code: &str, jour: &Jour) -> Resultat<Option<Value>> {
    let bar = ctx.bareme(jour)?;
    Ok(bar.get("tarifs_conventionnels").and_then(|t| t.get(code)).filter(|v| v.is_number()).cloned())
}

/// Les lignes d'un engagement au cachet dont la rubrique a un tarif de la convention (`tarif_conventionnel`) et qui ne
/// disent pas leur taux le prennent, au premier jour de la ligne (a defaut, au premier jour du mois saisi). Le
/// bareme ne se lit que pour une telle ligne.
fn lignes_au_tarif(ctx: &Contexte, lignes: &[Value], mois: &str) -> Resultat<Option<Vec<Value>>> {
    let vide = |v: Option<&Value>| matches!(v, None | Some(Value::Null)) || v.and_then(Value::as_str).is_some_and(|s| s.trim().is_empty());
    let mut neuves = lignes.to_vec();
    let mut change = false;
    for l in neuves.iter_mut() {
        let Some(code) = l.get("rubrique").and_then(Value::as_str) else { continue };
        if !vide(l.get("taux")) || !matches!(rubrique(&ctx.racine, code, &ctx.lue_au), Ok(r) if r.saisie.iter().any(|s| s == "taux")) {
            continue;
        }
        let premier = l.get("dates").and_then(Value::as_array).and_then(|a| a.first()).map(|x| d(x.get("jour").unwrap_or(x)));
        let jour = match premier {
            Some(j) => j?,
            None => Mois::lire(mois)?.premier_jour(),
        };
        if let Some(t) = tarif_conventionnel(ctx, code, &jour)? {
            l["taux"] = t;
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
/// Les cases d'une ligne de frais : la compagnie garde le justificatif de la depense ; la nuit est a Paris ou en petite
/// couronne ; le salarie est contraint de prendre son repas au restaurant.
pub const JUSTIFICATIF: &str = "justificatif";
pub const PARIS: &str = "paris";
pub const RESTAURANT: &str = "restaurant";
/// La marque, sur une ligne de frais calculee, des cases qui ont lieu d'etre (`cases` de l'apercu).
pub const CASES: &str = "_cases";
/// Le taux de la ligne avant que sa part au-dela de la limite s'en detache : celui que la saisie montre.
pub const TAUX_SAISI: &str = "_taux_saisi";

/// Le lieu d'une ligne de nuitee ou de repas (le choix « Lieu », sous « Avancé ») : absent, la metropole ; sinon un
/// territoire de `frais_hors_metropole` du bareme (`dom`, `pacifique`, `monaco`), les Terres australes et antarctiques
/// (`taaf`), ou un lieu a l'etranger, par son code de la table des missions (`droit::missions`).
pub const TERRITOIRE: &str = "territoire";
/// Les territoires hors metropole que le choix « Lieu » propose, avant les lieux a l'etranger : la cle du bareme
/// (`frais_hors_metropole`), et ce que le choix dit.
pub const TERRITOIRES: [(&str, &str); 4] = [
    ("dom", "Outre-mer : Antilles, Guyane, Réunion, Mayotte, Saint-Pierre-et-Miquelon, Saint-Barthélemy, Saint-Martin"),
    ("pacifique", "Outre-mer : Nouvelle-Calédonie, Wallis-et-Futuna, Polynésie française"),
    ("taaf", "Outre-mer : Terres australes et antarctiques françaises"),
    ("monaco", "Monaco"),
];
/// La case d'un repas a l'etranger un jour sans nuitee de l'engagement : la compagnie a paye l'hotel.
pub const LOGE: &str = "loge";
/// L'aide de chaque case d'une ligne de frais, les montants de son annee (`{case: aide}`, avec `CASES`).
pub const AIDES: &str = "_aides";
/// Le rang d'une ligne de frais parmi les lignes de la carte : les cases de l'ecran le lisent (`CASES`). Les lignes d'un
/// contrat paye au mois suivent celles que le moteur compose.
pub const RANG: &str = "_rang";

/// Les deux limites d'une rubrique de frais au barème, et la case de la ligne qui choisit la plus haute.
struct Limites {
    rubrique: &'static str,
    au_dela: &'static str,
    cle: &'static str,
    case: &'static str,
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

/// Une ligne de cette rubrique se saisit-elle sur la carte d'un contrat paye au mois ? Ses repas et ses nuitees, aux
/// memes limites que ceux de tout salarie ; le reste de sa paie se compose de ses faits.
pub fn se_saisit_au_mois(code: &str) -> bool {
    FRAIS_LIMITES.iter().any(|x| x.rubrique == code)
}

/// La reduction de l'indemnite de mission a l'etranger quand le salarie est loge gratuitement (arrete du 3 juillet
/// 2006, art. 2 ; BOSS § 1390).
const REDUCTION_LOGE: Decimal = Decimal::from_parts(65, 0, 0, false, 2);

/// L'aide d'une case, avec les limites de l'annee (`haute`, `basse`).
fn aide_de_case(case: &str, rubrique: &str, limites: Option<(Decimal, Decimal)>) -> Option<String> {
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

/// Le jour d'une date de ligne (`"AAAA-MM-JJ"` ou `{"jour": …}`).
fn jour_de(x: &Value) -> Resultat<Jour> {
    d(x.get("jour").unwrap_or(x))
}

/// LES FRAIS DE DEPLACEMENT AU-DELA DE LEUR LIMITE. Au-dela, la fraction excedentaire entre dans l'assiette, sauf si
/// l'employeur prouve qu'elle a ete utilisee conformement a son objet (BOSS § 270 ; § 1280) : une ligne soumise
/// (`au_dela`), sauf quand la ligne dit que la compagnie garde le justificatif (`JUSTIFICATIF`). La ligne de frais garde
/// la limite, hors assiette. Le lieu de la ligne (`TERRITOIRE`) dit la limite :
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
pub(crate) fn frais_au_dela(ctx: &Contexte, salarie: Option<&str>, lignes: Vec<Value>, mois: &str, qui: &str) -> Resultat<(Vec<Value>, Vec<Avis>)> {
    let limites_de = |l: &Value| FRAIS_LIMITES.iter().find(|x| l.get("rubrique").and_then(Value::as_str) == Some(x.rubrique));
    if !lignes.iter().any(|l| limites_de(l).is_some()) {
        return Ok((lignes, Vec::new()));
    }
    let sal = ctx.salarie(salarie.ok_or_else(|| socle::fichiers::absent("salarie"))?)?;
    let cochee = |l: &Value, k: &str| l.get(k).is_some_and(vrai);
    let territoire = |l: &Value| l.get(TERRITOIRE).and_then(Value::as_str).filter(|t| !t.is_empty()).map(String::from);
    let a_l_etranger = |l: &Value| territoire(l).is_some_and(|t| !TERRITOIRES.iter().any(|(k, _)| *k == t));
    // Les nuits payees en nuitee : le salarie y est en grand deplacement.
    let mut nuits: Vec<Jour> = Vec::new();
    for l in lignes.iter().filter(|l| l.get("rubrique").and_then(Value::as_str) == Some(NUITEE)) {
        for x in l.get("dates").and_then(Value::as_array).into_iter().flatten() {
            nuits.push(jour_de(x)?);
        }
    }
    let (mut out, mut au_dela, mut avis) = (Vec::new(), Vec::new(), Vec::new());
    // Les lignes a l'etranger se comptent ensemble, jour par jour, apres les autres : leur rang dans `out`.
    let mut etrangeres: Vec<usize> = Vec::new();
    for (rang, mut l) in lignes.into_iter().enumerate() {
        let Some(lim) = limites_de(&l) else {
            out.push(l);
            continue;
        };
        l[RANG] = Value::from(rang);
        let mut cases: Vec<&str> = Vec::new();
        let taux = match l.get("taux") {
            Some(t) if !t.is_null() && !t.as_str().is_some_and(|s| s.trim().is_empty()) => Some(dec(t)?),
            _ => None,
        };
        let dates: Vec<Value> = l.get("dates").and_then(Value::as_array).cloned().unwrap_or_default();
        let jour = match dates.first() {
            Some(x) => jour_de(x)?,
            None => Mois::lire(mois)?.premier_jour(),
        };
        let bar = ctx.bareme_lu(&jour)?;
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
                            let j = jour_de(x)?;
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
            let mut p = Map::new();
            p.insert("rubrique".into(), Value::String(lim.au_dela.into()));
            p.insert("quantite".into(), l.get("quantite").cloned().unwrap_or(Value::Null));
            p.insert("taux".into(), nombre_dec(taux - limite));
            if !dates.is_empty() {
                p.insert("dates".into(), Value::Array(dates));
            }
            au_dela.push(Value::Object(p));
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
            l[TAUX_SAISI] = nombre_dec(taux);
            l["taux"] = nombre_dec(limite);
        }
        out.push(l);
    }
    let (parts, dits) = frais_a_l_etranger(&mut out, &etrangeres, &nuits, qui)?;
    avis.extend(dits);
    // Les parts au-dela viennent apres les lignes saisies : une ligne garde son rang (la case de l'ecran le lit).
    out.extend(au_dela);
    out.extend(parts);
    Ok((out, avis))
}

/// Pose sur la ligne `l` les cases qui ont lieu d'etre (`CASES`) et leurs aides (`AIDES`).
fn marquer_les_cases(l: &mut Value, cases: &[&str], rubrique: &str, metropole: Option<(Decimal, Decimal)>) {
    if cases.is_empty() {
        return;
    }
    l[CASES] = Value::Array(cases.iter().map(|k| Value::String(k.to_string())).collect());
    let aides: Map<String, Value> = cases.iter().filter_map(|k| aide_de_case(k, rubrique, metropole).map(|a| (k.to_string(), Value::String(a)))).collect();
    l[AIDES] = Value::Object(aides);
}

/// LES LIGNES A L'ETRANGER (`etrangeres`, leurs rangs dans `lignes`), jour par jour : le montant du jour de chaque ligne
/// (son taux par sa quantite d'un jour), leur somme, et l'indemnite de mission du lieu du jour (celui de sa nuitee, sinon
/// du premier repas), reduite de 65 % un jour sans nuitee dont un repas dit que la compagnie a paye l'hotel (`LOGE`).
/// Ce qui depasse s'impute sur la nuitee du jour, puis sur les repas, hors des lignes au justificatif conserve : une
/// part soumise par ecart d'un jour, et la ligne garde ce qui reste exonere sur toute sa quantite. Rend les parts
/// soumises et les avis.
fn frais_a_l_etranger(lignes: &mut [Value], etrangeres: &[usize], nuits: &[Jour], qui: &str) -> Resultat<(Vec<Value>, Vec<Avis>)> {
    let cochee = |l: &Value, k: &str| l.get(k).is_some_and(vrai);
    if etrangeres.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    // Pour chaque ligne : ses jours, et ce qu'elle paie un jour (taux x quantite d'un jour).
    struct Etrangere {
        rang: usize,
        jours: Vec<(Jour, Value)>,
        par_jour: Decimal,
        unites: Decimal,
        taux: Decimal,
        nuitee: bool,
    }
    let mut es: Vec<Etrangere> = Vec::new();
    for &i in etrangeres {
        let l = &lignes[i];
        let taux = match l.get("taux") {
            Some(t) if !t.is_null() && !t.as_str().is_some_and(|s| s.trim().is_empty()) => dec(t)?,
            _ => continue,
        };
        let quantite = l.get("quantite").map(dec).transpose()?.unwrap_or(Decimal::ONE);
        let dates: Vec<Value> = l.get("dates").and_then(Value::as_array).cloned().unwrap_or_default();
        if dates.is_empty() {
            return Err(format!("{} : une ligne de frais à l'étranger dit ses jours : l'indemnité de mission est celle d'un jour. Rien n'est calculé.", qui));
        }
        let mut jours = Vec::new();
        for x in &dates {
            jours.push((jour_de(x)?, x.clone()));
        }
        let unites = quantite / Decimal::from(jours.len() as u64);
        es.push(Etrangere { rang: i, jours, par_jour: taux * unites, unites, taux, nuitee: l.get("rubrique").and_then(Value::as_str) == Some(NUITEE) });
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
        let lieu_de = |k: usize| lignes[es[k].rang].get(TERRITOIRE).and_then(Value::as_str).unwrap_or("").to_string();
        let lieu = lieu_de(nuitee.or(du_jour.first()).map_or(0, |x| x.0));
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
        limites_du_jour.push((*j, limite, droit::missions::nom(&lieu)?.unwrap_or(lieu)));
    }
    let (mut parts, mut avis) = (Vec::new(), Vec::new());
    for (k, e) in es.iter().enumerate() {
        let rubrique = if e.nuitee { NUITEE } else { REPAS };
        let au_dela = if e.nuitee { NUITEE_AU_DELA } else { REPAS_AU_DELA };
        let mut cases: Vec<&str> = Vec::new();
        // Le justificatif, sur la ligne qui porte un ecart (ou qui l'a deja coche).
        let depasse_un_jour = ecarts[k].iter().any(|x| *x > Decimal::ZERO);
        if !e.nuitee && e.jours.iter().any(|(j, _)| !nuits.contains(j)) || cochee(&lignes[e.rang], LOGE) {
            cases.push(LOGE);
        }
        if depasse_un_jour || cochee(&lignes[e.rang], JUSTIFICATIF) {
            cases.push(JUSTIFICATIF);
        }
        marquer_les_cases(&mut lignes[e.rang], &cases, rubrique, None);
        // Les jours groupes par ecart : une part soumise par groupe ; la ligne garde ce qui reste exonere, sur toute sa
        // quantite.
        let mut groupes: Vec<(Decimal, Vec<Value>)> = Vec::new();
        for ((_, x), ecart) in e.jours.iter().zip(&ecarts[k]) {
            match groupes.iter_mut().find(|(g, _)| g == ecart) {
                Some((_, v)) => v.push(x.clone()),
                None => groupes.push((*ecart, vec![x.clone()])),
            }
        }
        let soumis: Decimal = ecarts[k].iter().sum();
        if soumis > Decimal::ZERO {
            let quantite_ligne = e.unites * Decimal::from(e.jours.len() as u64);
            let l = &mut lignes[e.rang];
            l[TAUX_SAISI] = nombre_dec(e.taux);
            l["taux"] = nombre_dec(e.taux - soumis / quantite_ligne);
        }
        for (ecart, dates) in groupes.into_iter().filter(|(g, _)| *g > Decimal::ZERO) {
            let quantite = e.unites * Decimal::from(dates.len() as u64);
            let mut p = Map::new();
            p.insert("rubrique".into(), Value::String(au_dela.into()));
            p.insert("quantite".into(), nombre_dec(quantite));
            p.insert("taux".into(), nombre_dec(ecart / e.unites));
            p.insert("dates".into(), Value::Array(dates.clone()));
            parts.push(Value::Object(p));
            let premier = dates.first().map(jour_de).transpose()?;
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

/// Un montant en nombre JSON.
fn nombre_dec(x: Decimal) -> Value {
    serde_json::from_str(&x.normalize().to_string()).unwrap_or(Value::Null)
}

/// LES LIGNES DE LA CARTE D'UN CONTRAT PAYE AU MOIS : ses repas et ses nuitees (`FRAIS_LIMITES`). Le reste de sa paie se
/// compose de ses faits (`conges::lignes_du_mois`) : une autre ligne se refuse.
fn frais_d_un_mois_paye(ctx: &Contexte, lignes: &[Value], qui: &str) -> Resultat<()> {
    for (i, l) in lignes.iter().enumerate() {
        let code = l.get("rubrique").map(|c| c.as_str().map_or_else(|| en_texte(c), String::from)).unwrap_or_default();
        if se_saisit_au_mois(&code) {
            continue;
        }
        let libelle = rubrique(&ctx.racine, &code, &ctx.lue_au).map(|r| r.libelle).unwrap_or(code);
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
fn lignes_dites(ctx: &Contexte, lignes: &[Value], qui: &str) -> Resultat<()> {
    let vide = |v: Option<&Value>| matches!(v, None | Some(Value::Null)) || v.and_then(Value::as_str).is_some_and(|s| s.trim().is_empty());
    for (i, l) in lignes.iter().enumerate() {
        let Some(code) = l.get("rubrique") else { continue };
        let r = rubrique(&ctx.racine, &code.as_str().map_or_else(|| en_texte(code), String::from), &ctx.lue_au)?;
        let pourcentage = r.saisie.iter().any(|s| s == "taux_majoration");
        let (quantite, taux) = if pourcentage { ("base", "taux_majoration") } else { ("quantite", "taux") };
        let ligne = format!("sa ligne {} (« {} »)", i + 1, r.libelle);
        // UNE LIGNE QUE LE MOTEUR POSE (`saisie` vide : la part d'une nuitée au-delà de la limite) ne se saisit pas.
        if r.saisie.is_empty() {
            return Err(format!("{} : {} ne se saisit pas : le moteur la pose lui-même. Retirez-la.", qui, ligne));
        }
        if vide(l.get(quantite)) {
            let quoi = if pourcentage { "la base" } else { "la quantité" };
            return Err(format!("{}{}{} de {}.", qui, LIGNE_A_DIRE, quoi, ligne));
        }
        let n = flottant_opt(l.get(quantite)).ok().flatten();
        let a_venir_seulement = l.get("dates").and_then(Value::as_array).is_none_or(Vec::is_empty)
            && l.get("planning").and_then(Value::as_array).is_some_and(|p| !p.is_empty());
        if n.is_some_and(|n| n < 0.0 || (n == 0.0 && !a_venir_seulement)) {
            let quoi = if pourcentage { "la base" } else { "la quantité" };
            // La ligne dit ses jours et le mois ou elle se saisit : la ligne d'un mois suivant d'un contrat a cheval
            // (`stock::Objet::vue`) porte les jours de son planning, et ses heures se disent sur la carte de ce mois.
            let jours: Vec<Jour> = l.get("dates").and_then(Value::as_array).into_iter().flatten().filter_map(|x| d(x.get("jour").unwrap_or(x)).ok()).collect();
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
                en_texte(l.get(quantite).unwrap_or(&Value::Null)),
                QUANTITE_SANS_COMPTE,
                carte
            ));
        }
        if vide(l.get(taux)) {
            let quoi = if pourcentage { "le pourcentage" } else { "le montant" };
            return Err(format!("{}{}{} de {}.", qui, LIGNE_A_DIRE, quoi, ligne));
        }
    }
    match duree::plus_d_une_journee(ctx, lignes, qui)? {
        Some(refus) => Err(refus),
        None => Ok(()),
    }
}

/// Ce refus de `engagements` dit-il une saisie pas encore finie ([`SAISIE_INCOMPLETE`]) ? Sa premiere ligne le dit.
pub fn saisie_incomplete(message: &str) -> bool {
    let tete = message.lines().next().unwrap_or("");
    SAISIE_INCOMPLETE.iter().any(|p| tete.contains(p))
}

/// REGLE 1 : une ligne dont la rubrique demande ses heures (`heures` dans sa `saisie`, aucun forfait --
/// `Rubrique::heures_a_saisir` : le service de repetition, la journee, la semaine, le mois, l'absence a la journee)
/// les dit, sinon l'engagement se refuse. Sans elles, la ligne paierait et declarerait zero heure en DSN, en AEM, aux
/// Conges Spectacles et a l'assurance chomage, sans un mot : `Rubrique::heures` n'a rien d'autre a compter. Une ligne
/// de quantite nulle ne paie rien et ne compte rien : elle n'a pas d'heures a dire.
fn heures_dites(ctx: &Contexte, eng: &Engagement, qui: &str) -> Resultat<()> {
    for (i, l) in eng.lignes.iter().enumerate() {
        let Some(code) = l.get("rubrique") else { continue };
        let r = rubrique(&ctx.racine, &code.as_str().map_or_else(|| en_texte(code), String::from), &ctx.lue_au)?;
        if !r.heures_a_saisir() {
            continue;
        }
        let quantite = flottant(l.get("quantite").ok_or_else(|| socle::fichiers::absent("quantite"))?)?;
        if quantite == 0.0 || flottant_opt(l.get("heures"))?.map_or(false, |h| h != 0.0) {
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
/// (`calcul::fiche_de_l_engagement` ; vide, un artiste). Sinon l'engagement se refuse, en disant comment le payer.
fn unites_du_statut(ctx: &Contexte, eng: &Engagement, qui: &str) -> Resultat<()> {
    let mut statut: Option<String> = None;
    for (i, l) in eng.lignes.iter().enumerate() {
        let Some(code) = l.get("rubrique") else { continue };
        let r = rubrique(&ctx.racine, &code.as_str().map_or_else(|| en_texte(code), String::from), &ctx.lue_au)?;
        let Some(voulu) = r.reservee_au_statut() else { continue };
        if statut.is_none() {
            let fiche = crate::calcul::fiche_de_l_engagement(ctx, eng)?;
            statut = Some(fiche.statut.dite().map_or("artiste", |s| s.as_str()).to_string());
        }
        if statut.as_deref() != Some(voulu) {
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

/// "1", "2", ou "" si la fiche ne le dit pas (le NIR d'abord).
fn sexe(fiche: &droit::fiche::ChampsSalarie) -> String {
    let nir = fiche.nir.texte().trim();
    if nir.starts_with('1') || nir.starts_with('2') {
        return nir[..1].to_string();
    }
    fiche.sexe.dite().map_or(String::new(), |s| s.as_str().to_string())
}

// ------------------------------------------- le contrat qui deborde du mois

/// La cle de la fiche a laquelle un engagement brut de la saisie se rattache : son `salarie` lu par `Contexte::salarie`
/// (sans tenir compte de la casse ni des accents), comme le fait le calcul de sa paie. `None` s'il ne nomme aucune fiche.
/// La fiche du salarie de l'engagement ecrit `e`, lue comme le calcul la lit (`Carte::cle`).
pub fn cle_de_l_engagement(ctx: &Contexte, e: &stock::Ecrit) -> Option<droit::contexte::IdSalarie> {
    e.saisie.carte.cle(ctx)
}
/// Le debut de l'engagement ecrit `e`, comme `Engagement` le deduirait : `debut` s'il le dit, sinon son premier jour
/// joue (`Carte::debut_lu`).
pub fn debut_brut(racine: &Path, e: &stock::Ecrit) -> String {
    e.saisie.carte.debut_lu(racine)
}

/// Le debut dit, sinon le premier jour joue des lignes.
pub(crate) fn debut_de(racine: &Path, dit: Option<String>, lignes: &[Value]) -> String {
    let cle = |j: &Value| match j {
        Value::Object(m) => m.get("jour").map_or(String::new(), en_texte),
        autre => en_texte(autre),
    };
    if let Some(d) = dit.filter(|d| !d.is_empty()) {
        return d;
    }
    let mut jours: Vec<String> = jours_des_lignes(racine, lignes, "dates").iter().map(cle).collect();
    jours.sort();
    jours.into_iter().next().unwrap_or_default()
}

/// Le salarie d'une fiche, tel que l'ecran le nomme (`socle::texte::salarie_nomme`).
fn qui_de(fiche: &droit::fiche::ChampsSalarie) -> Resultat<String> {
    fiche.nomme().ok_or_else(|| socle::fichiers::absent("nom"))
}

/// « (n° FA1.26.13) » quand le contrat porte son numero ; rien sinon.
fn numero_dit(eng: &Engagement) -> String {
    eng.numero.as_ref().map_or_else(String::new, |n| format!(" (n° {})", n))
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
    signe_ailleurs_de(racine, &*eng.nat()?, &eng.salarie, &eng.debut.iso(), eng.numero.as_deref().unwrap_or(""), &eng.mois)
}

/// `signe_ailleurs`, de ce que l'engagement dit : sa nature, son salarie, son debut, son numero, et le mois d'ou on le
/// regarde.
pub fn signe_ailleurs_de(racine: &Path, nat: &Nature, salarie: &str, debut: &str, numero: &str, mois: &str) -> Resultat<bool> {
    if nat.modele_contrat().is_none() {
        return Ok(false);
    }
    Ok(stock::commence_avant_le_logiciel(racine, debut, mois)? || importe::vient_d_un_ancien_logiciel(racine, salarie, debut, numero))
}

/// Un contrat permanent court d'un bout a l'autre, son numero est celui de l'engagement, et le dernier mois le dit.
fn suivre_permanent(ctx: &Contexte, eng: &mut Engagement) -> Resultat<Vec<Avis>> {
    let mut avis = Vec::new();
    let fiche = ctx.salarie(&eng.salarie)?;
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
    let fiche = ctx.salarie(&eng.salarie)?;
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
    let _comme_a_la_validation = crate::etats::CommeALaValidation::ouvrir(ctx, &mois_vu);
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
        let metier = e.metier.clone().unwrap_or_default();
        let m = emplois::metier(&metier)?;
        if let Some(m) = m.filter(|m| e.cadre == Some(false) && m.cadre_par_defaut(ctx.entreprise.idcc.texte())) {
            fn texte<T: AsRef<str>>(c: &droit::fiche::Case<T>) -> Option<String> {
                c.remplie().map(str::to_string)
            }
            let libelle = Some(m.libelle()).filter(|l| !l.is_empty()).unwrap_or_else(|| metier.clone());
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
        Value::Null
    } else {
        ctx.spectacle(eng.spectacle.as_deref().ok_or_else(|| socle::fichiers::absent("spectacle"))?)?
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
pub fn preavis(ctx: &Contexte, e: &Value, anciennete: Decimal, motif: Option<&str>) -> Resultat<solde::Preavis> {
    solde::duree_du_preavis(ctx, e, anciennete, motif)
}

/// LES LIGNES `presentes` SONT-ELLES CELLES DE `partie` ARRETEES A LA `fin` (AAAA-MM-JJ) d'un contrat rompu
/// (`etats::seule_la_rupture`) : une ligne garde ses jours jusqu'a la fin, celle qui n'en a plus disparait, et la quantite
/// d'une ligne coupee ne croit pas.
pub(crate) fn lignes_coupees_a_la_fin(partie: &[Value], presentes: &[Value], fin: &str) -> bool {
    let dates = |l: &Value| -> Vec<String> { l.get("dates").and_then(Value::as_array).map(|a| a.iter().map(en_texte).collect()).unwrap_or_default() };
    let quantite = |l: &Value| l.get("quantite").and_then(Value::as_f64).unwrap_or(0.0);
    let sans = |l: &Value, k: &[&str]| -> Map<String, Value> {
        l.as_object().map(|o| o.iter().filter(|(c, _)| !k.contains(&c.as_str())).map(|(c, x)| (c.clone(), x.clone())).collect()).unwrap_or_default()
    };
    let mut attendues: Vec<(Value, bool)> = Vec::new();
    for l in partie {
        let jours = dates(l);
        let gardes: Vec<String> = jours.iter().filter(|j| j.as_str() <= fin).cloned().collect();
        if !jours.is_empty() && gardes.is_empty() {
            continue;
        }
        let mut l = l.clone();
        let coupee = gardes.len() != jours.len();
        if coupee {
            if let Some(o) = l.as_object_mut() {
                o.insert("dates".into(), Value::Array(gardes.into_iter().map(Value::String).collect()));
            }
        }
        attendues.push((l, coupee));
    }
    presentes.len() == attendues.len()
        && presentes.iter().zip(&attendues).all(|(p, (a, coupee))| if *coupee { sans(p, &["quantite"]) == sans(a, &["quantite"]) && quantite(p) <= quantite(a) } else { p == a })
}
