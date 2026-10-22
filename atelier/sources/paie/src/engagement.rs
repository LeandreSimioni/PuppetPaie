//! L'engagement : un salarie, un spectacle, des lignes de paie datees, dans
//! un mois (`calcul.Engagement`, PORTAGE-RUST §5), et sa construction depuis
//! un fichier de mois (`operations_paies._saisie_lue`, et la reprise du
//! contrat qui deborde du mois dans `charger`).
//!
//! Hors de ce module : les numeros de bulletin, les minima, et la deduction
//! des lignes d'un permanent (`operations_permanents.lignes_du_mois`), que
//! l'appelant fournit.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Map, Value};

use crate::catalogue::rubrique;
use crate::contexte::Contexte;
use crate::contrats::{self, Nature};
use crate::date::{d, dernier_du_mois, en_texte, mois_entre, premier_du_mois, repr_py, Jour};
use crate::emplois;
use crate::fichiers::{lire_json, vrai};
use crate::num::{dec, dec_f64, dec_py, div, flottant, Decimal, Resultat};

#[derive(Debug, Clone)]
pub struct Engagement {
    pub salarie: String,
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
    /// (regle 1 : le moteur prenait 130 EUR, un chiffre qu'aucun texte ne fixe ; lot F1).
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
    /// Il ne se recopie pas d'un mois a l'autre (`routes_contrats::DEFINIT_LE_MENSUEL`) : le mois suivant declare deja
    /// la nouvelle valeur.
    pub date_modification: Option<String>,
    pub transaction: Option<String>,
    pub suspensions: Vec<Value>,
    /// Les termes du contrat AVANT l'avenant que `date_modification` date en cours de mois (`salaire_mensuel`,
    /// `quotite_contrat`), quand la saisie les dit : la paie du mois se decoupe alors au jour de l'avenant
    /// (`periodes::bornes`, lot R10). Comme `date_modification`, il ne se recopie pas d'un mois a l'autre.
    pub avant: Option<Value>,
    /// Une periode elementaire de paie (lot R10) : les bornes de CE calcul quand la paie du mois est decoupee
    /// (`periodes`) ; `None` : la periode de paie entiere (le contrat borne au mois).
    pub periode: Option<(Jour, Jour)>,
}

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
        Some(autre) => Err(format!("`{}` : un entier est attendu, lu {}", nom, repr_py(autre))),
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

/// `_ligne_en_pourcentage` : `base` x `taux_majoration` % -> `quantite` x `taux`.
pub fn ligne_en_pourcentage(l: &Value) -> Resultat<Value> {
    let o = match l {
        Value::Object(o) if o.contains_key("base") || o.contains_key("taux_majoration") => o,
        _ => return Ok(l.clone()),
    };
    let rub = o.get("rubrique").map_or("None".to_string(), en_texte);
    let (base, pct) = match (o.get("base"), o.get("taux_majoration")) {
        (Some(b), Some(p)) => (dec_py(b)?, dec_py(p)?),
        _ => {
            return Err(format!(
                "Ligne << {} >> : une ligne en pourcentage dit sa `base` (en euros) \
                 ET son `taux_majoration` (en %). Recu : {}",
                rub,
                repr_py(l)
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
            if !vide && dec_py(x)? != attendu {
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
                            repr_py(x)
                        )
                    } else {
                        crate::fichiers::absent("jour")
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
        Some(autre) => Err(format!("`{}` : une liste est attendue, lu {}", nom, repr_py(autre))),
    }
}

impl Engagement {
    /// `Engagement(**v)` : `v` porte les champs de la dataclass (`salarie`,
    /// `spectacle`, `dates_travail`, `lignes`, `debut`, ...). `racine` est
    /// celle des donnees de la compagnie, pour le catalogue des rubriques.
    pub fn nouveau(racine: &Path, v: &Value) -> Resultat<Engagement> {
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

        let numero = chaine(get("numero"));
        let numero_ou = || match &numero {
            Some(n) if !n.is_empty() => n.clone(),
            _ => "?".to_string(),
        };
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
        let nat = contrats::nature(nature.as_deref())?;
        let mensuel = nat.mensuelle();
        let planning_brut = liste(get("planning"), "planning")?;
        if mensuel {
            if !jours_joues.is_empty() || !planning_brut.is_empty() {
                return Err(format!(
                    "Contrat {} : un contrat paye au mois ({}) ne porte pas \
                     de jours joues. Ce qu'il dit, c'est sa duree du travail \
                     (`quotite_contrat`, en heures par mois) et ses bornes \
                     (`debut`, et `fin` quand il se termine). Les jours \
                     d'absence se saisissent en lignes d'absence.",
                    numero_ou(),
                    nat.libelle
                ));
            }
            if !debut_saisi {
                return Err(format!(
                    "Contrat {} ({}) : la date de debut du contrat n'est pas \
                     dite. Elle part en DSN (S21.G00.40.001) et identifie le \
                     contrat : elle ne se deduit de rien.",
                    numero_ou(),
                    nat.libelle
                ));
            }
            if mois_saisi.is_empty() {
                return Err(format!(
                    "Contrat {} ({}) : le mois de paie n'est pas dit. Un \
                     contrat mensuel n'a pas de date jouee d'ou le tirer.",
                    numero_ou(),
                    nat.libelle
                ));
            }
            if !fin_saisie && !nat.duree_indeterminee {
                return Err(format!(
                    "Contrat {} ({}) : un contrat a duree determinee dit sa \
                     date de fin. Elle est obligatoire en DSN \
                     (S21.G00.40.010, CCH-12).",
                    numero_ou(),
                    nat.libelle
                ));
            }
        }

        // Ce que la declaration compte : cachets et heures, derives des lignes.
        let mut cachets_somme = 0.0;
        let mut autres = 0.0;
        let mut premier_cachet: Option<&Value> = None;
        for l in &lignes {
            let code = l.get("rubrique").ok_or_else(|| crate::fichiers::absent("rubrique"))?;
            let code = code.as_str().map_or_else(|| en_texte(code), String::from);
            let r = rubrique(racine, &code)?;
            let q = flottant(l.get("quantite").ok_or_else(|| crate::fichiers::absent("quantite"))?)?;
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
            taux_cachet = Some(flottant(l.get("taux").ok_or_else(|| crate::fichiers::absent("taux"))?)?);
        }

        // Les jours travailles : un fait qui se saisit, sinon les dates distinctes.
        let jours = entier_opt(get("jours"), "jours")?.unwrap_or(presents.len() as i64);
        if jours < 0 || (!lignes.is_empty() && !mensuel && jours == 0) {
            return Err(format!(
                "Contrat {} : {} jour(s) travaille(s) pour {} ligne(s) de \
                 paie. Les jours partent en DSN (bloc 53, unite 12) et en \
                 AEM, et ce sont eux qui ouvrent les droits : un mois paye \
                 en compte au moins un. Dater les lignes, ou dire `jours` \
                 sur l'engagement.",
                numero_ou(),
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
            *jours_joues.first().ok_or_else(|| crate::fichiers::trop_courte("les jours joués"))?
        };
        let fin = if fin_saisie {
            Some(d(get("fin").unwrap())?)
        } else if mensuel {
            None
        } else {
            let derniere = *jours_joues.last().ok_or_else(|| crate::fichiers::trop_courte("les jours joués"))?;
            Some(derniere.max(planning.last().copied().unwrap_or(derniere)))
        };

        let debut_mois_de = |m: &str| premier_du_mois(m);
        let fin_mois_de = |m: &str| dernier_du_mois(m);
        if !mensuel && lignes.is_empty() {
            let fin = fin.unwrap();
            if !(debut < debut_mois_de(&mois_saisi)? || fin > fin_mois_de(&mois_saisi)?) {
                return Err(format!(
                    "Contrat du {} au {} sans aucune ligne de paie : un contrat \
                     qui tient dans son mois sans rien jouer n'a pas de sens. Un \
                     mois sans cachet n'existe que pour un contrat qui deborde du \
                     mois.",
                    debut.jj_mm_aaaa(),
                    fin.jj_mm_aaaa()
                ));
            }
        }
        if let Some(f) = fin {
            if debut > f {
                return Err(format!(
                    "Contrat du {} au {} : la fin precede le debut.",
                    debut.jj_mm_aaaa(),
                    f.jj_mm_aaaa()
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
            jours_joues.first().ok_or_else(|| crate::fichiers::trop_courte("les jours joués"))?.mois_iso()
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
            spectacle: chaine(get("spectacle")),
            dates_travail: jours_joues,
            lignes,
            debut,
            fin,
            numero,
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
        })
    }

    // ------------------------------------------------ le contrat et le mois

    /// La nature de contrat, relue a chaque acces.
    pub fn nat(&self) -> Resultat<Arc<Nature>> {
        contrats::nature(self.nature.as_deref())
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
                "Contrat {} ({}) : la duree contractuelle de travail \
                 (S21.G00.40.013, en heures par mois) n'est pas dite. Elle \
                 figure au contrat signe -- 151,67 h pour un temps plein a \
                 35 h -- et la DSN la declare : elle ne se devine pas.",
                self.numero_ou_point(),
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

    fn numero_ou_point(&self) -> String {
        match &self.numero {
            Some(n) if !n.is_empty() => n.clone(),
            _ => "?".to_string(),
        }
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

    /// `oracle._engagement` : la forme canonique d'un engagement.
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
        // Absent du releve Python, qui ne le portait pas : ecrit seulement quand il est dit.
        if let Some(a) = &self.emploi_affichage {
            m.insert("emploi_affichage".into(), Value::String(a.clone()));
        }
        m.insert("pcs_ese".into(), texte(&self.pcs_ese));
        m.insert("code_emploi_ft".into(), texte(&self.code_emploi_ft));
        m.insert("statut".into(), texte(&self.statut));
        m.insert("statut_conventionnel".into(), texte(&self.statut_conventionnel));
        m.insert("statut_categoriel_rc".into(), texte(&self.statut_categoriel_rc));
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
        // Les proprietes de `oracle._PROPRIETES`.
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

/// `format(Decimal(str(x)).normalize(), "f")`, "-0" ecrit "0".
thread_local! {
    // `paie calculer` : les nombres gardent leur echelle (`48.0`, `0.3230`),
    // comme les `Decimal` du Python qui les relit.
    pub static FIDELE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn nombre(x: Decimal) -> String {
    let s = if FIDELE.with(|f| f.get()) { x.to_string() } else { x.normalize().to_string() };
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

/// `oracle.canon` sur une valeur JSON : nombres en chaines normalisees, cles
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

/// `operations_paies.lire_saisie`.
pub fn lire_saisie(chemin: &Path) -> Resultat<Value> {
    lire_json(chemin)
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

/// `Engagement.nature` par `e.get("nature")`, avec le `KeyError` rendu comme
/// le Python l'ecrit dans le message d'un refus (`%s` d'un `KeyError`).
fn nature_de(e: &Value, rang: usize, mois: &str) -> Resultat<Arc<Nature>> {
    contrats::nature(chaine(e.get("nature")).as_deref())
        .map_err(|m| format!("Engagement {} de {} : {}", rang, mois, m))
}

/// Ce que `operations_permanents.lignes_du_mois` rend pour un engagement paye
/// au mois : les lignes de paie du mois, et ce qu'il y a a en dire.
pub type LignesPermanent<'a> = &'a dyn Fn(&Contexte, &Value, &str) -> Resultat<(Vec<Value>, Vec<Avis>)>;

/// `operations_paies._saisie_lue` : les engagements d'un fichier de mois,
/// construits avant tout calcul. `lignes_permanent` fournit les lignes d'un
/// engagement paye au mois ; les avertissements rendus vont avec, un jeu par
/// engagement (vide hors des permanents).
pub fn engagements(
    ctx: &Contexte,
    data: &Value,
    lignes_permanent: LignesPermanent,
) -> Resultat<(Vec<Engagement>, Vec<Vec<Avis>>)> {
    let mois_dit = data.get("mois").map_or("?".to_string(), en_texte);
    // Le taux du mois, s'il en ecrit un ; sinon rien (regle 1, lot F1 : plus de 130 EUR).
    let defaut_taux = data.get("taux_cachet").cloned().unwrap_or(Value::Null);
    let (mut out, mut dits) = (Vec::new(), Vec::new());
    let tous = data.get("engagements").ok_or_else(|| crate::fichiers::absent("engagements"))?;
    let tous = tous.as_array().ok_or("`engagements` : une liste est attendue")?;
    for (i, e) in tous.iter().enumerate() {
        let rang = i + 1;
        let nat = nature_de(e, rang, &mois_dit)?;
        let mut spectacle = ou(e.get("spectacle"), data.get("spectacle"));
        if nat.mensuelle() {
            // Employer des permanents se declare.
            if !ctx.emploie_des_permanents() {
                return Err(format!(
                    "Engagement {} de {} : config/entreprise.json : cette saisie porte un contrat paye au mois \
                     ({}), et la fiche ne dit pas que la compagnie emploie des \
                     permanents.\n\
                     \x20 A faire, dans cet ordre : cocher << La compagnie emploie aussi des salaries au mois >> dans \
                     Configuration > Entreprise (bloc << Vos salaries >>), puis remplir ce que la case ouvre -- la \
                     complementaire sante de chaque permanent (obligatoire, art. L911-7, \
                     le moteur refuse de payer sans elle) et, pour un non cadre de moins \
                     d'un an dans l'entreprise, son entree dans la branche.\n\
                     \x20 Si ce n'etait pas voulu : la nature de l'engagement se change dans \
                     le bloc lui-meme -- un intermittent au cachet est un << CDD d'usage \
                     du spectacle >>, et c'est ce qu'un engagement muet vaut.",
                    rang, mois_dit, nat.libelle
                ));
            }
            // Un permanent ne joue pas un spectacle (S21.G00.40.050, CCH-11).
            if spectacle.as_deref().map_or(false, |s| !s.is_empty()) && truthy(e.get("spectacle")) {
                return Err(format!(
                    "L'engagement {} de {} est un {} et porte un spectacle \
                     ({}). Le numero d'objet spectacle est interdit sur cette \
                     nature de contrat (S21.G00.40.050, CCH-11) : un contrat \
                     rattache a un spectacle est un contrat d'usage, et se \
                     saisit en `cddu_spectacle`.",
                    rang,
                    mois_dit,
                    nat.libelle,
                    spectacle.as_deref().unwrap_or("")
                ));
            }
            spectacle = None;
        } else if spectacle.as_deref().map_or(true, str::is_empty) {
            return Err(format!(
                "L'engagement {} de {} ne dit pas quel spectacle il joue, et \
                 le mois n'en propose aucun. Le spectacle porte le numero \
                 d'objet, qui part en DSN, en AEM et sur le bulletin : il se \
                 choisit, il ne se devine pas.",
                rang, mois_dit
            ));
        }
        if ["dates", "cachets", "heures", "taux_horaire"].iter().any(|k| e.get(*k).is_some()) {
            return Err(format!(
                "L'engagement {} de {} est ecrit a l'ancienne (`dates`, \
                 `cachets`, `heures` ou `taux_horaire` sur l'engagement). \
                 Depuis le 13/09/2026 tout se porte sur les lignes de paie \
                 ([{{rubrique, quantite, taux, dates}}]), et l'engagement ne \
                 garde aucun de ces champs.",
                rang, mois_dit
            ));
        }
        let jours = if nat.mensuelle() { Vec::new() } else { dates_engagement(e) };
        // `lignes: []` ecrit est un mois du contrat sans cachet ; une ligne sans date est incomplete.
        if jours.is_empty() && !nat.mensuelle() && e.get("lignes") != Some(&Value::Array(Vec::new())) {
            return Err(format!(
                "L'engagement {} de {} ne dit aucun jour joue. Les jours se \
                 portent sur chaque ligne de paie (\"dates\"). Sans eux il \
                 n'y a ni debut, ni fin, ni contrat. Un mois du contrat sans \
                 aucun cachet s'ecrit `lignes: []`, avec `debut` et `fin`.",
                rang, mois_dit
            ));
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
            let salarie = ctx.salarie(&chaine(e.get("salarie")).ok_or_else(|| crate::fichiers::absent("salarie"))?)?;
            let feminin = sexe(&salarie) == "2";
            declare = emplois::resoudre(&code, cadre, feminin)
                .map_err(|m| format!("Engagement {} de {} : {}", rang, mois_dit, m))?;
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
        poser("mois", Some(data.get("mois").ok_or_else(|| crate::fichiers::absent("mois"))?));
        poser("jours", e.get("jours"));
        poser("planning", Some(&Value::Array(planning_engagement(e))));
        poser("lignes", lignes.as_ref());
        poser("date_paiement", e.get("date_paiement").or(data.get("date_paiement")));
        poser("lieu", e.get("lieu").or(data.get("lieu")));
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
        poser("motif_rupture", e.get("motif_rupture"));
        // La fin du contrat telle que la saisie la dit : la mensuelle et le FCTU en tirent le meme S21.G00.62.006
        // (`fctu::dernier_jour_paye`).
        poser("preavis", e.get("preavis"));
        poser("dernier_jour_paye", e.get("dernier_jour_paye"));
        poser("date_modification", e.get("date_modification"));
        poser("avant", e.get("avant"));
        let eng = Engagement::nouveau(&ctx.racine, &Value::Object(champs))?;
        // Les lignes que la saisie ecrit disent leurs heures (#125). Celles qu'un mois de permanent deduit, quand sa
        // saisie n'en ecrit pas (`conges::lignes_du_mois`), portent la quotite du contrat en heures : rien a controler.
        if !nat.mensuelle() || e.get("lignes").map_or(false, |l| !l.is_null()) {
            heures_dites(ctx, &eng, rang, &mois_dit)?;
        }
        out.push(eng);
    }
    Ok((out, dits))
}

/// Les phrases des refus qui disent une saisie pas encore finie, et non un defaut du logiciel : un spectacle pas encore
/// choisi, des jours pas encore coches, des heures pas encore dites. L'ecran enregistre a chaque frappe, et ces refus
/// montent sous `SaisieIncomplete` (`routes_mois::classer_saisie`, `routes_ouverture::refus_du_calcul`) ; leur message
/// commence par « L'engagement N de AAAA-MM ».
const SAISIE_INCOMPLETE: [&str; 3] = [" ne dit pas quel spectacle il joue", " ne dit aucun jour joue", " ne dit pas ses heures"];

/// Ce refus de `engagements` dit-il une saisie pas encore finie ([`SAISIE_INCOMPLETE`]) ?
pub fn saisie_incomplete(message: &str) -> bool {
    message.starts_with("L'engagement ") && SAISIE_INCOMPLETE.iter().any(|p| message.contains(p))
}

/// REGLE 1, ticket #125 : une ligne dont la rubrique demande ses heures (`heures` dans sa `saisie`, aucun forfait --
/// `Rubrique::heures_a_saisir` : le service de repetition, la journee, la semaine, le mois, l'absence a la journee)
/// les dit, sinon l'engagement se refuse. Sans elles, la ligne paierait et declarerait zero heure en DSN, en AEM, aux
/// Conges Spectacles et a l'assurance chomage, sans un mot : `Rubrique::heures` n'a rien d'autre a compter. Une ligne
/// de quantite nulle ne paie rien et ne compte rien : elle n'a pas d'heures a dire.
fn heures_dites(ctx: &Contexte, eng: &Engagement, rang: usize, mois: &str) -> Resultat<()> {
    for (i, l) in eng.lignes.iter().enumerate() {
        let Some(code) = l.get("rubrique") else { continue };
        let r = rubrique(&ctx.racine, &code.as_str().map_or_else(|| en_texte(code), String::from))?;
        if !r.heures_a_saisir() {
            continue;
        }
        let quantite = flottant(l.get("quantite").ok_or_else(|| crate::fichiers::absent("quantite"))?)?;
        if quantite == 0.0 || flottant_opt(l.get("heures"))?.map_or(false, |h| h != 0.0) {
            continue;
        }
        return Err(format!(
            "L'engagement {} de {} : sa ligne {} (« {} ») ne dit pas ses heures. Cette rubrique n'a pas de forfait \
             d'heures, comme le cachet en a un : ses heures sont un fait, que la DSN, l'AEM, les Congés Spectacles et \
             l'assurance chômage comptent, et le moteur ne les devine pas. Saisir sur la ligne ses heures par {}.",
            rang,
            mois,
            i + 1,
            r.libelle,
            r.unite
        ));
    }
    Ok(())
}

/// `ntt.sexe` : "1", "2", ou "" si la fiche ne le dit pas (le NIR d'abord).
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

/// `engagements` sur un fichier de mois (`paies/AAAA-MM.json`) : tel quel
/// s'il existe, sinon sous la racine des donnees.
pub fn engagements_du_fichier(
    ctx: &Contexte,
    chemin: &Path,
    lignes_permanent: LignesPermanent,
) -> Resultat<(Value, Vec<Engagement>, Vec<Vec<Avis>>)> {
    let p: PathBuf = if crate::fichiers::existe(chemin) || chemin.is_absolute() {
        chemin.to_path_buf()
    } else {
        let sous = ctx.racine.join(chemin);
        if crate::fichiers::existe(&sous) { sous } else { chemin.to_path_buf() }
    };
    let data = lire_saisie(&p)?;
    let (engs, dits) = engagements(ctx, &data, lignes_permanent)?;
    Ok((data, engs, dits))
}

// ------------------------------------------- le contrat qui deborde du mois

/// `libelle_mois` : '2026-10' -> 'octobre 2026' (sans accent, comme l'ecrivent les phrases et les documents qui
/// l'appellent). Ses appelants ecrivent une phrase et ne refusent pas pour le mois : un mois illisible ou hors 01-12
/// s'ecrit tel quel (« 2026-00 »), jamais sous le nom de janvier.
pub fn libelle_mois(mois: &str) -> String {
    crate::date::libelle_du_mois_ou_tel_quel(mois, crate::date::Graphie::Minuscule)
}

/// La cle de la fiche a laquelle un engagement brut de la saisie se rattache : son `salarie` lu par `Contexte::salarie`
/// (sans tenir compte de la casse ni des accents), comme le fait le calcul de sa paie. `None` s'il ne nomme aucune fiche.
pub(crate) fn cle_de_l_engagement(ctx: &Contexte, e: &Value) -> Option<String> {
    let nom = e.get("salarie").and_then(Value::as_str)?;
    ctx.salarie(nom).ok()?.get("_cle").and_then(Value::as_str).map(str::to_string)
}

/// `_engagements_du_mois` : les engagements bruts d'un fichier de mois, avec
/// la cle du salarie ; vide si le mois n'est pas sur le disque.
pub(crate) fn engagements_du_mois(ctx: &Contexte, mois: &str) -> Resultat<Vec<(String, Value)>> {
    let p = ctx.racine.join("paies").join(format!("{}.json", mois));
    if !crate::fichiers::existe(&p) {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let data = lire_saisie(&p)?;
    for e in data.get("engagements").and_then(|x| x.as_array()).map_or(&[][..], |a| &a[..]) {
        if let Some(cle) = cle_de_l_engagement(ctx, e) {
            out.push((cle, e.clone()));
        }
    }
    Ok(out)
}

/// `_bornes_brutes(e)[0]` : le debut d'un engagement brut, comme `Engagement` le deduirait.
pub(crate) fn debut_brut(e: &Value) -> String {
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

/// `_engagement_du_premier_mois` : l'engagement brut qui a ouvert ce contrat.
///
/// LE CONTRAT, PAS LE SALARIE DU JOUR (`jumeaux`) : quand le premier mois porte plusieurs contrats de ce salarie
/// qui commencent ce jour-la, c'est le numero qui dit lequel ; sans numero, on ne devine pas, on refuse.
fn engagement_du_premier_mois(ctx: &Contexte, eng: &Engagement, cle: &str) -> Resultat<Option<Value>> {
    let premier = eng.mois_du_contrat()?.remove(0);
    let debut = eng.debut.iso();
    let mut du_jour: Vec<Value> = engagements_du_mois(ctx, &premier)?.into_iter().filter(|(c, e)| c == cle && debut_brut(e) == debut).map(|(_, e)| e).collect();
    if du_jour.len() <= 1 {
        return Ok(du_jour.pop());
    }
    let numero = eng.numero.as_deref().filter(|n| !n.is_empty());
    if let Some(n) = numero {
        return Ok(du_jour.into_iter().find(|e| crate::jumeaux::numero_pose(e).as_deref() == Some(n)));
    }
    let lesquels: Vec<String> = du_jour.iter().map(|e| crate::jumeaux::numero_pose(e).unwrap_or_else(|| "sans numero".into())).collect();
    Err(format!(
        "{} : {} contrats commencent le {} dans la saisie de {} ({}), et celui-ci ne porte pas de numero : on ne devine pas \
         lequel il poursuit. Recopier son numero dans la saisie de {}.",
        cle,
        du_jour.len(),
        eng.debut.jj_mm_aaaa(),
        premier,
        lesquels.join(", "),
        eng.mois
    ))
}

/// `_suivre_contrat_permanent` : un contrat permanent court d'un bout a
/// l'autre, son numero ne se regenere pas (recopie du premier mois), et le
/// dernier mois le dit.
fn suivre_permanent(ctx: &Contexte, eng: &mut Engagement) -> Resultat<Vec<Avis>> {
    let mut avis = Vec::new();
    let fiche = ctx.salarie(&eng.salarie)?;
    let texte = |c: &str| fiche.get(c).filter(|x| vrai(x)).map(en_texte);
    let nom = texte("nom").ok_or_else(|| crate::fichiers::absent("nom"))?;
    let prenom = texte("prenom_affichage").or_else(|| texte("prenom")).ok_or_else(|| crate::fichiers::absent("prenom"))?;
    let qui = format!("{} {}", nom, prenom);
    let depuis = eng.debut.jj_mm_aaaa();
    let libelle = eng.nat()?.libelle.clone();
    if !eng.premier_mois()? && eng.numero.as_deref().map_or(true, str::is_empty) {
        let cle = fiche.get("_cle").and_then(|c| c.as_str()).unwrap_or("").to_string();
        let premier = engagement_du_premier_mois(ctx, eng, &cle)?;
        let mois_du_contrat = eng.mois_du_contrat()?;
        let numero = premier.as_ref().and_then(|p| p.get("numero")).filter(|n| vrai(n));
        let Some(numero) = numero else {
            return Err(format!(
                "{}, {} depuis le {} : son numero de contrat n'est pas connu \
                 -- le mois {} n'a pas ete produit, ou son fichier ne porte \
                 pas ce contrat. Le numero identifie le contrat en DSN \
                 (S21.G00.40.009) et il ne se regenere pas : produire {} \
                 d'abord, ou recopier le numero dans la saisie.",
                qui,
                libelle,
                depuis,
                mois_du_contrat[0],
                libelle_mois(&mois_du_contrat[0])
            ));
        };
        eng.numero = Some(en_texte(numero));
    }
    if eng.dernier_mois()? {
        let fin = eng.fin.ok_or("fin")?;
        avis.push(Avis {
            info: true,
            texte: format!(
                "{} : {} n° {}, dernier mois (fin le {}). Le bulletin porte le \
                 solde de tout compte ; ensuite, signaler la fin de contrat dans \
                 les cinq jours et remettre au salarie son certificat de travail \
                 et son recu pour solde de tout compte (Fin de contrat).",
                qui,
                libelle,
                eng.numero.as_deref().unwrap_or("None"),
                fin.jj_mm_aaaa()
            ),
        });
    }
    Ok(avis)
}

/// Ce que `charger` fait d'un contrat a cheval (§5.4) : sur un mois qui n'est
/// pas le premier, recopie `jours_contrat` et `numero` du fichier du premier
/// mois (refus s'il manque), et pose les avertissements. `e` est l'engagement
/// brut du fichier ; un contrat paye au mois a sa propre suite, non portee ici.
pub fn suivre_contrat(ctx: &Contexte, eng: &mut Engagement) -> Resultat<Vec<Avis>> {
    let mut avis = Vec::new();
    if eng.nat()?.mensuelle() {
        return suivre_permanent(ctx, eng);
    }
    if !eng.a_cheval()? {
        return Ok(avis);
    }
    let fiche = ctx.salarie(&eng.salarie)?;
    let texte = |c: &str| fiche.get(c).filter(|x| vrai(x)).map(en_texte);
    let nom = texte("nom").ok_or_else(|| crate::fichiers::absent("nom"))?;
    let prenom = texte("prenom_affichage").or_else(|| texte("prenom")).ok_or_else(|| crate::fichiers::absent("prenom"))?;
    let qui = format!("{} {}", nom, prenom);
    let fin = eng.fin.ok_or("contrat a cheval sans fin")?;
    let bornes = format!("du {} au {}", eng.debut.jj_mm_aaaa(), fin.jj_mm_aaaa());
    let mois_du_contrat = eng.mois_du_contrat()?;
    if !eng.premier_mois()? {
        let cle = fiche.get("_cle").and_then(|c| c.as_str()).unwrap_or("").to_string();
        let premier = engagement_du_premier_mois(ctx, eng, &cle)?;
        match &premier {
            Some(p) => eng.jours_contrat = jours_du_contrat(p) as i64,
            None => avis.push(Avis {
                info: false,
                texte: format!(
                    "{} : contrat {} -- la saisie de {} est introuvable, la \
                     DSN comptera {} jours de contrat (ceux de ce mois et du \
                     planning). Remettre le fichier de ce mois-la pour \
                     compter aussi les jours passes.",
                    qui,
                    bornes,
                    libelle_mois(&mois_du_contrat[0]),
                    eng.jours_contrat
                ),
            }),
        }
        if eng.numero.as_deref().map_or(true, str::is_empty) {
            let numero = premier.as_ref().and_then(|p| p.get("numero")).filter(|n| vrai(n));
            let Some(numero) = numero else {
                return Err(format!(
                    "{}, contrat {} : en cours depuis {}, mais son numero \
                     n'est pas connu -- le mois {} n'a pas ete produit, \
                     ou son fichier ne porte pas ce contrat. Produire \
                     {} d'abord : c'est la que le contrat et la DPAE \
                     partent, et que le numero se fige.",
                    qui,
                    bornes,
                    libelle_mois(&mois_du_contrat[0]),
                    mois_du_contrat[0],
                    libelle_mois(&mois_du_contrat[0])
                ));
            };
            eng.numero = Some(en_texte(numero));
        }
        avis.push(Avis {
            info: true,
            texte: format!(
                "{} : contrat {} en cours depuis {} (n° {}) -- le contrat et \
                 la DPAE sont deja faits, ce mois-ci n'a que son bulletin.",
                qui,
                bornes,
                libelle_mois(&mois_du_contrat[0]),
                eng.numero.as_deref().unwrap_or("None")
            ),
        });
    }
    if !eng.dernier_mois()? {
        let place = mois_du_contrat.iter().position(|m| *m == eng.mois).ok_or("mois hors du contrat")?;
        let suivant = mois_du_contrat.get(place + 1).ok_or_else(|| crate::fichiers::trop_courte("les mois du contrat"))?;
        let fin_de_phrase = if eng.premier_mois()? {
            if eng.planning_connu() {
                " Le contrat imprime couvre toutes les dates."
            } else {
                " Le contrat imprime chiffre ce mois-ci et renvoie au planning pour la suite."
            }
        } else {
            ""
        };
        avis.push(Avis {
            info: true,
            texte: format!(
                "{} : contrat {}, se poursuit en {} -- ce mois-ci est paye \
                 maintenant, la suite le sera avec {}. Le certificat Conges \
                 Spectacles partira a la fin du contrat, pour le contrat \
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

/// `operations_paies.charger(..., sans_numeros=True)` ; `minima::sans_minima` coupe les minima.
pub fn charger(ctx: &Contexte, chemin: &Path) -> Resultat<Saisie> {
    let p: PathBuf = if crate::fichiers::existe(chemin) || chemin.is_absolute() {
        chemin.to_path_buf()
    } else {
        let sous = ctx.racine.join(chemin);
        if crate::fichiers::existe(&sous) { sous } else { chemin.to_path_buf() }
    };
    charger_donnees(ctx, &lire_saisie(&p)?)
}

/// UN NUMERO DE CONTRAT EST CELUI D'UN SEUL SALARIE (regle 4 ; #139) : il se lit, il ne se partage pas. Un salarie
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

/// `charger` d'une saisie deja lue (l'interface calcule un formulaire non enregistre).
pub fn charger_donnees(ctx: &Contexte, data: &Value) -> Resultat<Saisie> {
    // Une derivation pour tout le mois : le brut d'un mois anterieur se
    // recalcule une fois, pas une fois par salarie (meme resultat).
    let _derivation = crate::conges::Derivation::ouvrir();
    let lignes: LignesPermanent = &crate::conges::lignes_du_mois;
    let (engs, dits) = engagements(ctx, data, lignes)?;
    let mois = data.get("mois").map(en_texte).ok_or_else(|| crate::fichiers::absent("mois"))?;
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
        p.avertissements.extend(dit);
        p.avertissements.extend(payes);
        p.engagement.suspensions = crate::imprevus::suspensions_du_mois(ctx, p, &mois)?;
    }
    // Les minima conventionnels, puis le SMIC : des avertissements, jamais un refus.
    if crate::minima::actif() {
        for p in paies.iter_mut() {
            let avis = crate::minima::avertissements(ctx, p)?;
            p.avertissements.extend(avis);
            let avis = crate::minima::sous_le_smic(ctx, p)?;
            p.avertissements.extend(avis);
        }
    }
    // La case cadre decochee sur un metier habituellement cadre : on le dit.
    let tous = data.get("engagements").and_then(Value::as_array).ok_or_else(|| crate::fichiers::absent("engagements"))?;
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
                texte("nom").ok_or_else(|| crate::fichiers::absent("nom"))?,
                texte("prenom_affichage").or_else(|| texte("prenom")).ok_or_else(|| crate::fichiers::absent("prenom"))?,
                libelle
            );
            p.avertissements.push(Avis { info: false, texte: msg });
        }
    }
    // Les contrats qui debordent du mois.
    for p in paies.iter_mut() {
        let avis = suivre_contrat(ctx, &mut p.engagement)?;
        p.avertissements.extend(avis);
    }
    Ok(Saisie { mois, paies })
}
