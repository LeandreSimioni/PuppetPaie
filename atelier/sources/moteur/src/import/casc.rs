//! LA REGULARISATION DE LA CASC-SVP DES MOIS IMPORTES : le dossier que prepare `exporter regularisation-casc-svp`, et
//! `casc-regularise`, qui note qu'une annee est regularisee. Un mois tenu dans un autre logiciel a pu partir sans la
//! CASC-SVP ; PuppetPaie la declare lui-meme dans les DSN qu'il fait (`declarations::dsn`).
//!
//! Tout se lit dans les engagements importes (`engagement::importe`) : les paies versees (le brut que la DSN deposee ou
//! le bulletin dit), la fiche du salarie, le contrat atteste (numero, bornes, cachets, heures et jours). Un mois dont la
//! DSN importee porte deja la CASC-SVP (`import::ecrite::lire_dsn_casc`), ou deja regularise
//! (`config/casc-svp.json`), n'y entre pas.
//!
//! Le classeur du CASC-SVP se recree avec les colonnes du modele (`donnees/modeles/casc-svp-liste-nominative.xlsx`) :
//! voir `liste_nominative_xlsx`.

use std::path::Path;
use std::str::FromStr;

use rust_decimal::RoundingStrategy;
use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use socle::ecriture::{ecrire_json, repr_flottant};
use socle::valeur::valeur_en_texte;
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
pub fn eur(v: Decimal) -> String {
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

/// Ce qui a ete verse a un salarie un mois importe, tel que l'engagement importe le porte.
struct Versee {
    salarie: Value,
    brut: Decimal,
    numero: Option<String>,
    debut: Option<String>,
    fin: Option<String>,
    /// Les cachets, heures et jours que le contrat atteste, quand il tient dans le mois.
    activite: Vec<(&'static str, Decimal)>,
}

/// Les paies versees de `mois`, d'apres les engagements importes : une par engagement qui a un brut ce mois-la. Un
/// salarie sans fiche refuse (sa ligne de la liste nominative ne se devine pas).
fn versees_du_mois(ctx: &Contexte, mois: &str) -> R<Vec<Versee>> {
    let mut out = Vec::new();
    let t = |x: &Value, k: &str| x.get(k).filter(|v| vrai(v)).map(valeur_en_texte);
    for i in engagement::importe::du_mois(&ctx.racine, mois).map_err(socle::echec::Echec::from)? {
        let Some(p) = i.lignes_des_paies().into_iter().find(|p| p["mois"] == json!(mois)) else { continue };
        let Some(brut) = t(&p, "brut") else { continue };
        let cle = t(&p, "salarie").unwrap_or_default();
        let salarie = ctx.salaries.get(&cle).cloned().ok_or_else(|| refus(format!("{} : la fiche du salarie « {} » d'un engagement importe n'existe pas.", mois, cle)))?;
        let (debut, fin) = (t(&p, "debut"), t(&p, "fin"));
        let mut activite = Vec::new();
        if let Some(c) = i.ligne_du_contrat() {
            let dans_le_mois = |b: &Option<String>| b.as_deref().is_some_and(|b| b.starts_with(mois));
            if dans_le_mois(&t(&c, "debut")) && dans_le_mois(&t(&c, "fin")) {
                for quoi in ["jours", "cachets", "heures"] {
                    if let Some(v) = t(&c, quoi).and_then(|v| dec(&v).ok()).filter(|v| !v.is_zero()) {
                        activite.push((quoi, v));
                    }
                }
            }
        }
        out.push(Versee { salarie, brut: dec(&brut)?, numero: t(&p, "contrat").filter(|_| t(&p, "declare_par").as_deref() != Some("bulletin")), debut, fin, activite });
    }
    Ok(out)
}

/// Un mois du dossier.
pub struct MoisReg {
    pub mois: String,
    pub libelle: String,
    pub masse: Decimal,
    pub taux: Decimal,
    pub cotisation: Decimal,
    pub source: String,
}

pub struct AnneeReg {
    pub annee: String,
    pub masse: Decimal,
    pub taux: Decimal,
    pub cotisation: Decimal,
    pub libelle_virement: String,
}

pub struct SalarieReg {
    pub annee: String,
    pub salarie: String,
    pub nir: Option<String>,
    pub date_naissance: Option<String>,
    pub adresse: String,
    pub masse: Decimal,
}

pub struct EmploiReg {
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

pub struct Courriel {
    pub a: String,
    pub objet: String,
    pub corps: String,
    pub piece_jointe: Option<&'static str>,
}

/// La regularisation CASC-SVP d'une annee : ce qui manque, par salarie et par mois, et ce qui en part.
pub struct Regularisation {
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

/// Ce que demande une regularisation (`regularisation`).
#[derive(Default)]
pub struct Demande {
    pub mois: Option<String>,
    pub annee: Option<String>,
    pub toutes_les_annees: bool,
    pub jusqu_a: Option<String>,
    pub annees: Vec<String>,
    /// {annee: AAAA-MM-JJ}
    pub vire_le: Vec<(String, String)>,
    /// La boite courriel de la compagnie est-elle liee ? Le dossier le recopie (`Regularisation::boite_liee`).
    pub boite_liee: bool,
}

/// « mars 2025 » : le nom d'un mois de la regularisation. « 00 » n'est aucun mois et se refuse
/// (`date::numero_du_mois`).
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
                autre => valeur_en_texte(autre),
            };
            Ok(Some(dec(&t)?))
        }
        _ => Ok(None),
    }
}

/// La regularisation CASC-SVP que demande `d`, calculee sans rien ecrire (l'ecriture des fichiers : `ecrire`).
pub fn regularisation(ctx: &Contexte, d: &Demande) -> R<Regularisation> {
    let base = declarations::casc::base().map_err(|m| erreur("OSError", m))?;
    let ent = &ctx.entreprise;
    let idcc = ent.get("idcc").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    if idcc != valeur_en_texte(&base["idcc"]) {
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
    let faits = declarations::casc::mois_regularises(ctx).map_err(|m| erreur("OSError", m))?;
    let depuis = valeur_en_texte(&base["depuis"]);
    let mut candidats: Vec<String> = crate::import::mois::mois_importes(&ctx.racine)?
        .into_iter()
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
    let mut avert: Vec<String> = vec![];
    let mut portees: Vec<String> = vec![];
    let mut lues: Vec<(String, Vec<Versee>)> = vec![];
    let contrat_casc = valeur_en_texte(&base["dsn"]["contrat"]);
    for m in &candidats {
        let paies = versees_du_mois(ctx, m)?;
        if paies.is_empty() {
            continue;
        }
        let dsn = crate::import::ecrite::dsn_importee(ctx, m).map_err(refus)?;
        if dsn.as_deref().and_then(|c| crate::import::ecrite::lire_dsn_casc(c, &contrat_casc)).is_some_and(|(_, porte)| porte) {
            portees.push(m.clone());
            continue;
        }
        lues.push((m.clone(), paies));
    }
    if lues.is_empty() {
        if !faits.is_empty() {
            return Err(refus(format!(
                "Tout est regularise : {}. Les DSN suivantes portent la CASC-SVP elles-memes.",
                deja_faites(ctx)
            )));
        }
        return Err(refus(format!(
            "Aucun mois importe sans la CASC-SVP dans sa DSN depuis {} dans la periode demandee : rien a regulariser.",
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
    for (m, paies) in &lues {
        let source = format!("engagements importes de {}", m);
        let mut masse = Decimal::ZERO;
        let mut mois_sal: Vec<(String, String, Decimal)> = vec![];
        let an: String = m.chars().take(4).collect();
        for p in paies {
            let s = &p.salarie;
            let champ = |cle: &str| s.get(cle).filter(|v| vrai(v)).map(valeur_en_texte);
            let brut = p.brut;
            let nir = champ("nir").unwrap_or_default();
            masse += brut;
            match mois_sal.iter_mut().find(|(n, _, _)| *n == nir) {
                Some((_, _, t)) => *t += brut,
                None => mois_sal.push((nir.clone(), champ("nom").unwrap_or_else(|| nir.clone()), brut)),
            }
            let cle = (an.clone(), nir.clone());
            let pos = match par_salarie.iter().position(|(k, _)| *k == cle) {
                Some(i) => i,
                None => {
                    let adresse: Vec<String> = ["adresse", "code_postal", "commune"].iter().filter_map(|k| champ(k)).collect();
                    par_salarie.push((
                        cle,
                        SalarieReg {
                            annee: an.clone(),
                            salarie: format!("{} {}", champ("nom").unwrap_or_default(), champ("prenom").unwrap_or_default()),
                            nir: Some(nir.clone()),
                            date_naissance: champ("date_naissance"),
                            adresse: adresse.join(" "),
                            masse: Decimal::ZERO,
                        },
                    ));
                    par_salarie.len() - 1
                }
            };
            par_salarie[pos].1.masse += brut;
            let mut individu = Map::new();
            for k in ["nir", "nom", "prenom", "date_naissance", "adresse", "code_postal", "commune", "email"] {
                if let Some(v) = champ(k) {
                    individu.insert(k.to_string(), json!(v));
                }
            }
            let mut contrat = Map::new();
            for (k, v) in [("numero", p.numero.clone()), ("debut", p.debut.clone()), ("fin", p.fin.clone()), ("pcs_ese", champ("pcs_ese")), ("emploi", champ("emploi"))] {
                if let Some(v) = v {
                    contrat.insert(k.to_string(), json!(v));
                }
            }
            emplois.push(EmploiReg {
                annee: an.clone(),
                mois: m.clone(),
                individu: Value::Object(individu),
                activite: p.activite.clone(),
                debut: p.debut.clone(),
                fin: p.fin.clone(),
                contrat: Value::Object(contrat),
                brut,
                fichier: source.clone(),
            });
        }
        if let Some(plafond) = plafond_mensuel(&an)? {
            for (_, nom, b) in &mois_sal {
                if *b > plafond {
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
            autre => valeur_en_texte(autre),
        })?;
        let mut cot = (masse * taux / Decimal::from(100)).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
        cot.rescale(2);
        par_mois.push(MoisReg { mois: m.clone(), libelle: libelle_du_mois(m, &an)?, masse, taux, cotisation: cot, source });
    }

    let siret = ent.get("siret").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    let mut annees: Vec<AnneeReg> = vec![];
    for r in &par_mois {
        let an: String = r.mois.chars().take(4).collect();
        let pos = match annees.iter().position(|a| a.annee == an) {
            Some(p) => p,
            None => {
                annees.push(AnneeReg {
                    libelle_virement: valeur_en_texte(&base["reglement"]["libelle"]).replace("{siret}", &siret).replace("{annee}", &an),
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

    let nom = ent.get("raison_sociale").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    let sig = ent.get("signataire").cloned().unwrap_or(Value::Null);
    let sig = if vrai(&sig) { sig } else { json!({}) };
    let ou = |a: &str, b: &str| sig.get(a).filter(|v| vrai(v)).or_else(|| sig.get(b)).filter(|v| vrai(v)).map(valeur_en_texte);
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
    let beneficiaire = valeur_en_texte(&base["reglement"]["beneficiaire"]);
    let corps_audiens = format!(
        "Bonjour,\n\nNotre logiciel de paie n'était pas paramétré pour déclarer la cotisation CASC-SVP en DSN (contrat CASCSVP) : elle manque à nos DSN de {}. Nous régularisons par virement, comme l'indique la page employeurs du CASC-SVP, et nos DSN la porteront désormais.\n\n{} — SIRET {} — IDCC 3090.\n\nMasses salariales soumises à cotisation (tranche 1 Agirc-Arrco) :\n\n{}\n\nVirements effectués vers {} :\n{}\n\nLa liste nominative des salariés part en parallèle au CASC-SVP.\n\nBien cordialement,\n\n{}\n{}\n",
        periode, nom, siret, tableau.join("\n"), beneficiaire, virements.join("\n"), signe, nom
    );
    let a_audiens = valeur_en_texte(&base["courriel_audiens"]["a"]);
    let corps_casc = format!(
        "Bonjour,\n\nNotre logiciel de paie ne déclarait pas la cotisation CASC-SVP en DSN. Nous régularisons auprès d'Audiens ({}) les cotisations de {}, et vous trouverez en pièce jointe la liste nominative de nos salariés pour cette période.\n\n{} — SIRET {} — IDCC 3090.\n\nBien cordialement,\n\n{}\n{}\n",
        a_audiens, periode, nom, siret, signe, nom
    );
    let courriel_audiens = Courriel { a: a_audiens.clone(), objet: valeur_en_texte(&base["courriel_audiens"]["objet"]).replace("{siret}", &siret), corps: corps_audiens, piece_jointe: None };
    let courriel_casc = Courriel {
        a: valeur_en_texte(&base["courriel_casc"]["a"]),
        objet: valeur_en_texte(&base["courriel_casc"]["objet"]).replace("{siret}", &siret),
        corps: corps_casc,
        piece_jointe: Some("CASC-SVP_liste-nominative.xlsx (le modele du CASC-SVP, rempli)"),
    };
    let mut a_faire = vec![format!(
        "Faire {} virement(s), un par année, à {} — l'IBAN se lit sur la page employeurs du CASC-SVP ({}) ou dans l'espace Audiens :",
        annees.len(),
        beneficiaire,
        valeur_en_texte(&base["page"])
    )];
    for a in &annees {
        a_faire.push(format!("    {} : {}, libellé « {} »", a.annee, eur(a.cotisation), a.libelle_virement));
    }
    a_faire.push(format!("Reporter la date de chaque virement dans le courriel à Audiens, puis l'envoyer à {} (objet : {}).", courriel_audiens.a, courriel_audiens.objet));
    a_faire.push(format!(
        "Envoyer à {} (objet : {}) le courriel au CASC-SVP, avec en pièce jointe CASC-SVP_liste-nominative.xlsx — leur modèle, déjà rempli.",
        courriel_casc.a, courriel_casc.objet
    ));
    a_faire.push("Une fois les virements faits, le noter dans le logiciel : Exports, bouton « Noter comme régularisé », ou `moteur casc-regularise ANNEE --vire-le AAAA-MM-JJ --montant X` — les mois sortent alors des dossiers suivants.".to_string());
    a_faire.push("Vérifier que la fiche de paramétrage Audiens porte les deux lignes CASC-SVP, pour que les DSN suivantes la déclarent.".to_string());

    let boite = d.boite_liee;
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
            let g = |c: &str| x.get(c).map_or_else(|| "None".to_string(), valeur_en_texte);
            format!("{} ({} €, viré le {})", g("annee"), g("montant"), jjmmaaaa(&x.get("vire_le").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default()))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// `Regularisation` en JSON, tel que `sortie_json.brut` le rend (Decimal en texte).
pub fn vers_json(r: &Regularisation) -> Value {
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

/// L'ecriture de la regularisation : `exports/casc-svp/` -- les CSV, les deux courriels, la liste des gestes.
pub fn ecrire(ctx: &Contexte, r: &mut Regularisation) -> R<()> {
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

/// La liste nominative du CASC-SVP, une ligne par emploi, a partir de la 4e ligne.
///
/// Le modele officiel (`donnees/modeles/casc-svp-liste-nominative.xlsx`) ne s'ouvre pas : le classeur est RECREE par
/// `rust_xlsxwriter` avec les memes colonnes (37), les memes trois lignes de consignes (memes textes, memes cellules
/// fusionnees) et les memes valeurs ; la mise en forme est plus simple (gras, retour a la ligne, volet fige sous la
/// 3e ligne). Les consignes de la 4e ligne du modele (« NE PAS REMPLIR »...) n'y sont pas : elles sont
/// effacees avant d'ecrire.
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
    let siret = ctx.entreprise.get("siret").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    let aujourdhui = droit::contexte::jour_reel().jj_mm_aaaa();
    let mut cumul: Vec<((String, Option<String>), Decimal)> = Vec::new();
    let texte_de = |m: &Value, cle: &str| -> String { m.get(cle).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default() };
    let coupe = |t: String, n: usize| -> String { t.chars().take(n).collect() };
    let arrondi = |d: Decimal| -> f64 {
        use rust_decimal::prelude::ToPrimitive;
        d.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero).to_f64().unwrap_or(0.0)
    };
    for (n, x) in r.emplois.iter().enumerate() {
        let ligne = 3 + n as u32;
        let (i, c) = (&x.individu, &x.contrat);
        let artiste = texte_de(c, "pcs_ese").starts_with("354");
        let nir = i.get("nir").filter(|v| vrai(v)).map(valeur_en_texte);
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
        let cle_nir = nir.as_deref().and_then(declarations::champs::cle_nir).unwrap_or_default();
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
            (9, jj(&i.get("date_naissance").filter(|v| vrai(v)).map(valeur_en_texte))),
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

/// La route `casc-regularise` note qu'une annee est regularisee. S'ecrit
/// APRES le virement, jamais avant.
pub fn marquer_regularise(ctx: &Contexte, annee: &Value, vire_le: &Value, montant: &Value, qui: &str, aujourd_hui: &str) -> R<Value> {
    let annee_t = valeur_en_texte(annee);
    let r = regularisation(ctx, &Demande { annee: Some(annee_t.clone()), ..Default::default() })?;
    let p = fichier_suivi(ctx);
    let mut d = match lire_json(&p) {
        Ok(Value::Object(o)) => Value::Object(o),
        Ok(autre) => autre,
        Err(_) => json!({"_quoi": "Les regularisations CASC-SVP faites hors DSN. Un mois ici ne revient plus dans un dossier de regularisation."}),
    };
    let calcule = r.mois.iter().fold(Decimal::ZERO, |t, m| t + m.cotisation);
    let entree = json!({
        "annee": annee_t,
        "mois": r.mois.iter().map(|m| m.mois.clone()).collect::<Vec<_>>(),
        "montant": valeur_en_texte(montant),
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
