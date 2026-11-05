//! `depot-apercu` pour une DSN : ce qui partirait, en clair, sans rien envoyer ni demander -- le fichier lu, son
//! annule-et-remplace, et ce qu'il declare compare aux engagements valides du mois, contrat par contrat.
//!
//! Aucun reseau, aucun mot de passe, aucune ecriture de ce module.
//!
//! Les pieces reprises du reste du moteur : `Moteur::etat_remplacement` (l'annule-et-
//! remplace d'un etablissement), `engagement::etats::validees_du_mois` et
//! `ecrire_documents::paies_aux_numeros_declares` (les paies validees, numerotees), `depot::lire` (la lecture du
//! fichier), `etats::{deja_depose, repetition}` (le journal).
//!
//! Un nombre `NaN`, `Infinity`, ou dont l'exposant sortirait des 28 chiffres de `rust_decimal`, se refuse comme un
//! nombre illisible.

use std::collections::{HashMap, HashSet};

use serde_json::{json, Value};

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use envois::depot::{self, Apercu};
use envois::etats;
use socle::echec::erreur;
use socle::fichiers::{repr, vrai};
use socle::num::{decimal_lu, entier_lu, q2};

use socle::echec::Echec;
use socle::mois::Mois;
use serde::Serialize;
use crate::routes::Pastille;
use crate::routes::depots::{lire_octets, nom_du_chemin, resoudre};
use crate::routes::etapes::Moteur;
use documents::ecrire_documents::paies_aux_numeros_declares;
use declarations::dsn_lue::{ContratDeclare, DsnLue, Mesures, VersementLu};

type R<T> = Result<T, Echec>;

const TYPES: [(&str, &str); 5] = depot::TYPES;

/// La meme, sans le detail du refus : le moindre refus, quel qu'il soit, rend `Err`.
fn saisie_aux_numeros_declares(ctx: &Contexte, mois: &str) -> Result<Vec<Paie>, String> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    paies_aux_numeros_declares(ctx, mois, None, &departs).map(|t| t.1).map_err(|r| r.message)
}

// ------------------------------------------------------------------ recapitulatif_dsn

/// `format(d.normalize(), "f")`, avec `0` pour zero.
fn norm(d: rust_decimal::Decimal) -> String {
    if d.is_zero() {
        "0".to_string()
    } else {
        d.normalize().to_string()
    }
}

/// Une mesure d'activite (`jours`, `cachets`, `heures`) en texte.
fn mesure_texte(m: &Mesures, cle: &str) -> String {
    norm(match cle {
        "jours" => m.jours,
        "cachets" => m.cachets,
        _ => m.heures,
    })
}

/// Un jour de la DSN en AAAA-MM-JJ, vide s'il manque.
fn jour_texte(j: Option<socle::date::Jour>) -> String {
    j.map(|j| j.iso()).unwrap_or_default()
}

/// Un contrat du fichier (`DsnLue`) en texte, et ce qu'il declare pour le mois et en rappel d'un mois deja declare.
struct ContratLu {
    numero: String,
    debut: String,
    fin: String,
    emploi: String,
    nature: String,
    objet: String,
    fin_declaree: String,
    motif_fin: String,
    mois: Mesures,
    rappel: Mesures,
}

impl ContratLu {
    fn de(d: &DsnLue, c: &ContratDeclare) -> ContratLu {
        let numero = c.numero.clone().unwrap_or_default();
        let t = |v: &Option<String>| v.clone().unwrap_or_default();
        ContratLu {
            mois: d.mesures(&c.nir, &numero, false),
            rappel: d.mesures(&c.nir, &numero, true),
            debut: jour_texte(c.debut),
            fin: jour_texte(c.fin),
            emploi: t(&c.emploi.emploi),
            nature: t(&c.nature_dsn),
            objet: t(&c.objet),
            fin_declaree: jour_texte(c.fin_declaree),
            motif_fin: t(&c.motif_fin),
            numero,
        }
    }
}

/// Un versement du fichier (S21.G00.50.001, .004, .002, .006, .009), tel qu'il l'ecrit.
#[derive(Debug, Serialize)]
struct VersementDuFichier {
    date: String,
    net_verse: String,
    net_fiscal: String,
    taux_pas: String,
    montant_pas: String,
}

impl VersementDuFichier {
    fn de(v: &VersementLu) -> VersementDuFichier {
        let t = |m: Option<rust_decimal::Decimal>| m.map(|m| m.to_string()).unwrap_or_default();
        VersementDuFichier { date: jour_texte(v.verse_le), net_verse: t(v.net_verse), net_fiscal: t(v.net_imposable), taux_pas: t(v.taux_pas), montant_pas: t(v.pas) }
    }
}

/// `recapitulatif_dsn` : ce que la DSN declare, contrat par contrat, comparé aux contrats saisis.
fn recapitulatif_dsn(ctx: &Contexte, octets: &[u8], mois: &str, nic: &str) -> Result<RecapDsn, String> {
    let d = declarations::dsn_lue::lire(octets, "").map_err(|e| e.message)?;
    // Un mois sans engagement n'a rien a comparer.
    let (mut compare, liste) = match saisie_aux_numeros_declares(ctx, mois) {
        Ok(l) if !l.is_empty() => (true, l),
        _ => (false, Vec::new()),
    };
    // `paies` : numero -> paie, dans l'ordre ou les numeros sont apparus
    let mut ordre: Vec<String> = Vec::new();
    let mut par_numero: HashMap<String, usize> = HashMap::new();
    for (i, p) in liste.iter().enumerate() {
        if !nic.is_empty() {
            match droit::bareme::etablissement_de(&ctx.entreprise, &p.salarie) {
                Ok(e) if e == nic => {}
                Ok(_) => continue,
                Err(_) => {
                    compare = false;
                    break;
                }
            }
        }
        let n = p.engagement.numero.as_deref().unwrap_or("None").to_string();
        if !par_numero.contains_key(&n) {
            ordre.push(n.clone());
        }
        par_numero.insert(n, i);
    }
    let mut vus: HashSet<String> = HashSet::new();
    let mut salaries_json = Vec::new();
    let (mut n_contrats, mut n_ecarts, mut n_corrections) = (0usize, 0usize, 0usize);
    for sal in &d.individus {
        let contrats: Vec<ContratLu> = d.contrats_de(&sal.nir).map(|c| ContratLu::de(&d, c)).collect();
        let mut contrats_json = Vec::new();
        // Un contrat se dit par ce qu'il declare pour le mois, comparé a la paie du mois, et par ses rappels d'un mois deja
        // declare (`dsn_lue::Remuneration::rappel`), qui ne se comparent pas a la paie du mois. Un contrat qui n'a que des rappels
        // n'a pas de ligne du mois.
        for (c, correction) in contrats.iter().flat_map(|c| [(c, false), (c, true)]) {
            let m = if correction { &c.rappel } else { &c.mois };
            if (correction || c.rappel.remunere) && !m.remunere {
                continue;
            }
            vus.insert(c.numero.clone());
            let p = par_numero.get(&c.numero).map(|&i| &liste[i]);
            let mut ecarts: Vec<String> = Vec::new();
            let brut = q2(m.brut);
            match p {
                _ if correction => {}
                None => {
                    if compare {
                        ecarts.push("aucune paie du mois ne porte ce contrat".to_string());
                    }
                }
                Some(p) => {
                    let mut attendu: HashMap<&str, String> = HashMap::new();
                    // Le brut que la paie declare en 51 de type 001 (`dsn::brut_du_51` : abattu sous deduction forfaitaire
                    // specifique), a deux decimales.
                    attendu.insert("brut", q2(declarations::dsn::brut_du_51(p)));
                    for k in ["jours", "cachets", "heures"] {
                        attendu.insert(k, "0".to_string());
                    }
                    // Les mesures du travail remunere (type 01) que la DSN ecrit pour cette paie (`dsn::mesures_d_activite`).
                    for (t, valeur, code) in declarations::dsn::mesures_d_activite(p)? {
                        if t != "01" {
                            continue;
                        }
                        let Some(cle) = (match code.as_str() {
                            "10" | "35" => Some("heures"),
                            "12" => Some("jours"),
                            "37" => Some("cachets"),
                            _ => None,
                        }) else {
                            continue;
                        };
                        let avant = decimal_lu(&attendu[cle]).ok_or_else(|| socle::num::illisible(&attendu[cle]))?;
                        attendu.insert(cle, norm(avant + valeur));
                    }
                    for cle in ["brut", "jours", "cachets", "heures"] {
                        let fichier = if cle == "brut" { brut.clone() } else { mesure_texte(m, cle) };
                        let a = decimal_lu(&attendu[cle]).ok_or_else(|| socle::num::illisible(&attendu[cle]))?;
                        let f = decimal_lu(&fichier).ok_or_else(|| socle::num::illisible(&fichier))?;
                        if norm(f) != norm(a) {
                            ecarts.push(format!("{} : la paie dit {}, le fichier {}", cle, attendu[cle], fichier));
                        }
                    }
                    let emploi = p.salarie.emploi.en_valeur();
                    let attendu_emploi: Option<&str> = match &emploi {
                        Value::Null => Some(""),
                        Value::String(s) => Some(s.as_str()),
                        v if !vrai(v) => Some(""),
                        _ => None,
                    };
                    if attendu_emploi != Some(c.emploi.as_str()) {
                        let dit = match &emploi {
                            Value::Null => "None".to_string(),
                            v => socle::valeur::valeur_en_texte(v),
                        };
                        ecarts.push(format!("emploi : la paie dit {}, le fichier {}", dit, c.emploi));
                    }
                }
            }
            if correction {
                n_corrections += 1;
            } else {
                n_contrats += 1;
            }
            if !ecarts.is_empty() {
                n_ecarts += 1;
            }
            let (cachets, heures, jours) = (mesure_texte(m, "cachets"), mesure_texte(m, "heures"), mesure_texte(m, "jours"));
            let fin_dit = if correction {
                "mois déjà déclaré".to_string()
            } else if !c.fin_declaree.is_empty() {
                format!("fin le {}", crate::routes::jour_en_clair(&json!(c.fin_declaree)))
            } else {
                "en cours".to_string()
            };
            contrats_json.push(ContratDuFichier {
                activite_dit: activite_dite(&cachets, &heures, &jours, correction),
                dates_dit: dates_dites(&c.debut, &c.fin),
                fin_dit,
                numero: c.numero.clone(),
                debut: c.debut.clone(),
                fin: c.fin.clone(),
                emploi: c.emploi.clone(),
                nature: c.nature.clone(),
                objet: c.objet.clone(),
                brut,
                jours,
                cachets,
                heures,
                fin_declaree: c.fin_declaree.clone(),
                motif_fin: c.motif_fin.clone(),
                remunere: m.remunere,
                paie: p.is_some(),
                correction,
                // Un contrat du fichier qui ne dit pas ce que dit la paie : a corriger, la compagnie le peut.
                pastille_ecarts: (!ecarts.is_empty()).then(|| Pastille::de("a_faire", &ecarts.join(" ; "))),
                ecarts,
            });
        }
        let versements = d.versements_de(&sal.nir).map(VersementDuFichier::de).collect();
        let (nom, prenoms) = (sal.nom.clone().unwrap_or_default(), sal.prenom.clone().unwrap_or_default());
        salaries_json.push(SalarieDuFichier { nir: sal.nir.clone(), nom, prenoms, contrats: contrats_json, versements });
    }
    let mut absents = Vec::new();
    for n in &ordre {
        if vus.contains(n) {
            continue;
        }
        let p = &liste[par_numero[n]];
        let nom = p.salarie.nomme().unwrap_or_else(|| ("Ce salarié").to_string());
        absents.push(AbsentDuFichier {
            salarie: p.salarie.id.to_string(),
            dit: format!("{} — contrat {}, du {}", nom, n, p.engagement.debut.jj_mm_aaaa()),
            nom,
            numero: n.clone(),
            debut: p.engagement.debut.iso(),
            fin: p.engagement.fin.map_or(String::new(), |f| f.iso()),
            pastille: Pastille::de("a_faire", "absent du fichier"),
        });
    }
    let conforme = compare && absents.is_empty() && n_ecarts == 0;
    let s = |n: usize| if n > 1 { "s" } else { "" };
    let mut titre = format!("{} contrat{} dans ce fichier", n_contrats, s(n_contrats));
    if n_corrections > 0 {
        titre += &format!(" (et {} correction{} d'un mois déjà déclaré)", n_corrections, s(n_corrections));
    }
    if compare {
        titre += &format!(", {} saisi{} dans la paie du mois", par_numero.len(), s(par_numero.len()));
    }
    let phrase = if conforme {
        "Tous les contrats de la paie sont dans la DSN, tels que la paie les dit.".to_string()
    } else if compare {
        let mut p = Vec::new();
        if !absents.is_empty() {
            let n = absents.len();
            p.push(format!("{} contrat{} de la paie manque{} au fichier.", n, s(n), if n > 1 { "nt" } else { "" }));
        }
        if n_ecarts > 0 {
            let dit = if n_ecarts > 1 { "disent" } else { "dit" };
            p.push(format!("{} contrat{} du fichier ne {} pas ce que dit la paie : réécrire le mois.", n_ecarts, s(n_ecarts), dit));
        }
        p.join(" ")
    } else if n_contrats == 0 && n_corrections == 0 {
        // Un mois sans salarie a sa DSN, « Sans individu » (cahier technique DSN 2026.1, § 2.4,
        // `atelier/reference/norme-dsn/texte/chapitre-2-principes-de-constitution.txt` l. 2291-2309).
        "Aucun salarié ce mois-ci : la DSN se dépose sans individu.".to_string()
    } else {
        String::new()
    };
    Ok(RecapDsn {
        salaries: salaries_json,
        absents,
        titre,
        phrase,
        compare,
        conforme,
        // La couleur du bilan : conforme, vert ; un ecart ou un absent, orange ; rien a comparer, gris.
        couleur: if conforme { "vert" } else if compare { "orange" } else { "gris" },
    })
}

// ------------------------------------------------------------------ l'apercu

/// CE QUE LA DSN DECLARE, CONTRAT PAR CONTRAT, compare aux contrats saisis (`recapitulatif_dsn`) : ses salaries, les
/// contrats de la paie absents du fichier, le titre et la phrase de l'encart, sa couleur.
#[derive(Debug, Serialize)]
pub(crate) struct RecapDsn {
    salaries: Vec<SalarieDuFichier>,
    absents: Vec<AbsentDuFichier>,
    titre: String,
    pub(crate) phrase: String,
    compare: bool,
    conforme: bool,
    couleur: &'static str,
}

/// Le recapitulatif, ou pourquoi il ne se lit pas.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum Recapitulatif {
    Lu(RecapDsn),
    Illisible { erreur: String },
}

/// Un salarie du fichier : son identite, ses contrats, ses versements.
#[derive(Debug, Serialize)]
struct SalarieDuFichier {
    nir: String,
    nom: String,
    prenoms: String,
    contrats: Vec<ContratDuFichier>,
    versements: Vec<VersementDuFichier>,
}

/// Un contrat du fichier, ce qu'il declare pour le mois (ou en correction d'un mois deja declare), en clair, et ses
/// ecarts a la paie.
#[derive(Debug, Serialize)]
struct ContratDuFichier {
    activite_dit: String,
    dates_dit: String,
    fin_dit: String,
    numero: String,
    debut: String,
    fin: String,
    emploi: String,
    nature: String,
    objet: String,
    brut: String,
    jours: String,
    cachets: String,
    heures: String,
    fin_declaree: String,
    motif_fin: String,
    remunere: bool,
    paie: bool,
    correction: bool,
    pastille_ecarts: Option<Pastille>,
    ecarts: Vec<String>,
}

/// Un contrat de la paie du mois que le fichier ne porte pas.
#[derive(Debug, Serialize)]
struct AbsentDuFichier {
    salarie: String,
    nom: String,
    numero: String,
    debut: String,
    fin: String,
    dit: String,
    pastille: Pastille,
}

/// L'ANNULE-ET-REMPLACE DU MOIS (`Moteur::etat_remplacement`), et si ce fichier remplace la DSN reelle deja partie.
#[derive(Debug, Serialize)]
pub(crate) struct RemplacementDuFichier {
    #[serde(flatten)]
    pub(crate) etat: crate::routes::etapes::Remplacement,
    pub(crate) fichier_remplace: bool,
}

/// L'annule-et-remplace du mois, ou rien s'il ne se lit pas.
fn remplacement_du_fichier(m: &mut Moteur, a: &Apercu, mois: Mois) -> R<Option<RemplacementDuFichier>> {
    let nic = a.champ("etablissement");
    let nic = if nic.is_empty() { Value::Null } else { json!(nic) };
    let Ok(r) = m.etat_remplacement(mois, &nic) else { return Ok(None) };
    let genre = a.champ("type_declaration");
    let remplace = if (genre == "03" || genre == "05") && r.deposee {
        let ordre = a.champ("ordre");
        let n = if ordre.is_empty() {
            0
        } else {
            entier_lu(ordre).ok_or_else(|| erreur("ValueError", format!("nombre entier illisible : {}", repr(ordre))))?
        };
        n > i128::from(r.dernier_ordre.unwrap_or(0))
    } else {
        false
    };
    Ok(Some(RemplacementDuFichier { etat: r, fichier_remplace: remplace }))
}

/// CE QUE LE FICHIER DSN DECLARE, en clair (`apercu_depot_dsn`) : son en-tete, ce qu'il declare, ses ordres de
/// telepaiement, s'il est deja depose (`deja_depose` : la ligne du journal), l'annule-et-remplace, le recapitulatif et le
/// test.
#[derive(Debug, Serialize)]
pub(crate) struct ApercuDsn {
    pub(crate) fichier: String,
    pub(crate) nom: String,
    pub(crate) description: String,
    pub(crate) mois: Mois,
    pub(crate) nature_envoi: String,
    pub(crate) reel: bool,
    pub(crate) signalement: bool,
    pub(crate) nature: String,
    pub(crate) libelle_nature: String,
    pub(crate) evenement: String,
    pub(crate) type_declaration: String,
    pub(crate) libelle_type: &'static str,
    pub(crate) ordre: String,
    pub(crate) fraction: String,
    pub(crate) remplace: String,
    pub(crate) constitution: String,
    pub(crate) siren: String,
    pub(crate) salaries: usize,
    pub(crate) lignes: usize,
    pub(crate) pas_versements: usize,
    pub(crate) pas_bareme: usize,
    pub(crate) ordres_phrase: &'static str,
    pub(crate) ordres_titre: String,
    pub(crate) ordres: Vec<declarations::ordres_paiement::OrdreEnClair>,
    pub(crate) ordres_erreur: Option<&'static str>,
    pub(crate) couleur_ordres: &'static str,
    pub(crate) deja_depose: Option<envois::ligne::Ligne>,
    pub(crate) remplacement: Option<RemplacementDuFichier>,
    pub(crate) recapitulatif: Option<Recapitulatif>,
    pub(crate) repetition: etats::Repetition,
}

/// `apercu_depot_dsn` : ce qui partirait, en clair, sans rien envoyer ni demander.
pub(crate) fn apercu_depot_dsn(ctx: &Contexte, fichier: &str) -> R<ApercuDsn> {
    let chemin = resoudre(&ctx.racine, fichier)?;
    let contenu = lire_octets(&chemin)?;
    let a = depot::lire(&contenu);
    let mut m = Moteur::nouveau(ctx);
    let lignes = m.lignes()?;
    let empreinte = envois::journal::empreinte(&contenu);
    let deja = etats::deja_depose(&lignes, envois::journal::Acte::Dsn, &empreinte);
    let nom = nom_du_chemin(&chemin);
    let signalement = a.est_signalement();
    let mois = a.mois_declare().map_err(|m| erreur("ErreurOperation", format!("Ce fichier ne dit pas le mois qu'il déclare ({}).", m)))?;
    let mois_t = mois.to_string();
    let remplacement = if signalement { None } else { remplacement_du_fichier(&mut m, &a, mois)? };
    let recapitulatif = if signalement {
        None
    } else {
        Some(match recapitulatif_dsn(ctx, &contenu, &mois_t, a.champ("etablissement")) {
            Ok(v) => Recapitulatif::Lu(v),
            Err(erreur) => Recapitulatif::Illisible { erreur },
        })
    };
    let repetition = etats::repetition(&lignes, envois::journal::Acte::Dsn, &empreinte, &nom);
    // LES ORDRES DE PAIEMENT EN CLAIR : a qui, pour quoi, combien, pour quand -- ceux de la DSN fabriquee
    // depuis les paies qu'elle couvre (`depart::refabriquer`) ; un fichier qui ne dit plus ce que disent les paies n'en
    // montre aucun.
    let organismes = ctx.du_mois(&mois_t)?.organismes()?;
    let (ordres, ordres_erreur) = if signalement {
        (Vec::new(), None)
    } else {
        let nic = a.champ("etablissement");
        let nic = Some(nic).filter(|n| !n.is_empty());
        match crate::compagnie::depart::refabriquer(ctx, std::path::Path::new(&chemin), mois, nic)? {
            Some(f) => (declarations::ordres_paiement::decrire(&f.ordres, &mois_t, &organismes, ctx.echeance_dsn()?)?, None),
            None => (Vec::new(), Some("Ce fichier ne dit plus ce que disent les paies du mois : le refaire (Produire la DSN) avant de le déposer.")),
        }
    };
    let genre = a.champ("type_declaration").to_string();
    let nature = a.champ("nature");
    let libelle_type = TYPES.iter().find(|(c, _)| *c == genre).map_or("?", |(_, l)| *l);
    let description = format!(
        "{} — {} (type {}), ordre {}, fraction {}, constituée le {}. {} salarié{}, {} lignes, SIREN {}.",
        nom,
        libelle_type,
        genre,
        a.champ("ordre"),
        a.champ("fraction"),
        en_clair_jjmmaaaa(a.champ("constitution")),
        a.salaries,
        if a.salaries > 1 { "s" } else { "" },
        a.lignes,
        a.champ("siren")
    );
    Ok(ApercuDsn {
        fichier: chemin.clone(),
        nom,
        description,
        mois,
        nature_envoi: a.champ("nature_envoi").to_string(),
        reel: a.reel(),
        signalement,
        nature: if nature.is_empty() { depot::MENSUELLE.to_string() } else { nature.to_string() },
        libelle_nature: a.nature_lisible(),
        evenement: if signalement { a.evenement_lisible() } else { String::new() },
        type_declaration: genre,
        libelle_type,
        ordre: a.champ("ordre").to_string(),
        fraction: a.champ("fraction").to_string(),
        remplace: a.champ("remplace").to_string(),
        constitution: en_clair_jjmmaaaa(a.champ("constitution")),
        siren: a.champ("siren").to_string(),
        salaries: a.salaries,
        lignes: a.lignes,
        pas_versements: a.pas_versements,
        pas_bareme: a.pas_bareme,
        // Ce que la boite dit sous les ordres.
        ordres_phrase: if ordres.is_empty() { "Aucun ordre de télépaiement dans ce fichier." } else { "Chaque organisme prélèvera ce montant sur le compte de la compagnie ; la date se lit dans la DSN ou dans le texte qui la fixe (au survol)." },
        ordres_titre: format!("Ordres de télépaiement ({})", ordres.len()),
        ordres,
        ordres_erreur,
        // Un depot reel les declenche tous : a regarder (orange) ; joues, rien ne part (une information).
        couleur_ordres: crate::routes::couleur_de_l_etat(if crate::compagnie::bridage::branchement(&ctx.racine).joue() { "information" } else { "a_faire" }),
        deja_depose: deja,
        remplacement,
        recapitulatif,
        repetition,
    })
}

/// Une date de la norme (JJMMAAAA, S20.G00.05.007) telle qu'on la lit : JJ/MM/AAAA. Autre chose se montre tel quel.
fn en_clair_jjmmaaaa(d: &str) -> String {
    if d.len() == 8 && d.bytes().all(|b| b.is_ascii_digit()) {
        format!("{}/{}/{}", &d[..2], &d[2..4], &d[4..])
    } else {
        d.to_string()
    }
}

/// Un nombre du fichier, a la francaise ; une correction le signe d'un vrai moins (« −184 » : ce qui s'ajoute a ce qui a
/// deja ete declare).
fn nombre_dit(t: &str) -> String {
    let x = decimal_lu(t).unwrap_or_default();
    socle::texte::virgule(x).replacen('-', "−", 1)
}

/// L'ACTIVITE D'UN CONTRAT EN UNE COLONNE : « 4 cachets · 4 j », « 151,67 h » ; « correction : » devant celle d'un mois
/// deja declare ; « — » sans rien.
fn activite_dite(cachets: &str, heures: &str, jours: &str, correction: bool) -> String {
    let vide = |t: &str| decimal_lu(t).is_none_or(|x| x.is_zero());
    let mut l = Vec::new();
    if !vide(cachets) {
        let n = decimal_lu(cachets).unwrap_or_default().abs();
        l.push(format!("{} {}", nombre_dit(cachets), if n > rust_decimal::Decimal::ONE { "cachets" } else { "cachet" }));
    }
    if !vide(heures) {
        l.push(format!("{} h", nombre_dit(heures)));
    }
    if !vide(jours) {
        l.push(format!("{} j", nombre_dit(jours)));
    }
    let a = if l.is_empty() { "—".to_string() } else { l.join(" · ") };
    if correction { format!("correction : {}", a) } else { a }
}

/// Les dates d'un contrat sur une ligne : « 06/10 → 09/10/2026 » dans une meme annee, le jour seul s'il n'en a qu'un.
fn dates_dites(debut: &str, fin: &str) -> String {
    let jour = |t: &str| crate::routes::jour_en_clair(&json!(t));
    if fin.is_empty() || fin == debut {
        return jour(debut);
    }
    let d = jour(debut);
    let d = if debut.get(..4) == fin.get(..4) { d.chars().take(5).collect() } else { d };
    format!("{} → {}", d, jour(fin))
}
