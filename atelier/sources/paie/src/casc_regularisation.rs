//! La regularisation de la CASC-SVP (`moteur/operations_casc.py`) : le dossier que prepare `exporter
//! regularisation-casc-svp`, et `casc-regularise`, qui note qu'une annee est regularisee.
//!
//! Tout se lit dans les DSN DEPOSEES (`casc::dsn_deposee`) : masses, salaries, contrats. Un mois dont la DSN
//! porte deja la CASC-SVP, ou deja regularise (`config/casc-svp.json`), n'y entre pas.
//!
//! Le classeur modele du CASC-SVP (`donnees/modeles/casc-svp-liste-nominative.xlsx`, ouvert et rempli par openpyxl
//! dans l'ancien Python) N'EST PAS PORTE : voir `liste_nominative_xlsx`.

use std::path::Path;
use std::str::FromStr;

use rust_decimal::RoundingStrategy;
use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use socle::ecriture::{ecrire_json, repr_flottant};
use socle::valeur::py_str;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::{lire_json, vrai};
use socle::num::Decimal;

#[track_caller]
fn refus(m: impl Into<String>) -> socle::echec::Echec {
    erreur("ErreurOperation", m)
}

fn dec(t: &str) -> R<Decimal> {
    Decimal::from_str(t.trim()).map_err(|_| erreur("InvalidOperation", socle::num::illisible(t)))
}

/// `_eur` : `1 234,50 €` (deux decimales, espace des milliers, virgule).
pub(crate) fn eur(v: Decimal) -> String {
    let r = v.round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven);
    let mut s = format!("{:.2}", r);
    let negatif = s.starts_with('-');
    if negatif {
        s.remove(0);
    }
    let (entier, frac) = s.split_once('.').unwrap_or((&s, "00"));
    let mut groupes = String::new();
    for (i, c) in entier.chars().enumerate() {
        if i > 0 && (entier.len() - i) % 3 == 0 {
            groupes.push(' ');
        }
        groupes.push(c);
    }
    format!("{}{},{} €", if negatif { "-" } else { "" }, groupes, frac)
}

fn jjmmaaaa(iso: &str) -> String {
    if iso.is_empty() {
        return String::new();
    }
    let c: Vec<char> = iso.chars().collect();
    let t = |a: usize, b: usize| -> String { c.iter().skip(a).take(b - a).collect() };
    format!("{}/{}/{}", t(8, 10), t(5, 7), t(0, 4))
}

/// Une DSN deposee, lue par le lecteur de l'import, et en plus : porte-t-elle la CASC-SVP, et l'activite (bloc 53).
pub(crate) struct DsnCasc {
    pub lue: Map<String, Value>,
    pub porte_casc: bool,
    /// contrat -> [(jours|cachets|heures, quantite)], dans l'ordre ou ils sont apparus.
    pub activite: Vec<(String, Vec<(&'static str, Decimal)>)>,
}

/// `operations_casc.lire_dsn_casc`.
pub(crate) fn lire_dsn_casc(chemin: &Path, contrat_casc: &str) -> R<DsnCasc> {
    let Some(Value::Object(lue)) = crate::importation::lire_une_dsn(chemin) else {
        return Err(refus(format!("DSN illisible : {}.", chemin.display())));
    };
    let octets = std::fs::read(chemin).map_err(|e| erreur("OSError", format!("{} : {}", chemin.display(), e)))?;
    let texte = crate::apercu_dsn::texte_latin1(&octets);
    let (mut refs, mut rattachees): (Vec<String>, Vec<String>) = (vec![], vec![]);
    let mut libelle: Option<String> = None;
    let mut activite: Vec<(String, Vec<(&'static str, Decimal)>)> = vec![];
    let mut numero: Option<String> = None;
    let (mut type53, mut mesure): (Option<String>, Option<String>) = (None, None);
    for ligne in crate::apercu_dsn::lignes_py(&texte) {
        let (k, v) = ligne.split_once(',').unwrap_or((&ligne, ""));
        let v = v.trim_matches(socle::texte::blanc).trim_matches('\'');
        match k {
            "S21.G00.51.010" => numero = Some(v.to_string()),
            "S21.G00.53.001" => {
                type53 = Some(v.to_string());
                mesure = None;
            }
            "S21.G00.53.002" => mesure = Some(v.to_string()),
            "S21.G00.53.003" => {
                if let (Some(n), Some("01"), Some(m)) = (&numero, type53.as_deref(), mesure.as_deref().filter(|m| !m.is_empty())) {
                    let quoi = match v {
                        "12" => Some("jours"),
                        "37" => Some("cachets"),
                        "35" | "10" => Some("heures"),
                        _ => None,
                    };
                    if let Some(quoi) = quoi {
                        let q = dec(m)?;
                        let pos = match activite.iter().position(|(c, _)| c == n) {
                            Some(p) => p,
                            None => {
                                activite.push((n.clone(), vec![]));
                                activite.len() - 1
                            }
                        };
                        let a = &mut activite[pos].1;
                        match a.iter_mut().find(|(x, _)| *x == quoi) {
                            Some((_, t)) => *t += q,
                            None => a.push((quoi, q)),
                        }
                    }
                }
            }
            "S21.G00.15.001" => libelle = Some(v.to_string()),
            "S21.G00.15.005" => {
                if libelle.as_deref() == Some(contrat_casc) {
                    refs.push(v.to_string());
                }
            }
            "S21.G00.70.013" => rattachees.push(v.to_string()),
            _ => {}
        }
    }
    let porte_casc = refs.iter().any(|r| rattachees.contains(r));
    Ok(DsnCasc { lue, porte_casc, activite })
}

fn texte_de(m: &Map<String, Value>, cle: &str) -> Option<String> {
    m.get(cle).filter(|v| vrai(v)).map(py_str)
}

/// Un mois du dossier.
pub(crate) struct MoisReg {
    pub mois: String,
    pub libelle: String,
    pub masse: Decimal,
    pub taux: Decimal,
    pub cotisation: Decimal,
    pub source: String,
}

pub(crate) struct AnneeReg {
    pub annee: String,
    pub masse: Decimal,
    pub taux: Decimal,
    pub cotisation: Decimal,
    pub libelle_virement: String,
}

pub(crate) struct SalarieReg {
    pub annee: String,
    pub salarie: String,
    pub nir: Option<String>,
    pub date_naissance: Option<String>,
    pub adresse: String,
    pub masse: Decimal,
}

pub(crate) struct EmploiReg {
    pub annee: String,
    pub mois: String,
    pub individu: Value,
    pub contrat: Value,
    pub activite: Vec<(&'static str, Decimal)>,
    pub debut: Option<String>,
    pub fin: Option<String>,
    pub brut: Decimal,
    pub fichier: String,
}

pub(crate) struct Courriel {
    pub a: String,
    pub objet: String,
    pub corps: String,
    pub piece_jointe: Option<&'static str>,
}

/// `operations_casc.Regularisation`.
pub(crate) struct Regularisation {
    pub periode: String,
    pub mois: Vec<MoisReg>,
    pub annees: Vec<AnneeReg>,
    pub salaries: Vec<SalarieReg>,
    pub total: Decimal,
    pub a_faire: Vec<String>,
    pub courriel_audiens: Courriel,
    pub courriel_casc: Courriel,
    pub avertissements: Vec<String>,
    pub fichiers: Vec<String>,
    pub dossier: String,
    pub boite_liee: bool,
    pub emplois: Vec<EmploiReg>,
}

/// Les demandes de `regularisation_casc_svp`.
#[derive(Default)]
pub(crate) struct Demande {
    pub mois: Option<String>,
    pub annee: Option<String>,
    pub toutes_les_annees: bool,
    pub jusqu_a: Option<String>,
    pub annees: Vec<String>,
    /// {annee: AAAA-MM-JJ}
    pub vire_le: Vec<(String, String)>,
}

/// « mars 2025 » : le nom d'un mois de la regularisation. « 00 » n'est aucun mois et se refuse
/// (`date::numero_du_mois`, lot F1 ; l'indice `0 - 1` paniquait, un mois illisible se lisait janvier).
fn libelle_du_mois(m: &str, an: &str) -> R<String> {
    let mo = socle::date::numero_du_mois(m).map_err(refus)?;
    Ok(format!("{} {}", socle::date::nom_du_mois(mo as i64, socle::date::Graphie::Accentuee).map_err(refus)?, an))
}

fn plafond_mensuel(annee: &str) -> R<Option<Decimal>> {
    let f = socle::fichiers::dossier_du_code().join("baremes").join(format!("{}.json", annee));
    if !f.exists() {
        return Ok(None);
    }
    let v = lire_json(&f).map_err(|m| erreur("OSError", m))?;
    let m = v.get("plafond_securite_sociale").and_then(|p| p.get("mensuel"));
    match m {
        Some(x) if vrai(x) => {
            let t = match x {
                Value::Number(n) if n.is_f64() => repr_flottant(n.as_f64().unwrap_or(0.0)),
                autre => py_str(autre),
            };
            Ok(Some(dec(&t)?))
        }
        _ => Ok(None),
    }
}

/// `operations_casc.regularisation_casc_svp` (sans l'ecriture des fichiers : `ecrire`).
pub(crate) fn regularisation(ctx: &Contexte, d: &Demande) -> R<Regularisation> {
    let base = crate::casc::base().map_err(|m| erreur("OSError", m))?;
    let ent = &ctx.entreprise;
    let idcc = ent.get("idcc").filter(|v| vrai(v)).map(py_str).unwrap_or_default();
    if idcc != py_str(&base["idcc"]) {
        return Err(refus(format!(
            "La CASC-SVP est une cotisation de la convention 3090 ; la fiche entreprise dit {}. Il n'y a rien a regulariser.",
            if idcc.is_empty() { "rien".to_string() } else { idcc }
        )));
    }
    match droit::bareme::casc_exemptee(ent) {
        Err(m) => return Err(refus(m)),
        Ok(true) => {
            return Err(refus(
                "L'entreprise est declaree hors du dispositif CASC-SVP (article 4.22 B : au moins 50 salaries, un CSE qui contribue au moins autant) : il n'y a rien a regulariser.",
            ))
        }
        Ok(false) => {}
    }
    let faits = crate::casc::mois_regularises(ctx).map_err(|m| erreur("OSError", m))?;
    let depuis = py_str(&base["depuis"]);
    let mut candidats: Vec<String> = crate::routes_dossiers::mois_disponibles(ctx)?
        .into_iter()
        .map(|(m, _)| m)
        .filter(|m| m.as_str() >= depuis.as_str() && !faits.contains(m))
        .collect();
    if let Some(m) = d.mois.as_ref().filter(|m| !m.is_empty()) {
        candidats.retain(|x| x == m);
    } else if let (Some(a), false) = (d.annee.as_ref().filter(|a| !a.is_empty()), d.toutes_les_annees) {
        candidats.retain(|x| x.starts_with(a.as_str()));
    }
    if let Some(j) = d.jusqu_a.as_ref().filter(|j| !j.is_empty()) {
        candidats.retain(|x| x.as_str() <= j.as_str());
    }
    if !d.annees.is_empty() {
        candidats.retain(|x| d.annees.iter().any(|a| x.chars().take(4).collect::<String>() == *a));
    }
    let contrat_casc = py_str(&base["dsn"]["contrat"]);
    let mut avert: Vec<String> = vec![];
    let mut portees: Vec<String> = vec![];
    let mut lues: Vec<(String, DsnCasc)> = vec![];
    for m in &candidats {
        let chemin = crate::casc::dsn_deposee(ctx, m, None).map_err(refus)?;
        let Some(chemin) = chemin else { continue };
        let lu = lire_dsn_casc(&chemin, &contrat_casc)?;
        if lu.porte_casc {
            portees.push(m.clone());
            continue;
        }
        lues.push((m.clone(), lu));
    }
    if lues.is_empty() {
        if !faits.is_empty() {
            return Err(refus(format!(
                "Tout est regularise : {}. Les DSN suivantes portent la CASC-SVP elles-memes.",
                deja_faites(ctx)
            )));
        }
        return Err(refus(format!(
            "Aucune DSN deposee sans la CASC-SVP depuis {} dans la periode demandee : rien a regulariser.",
            depuis
        )));
    }
    if !faits.is_empty() {
        let mut tries = faits.clone();
        tries.sort();
        avert.push(format!("Deja regularises, donc hors du dossier : {}.", tries.join(", ")));
    }
    if !portees.is_empty() {
        avert.push(format!("La DSN porte deja la CASC-SVP, donc hors du dossier : {}.", portees.join(", ")));
    }

    let mut par_mois: Vec<MoisReg> = vec![];
    let mut par_salarie: Vec<((String, String), SalarieReg)> = vec![];
    let mut emplois: Vec<EmploiReg> = vec![];
    for (m, lu) in &lues {
        let individus: Vec<&Map<String, Value>> = lu.lue["individus"].as_array().into_iter().flatten().filter_map(Value::as_object).collect();
        let contrats: Vec<&Map<String, Value>> = lu.lue["contrats"].as_array().into_iter().flatten().filter_map(Value::as_object).collect();
        let fichier = py_str(&lu.lue["fichier"]);
        let mut masse = Decimal::ZERO;
        let mut mois_sal: Vec<(String, Decimal)> = vec![];
        let an: String = m.chars().take(4).collect();
        for v in lu.lue["versements"].as_array().into_iter().flatten().filter_map(Value::as_object) {
            let contrat_num = texte_de(v, "contrat").unwrap_or_default();
            let Some(brut_t) = texte_de(v, "brut") else {
                avert.push(format!("{} ({}), contrat {} : brut absent de la DSN, ligne ignoree.", m, fichier, contrat_num));
                continue;
            };
            let brut = dec(&brut_t)?;
            let nir = py_str(&v["nir"]);
            let i = individus.iter().rev().find(|i| texte_de(i, "nir").unwrap_or_default() == nir).copied();
            let c = contrats.iter().rev().find(|c| texte_de(c, "numero").is_some_and(|n| n == contrat_num)).copied();
            masse += brut;
            match mois_sal.iter_mut().find(|(n, _)| *n == nir) {
                Some((_, t)) => *t += brut,
                None => mois_sal.push((nir.clone(), brut)),
            }
            let champ = |cle: &str| i.and_then(|i| texte_de(i, cle));
            let cle = (an.clone(), nir.clone());
            let pos = match par_salarie.iter().position(|(k, _)| *k == cle) {
                Some(p) => p,
                None => {
                    let adresse: Vec<String> = ["adresse", "code_postal", "commune"].iter().filter_map(|k| champ(k)).collect();
                    par_salarie.push((
                        cle,
                        SalarieReg {
                            annee: an.clone(),
                            salarie: format!("{} {}", champ("nom").unwrap_or_default(), champ("prenom").unwrap_or_default()),
                            nir: Some(champ("nir").unwrap_or_else(|| nir.clone())),
                            date_naissance: champ("date_naissance"),
                            adresse: adresse.join(" "),
                            masse: Decimal::ZERO,
                        },
                    ));
                    par_salarie.len() - 1
                }
            };
            par_salarie[pos].1.masse += brut;
            let individu = i.map_or_else(|| json!({"nir": nir}), |i| Value::Object(i.clone()));
            let contrat = c.map_or_else(|| json!({}), |c| Value::Object(c.clone()));
            emplois.push(EmploiReg {
                annee: an.clone(),
                mois: m.clone(),
                individu,
                activite: lu.activite.iter().find(|(n, _)| *n == contrat_num).map(|(_, a)| a.clone()).unwrap_or_default(),
                debut: c.and_then(|c| texte_de(c, "debut")),
                fin: c.and_then(|c| texte_de(c, "fin")),
                contrat,
                brut,
                fichier: fichier.clone(),
            });
        }
        if let Some(plafond) = plafond_mensuel(&an)? {
            for (nir, b) in &mois_sal {
                if *b > plafond {
                    let nom = individus.iter().rev().find(|i| texte_de(i, "nir").unwrap_or_default() == *nir).and_then(|i| texte_de(i, "nom")).unwrap_or_else(|| nir.clone());
                    avert.push(format!("{}, {} : brut {} au-dessus du plafond mensuel -- la T1 n'est plus egale au brut, base a recalculer.", m, nom, eur(*b)));
                }
            }
        }
        let taux_v = base["taux"].get(&an).ok_or_else(|| {
            refus(format!(
                "Le taux de la CASC-SVP pour {} n'est pas releve (donnees/casc-svp-base.json) : l'export refuse plutot que de prolonger celui d'une autre annee.",
                an
            ))
        })?;
        let taux = dec(&match taux_v {
            Value::Number(n) if n.is_f64() => repr_flottant(n.as_f64().unwrap_or(0.0)),
            autre => py_str(autre),
        })?;
        let mut cot = (masse * taux / Decimal::from(100)).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
        cot.rescale(2);
        par_mois.push(MoisReg { mois: m.clone(), libelle: libelle_du_mois(m, &an)?, masse, taux, cotisation: cot, source: fichier });
    }

    let siret = ent.get("siret").filter(|v| vrai(v)).map(py_str).unwrap_or_default();
    let mut annees: Vec<AnneeReg> = vec![];
    for r in &par_mois {
        let an: String = r.mois.chars().take(4).collect();
        let pos = match annees.iter().position(|a| a.annee == an) {
            Some(p) => p,
            None => {
                annees.push(AnneeReg {
                    libelle_virement: py_str(&base["reglement"]["libelle"]).replace("{siret}", &siret).replace("{annee}", &an),
                    annee: an,
                    masse: Decimal::ZERO,
                    taux: r.taux,
                    cotisation: Decimal::ZERO,
                });
                annees.len() - 1
            }
        };
        annees[pos].masse += r.masse;
        annees[pos].cotisation += r.cotisation;
    }
    let total = annees.iter().fold(Decimal::ZERO, |t, a| t + a.cotisation);

    let nom = ent.get("raison_sociale").filter(|v| vrai(v)).map(py_str).unwrap_or_default();
    let sig = ent.get("signataire").cloned().unwrap_or(Value::Null);
    let sig = if vrai(&sig) { sig } else { json!({}) };
    let ou = |a: &str, b: &str| sig.get(a).filter(|v| vrai(v)).or_else(|| sig.get(b)).filter(|v| vrai(v)).map(py_str);
    let signe: String = [ou("prenom_affichage", "prenom"), ou("nom_affichage", "nom")].into_iter().flatten().collect::<Vec<_>>().join(" ");
    let periode = format!("{} à {}", par_mois[0].libelle, par_mois[par_mois.len() - 1].libelle);

    let mut tableau = vec!["| mois | masse | cotisation |".to_string(), "|---|---:|---:|".to_string()];
    for a in &annees {
        for r in &par_mois {
            if r.mois.starts_with(&a.annee) {
                tableau.push(format!("| {} | {} | {} |", r.libelle, eur(r.masse), eur(r.cotisation)));
            }
        }
        tableau.push(format!("| **{}, à {} %** | **{}** | **{}** |", a.annee, a.taux.to_string().replace('.', ","), eur(a.masse), eur(a.cotisation)));
    }
    let dates: Vec<(&String, &String)> = d.vire_le.iter().filter(|(_, v)| !v.is_empty()).map(|(k, v)| (k, v)).collect();
    let virements: Vec<String> = annees
        .iter()
        .map(|a| {
            let date = dates.iter().find(|(k, _)| **k == a.annee).map(|(_, v)| jjmmaaaa(v)).filter(|j| !j.is_empty()).unwrap_or_else(|| "__/__/____".to_string());
            format!("- le {}, {}, libellé « {} » ;", date, eur(a.cotisation), a.libelle_virement)
        })
        .collect();
    let beneficiaire = py_str(&base["reglement"]["beneficiaire"]);
    let corps_audiens = format!(
        "Bonjour,\n\nNotre logiciel de paie n'était pas paramétré pour déclarer la cotisation CASC-SVP en DSN (contrat CASCSVP) : elle manque à nos DSN de {}. Nous régularisons par virement, comme l'indique la page employeurs du CASC-SVP, et nos DSN la porteront désormais.\n\n{} — SIRET {} — IDCC 3090.\n\nMasses salariales soumises à cotisation (tranche 1 Agirc-Arrco) :\n\n{}\n\nVirements effectués vers {} :\n{}\n\nLa liste nominative des salariés part en parallèle au CASC-SVP.\n\nBien cordialement,\n\n{}\n{}\n",
        periode, nom, siret, tableau.join("\n"), beneficiaire, virements.join("\n"), signe, nom
    );
    let a_audiens = py_str(&base["courriel_audiens"]["a"]);
    let corps_casc = format!(
        "Bonjour,\n\nNotre logiciel de paie ne déclarait pas la cotisation CASC-SVP en DSN. Nous régularisons auprès d'Audiens ({}) les cotisations de {}, et vous trouverez en pièce jointe la liste nominative de nos salariés pour cette période.\n\n{} — SIRET {} — IDCC 3090.\n\nBien cordialement,\n\n{}\n{}\n",
        a_audiens, periode, nom, siret, signe, nom
    );
    let courriel_audiens = Courriel { a: a_audiens.clone(), objet: py_str(&base["courriel_audiens"]["objet"]).replace("{siret}", &siret), corps: corps_audiens, piece_jointe: None };
    let courriel_casc = Courriel {
        a: py_str(&base["courriel_casc"]["a"]),
        objet: py_str(&base["courriel_casc"]["objet"]).replace("{siret}", &siret),
        corps: corps_casc,
        piece_jointe: Some("CASC-SVP_liste-nominative.xlsx (le modele du CASC-SVP, rempli)"),
    };
    let mut a_faire = vec![format!(
        "Faire {} virement(s), un par année, à {} — l'IBAN se lit sur la page employeurs du CASC-SVP ({}) ou dans l'espace Audiens :",
        annees.len(),
        beneficiaire,
        py_str(&base["page"])
    )];
    for a in &annees {
        a_faire.push(format!("    {} : {}, libellé « {} »", a.annee, eur(a.cotisation), a.libelle_virement));
    }
    a_faire.push(format!("Reporter la date de chaque virement dans le courriel à Audiens, puis l'envoyer à {} (objet : {}).", courriel_audiens.a, courriel_audiens.objet));
    a_faire.push(format!(
        "Envoyer à {} (objet : {}) le courriel au CASC-SVP, avec en pièce jointe CASC-SVP_liste-nominative.xlsx — leur modèle, déjà rempli.",
        courriel_casc.a, courriel_casc.objet
    ));
    a_faire.push("Une fois les virements faits, le noter dans le logiciel : Exports, bouton « Noter comme régularisé », ou `paie casc-regularise ANNEE --vire-le AAAA-MM-JJ --montant X` — les mois sortent alors des dossiers suivants.".to_string());
    a_faire.push("Vérifier que la fiche de paramétrage Audiens porte les deux lignes CASC-SVP, pour que les DSN suivantes la déclarent.".to_string());

    let boite = crate::routes_accueil::boite_liee(ctx).unwrap_or(false);
    Ok(Regularisation {
        periode,
        mois: par_mois,
        annees,
        salaries: par_salarie.into_iter().map(|(_, s)| s).collect(),
        total,
        a_faire,
        courriel_audiens,
        courriel_casc,
        avertissements: avert,
        fichiers: vec![],
        dossier: String::new(),
        boite_liee: boite,
        emplois,
    })
}

fn fichier_suivi(ctx: &Contexte) -> std::path::PathBuf {
    ctx.racine.join("config").join("casc-svp.json")
}

fn deja_faites(ctx: &Contexte) -> String {
    let Ok(d) = lire_json(&fichier_suivi(ctx)) else { return String::new() };
    d.get("regularisations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|x| {
            let g = |c: &str| x.get(c).map_or_else(|| "None".to_string(), py_str);
            format!("{} ({} €, viré le {})", g("annee"), g("montant"), jjmmaaaa(&x.get("vire_le").filter(|v| vrai(v)).map(py_str).unwrap_or_default()))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// `Regularisation` en JSON, tel que `sortie_json.brut` le rend (Decimal en texte).
pub(crate) fn vers_json(r: &Regularisation) -> Value {
    let courriel = |c: &Courriel| {
        let mut m = Map::new();
        m.insert("a".into(), json!(c.a));
        m.insert("objet".into(), json!(c.objet));
        m.insert("corps".into(), json!(c.corps));
        if let Some(p) = c.piece_jointe {
            m.insert("piece_jointe".into(), json!(p));
        }
        Value::Object(m)
    };
    let sans_blanc = |v: &Value| -> Value {
        match v {
            Value::Object(o) => Value::Object(o.iter().filter(|(k, _)| !k.starts_with('_')).map(|(k, v)| (k.clone(), v.clone())).collect()),
            autre => autre.clone(),
        }
    };
    json!({
        "etat": "regularisation-casc-svp",
        "periode": r.periode,
        "mois": r.mois.iter().map(|m| json!({"mois": m.mois, "libelle": m.libelle, "masse": m.masse.to_string(), "taux": m.taux.to_string(), "cotisation": m.cotisation.to_string(), "source": m.source})).collect::<Vec<_>>(),
        "annees": r.annees.iter().map(|a| json!({"annee": a.annee, "masse": a.masse.to_string(), "taux": a.taux.to_string(), "cotisation": a.cotisation.to_string(), "libelle_virement": a.libelle_virement})).collect::<Vec<_>>(),
        "salaries": r.salaries.iter().map(|s| json!({"annee": s.annee, "salarie": s.salarie, "nir": s.nir, "date_naissance": s.date_naissance, "adresse": s.adresse, "masse": s.masse.to_string()})).collect::<Vec<_>>(),
        "total": r.total.to_string(),
        "a_faire": r.a_faire,
        "courriel_audiens": courriel(&r.courriel_audiens),
        "courriel_casc": courriel(&r.courriel_casc),
        "avertissements": r.avertissements,
        "fichiers": r.fichiers,
        "dossier": r.dossier,
        "boite_liee": r.boite_liee,
        "emplois": r.emplois.iter().map(|e| {
            let mut act = Map::new();
            for (q, v) in &e.activite {
                act.insert((*q).to_string(), json!(v.to_string()));
            }
            json!({"annee": e.annee, "mois": e.mois, "individu": sans_blanc(&e.individu), "contrat": sans_blanc(&e.contrat), "activite": act, "debut": e.debut, "fin": e.fin, "brut": e.brut.to_string(), "fichier": e.fichier})
        }).collect::<Vec<_>>(),
    })
}

/// `operations_casc._ecrire` : `exports/casc-svp/` -- les CSV, les deux courriels, la liste des gestes.
pub(crate) fn ecrire(ctx: &Contexte, r: &mut Regularisation) -> R<()> {
    let dossier = ctx.racine.join("exports").join("casc-svp");
    let io = |p: &Path, e: std::io::Error| erreur("OSError", format!("{} : {}", p.display(), e));
    std::fs::create_dir_all(&dossier).map_err(|e| io(&dossier, e))?;
    let virgule = |d: &Decimal| d.to_string().replace('.', ",");
    let mut fichiers: Vec<std::path::PathBuf> = vec![];
    // Le classeur modele : voir `liste_nominative_xlsx`.
    liste_nominative_xlsx(ctx, r, &dossier, &mut fichiers)?;
    let ecrit_csv = |nom: &str, entetes: &[&str], lignes: Vec<Vec<String>>| -> R<std::path::PathBuf> {
        let mut t = socle::csv::ligne(&entetes.iter().map(|s| s.to_string()).collect::<Vec<_>>(), ';');
        for l in &lignes {
            t.push_str(&socle::csv::ligne(l, ';'));
        }
        let p = dossier.join(nom);
        std::fs::write(&p, format!("\u{feff}{}", t)).map_err(|e| io(&p, e))?;
        Ok(p)
    };
    fichiers.push(ecrit_csv(
        "CASC-SVP_masses-par-mois.csv",
        &["mois", "masse", "taux %", "cotisation", "source"],
        r.mois.iter().map(|x| vec![x.mois.clone(), virgule(&x.masse), virgule(&x.taux), virgule(&x.cotisation), x.source.clone()]).collect(),
    )?);
    fichiers.push(ecrit_csv(
        "CASC-SVP_liste-nominative.csv",
        &["annee", "salarie", "NIR", "date de naissance", "adresse", "masse"],
        r.salaries
            .iter()
            .map(|s| vec![s.annee.clone(), s.salarie.clone(), s.nir.clone().unwrap_or_default(), s.date_naissance.clone().unwrap_or_default(), s.adresse.clone(), virgule(&s.masse)])
            .collect(),
    )?);
    for (nom, c) in [("CASC-SVP_courriel-audiens.txt", &r.courriel_audiens), ("CASC-SVP_courriel-casc.txt", &r.courriel_casc)] {
        let p = dossier.join(nom);
        std::fs::write(&p, format!("À : {}\nObjet : {}\n\n{}", c.a, c.objet, c.corps)).map_err(|e| io(&p, e))?;
        fichiers.push(p);
    }
    let p = dossier.join("CASC-SVP_a-faire.txt");
    let verifier = if r.avertissements.is_empty() { String::new() } else { format!("\nÀ vérifier :\n{}", r.avertissements.join("\n")) };
    std::fs::write(&p, format!("Régularisation CASC-SVP — {} — total {}\n\n{}\n{}\n", r.periode, eur(r.total), r.a_faire.join("\n"), verifier)).map_err(|e| io(&p, e))?;
    fichiers.insert(0, p);
    r.fichiers = fichiers.iter().map(|f| f.display().to_string()).collect();
    r.dossier = dossier.display().to_string();
    Ok(())
}

const ENTETES: [(u32, u16, &str); 42] = [
    (0, 0, "Les 3 lignes du haut de la feuille sont à supprimer avant d'enregistrer le fichier au format .CSV (séparateur point-virgule) pour télétransmission."),
    (0, 26, "Salariés autre qu'Intermittents"),
    (0, 30, "Salariés Intermittents"),
    (1, 26, "Nombre d'heures"),
    (1, 30, "\"A\" si artiste\n\"T\" si technicien"),
    (1, 31, "Techniciens"),
    (1, 33, "Artistes"),
    (2, 0, "mention\n\"CASC-SVP\""),
    (2, 1, "N°Siret"),
    (2, 2, "VIDE"),
    (2, 3, "VIDE"),
    (2, 4, "Année de déclaration concernée"),
    (2, 5, "Date de constitution du Fichier"),
    (2, 6, "Nom de naissance"),
    (2, 7, "Nom d'artiste\nNom Marital"),
    (2, 8, "Prénom"),
    (2, 9, "Date de Naissance"),
    (2, 10, "Numéro de Sécurité Sociale avec Clé"),
    (2, 11, "n° App ou Bal / couloir / escalier"),
    (2, 12, "Entrée / bâtiment / immeuble / résidence"),
    (2, 13, "Numéro et libellé de la voie"),
    (2, 14, "Lieu dit ou boîte postale"),
    (2, 15, "Code Postal  en France"),
    (2, 16, "Libellé Commune"),
    (2, 17, "Code postal si hors France"),
    (2, 18, "Pays"),
    (2, 19, "Adresse mail personnelle"),
    (2, 20, "Adresse mail professionnelle"),
    (2, 21, "Libellé de l'Emploi"),
    (2, 22, "Date de début d' emploi"),
    (2, 23, "Date de fin d'emploi"),
    (2, 24, "Salaire de la période"),
    (2, 25, "\"I\" si intermittent\n\"P\" si permanent"),
    (2, 26, "VIDE"),
    (2, 28, "VIDE"),
    (2, 29, "VIDE"),
    (2, 31, "NbJ"),
    (2, 32, "NbH"),
    (2, 33, "NbC"),
    (2, 34, "NbH"),
    (2, 35, "Si P : \ntotalisation des salaires ligne à ligne ex: (ligne1 =salaire1, ligne2=salaire 1+ salaire2, ligne 3=salaire1+2+3.....)"),
    (2, 36, "Si I : \ntotalisation des salaires ligne à ligne ex: (ligne1 =salaire1, ligne2=salaire 1+ salaire2, ligne 3=salaire1+2+3.....)"),
];

/// Les cellules fusionnees du modele : (premiere ligne, premiere colonne, derniere ligne, derniere colonne), a partir de 0.
const FUSIONS: [(u32, u16, u32, u16); 7] = [(0, 0, 1, 25), (0, 26, 0, 29), (0, 30, 0, 34), (1, 26, 1, 29), (1, 30, 2, 30), (1, 31, 1, 32), (1, 33, 1, 34)];

/// Un `Decimal` entier ou non, comme `n(q)` : un nombre s'il y en a un, rien pour zero ou absent.
fn quantite_xlsx(q: Option<&Decimal>) -> Option<f64> {
    use rust_decimal::prelude::ToPrimitive;
    let q = q?;
    if q.is_zero() {
        return None;
    }
    q.to_f64()
}

/// `operations_casc._liste_nominative_xlsx` : la liste nominative du CASC-SVP, une ligne par emploi, a partir de la 4e ligne.
///
/// Le modele officiel (`donnees/modeles/casc-svp-liste-nominative.xlsx`) n'est plus ouvert : le classeur est RECREE par
/// `rust_xlsxwriter` avec les memes colonnes (37), les memes trois lignes de consignes (memes textes, memes cellules
/// fusionnees) et les memes valeurs ; la mise en forme est plus simple (gras, retour a la ligne, volet fige sous la
/// 3e ligne). Les consignes de la 4e ligne du modele (« NE PAS REMPLIR »...) n'y sont pas, comme dans le Python, qui les
/// effacait avant d'ecrire.
fn liste_nominative_xlsx(ctx: &Contexte, r: &mut Regularisation, dossier: &Path, fichiers: &mut Vec<std::path::PathBuf>) -> R<()> {
    use rust_xlsxwriter::{Format, FormatAlign, Workbook};
    let xl = |e: rust_xlsxwriter::XlsxError| erreur("OSError", format!("classeur CASC-SVP : {}", e));
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_name("Feuil1").map_err(xl)?;
    let titre = Format::new().set_bold().set_text_wrap().set_align(FormatAlign::Center).set_align(FormatAlign::VerticalCenter);
    let entete = Format::new().set_text_wrap().set_align(FormatAlign::Top);
    for (a, b, c, d) in FUSIONS {
        ws.merge_range(a, b, c, d, "", &titre).map_err(xl)?;
    }
    for (ligne, col, texte) in ENTETES {
        ws.write_string_with_format(ligne, col, texte, if ligne < 2 { &titre } else { &entete }).map_err(xl)?;
    }
    ws.set_row_height(2, 132.0).map_err(xl)?;
    ws.set_freeze_panes(3, 0).map_err(xl)?;
    ws.set_column_range_width(0, 36, 14.0).map_err(xl)?;
    let siret = ctx.entreprise.get("siret").filter(|v| vrai(v)).map(py_str).unwrap_or_default();
    let aujourdhui = droit::contexte::jour_reel().jj_mm_aaaa();
    let mut cumul: Vec<((String, Option<String>), Decimal)> = Vec::new();
    let texte_de = |m: &Value, cle: &str| -> String { m.get(cle).filter(|v| vrai(v)).map(py_str).unwrap_or_default() };
    let coupe = |t: String, n: usize| -> String { t.chars().take(n).collect() };
    let arrondi = |d: Decimal| -> f64 {
        use rust_decimal::prelude::ToPrimitive;
        d.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero).to_f64().unwrap_or(0.0)
    };
    for (n, x) in r.emplois.iter().enumerate() {
        let ligne = 3 + n as u32;
        let (i, c) = (&x.individu, &x.contrat);
        let artiste = texte_de(c, "pcs_ese").starts_with("354");
        let nir = i.get("nir").filter(|v| vrai(v)).map(py_str);
        let cle = (x.annee.clone(), nir.clone());
        let total = match cumul.iter_mut().find(|(k, _)| *k == cle) {
            Some((_, t)) => {
                *t += x.brut;
                *t
            }
            None => {
                cumul.push((cle, x.brut));
                x.brut
            }
        };
        let cle_nir = nir.as_deref().and_then(crate::champs::cle_nir).unwrap_or_default();
        let jj = |iso: &Option<String>| -> String {
            match iso {
                Some(t) if !t.is_empty() => jjmmaaaa(t),
                _ => String::new(),
            }
        };
        let textes: Vec<(u16, String)> = vec![
            (0, "CASC-SVP".to_string()),
            (1, siret.clone()),
            (4, x.annee.clone()),
            (5, aujourdhui.clone()),
            (6, coupe(texte_de(i, "nom"), 30)),
            (8, coupe(texte_de(i, "prenom"), 32)),
            (9, jj(&i.get("date_naissance").filter(|v| vrai(v)).map(py_str))),
            (10, format!("{}{}", nir.clone().unwrap_or_default(), cle_nir)),
            (13, coupe(texte_de(i, "adresse"), 32)),
            (15, coupe(texte_de(i, "code_postal"), 5)),
            (16, coupe(texte_de(i, "commune"), 26)),
            (19, texte_de(i, "email")),
            (21, coupe(texte_de(c, "emploi"), 34)),
            (22, jj(&x.debut)),
            (23, jj(&x.fin)),
            (25, "I".to_string()),
            (30, if artiste { "A" } else { "T" }.to_string()),
        ];
        for (col, t) in textes {
            ws.write_string(ligne, col, &t).map_err(xl)?;
        }
        ws.write_number(ligne, 24, arrondi(x.brut)).map_err(xl)?;
        ws.write_number(ligne, 36, arrondi(total)).map_err(xl)?;
        let act = |q: &str| x.activite.iter().find(|(k, _)| *k == q).map(|(_, v)| v);
        let colonnes: [(u16, &str); 2] = if artiste { [(33, "cachets"), (34, "heures")] } else { [(31, "jours"), (32, "heures")] };
        for (col, quoi) in colonnes {
            if let Some(v) = quantite_xlsx(act(quoi)) {
                ws.write_number(ligne, col, v).map_err(xl)?;
            }
        }
    }
    let sans: Vec<&EmploiReg> = r.emplois.iter().filter(|x| x.activite.is_empty()).collect();
    if !sans.is_empty() {
        let mut mois: Vec<&str> = sans.iter().map(|x| x.mois.as_str()).collect();
        mois.sort();
        mois.dedup();
        r.avertissements.push(format!(
            "Liste nominative : {} emploi(s) sans jours, cachets ni heures dans leur DSN ({}) -- colonnes laissees vides.",
            sans.len(),
            mois.join(", ")
        ));
    }
    let chemin = dossier.join("CASC-SVP_liste-nominative.xlsx");
    wb.save(&chemin).map_err(|e| erreur("OSError", format!("{} : {}", chemin.display(), e)))?;
    fichiers.push(chemin);
    Ok(())
}

/// `operations_casc.marquer_regularise` (route `casc-regularise`) : note qu'une annee est regularisee. S'ecrit
/// APRES le virement, jamais avant.
pub(crate) fn marquer_regularise(ctx: &Contexte, annee: &Value, vire_le: &Value, montant: &Value, qui: &str, aujourd_hui: &str) -> R<Value> {
    let annee_t = py_str(annee);
    let r = regularisation(ctx, &Demande { annee: Some(annee_t.clone()), ..Default::default() })?;
    let p = fichier_suivi(ctx);
    let mut d = match lire_json(&p) {
        Ok(Value::Object(o)) => Value::Object(o),
        Ok(autre) => autre,
        Err(_) => json!({"_quoi": "Les regularisations CASC-SVP faites hors DSN (moteur/operations_casc.py). Un mois ici ne revient plus dans un dossier de regularisation."}),
    };
    let calcule = r.mois.iter().fold(Decimal::ZERO, |t, m| t + m.cotisation);
    let entree = json!({
        "annee": annee_t,
        "mois": r.mois.iter().map(|m| m.mois.clone()).collect::<Vec<_>>(),
        "montant": py_str(montant),
        "calcule": calcule.to_string(),
        "vire_le": vire_le,
        "note_le": aujourd_hui,
        "qui": qui,
    });
    let Value::Object(o) = &mut d else {
        return Err(erreur("AttributeError", "config/casc-svp.json : un objet est attendu"));
    };
    match o.entry("regularisations").or_insert_with(|| json!([])) {
        Value::Array(a) => a.push(entree.clone()),
        _ => return Err(erreur("AttributeError", "'regularisations' n'est pas une liste")),
    }
    ecrire_json(&p, &d, 2).map_err(|m| erreur("OSError", m))?;
    Ok(entree)
}

#[cfg(test)]
mod tests_mois {
    /// Lot F1 : le mois « 00 » d'une regularisation CASC-SVP se refuse (l'indice `0 - 1` paniquait).
    #[test]
    fn le_mois_00_de_la_regularisation_se_refuse() {
        let refus = std::panic::catch_unwind(|| super::libelle_du_mois("2026-00", "2026").map_err(|e| e.message));
        let refus = refus.expect("un refus, pas une panique").unwrap_err();
        assert!(refus.contains(&socle::date::mois_impossible(0)), "{}", refus);
        assert_eq!(super::libelle_du_mois("2025-03", "2025").unwrap(), "mars 2025");
    }
}
