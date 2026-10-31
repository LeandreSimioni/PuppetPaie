

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
pub mod etats;
pub mod importe;
pub mod imprevus;
pub mod minima;
pub mod periodes;
pub mod relire;
pub mod solde;
pub mod numeros;
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
use socle::date::{d, dernier_du_mois, en_texte, mois_entre, premier_du_mois, forme_ecrite, Jour};
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
    pub numero: Option<String>,
    pub mois: String,
    pub planning: Vec<Jour>,
    pub planning_lieux: BTreeMap<Jour, String>,
    pub jours: i64,
    pub jours_contrat: i64,
    pub date_paiement: Option<String>,
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
            .ok_or_else(|| format!("`{}` : un entier est attendu, lu {}", nom, n)),
        Some(autre) => Err(format!("`{}` : un entier est attendu, lu {}", nom, forme_ecrite(autre))),
    }
}

fn truthy(v: Option<&Value>) -> bool {
    v.map_or(false, vrai)
}

/// `str(x.get("lieu") or "").strip()`.
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
            return Err(format!(
                "Ligne << {} >> : une ligne en pourcentage dit sa `base` (en euros) \
                 ET son `taux_majoration` (en %). Recu : {}",
                rub,
                forme_ecrite(l)
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
                return Err(format!(
                    "Ligne << {} >> : `{}` ({}) contredit `base` x `taux_majoration` \
                     ({} x {} %). N'ecrire que base et pourcentage.",
                    rub,
                    champ,
                    en_texte(x),
                    base,
                    pct
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
        Some(autre) => Err(format!("`{}` : une liste est attendue, lu {}", nom, forme_ecrite(autre))),
    }
}

impl Engagement {
    /// « NOM Prénom, contrat N » : qui une phrase de refus désigne.
    pub fn qui(&self) -> String {
        format!("{}, contrat {}", self.nom, self.numero.as_deref().filter(|n| !n.is_empty()).unwrap_or("sans numero"))
    }

    /// `Engagement(**v)` : `v` porte les champs de la dataclass (`salarie`,
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
            return Err("Un engagement se saisit en lignes de paie ([{rubrique, \
                        quantite, taux, dates}]). L'ancienne ecriture -- `dates`, \
                        `cachets`, `heures`, `taux_horaire` sur l'engagement -- \
                        n'est plus lue, et celui-ci n'a pas de `lignes`."
                .to_string());
        };
        let lignes: Vec<Value> = lignes_brutes.iter().map(ligne_en_pourcentage).collect::<Resultat<_>>()?;

        // Un refus nomme le salarie : c'est lui que la personne voit sur la carte.
        let qui = qui.to_string();
        let mois_saisi = ou(get("mois"), None).unwrap_or_default();
        let debut_saisi = truthy(get("debut"));
        let fin_saisie = truthy(get("fin"));
        if lignes.is_empty() && !(debut_saisi && fin_saisie) {
            return Err("Un engagement sans aucune ligne de paie est un mois du \
                        contrat sans cachet : il doit dire `debut` et `fin` du \
                        contrat (et `numero` s'il est connu), rien ne permet de les \
                        deduire."
                .to_string());
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
        let jour_lu = dernier_du_mois(&mois_saisi)
            .ok()
            .or_else(|| jours_joues.first().and_then(|j| dernier_du_mois(&j.mois_iso()).ok()))
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

        let debut_mois_de = |m: &str| premier_du_mois(m);
        let fin_mois_de = |m: &str| dernier_du_mois(m);
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
            .filter(|j| !mois_saisi.is_empty() && j.mois_iso() != mois_saisi)
            .copied()
            .collect();
        let mois = if mois_saisi.is_empty() {
            jours_joues.first().ok_or_else(|| socle::fichiers::trop_courte("les jours joués"))?.mois_iso()
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
                "Le {} n'est pas dans le mois {}. Les jours joues d'un \
                 engagement sont ceux du mois de paie ; un contrat qui se \
                 poursuit le mois suivant garde `fin` au-dela du mois et \
                 porte ses jours suivants dans le fichier du mois suivant.",
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
            numero: chaine(get("numero")),
            mois,
            planning,
            planning_lieux,
            jours,
            jours_contrat,
            date_paiement: chaine(get("date_paiement")),
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
        premier_du_mois(&self.mois)
    }

    pub fn fin_mois(&self) -> Resultat<Jour> {
        dernier_du_mois(&self.mois)
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
            _ => nat.quotite_reference_defaut,
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
        let reference = match (self.quotite_reference()?, nat.quotite_reference_defaut.filter(|_| nat.mensuelle())) {
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

    /// Les mois AAAA-MM que couvre le contrat, du premier au dernier.
    pub fn mois_du_contrat(&self) -> Resultat<Vec<String>> {
        let fin = match self.fin {
            Some(f) => f,
            None => self.fin_mois()?,
        };
        Ok(mois_entre(&self.debut, &fin))
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
        m.insert("numero".into(), texte(&self.numero));
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

/// `dates_engagement` (`cle` = "dates") et `planning_engagement` (`cle` =
/// "planning") : la reunion, dedoublonnee, des jours des lignes, chacun nu ou
/// detaille `{jour, lieu}` quand une ligne dit ou elle se joue.
fn jours_des_lignes(e: &Value, cle: &str) -> Vec<Value> {
    let (mut out, mut vus) = (Vec::new(), BTreeSet::new());
    let lignes = e.get("lignes").and_then(|l| l.as_array()).map_or(&[][..], |a| &a[..]);
    for l in lignes {
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

pub fn dates_engagement(e: &Value) -> Vec<Value> {
    jours_des_lignes(e, "dates")
}

pub fn planning_engagement(e: &Value) -> Vec<Value> {
    jours_des_lignes(e, "planning")
}

/// Les jours du contrat entier qu'un engagement brut connait : ses dates
/// jouees et son planning, sans doublon.
pub fn jours_du_contrat(e: &Value) -> usize {
    let cle = |j: &Value| match j {
        Value::Object(m) => m.get("jour").map_or(String::new(), |x| x.to_string()),
        autre => autre.to_string(),
    };
    let mut vus: BTreeSet<String> = dates_engagement(e).iter().map(cle).collect();
    vus.extend(planning_engagement(e).iter().map(cle));
    vus.len()
}

/// Le salarie qu'un refus nomme en tete, tel que l'ecran le nomme partout (`Contexte::nomme` : « NOM Prénom ») ;
/// « Cet engagement » quand la carte n'a pas encore de salarie.
pub(crate) fn salarie_nomme(ctx: &Contexte, salarie: Option<&Value>) -> String {
    chaine(salarie).filter(|s| !s.trim().is_empty()).map(|c| ctx.nomme(&c)).unwrap_or_else(|| "Cet engagement".to_string())
}

/// La nature de l'engagement `e` (`e.get("nature")`) ; un refus nomme son salarie.
fn nature_de(ctx: &Contexte, e: &Value) -> Resultat<Arc<Nature>> {
    natures::nature(chaine(e.get("nature")).as_deref()).map_err(|m| format!("{} : {}", salarie_nomme(ctx, e.get("salarie")), m))
}

/// Ce que `conges::lignes_du_mois` rend pour un engagement paye
/// au mois : les lignes de paie du mois, et ce qu'il y a a en dire.
pub type LignesPermanent<'a> = &'a dyn Fn(&Contexte, &Value, &str) -> Resultat<(Vec<Value>, Vec<Avis>)>;

/// Les engagements d'un fichier de mois,
/// construits avant tout calcul. `lignes_permanent` fournit les lignes d'un
/// engagement paye au mois ; les avertissements rendus vont avec, un jeu par
/// engagement (vide hors des permanents).
pub(crate) fn engagements(
    ctx: &Contexte,
    data: &Value,
    lignes_permanent: LignesPermanent,
) -> Resultat<(Vec<Engagement>, Vec<Vec<Avis>>)> {
    let mois_dit = data.get("mois").map_or("?".to_string(), en_texte);
    // Le taux du mois, s'il en ecrit un ; sinon rien (regle 1).
    let defaut_taux = data.get("taux_cachet").cloned().unwrap_or(Value::Null);
    let (mut out, mut dits) = (Vec::new(), Vec::new());
    let tous = data.get("engagements").ok_or_else(|| socle::fichiers::absent("engagements"))?;
    let tous = tous.as_array().ok_or("`engagements` : une liste est attendue")?;
    for e in tous {
        let qui = salarie_nomme(ctx, e.get("salarie"));
        let nat = nature_de(ctx, e)?;
        let mut spectacle = ou(e.get("spectacle"), None);
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
            return Err(format!(
                "{}{}\n\
                 Le spectacle porte le numéro d'objet, qui part en DSN, en AEM et sur le bulletin : il se choisit, il ne \
                 se devine pas.",
                qui, PAS_DE_SPECTACLE
            ));
        }
        if ["dates", "cachets", "heures", "taux_horaire"].iter().any(|k| e.get(*k).is_some()) {
            return Err(format!(
                "{} : l'engagement porte `dates`, `cachets`, `heures` ou `taux_horaire`, que le logiciel ne lit pas : \
                 tout se porte sur les lignes de paie ([{{rubrique, quantite, taux, dates}}]).",
                qui
            ));
        }
        let jours = if nat.mensuelle() { Vec::new() } else { dates_engagement(e) };
        // `lignes: []` ecrit est un mois du contrat sans cachet ; une ligne sans date est incomplete.
        if jours.is_empty() && !nat.mensuelle() && e.get("lignes") != Some(&Value::Array(Vec::new())) {
            return Err(format!("{}{} Sans eux il n'y a ni début, ni fin, ni contrat.", qui, PAS_DE_JOUR));
        }
        if !nat.mensuelle() {
            lignes_dites(ctx, e, &qui)?;
        }
        // Le metier de ce contrat : six champs declares, que l'engagement
        // ecrit par-dessus, et son libelle lisible (`emploi_affichage`).
        let mut declare = Value::Object(Map::new());
        if truthy(e.get("metier")) {
            let code = en_texte(e.get("metier").unwrap());
            let cadre = match e.get("cadre") {
                Some(Value::Bool(b)) => Some(*b),
                _ => None,
            };
            let salarie = ctx.salarie(&chaine(e.get("salarie")).ok_or_else(|| socle::fichiers::absent("salarie"))?)?;
            let feminin = sexe(&salarie) == "2";
            declare = emplois::resoudre(&code, cadre, feminin)
                .map_err(|m| format!("{} : {}", qui, m))?;
        }
        let lignes = if nat.mensuelle() {
            let (lignes, avis) =
                lignes_permanent(ctx, e, data.get("mois").and_then(|m| m.as_str()).unwrap_or(""))?;
            dits.push(avis);
            Some(Value::Array(lignes))
        } else {
            dits.push(Vec::new());
            e.get("lignes").cloned()
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
        poser("taux_cachet", Some(e.get("taux_cachet").unwrap_or(&defaut_taux)));
        poser("debut", e.get("debut"));
        poser("fin", e.get("fin"));
        poser("numero", e.get("numero"));
        poser("mois", Some(data.get("mois").ok_or_else(|| socle::fichiers::absent("mois"))?));
        poser("jours", e.get("jours"));
        // Un contrat d'usage commence dans l'ancien logiciel et continue ici : les jours du contrat entier comptent ceux
        // que les pieces importees attestent (`importe::Importe::jours_du_contrat_continue`).
        let continue_ = match (e.get("jours_contrat"), e.as_object()) {
            (None, Some(o)) if !nat.mensuelle() && crate::debut_brut(e).get(..7).is_some_and(|m| m < mois_dit.as_str()) => {
                crate::importe::continue_par(&ctx.racine, o)?.and_then(|i| i.jours_du_contrat_continue(jours_du_contrat(e))).map(Value::from)
            }
            _ => None,
        };
        poser("jours_contrat", e.get("jours_contrat").or(continue_.as_ref()));
        poser("planning", Some(&Value::Array(planning_engagement(e))));
        poser("lignes", lignes.as_ref());
        poser("date_paiement", e.get("date_paiement").or(data.get("date_paiement")));
        poser("lieu", e.get("lieu"));
        for champ in ["emploi", "pcs_ese", "code_emploi_ft", "statut", "statut_conventionnel", "statut_categoriel_rc"] {
            let v = ou(e.get(champ), declare.get(champ));
            poser(champ, v.map(Value::String).as_ref());
        }
        // Le libelle que `resoudre` a donne a CE metier, feminin compris ; un
        // `emploi` ecrit a la main dirait un autre metier que lui.
        if !truthy(e.get("emploi")) {
            poser("emploi_affichage", declare.get("emploi_affichage"));
        }
        poser("nature", e.get("nature"));
        poser("quotite_contrat", e.get("quotite_contrat"));
        let q = if truthy(e.get("quotite_reference")) {
            e.get("quotite_reference")
        } else {
            ctx.entreprise.get("quotite_reference")
        };
        poser("quotite_reference_saisie", q);
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
        let eng = Engagement::nouveau(&ctx.racine, &Value::Object(champs), &qui)?;
        notifiee_pendant_le_contrat(e, &eng)?;
        // Les lignes que la saisie ecrit disent leurs heures. Celles qu'un mois de permanent deduit, quand sa
        // saisie n'en ecrit pas (`conges::lignes_du_mois`), portent la quotite du contrat en heures : rien a controler.
        if !nat.mensuelle() || e.get("lignes").map_or(false, |l| !l.is_null()) {
            heures_dites(ctx, &eng, &qui)?;
        }
        out.push(eng);
    }
    Ok((out, dits))
}

/// La rupture se notifie pendant le contrat : « La date de notification de la rupture de contrat doit être supérieure
/// ou égale à la date de début de contrat de travail et inférieure ou égale à la date de fin de contrat de travail »
/// (CCH-11 de S21.G00.62.003, `atelier/reference/norme-dsn/texte/bloc-S21.G00.62.txt`). La saisie `e` qui la dit
/// (`notification_rupture`) se refuse, en disant les bornes du contrat `eng` ; un contrat sans fin ne borne que le debut.
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
const PAS_DE_JOUR: &str = " : cochez les jours joués sur sa ligne de paie.";
const LIGNE_A_DIRE: &str = " : saisissez ";
const QUANTITE_SANS_COMPTE: &str = " : saisissez un nombre plus grand que zéro";
/// Une journee de plus de 24 heures (`duree::plus_d_une_journee`) : une faute de frappe, a corriger comme une saisie pas finie.
pub(crate) const JOURNEE_DE_PLUS_DE_24: &str = ", et une journée n'a que 24 heures";
const SAISIE_INCOMPLETE: [&str; 5] = [PAS_DE_SPECTACLE, PAS_DE_JOUR, LIGNE_A_DIRE, QUANTITE_SANS_COMPTE, JOURNEE_DE_PLUS_DE_24];

/// Chaque ligne de paie saisie dit sa quantite et son taux (ou sa base et son pourcentage) : la saisie s'enregistre
/// sans eux, en saisie, et ce refus dit ce qui manque, ligne par ligne, avec le salarie (`qui`).
///
/// Une quantite (ou une base) dit combien la ligne paie : nulle, elle ne paie rien ; negative, elle retirerait ce
/// qu'elle dit payer, et ses jours partiraient en DSN sans un euro (une absence se saisit en ligne d'absence, de
/// quantite positive : c'est sa rubrique qui retire). L'une et l'autre se refusent, en disant la ligne. Seule une ligne
/// qui ne porte que des jours a venir (`planning`, sans jour de ce mois) a une quantite nulle : elle ne paie rien ce
/// mois-ci.
fn lignes_dites(ctx: &Contexte, e: &Value, qui: &str) -> Resultat<()> {
    let vide = |v: Option<&Value>| matches!(v, None | Some(Value::Null)) || v.and_then(Value::as_str).is_some_and(|s| s.trim().is_empty());
    for (i, l) in e.get("lignes").and_then(Value::as_array).into_iter().flatten().enumerate() {
        let Some(code) = l.get("rubrique") else { continue };
        let r = rubrique(&ctx.racine, &code.as_str().map_or_else(|| en_texte(code), String::from), &ctx.lue_au)?;
        let pourcentage = r.saisie.iter().any(|s| s == "taux_majoration");
        let (quantite, taux) = if pourcentage { ("base", "taux_majoration") } else { ("quantite", "taux") };
        let ligne = format!("sa ligne {} (« {} »)", i + 1, r.libelle);
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
            // (`stock::reconduire`) porte les jours de son planning, et ses heures se disent sur la carte de ce mois.
            let jours: Vec<Jour> = l.get("dates").and_then(Value::as_array).into_iter().flatten().filter_map(|x| d(x.get("jour").unwrap_or(x)).ok()).collect();
            let (ou, carte) = match jours.first() {
                Some(j) => (
                    format!(", {} {}", if jours.len() > 1 { "les" } else { "le" }, jours.iter().map(Jour::jj_mm_aaaa).collect::<Vec<_>>().join(", ")),
                    format!(" sur sa carte {}", socle::texte::de(&libelle_mois(&j.mois_iso()))),
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
    match duree::plus_d_une_journee(ctx, e, qui)? {
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

/// "1", "2", ou "" si la fiche ne le dit pas (le NIR d'abord).
fn sexe(fiche: &Value) -> String {
    let texte = |c: &str| fiche.get(c).filter(|x| vrai(x)).map(en_texte).unwrap_or_default();
    let nir = texte("nir");
    let nir = nir.trim();
    if nir.starts_with('1') || nir.starts_with('2') {
        return nir[..1].to_string();
    }
    let v = texte("sexe");
    let v = v.trim();
    if v == "1" || v == "2" {
        v.to_string()
    } else {
        String::new()
    }
}

// ------------------------------------------- le contrat qui deborde du mois

/// `libelle_mois` : '2026-12' -> 'décembre 2026', le mois en toutes lettres, avec ses accents, des phrases et des documents
/// qui l'appellent. Ses appelants ecrivent une phrase et ne refusent pas pour le mois : un mois illisible ou hors 01-12
/// s'ecrit tel quel (« 2026-00 »), jamais sous le nom de janvier.
pub fn libelle_mois(mois: &str) -> String {
    socle::date::libelle_du_mois_ou_tel_quel(mois, socle::date::Graphie::Accentuee)
}

/// La cle de la fiche a laquelle un engagement brut de la saisie se rattache : son `salarie` lu par `Contexte::salarie`
/// (sans tenir compte de la casse ni des accents), comme le fait le calcul de sa paie. `None` s'il ne nomme aucune fiche.
pub fn cle_de_l_engagement(ctx: &Contexte, e: &Value) -> Option<String> {
    let nom = e.get("salarie").and_then(Value::as_str)?;
    ctx.salarie(nom).ok()?.get("_cle").and_then(Value::as_str).map(str::to_string)
}
/// Les engagements que le mois montre (`stock::vue_du_mois`), bruts, avec la cle du salarie ; vide si aucun ne le
/// touche.
pub fn engagements_du_mois(ctx: &Contexte, mois: &str) -> Resultat<Vec<(String, Value)>> {
    let Some(data) = stock::vue_du_mois(&ctx.racine, mois)? else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    for e in data.get("engagements").and_then(|x| x.as_array()).map_or(&[][..], |a| &a[..]) {
        if let Some(cle) = cle_de_l_engagement(ctx, e) {
            out.push((cle, e.clone()));
        }
    }
    Ok(out)
}

/// Le debut d'un engagement brut, comme `Engagement` le deduirait : `debut` s'il le dit, sinon son premier jour joue.
pub fn debut_brut(e: &Value) -> String {
    let cle = |j: &Value| match j {
        Value::Object(m) => m.get("jour").map_or(String::new(), en_texte),
        autre => en_texte(autre),
    };
    if truthy(e.get("debut")) {
        return en_texte(e.get("debut").unwrap());
    }
    let mut jours: Vec<String> = dates_engagement(e).iter().map(cle).collect();
    jours.sort();
    jours.into_iter().next().unwrap_or_default()
}

/// Le salarie d'une fiche, tel que l'ecran le nomme (`socle::texte::salarie_nomme`).
fn qui_de(fiche: &Value) -> Resultat<String> {
    socle::texte::salarie_nomme(fiche).ok_or_else(|| socle::fichiers::absent("nom"))
}

/// « (n° FA1.26.13) » quand le contrat porte son numero ; rien sinon.
fn numero_dit(eng: &Engagement) -> String {
    eng.numero.as_deref().filter(|n| !n.is_empty()).map_or_else(String::new, |n| format!(" (n° {})", n))
}

/// LE CONTRAT DE CET ENGAGEMENT SE FAIT-IL SIGNER PAR LE LOGICIEL ? Oui quand sa nature a un contrat ecrit (`contrat_ecrit`
/// de `contrats-base.json` : le CDD « est etabli par ecrit », L1242-12 ; tout salarie recoit l'ecrit de L1221-5-1). Un
/// contrat repris d'un ancien logiciel -- celui d'un permanent, ou le contrat d'usage qui continue ici un contrat importe --
/// a ete signe la-bas (`importe::vient_d_un_ancien_logiciel`) : il ne se refait pas signer ici.
pub fn contrat_a_signer(racine: &Path, eng: &Engagement) -> Resultat<bool> {
    let nat = eng.nat()?;
    if nat.modele_contrat().is_none() {
        return Ok(false);
    }
    Ok(!importe::vient_d_un_ancien_logiciel(racine, &eng.salarie, &eng.debut.iso(), eng.numero.as_deref().unwrap_or("")))
}

/// UN NUMERO DE CONTRAT NE SE DONNE QU'AU PREMIER MOIS : le numero identifie le contrat en DSN (S21.G00.40.009), il se
/// pose a la validation de l'engagement (`numeros::a_la_validation`), et il ne se regenere pas (regle 4). Ce qui
/// numerote un mois du contrat qui n'est pas le premier, sans numero, refuse ici.
pub fn refus_sans_numero(ctx: &Contexte, eng: &Engagement) -> Resultat<()> {
    if eng.numero.as_deref().is_some_and(|n| !n.is_empty()) || eng.premier_mois()? {
        return Ok(());
    }
    let qui = qui_de(&ctx.salarie(&eng.salarie)?)?;
    let contrat = match eng.fin {
        Some(fin) if !eng.nat()?.mensuelle() => format!("contrat du {} au {}", eng.debut.jj_mm_aaaa(), fin.jj_mm_aaaa()),
        _ => format!("{} depuis le {}", eng.nat()?.libelle, eng.debut.jj_mm_aaaa()),
    };
    Err(numero_inconnu(&qui, &contrat, &eng.mois_du_contrat()?.remove(0)))
}

fn numero_inconnu(qui: &str, contrat: &str, premier: &str) -> String {
    format!(
        "{}, {} : en cours depuis {}, mais son numéro n'est pas connu : l'engagement n'est pas validé. Le numéro \
         identifie le contrat en DSN (S21.G00.40.009) et s'écrit à la validation : validez d'abord l'engagement {}. \
         Un contrat repris d'un autre logiciel dit le sien, lu sur sa DSN, dans Configuration › Reprendre l'historique.",
        qui,
        contrat,
        libelle_mois(premier),
        socle::texte::de(&libelle_mois(premier))
    )
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
                libelle_mois(&mois_du_contrat[0]),
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
                libelle_mois(suivant),
                libelle_mois(suivant),
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
pub fn saisie_du_mois(racine: &Path, mois: &str) -> Resultat<Value> {
    stock::vue_du_mois(racine, mois)?.ok_or_else(|| format!("{} : aucun engagement ne touche ce mois ({}/).", mois, stock::DOSSIER))
}

/// UN NUMERO DE CONTRAT EST CELUI D'UN SEUL SALARIE (regle 4) : il se lit, il ne se partage pas. Un salarie
/// coche en plus dans un bloc deja numerote, ou un numero recopie a la main, le porterait sur deux contrats : la DSN
/// declarerait deux salaries sous le meme 40.009. Le refus nomme le numero et les deux salaries, quelle que soit la
/// face qui a ecrit la saisie.
fn un_numero_un_salarie(paies: &[crate::calcul::Paie], mois: &str) -> Resultat<()> {
    let mut vus: BTreeMap<String, String> = BTreeMap::new();
    for p in paies {
        let Some(n) = p.engagement.numero.as_deref().filter(|n| !n.is_empty()) else { continue };
        let cle = p.salarie.get("_cle").map(en_texte).unwrap_or_default();
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
pub(crate) fn charger_donnees(ctx: &Contexte, data: &Value) -> Resultat<Saisie> {
    // Une derivation pour tout le mois : le brut d'un mois anterieur se
    // recalcule une fois, pas une fois par salarie (meme resultat).
    let _derivation = crate::conges::Derivation::ouvrir();
    // Un engagement en saisie se calcule comme sa validation le calculera : ses propres mois d'avant comptent.
    let mois_vu = data.get("mois").map(en_texte).unwrap_or_default();
    let _comme_a_la_validation = crate::etats::CommeALaValidation::ouvrir(ctx, &mois_vu);
    let lignes: LignesPermanent = &crate::conges::lignes_du_mois;
    let (engs, dits) = engagements(ctx, data, lignes)?;
    let mois = data.get("mois").map(en_texte).ok_or_else(|| socle::fichiers::absent("mois"))?;
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
    let tous = data.get("engagements").and_then(Value::as_array).ok_or_else(|| socle::fichiers::absent("engagements"))?;
    for (p, e) in paies.iter_mut().zip(tous) {
        let metier = e.get("metier").filter(|m| vrai(m)).map(en_texte).unwrap_or_default();
        let m = emplois::metier(&metier)?.unwrap_or(Value::Null);
        if e.get("cadre") == Some(&Value::Bool(false)) && m.get("cadre_par_defaut").map_or(false, vrai) {
            let texte = |c: &str| p.salarie.get(c).filter(|x| vrai(x)).map(en_texte);
            let libelle = m.get("libelle").filter(|x| vrai(x)).map(en_texte).unwrap_or_else(|| metier.clone());
            let msg = format!(
                "{} {}, {} : case cadre decochee, alors que ce metier est \
                 habituellement cadre. Si c'est voulu, rien a faire ; sinon, \
                 la cocher -- la retraite complementaire en depend.",
                texte("nom").ok_or_else(|| socle::fichiers::absent("nom"))?,
                texte("prenom_affichage").or_else(|| texte("prenom")).ok_or_else(|| socle::fichiers::absent("prenom"))?,
                libelle
            );
            p.ecrire().avertissements.push(Avis { info: false, texte: msg });
        }
        // Une journee au-dela de la duree quotidienne maximale : on le dit, l'employeur en repond.
        if let Some(texte) = duree::au_dela_du_maximum(ctx, e, &qui_de(&p.salarie)?)? {
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
pub fn pour_l_ecran(ctx: &Contexte, data: &Value) -> Resultat<Saisie> {
    let mut s = charger_donnees(ctx, data)?;
    for p in s.paies.iter_mut() {
        let pas = etats::prelevement(ctx, p)?;
        p.ecrire().pas = pas;
    }
    Ok(s)
}

/// `pour_l_ecran` de la saisie de `mois` telle que ses engagements la montrent.
#[cfg(feature = "ecran")]
pub fn mois_pour_l_ecran(ctx: &Contexte, mois: &str) -> Resultat<Saisie> {
    charger_donnees(ctx, &saisie_du_mois(&ctx.racine, mois)?)
}

/// Les engagements de la saisie `data`, lus tels que l'ecran les montre (les lignes d'un permanent deduites de ses
/// conges et de sa fin de contrat), sans paie.
pub fn engagements_de_la_saisie(ctx: &Contexte, data: &Value) -> Resultat<Vec<Engagement>> {
    let _derivation = conges::Derivation::ouvrir();
    Ok(engagements(ctx, data, &conges::lignes_du_mois)?.0)
}

/// Le refus que donne la lecture de la saisie `data` quand elle est encore incomplete (`saisie_incomplete`), lignes
/// d'un permanent mises a part ; `None` si elle se lit, ou si elle refuse pour une autre raison.
pub fn refus_de_saisie_incomplete(ctx: &Contexte, data: &Value) -> Option<String> {
    let vide: LignesPermanent = &|_, _, _| Ok((Vec::new(), Vec::new()));
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

/// Les embauches (`embauche`) de la saisie `data`, dans l'ordre de l'ecran, chaque contrat qui deborde du mois suivi
/// (`suivre_contrat`) : le salarie, son emploi et son spectacle d'abord, pour tous les engagements, puis le contrat qui
/// deborde reprend le numero de son premier mois.
pub fn embauches_suivies(ctx: &Contexte, data: &Value) -> Resultat<Vec<calcul::Paie>> {
    let mut paies = Vec::new();
    for e in &engagements_de_la_saisie(ctx, data)? {
        paies.push(embauche(ctx, e)?);
    }
    for p in paies.iter_mut() {
        suivre_contrat(ctx, &mut p.ecrire().engagement)?;
    }
    Ok(paies)
}

/// LA SAISIE LUE SANS PAIE, pour l'ecran : les embauches de la saisie `data` (`embauche`), chaque contrat qui deborde du
/// mois suivi (`suivre_contrat`), et les lignes de remuneration de chaque engagement paye au cachet telles que le calcul
/// les comprend (elles ne dependent que de l'engagement). Rien ne se calcule au-dela.
#[cfg(feature = "ecran")]
pub fn lue_sans_paie(ctx: &Contexte, data: &Value) -> Resultat<Vec<calcul::Paie>> {
    let mut paies = embauches_suivies(ctx, data)?;
    for p in paies.iter_mut() {
        let p = p.ecrire();
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
