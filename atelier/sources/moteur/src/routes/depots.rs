//! Les routes des declarations : ecrire ou reprendre la DSN ou la DPAE du mois et dire ce
//! qu'elle declare (`depot-apercu`), la deposer (`deposer`), suivre un depot (`suivre-depot`), le bilan de son test (`bilan-repetition`).
//!
//! Les formats sont ailleurs (`dpae`, la DSN de `declarations`, la fabrique de PDF) : ce module enchaine -- charger,
//! choisir, refuser, nommer, ecrire, et passer a `envois` ce qui part. Le jour est celui du contexte
//! (`PUPPETPAIE_AUJOURDHUI`) ; l'horodatage d'un fichier partiel de DPAE est la minute de l'horloge, a l'heure de Paris.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::compagnie::depot_acte::{self, phrase_de, Preuve};
use crate::routes::catalogue::{Acte, Champ, Fiche, Refus, MANQUE, MOIS, OPERATION};
use declarations::dpae::{Chemins, Embauche, Parametres};
use droit::contexte::Contexte;
use engagement::calcul::Paie;
use envois::etats;
use envois::releve_retours;
use socle::date::en_texte;
use socle::echec::{erreur, Echec, R};
use socle::fichiers::{dossier_du_code, vrai};
use socle::valeur::{nom_type, valeur_en_texte};
use crate::routes::catalogue::{Ecran, Place};
use super::mot;

// LA DESIGNATION d'une declaration (`designee`) : la DSN ou la DPAE d'un mois, ou un signalement ecrit.
const QUOI: Champ = Champ { nom: "quoi", sens: "dpae ou dsn", avance: false };
const ETABLISSEMENT: Champ = Champ { nom: "etablissement", sens: "le NIC de l'établissement (DSN), le principal sinon", avance: true };
const ENGAGEMENTS: Champ = Champ { nom: "engagements", sens: "seulement ces embauches (DPAE) : [{salarie, numero}]", avance: true };
const SIGNALEMENT: Champ = Champ { nom: "fichier", sens: "un signalement écrit, tel que sa route le rend, à la place du mois", avance: true };
/// L'exemple : la DPAE de novembre, que `depot-apercu` ecrit.
const DPAE_DE_L_EXEMPLE: &str = r#"{"quoi":"dpae","mois":"2026-11"}"#;
const ILLISIBLE_FICHIER: Refus = Refus { classe: "FileNotFoundError", corriger: "le signalement : le fichier que sa route a rendu" };

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Paie);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        nom: "depot-apercu",
        geste: "Écrire ou reprendre la DSN ou la DPAE du mois ; dire ce qu'elle déclare.",
        charge: &[QUOI, MOIS, ETABLISSEMENT, ENGAGEMENTS, SIGNALEMENT],
        exemple: DPAE_DE_L_EXEMPLE,
        rend: "ce qu'elle déclare (description, embauches, ou salariés, recapitulatif avec son titre et sa phrase, ordres et ordres_phrase), deja_depose, repetition (où en est son test, titre et phrase de l'encart), encarts (titre, phrase, couleur), phrase, avertissements, informations",
        refus: &[ILLISIBLE_FICHIER, MANQUE, OPERATION],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 300,
        ecran: ECRAN,
    },
    Fiche {
        nom: "deposer",
        geste: "Déposer la DSN ou la DPAE du mois : en test, ou en réel après son test.",
        charge: &[
            QUOI,
            MOIS,
            ETABLISSEMENT,
            ENGAGEMENTS,
            SIGNALEMENT,
            Champ { nom: "reel", sens: "vrai : le dépôt réel (le mot de passe du logiciel, tapé par un humain au moment de l'acte)", avance: false },
            Champ { nom: "forcer", sens: "vrai : l'annule-et-remplace d'une DSN déjà déposée, après l'avertissement", avance: true },
            Champ { nom: "contournement", sens: "le motif écrit d'un dépôt réel sans test", avance: true },
        ],
        exemple: DPAE_DE_L_EXEMPLE,
        rend: "le dépôt : accepté ou non, le retour de l'organisme, ses suites (chaque demande de taux), phrase, avis (l'encart à montrer après la DSN réelle d'un mois qui a des salariés)",
        refus: &[
            Refus { classe: "PreuveInsuffisante", corriger: "le mot de passe du logiciel, tapé par un humain ; rien n'est parti" },
            Refus { classe: "RepetitionManquante", corriger: "déposer d'abord ce fichier en test" },
            Refus { classe: "DsnDejaDeposee", corriger: "rien, ou renvoyer avec forcer si l'annule-et-remplace est voulu" },
            ILLISIBLE_FICHIER,
            OPERATION,
        ],
        acte: Acte::Engage,
        avance: false,
        phrase: 400,
        ecran: ECRAN,
    },
    Fiche {
        nom: "suivre-depot",
        geste: "Après un dépôt réel, relever les retours et dire s'il est passé ou refusé.",
        charge: &[QUOI, Champ { nom: "flux", sens: "le flux du dépôt, tel que deposer le rend", avance: false }, MOIS],
        exemple: r#"{"quoi":"dpae"}"#,
        rend: "fini, passe, refuse, incident (le relevé qui n'a pu se faire), phrase et couleur",
        refus: &[MANQUE],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 80,
        ecran: ECRAN,
    },
    Fiche {
        nom: "bilan-repetition",
        geste: "Dire où en est le test de la DSN ou de la DPAE ; chercher son compte rendu.",
        charge: &[QUOI, MOIS, ETABLISSEMENT, ENGAGEMENTS, SIGNALEMENT],
        exemple: DPAE_DE_L_EXEMPLE,
        rend: "etat (testee : le réel peut partir ; en_test ; test_refuse, et son motif ; ecrite), titre et phrase de l'encart, conforme, incident",
        refus: &[ILLISIBLE_FICHIER, MANQUE, OPERATION],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 500,
        ecran: ECRAN,
    },
];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    let mut v = match nom {
        // Les phrases de ses parties (le remplacement, le recapitulatif, le test), a la suite de la sienne.
        "depot-apercu" => depot_apercu(ctx, charge).map(|mut v| {
            let encarts = encarts_du_depot(&v);
            let mut lignes: Vec<Value> = ["remplacement", "recapitulatif", "repetition"].iter().filter_map(|k| v[*k]["phrase"].as_str().filter(|p| !p.is_empty()).map(|p| json!(p))).collect();
            lignes.extend(super::encarts_en_lignes(&encarts));
            v["encarts"] = Value::Array(encarts);
            lignes.extend(v["avertissements"].as_array().into_iter().flatten().cloned());
            v["phrases"] = Value::Array(lignes);
            // Une DSN se dit par sa nature, son mois et ses salaries quand rien d'autre n'est a dire ; une DPAE par ses
            // embauches.
            if !v["phrase"].as_str().is_some_and(|p| !p.is_empty()) {
                let mois = engagement::libelle_mois(v["mois"].as_str().unwrap_or(""));
                let (nature, combien) = match v.get("embauches") {
                    Some(e) => ("DPAE".to_string(), super::compte(super::nombre(e), "embauche", "embauches")),
                    None => (super::mot(&v["libelle_nature"]), super::compte(v["salaries"].as_u64().unwrap_or(0) as usize, "salarié", "salariés")),
                };
                let p = format!("{} {} : {}.", nature, socle::texte::de(&mois), combien);
                v = super::phrase_en_tete(v, p);
            }
            v
        }),
        "deposer" => deposer(ctx, charge),
        "suivre-depot" => suivre_depot(ctx, charge),
        "bilan-repetition" => bilan_repetition(ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }?;
    poser_les_pastilles(&mut v);
    Ok(v)
}

/// CE QUE LA BOITE DU DEPOT DIT DU FICHIER, encart par encart, dans l'ordre ou elle les montre : l'annule-et-remplace
/// qu'il porte, le prelevement a la source sur la grille par defaut (CGI, art. 204 H), les embauches deja declarees, le
/// fichier deja depose, le fichier code envoi de test.
fn encarts_du_depot(v: &Value) -> Vec<Value> {
    let mut e = Vec::new();
    let remplace = mot(&v["remplace"]);
    if !remplace.is_empty() {
        e.push(super::encart("information", "Annule et remplace", format!("Ce fichier remplace la déclaration {}.", remplace)));
    }
    let r = &v["remplacement"];
    if r["fichier_remplace"].as_bool() == Some(true) {
        e.push(super::encart(
            "a_faire",
            format!("Annule-et-remplace intégral — ordre {}", mot(&v["ordre"])),
            format!(
                "Il remplace la DSN réelle déjà partie pour ce mois (ordre {}), ordres de télépaiement compris : c'est lui qui vaudra. Admis jusqu'au {} à minuit.",
                mot(&r["dernier_ordre"]),
                super::jour_en_clair(&r["limite"])
            ),
        ));
    }
    let bareme = v["pas_bareme"].as_u64().unwrap_or(0);
    if bareme > 0 {
        e.push(super::encart(
            "a_faire",
            format!("Prélèvement à la source : {} versement{} sur {} sur la grille par défaut", bareme, if bareme > 1 { "s" } else { "" }, mot(&v["pas_versements"])),
            "Sans taux transmis par la DGFiP, la grille de l'article 204 H du CGI s'applique ; le dépôt n'est pas bloqué. Elle prélève souvent plus que le taux réel, et le salarié ne récupère la différence qu'à sa déclaration de revenus. Relever les retours avant de déposer peut changer cela.",
        ));
    }
    let deja = v["embauches"].as_array().into_iter().flatten().filter(|x| x["deja_declaree"].as_bool() == Some(true)).count();
    if deja > 0 {
        e.push(super::encart(
            "a_faire",
            format!("{} déjà envoyée{}", super::compte(deja, "embauche", "embauches"), if deja > 1 { "s" } else { "" }),
            "Le fichier porte tout le mois : le déposer les redéclare, et l'Urssaf reçoit deux déclarations pour un seul contrat.",
        ));
    }
    let d = &v["deja_depose"];
    if vrai(d) {
        e.push(super::encart(
            "a_faire",
            "Ce fichier a déjà été déposé",
            format!("Le {}, en mode {}. Redéposer le même fichier ne remplace rien : un annule-et-remplace se produit, il ne se redépose pas.", super::quand(&mot(&d["quand"])), if mot(&d["mode"]) == "reel" { "réel".to_string() } else { mot(&d["mode"]) }),
        ));
    }
    if v["reel"].as_bool() == Some(false) {
        e.push(super::encart("information", "Envoi de test", "Le fichier est codé envoi de test : déposé pour de vrai, il ne déclarerait rien et rien ne serait conservé."));
    }
    e
}

/// LES PASTILLES DE CE QUE CES ROUTES RENDENT (`routes::pastille`) : la ligne d'un certificat de conformite DPAE
/// (`enregistree`), enregistree, faite ; deja existante, une information ; refusee, refusee ; une embauche qu'une DPAE
/// va declarer (`deja_declaree`), deja declaree, a voir (elle partirait deux fois) ; a declarer, une information.
fn poser_les_pastilles(v: &mut Value) {
    match v {
        Value::Object(o) => {
            if let Some(enregistree) = o.get("enregistree").and_then(Value::as_bool) {
                let texte = |k: &str| o.get(k).filter(|x| vrai(x)).map(valeur_en_texte);
                let p = if enregistree {
                    crate::routes::pastille("fait", &texte("dossier").map_or_else(|| "enregistrée".to_string(), |d| format!("enregistrée — dossier {}", d)))
                } else if o.get("deja_existante").is_some_and(vrai) {
                    crate::routes::pastille("information", "déjà existante")
                } else {
                    crate::routes::pastille("refuse", &texte("libelle").or_else(|| texte("code")).unwrap_or_else(|| "refusée".into()))
                };
                o.insert("pastille".into(), p);
            }
            if let Some(deja) = o.get("deja_declaree").and_then(Value::as_bool) {
                let p = if deja { crate::routes::pastille("a_faire", "déjà déclarée") } else { crate::routes::pastille("information", "à déclarer") };
                o.insert("pastille".into(), p);
            }
            for x in o.values_mut() {
                poser_les_pastilles(x);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(poser_les_pastilles),
        _ => {}
    }
}

// ------------------------------------------------------------------ outils

#[track_caller]
fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

#[track_caller]
fn plain(m: String) -> Echec {
    socle::echec::noter_et(None, m)
}

/// `charge["cle"]` : un texte requis.
fn texte_requis<'a>(charge: &'a Value, cle: &str) -> R<&'a str> {
    crate::routes::fiches::exiger(charge, cle)?.as_str().ok_or_else(|| erreur("TypeError", format!("`{}` : un texte est attendu", cle)))
}

/// Les secondes unix de l'horloge.
fn maintenant() -> f64 {
    socle::temps::secondes()
}

/// L'heure de Paris, en `AAAA-MM-JJTHH:MM:SS`.
fn maintenant_iso() -> String {
    socle::paris::iso_secondes(maintenant())
}

fn ecrire_texte(chemin: &Path, texte: &str) -> R<()> {
    if let Some(d) = chemin.parent() {
        std::fs::create_dir_all(d).map_err(|e| plain(format!("{} : {}", d.display(), e)))?;
    }
    std::fs::write(chemin, texte).map_err(|e| plain(format!("{} : {}", chemin.display(), e)))
}

// ------------------------------------------------------------------ dpae

/// Les noms `DPAE_<etiquette>_AAAAMMJJ-HHMM.txt` d'un dossier, du plus grand au plus petit
/// (le plus récent d'abord).
fn fichiers_partiels(dossier: &Path, etiq: &str) -> Vec<PathBuf> {
    let debut = format!("DPAE_{}_", etiq);
    let Ok(lecture) = std::fs::read_dir(dossier) else { return Vec::new() };
    let mut out = Vec::new();
    for e in lecture.flatten() {
        let Some(nom) = e.file_name().to_str().map(str::to_string) else { continue };
        let Some(reste) = nom.strip_prefix(&debut) else { continue };
        let Some(reste) = reste.strip_suffix(".txt") else { continue };
        let c: Vec<char> = reste.chars().collect();
        // `AAAAMMJJ-HHMM`, et `AAAAMMJJ-HHMM-N` : le nom que prend un fichier partiel quand un autre porte deja le sien
        // dans la meme minute (`nom_libre_du_partiel`).
        let rang_ok = c.len() == 13 || (c.len() > 14 && c[13] == '-' && c[14..].iter().all(char::is_ascii_digit));
        if c.len() >= 13 && c[8] == '-' && rang_ok {
            out.push(e.path());
        }
    }
    out.sort();
    out.reverse();
    out
}

/// La marque `_AAAAMMJJ-HHMM` d'un fichier partiel, suivie de `-2`, `-3`... tant que `DPAE_<etiq><marque>.txt` existe deja.
pub(crate) fn nom_libre_du_partiel(dossier: &Path, etiq: &str, marque: &str) -> String {
    let mut essai = marque.to_string();
    let mut rang = 2;
    while dossier.join(format!("DPAE_{}{}.txt", etiq, essai)).exists() {
        essai = format!("{}-{}", marque, rang);
        rang += 1;
    }
    essai
}

fn latin1(octets: &[u8]) -> String {
    octets.iter().map(|&o| o as char).collect()
}

/// Le code Urssaf que la DPAE de cette compagnie porte.
fn code_urssaf(ent: &Value) -> R<String> {
    let ecrit = ent.get("code_urssaf").filter(|c| vrai(c)).map(valeur_en_texte).unwrap_or_default();
    let ecrit = ecrit.trim();
    if !ecrit.is_empty() {
        return Ok(ecrit.to_string());
    }
    let territoire = crate::routes::fiches::urssaf_du_code_postal(ent.get("code_postal"))?;
    Ok(if territoire.is_empty() { "117".to_string() } else { territoire })
}

/// `Embauche` de la DPAE, lue d'une paie sans montants.
fn embauche_dpae(p: &Paie, spectacle: bool) -> Embauche {
    Embauche {
        salarie: p.salarie.clone(),
        debut: json!(p.engagement.debut.iso()),
        fin: p.engagement.fin.map_or(Value::Null, |f| json!(f.iso())),
        nature: p.engagement.nature.clone(),
        spectacle: if spectacle && vrai(&p.spectacle) { p.spectacle.clone() } else { Value::Null },
    }
}

/// `{"nom", "chemin", "octets"}` d'un fichier (`sortie_json.fichier`).
fn fichier_json(f: &Path) -> Value {
    let octets = std::fs::metadata(f).ok().map(|m| m.len());
    json!({"nom": f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), "chemin": f.to_string_lossy(), "octets": octets})
}

/// La DPAE, due AVANT le premier jour travaille : tout le mois, ou les engagements choisis (`engagements` : une liste de
/// `{salarie, numero}` : le salarie et le numero du contrat de l'engagement) dans un fichier horodate a cote du complet.
/// La phrase rendue dit combien d'embauches elle declare ; la demande de taux de PAS part avec son depot
/// (`depot_acte::deposer_dpae`).
pub(crate) fn dpae(ctx: &Contexte, charge: &Value) -> R<Dpae> {
    let mois_demande = texte_requis(charge, "mois")?;
    // L'heure d'embauche que la DPAE declare.
    let heure = json!("0800");
    let selection: Option<&Vec<Value>> = match charge.get("engagements") {
        None | Some(Value::Null) => None,
        Some(Value::Array(a)) => Some(a),
        Some(autre) => return Err(erreur("TypeError", socle::fichiers::attendu("une liste", socle::valeur::nom_type(autre)))),
    };

    // Les engagements valides du mois (rien ne part d'un engagement en saisie), vus comme des embauches, sans relire leur
    // paie.
    let journal = envois::journal::lu(&ctx.racine).map_err(plain)?;
    let data = engagement::etats::validees_du_mois(ctx, mois_demande, &journal.departs).map_err(plain)?.map(|(d, _)| d)
        .ok_or_else(|| erreur("FileNotFoundError", format!("Aucune saisie pour {}.", mois_demande)))?;
    let paies: Vec<Paie> = {
        let engs = engagement::engagements_de_la_saisie(ctx, &data).map_err(plain)?;
        engs.iter().map(|e| engagement::embauche(ctx, e)).collect::<Result<_, _>>().map_err(plain)?
    };
    let mois = data.get("mois").map(en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("mois")))?;
    let etiq = socle::date::etiquette_du_mois(&mois).map_err(|e| erreur("ValueError", e))?;
    let sortie = ctx.racine.join("sorties").join(&mois);
    let cle_de = |p: &Paie| -> (String, String) { (p.salarie.get("_cle").map(valeur_en_texte).unwrap_or_default(), p.engagement.numero.clone().unwrap_or_default()) };

    // LES DPAE A FAIRE DU MOIS (`Moteur::embauches_a_declarer`, la seule liste) : une embauche deja partie en DPAE
    // reelle, ou un contrat en cours depuis un mois precedent (sa DPAE part avec son premier mois), n'y est pas.
    let a_faire: BTreeSet<(String, String)> = crate::routes::etapes::Moteur::nouveau(ctx)
        .embauches_a_declarer(Some(&mois))?
        .as_array()
        .into_iter()
        .flatten()
        .map(|e| (valeur_en_texte(&e["salarie"]), valeur_en_texte(&e["numero"])))
        .collect();
    let mut retenues: Vec<&Paie> = paies.iter().filter(|p| a_faire.contains(&cle_de(p))).collect();
    let mut marque = String::new();
    if let Some(sel) = selection {
        let mut voulues: BTreeSet<(String, String)> = BTreeSet::new();
        for x in sel {
            match x {
                Value::Object(_) => {
                    let g = |k: &str| x.get(k).map(en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k)));
                    voulues.insert((g("salarie")?, g("numero")?));
                }
                autre => return Err(erreur("TypeError", socle::fichiers::attendu("une liste", socle::valeur::nom_type(autre)))),
            }
        }
        let joint = |s: BTreeSet<&(String, String)>| s.into_iter().map(|(a, b)| format!("{} (contrat {})", a, b)).collect::<Vec<_>>().join(", ");
        let hors: BTreeSet<&(String, String)> = voulues.difference(&a_faire).collect();
        if !hors.is_empty() {
            let tous = hors.len() == voulues.len();
            let phrase = format!(
                "Aucune DPAE a faire pour : {} (deja partie, en cours depuis un mois precedent, ou absente de la saisie de {}). Rien n'est ecrit.",
                joint(hors),
                mois
            );
            return if tous { Ok(Dpae::Rien(phrase)) } else { Err(refus(phrase)) };
        }
        retenues.retain(|p| voulues.contains(&cle_de(p)));
        let now = maintenant_iso();
        marque = format!("_{}{}{}-{}{}", &now[0..4], &now[5..7], &now[8..10], &now[11..13], &now[14..16]);
    }
    if retenues.is_empty() {
        return Ok(Dpae::Rien(format!("Aucune DPAE à faire en {}. Rien n'est écrit.", engagement::libelle_mois(&mois))));
    }

    // RETOURS WINDOWS, D : DEUX LIGNES IDENTIQUES d'une saisie declareraient deux fois la meme embauche : la DPAE ne se
    // produit pas, le refus dit lesquelles (salarie, jour) et qu'il faut corriger la saisie. Ni le numero de contrat ni
    // la validation ne distinguent deux lignes.
    if let Some(brutes) = data.get("engagements").and_then(Value::as_array).filter(|b| b.len() == paies.len()) {
        let sans_numero = |v: &Value| -> Value {
            match v {
                Value::Object(o) => Value::Object(o.iter().filter(|(k, _)| *k != "numero" && *k != "valide").map(|(k, x)| (k.clone(), x.clone())).collect()),
                autre => autre.clone(),
            }
        };
        let retenue = |i: usize| retenues.iter().any(|r| std::ptr::eq(*r, &paies[i]));
        let mut doubles: Vec<String> = Vec::new();
        for i in 1..paies.len() {
            if retenue(i) && (0..i).any(|j| retenue(j) && sans_numero(&brutes[i]) == sans_numero(&brutes[j])) {
                let (c, d) = (cle_de(&paies[i]).0, paies[i].engagement.debut.iso());
                let texte = format!("{} au {}", c, d);
                if !doubles.contains(&texte) {
                    doubles.push(texte);
                }
            }
        }
        if !doubles.is_empty() {
            return Err(refus(format!(
                "Deux lignes identiques dans la saisie de {} declareraient deux fois la meme embauche : {}. Corrigez la saisie (une seule ligne par embauche). Rien n'est ecrit.",
                mois,
                doubles.join(", ")
            )));
        }
    }

    // LE MEME CONTENU, LE MEME FICHIER : une DPAE deja ecrite qui dit la meme chose (la selection, ou tout ce qui reste a
    // declarer) se reprend telle quelle : le test qu'elle a passe vaut pour le reel (regle 10).
    let mut ent = ctx.entreprise.clone();
    if let Some(o) = ent.as_object_mut() {
        o.insert("_gabarits".into(), Value::String(ctx.gabarits().to_string_lossy().into_owned()));
    }
    let code = code_urssaf(&ctx.entreprise)?;
    let chemins = Chemins {
        defaut: dossier_du_code().join("modeles").join("due_tel_gabarit.txt"),
        poste: ctx.gabarits(),
    };
    let heure_texte = en_texte(&heure);
    let embauches: Vec<Embauche> = retenues.iter().map(|p| embauche_dpae(p, false)).collect();
    let mut f: Option<PathBuf> = None;
    {
        let h = horodatage(&maintenant_iso());
        let voulu = declarations::dpae::contenu(&embauches, &parametres_de(&h, &heure_texte, &code, &ent, &chemins)).map_err(plain)?;
        let voulu = latin1(&voulu);
        let mut pareils = Vec::new();
        let complet = sortie.join(format!("DPAE_{}.txt", etiq));
        let candidats = if marque.is_empty() { vec![complet].into_iter().filter(|c| c.is_file()).collect() } else { fichiers_partiels(&sortie, &etiq) };
        for x in candidats {
            let contenu = std::fs::read(&x).map_err(|e| erreur("OSError", format!("{} : {}", x.display(), e)))?;
            if declarations::dpae::meme_declaration(&latin1(&contenu), &voulu) {
                pareils.push(x);
            }
        }
        let mut repetes = Vec::new();
        for x in &pareils {
            let empreinte = envois::journal::empreinte_fichier(x).map_err(plain)?;
            let nom = x.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let r = etats::repetition(&journal.lignes, etats::DPAE, &empreinte, &nom)?;
            if vrai(&r["conforme"]) {
                repetes.push(x.clone());
            }
        }
        if !pareils.is_empty() {
            let choisi = repetes.first().unwrap_or(&pareils[0]).clone();
            let stem = choisi.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            marque = stem.chars().skip(format!("DPAE_{}", etiq).chars().count()).collect();
            f = Some(choisi);
        }
    }
    // JAMAIS PAR-DESSUS UN AUTRE (deux DPAE jouees dans la meme minute s'ecraseraient) : si le
    // nom a la minute est pris par une declaration DIFFERENTE, le fichier prend `-2`, `-3`... ; le nom ordinaire ne change pas.
    if f.is_none() && !marque.is_empty() {
        marque = nom_libre_du_partiel(&sortie, &etiq, &marque);
    }
    let recap = sortie.join(format!("DPAE_{}{}_recapitulatif.txt", etiq, marque));
    let f = match f {
        Some(f) => f,
        None => {
            let cible = sortie.join(format!("DPAE_{}{}.txt", etiq, marque));
            ecrire_la_dpae(ctx, &retenues, &cible, &recap, &heure, &maintenant_iso())?;
            crate::compagnie::depart::noter_piece_ecrite(&ctx.racine, &crate::compagnie::depart::cle_de_piece(&cible), None, None)?;
            cible
        }
    };

    let fichiers: Vec<Value> = vec![fichier_json(&f), fichier_json(&PathBuf::from(format!("{}.zip", f.to_string_lossy()))), fichier_json(&recap)];
    let n = retenues.len();
    let phrase = format!("DPAE {} écrite : {} embauche{}.", socle::texte::de(&engagement::libelle_mois(&mois)), n, if n > 1 { "s" } else { "" });
    Ok(Dpae::Ecrite(json!({ "fichiers": fichiers, "phrase": phrase })))
}

/// CE QUE `dpae` REND : la DPAE ecrite (ses fichiers, sa phrase), ou rien a declarer de ce que la charge designe (tout
/// est parti, ou rien n'est valide), avec la phrase qui le dit. Un manque (une case de fiche, deux lignes identiques)
/// est un refus, jamais « rien » : celui qui depose doit le lire.
pub(crate) enum Dpae {
    Ecrite(Value),
    Rien(String),
}

/// LES FICHIERS D'UNE DPAE : l'enregistrement de chaque embauche de `retenues` dans `cible` (et son `.zip`), horodate
/// `quand` (`AAAA-MM-JJTHH:MM:SS`, a Paris), l'heure d'embauche `heure` ; et son recapitulatif en clair dans `recap`.
/// Ce que `dpae` ecrit, et ce que `compagnie::depart::piece_partie` refait d'une DPAE partie dont le fichier manque.
pub(crate) fn ecrire_la_dpae(ctx: &Contexte, retenues: &[&Paie], cible: &Path, recap: &Path, heure: &Value, quand: &str) -> R<()> {
    let mut ent = ctx.entreprise.clone();
    if let Some(o) = ent.as_object_mut() {
        o.insert("_gabarits".into(), Value::String(ctx.gabarits().to_string_lossy().into_owned()));
    }
    let code = code_urssaf(&ctx.entreprise)?;
    let chemins = Chemins { defaut: dossier_du_code().join("modeles").join("due_tel_gabarit.txt"), poste: ctx.gabarits() };
    let embauches: Vec<Embauche> = retenues.iter().map(|p| embauche_dpae(p, false)).collect();
    let h = horodatage(quand);
    declarations::dpae::ecrire(&embauches, cible, &parametres_de(&h, &en_texte(heure), &code, &ent, &chemins), true).map_err(plain)?;
    let pour_recap: Vec<Embauche> = retenues.iter().map(|p| embauche_dpae(p, true)).collect();
    ecrire_texte(recap, &declarations::dpae::recapitulatif(&pour_recap, heure).map_err(plain)?)
}

/// Les parametres d'une ecriture de DPAE.
fn parametres_de<'a>(horodatage: &'a str, heure: &str, code: &str, ent: &'a Value, chemins: &'a Chemins) -> Parametres<'a> {
    Parametres { horodatage, heure_embauche: heure.to_string(), code_urssaf: json!(code), ent: Some(ent), chemins }
}

/// `AAAA-MM-JJTHH:MM:SS` -> `JJMMAAAAHHMM`.
fn horodatage(iso: &str) -> String {
    format!("{}{}{}{}{}", &iso[8..10], &iso[5..7], &iso[0..4], &iso[11..13], &iso[14..16])
}

#[cfg(test)]
mod tests_dpae_partielles {
    use super::*;

    fn dossier() -> PathBuf {
        let r = crate::horloge::racine_d_essai(false).join("sorties").join("2026-10");
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    #[test]
    fn deux_dpae_dans_la_meme_minute_ne_s_ecrasent_jamais() {
        // Le nom `DPAE_<mois>_<AAAAMMJJ-HHMM>.txt` est a la minute : la seconde declaration de la minute ecraserait la
        // premiere, et le depot du premier jour ne serait plus relisible tel qu'il a ete depose.
        let d = dossier();
        let marque = "_20261007-1815";
        // Cas ordinaire : le nom ne change pas.
        assert_eq!(nom_libre_du_partiel(&d, "Octobre2026", marque), marque);
        std::fs::write(d.join("DPAE_Octobre2026_20261007-1815.txt"), "premiere").unwrap();
        assert_eq!(nom_libre_du_partiel(&d, "Octobre2026", marque), "_20261007-1815-2");
        std::fs::write(d.join("DPAE_Octobre2026_20261007-1815-2.txt"), "seconde").unwrap();
        assert_eq!(nom_libre_du_partiel(&d, "Octobre2026", marque), "_20261007-1815-3");
        // Une autre minute, ou un autre mois, garde son nom.
        assert_eq!(nom_libre_du_partiel(&d, "Octobre2026", "_20261007-1816"), "_20261007-1816");
        assert_eq!(nom_libre_du_partiel(&d, "Novembre2026", marque), marque);
        // Les fichiers deja ecrits sont intacts.
        assert_eq!(std::fs::read_to_string(d.join("DPAE_Octobre2026_20261007-1815.txt")).unwrap(), "premiere");
    }
}

/// `charge["cle"]` : `KeyError` si elle manque.
fn requis<'a>(charge: &'a Value, cle: &str) -> R<&'a Value> {
    charge.get(cle).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle)))
}

// ------------------------------------------------------------------------ deposer

/// LA DECLARATION QUE LA CHARGE DESIGNE (son chemin), et ce que son ecriture a dit. `{quoi:"dsn", mois, etablissement?}` :
/// la DSN du mois, fabriquee depuis les engagements valides (`fabriquer_dsn`, qui reprend tel quel le fichier qui dit la
/// meme chose : le test vaut pour le reel, regle 10) ; partie en reel, celle qui est partie, sauf si elle est a corriger.
/// `{quoi:"dpae", mois, engagements?}` : la DPAE de ces embauches, ou
/// de celles qui restent a declarer (`dpae`, meme reprise) ; une fois partie, il n'en reste aucune a declarer, et c'est
/// celle qui est partie (`partie`), jamais refaite (regle 4). `{quoi, fichier}` : un signalement ecrit.
pub(crate) fn designee(ctx: &Contexte, charge: &Value) -> R<(Value, Value)> {
    let quoi = requis(charge, "quoi")?;
    // Un signalement se nomme par son fichier, la declaration du mois par son mois : les deux ne designent pas la meme.
    if charge.get("mois").is_some_and(vrai) && charge.get("fichier").is_some_and(vrai) {
        return Err(refus(
            "`fichier` et `mois` désignent deux déclarations : `fichier` seul pour un signalement écrit, `mois` seul pour la \
             déclaration du mois. Rien n'est parti.",
        ));
    }
    let Some(mois) = charge.get("mois").filter(|m| vrai(m)).map(valeur_en_texte) else {
        return Ok((requis(charge, "fichier")?.clone(), json!({})));
    };
    if etats::est(quoi, "dsn") {
        // UNE DSN PARTIE EN REEL NE SE REFAIT QUE SI CE QU'ELLE A EMPORTE EST A CORRIGER (regle 4) : un engagement du
        // mois change depuis, ajoute ou retire (`Moteur::etat_remplacement`, `a_envoyer`) ; sinon, c'est celle qui est
        // partie. Une DSN refusee au bilan n'est pas partie (regle 5).
        let nic = charge.get("etablissement").filter(|n| vrai(n)).cloned().unwrap_or(Value::Null);
        let e = crate::compagnie::etat_dsn::du_mois(ctx, &mois, &nic)?;
        if e.deposee() && !e.refusee() && !vrai(&crate::routes::etapes::Moteur::nouveau(ctx).etat_remplacement(&mois, &nic)?["a_envoyer"]) {
            if let Some(c) = partie(ctx, charge)? {
                return Ok((c, json!({})));
            }
        }
        let dsn = crate::compagnie::depart::fabriquer_dsn(ctx, &mois, "interface")?;
        let chemin = match charge.get("etablissement").filter(|n| vrai(n)).map(valeur_en_texte) {
            Some(n) => dsn.fichiers.iter().find(|f| f.file_name().is_some_and(|x| x.to_string_lossy().contains(&format!("_{}", n)))),
            None => dsn.fichiers.first(),
        };
        let chemin = chemin.ok_or_else(|| refus("Aucune DSN de cet etablissement ce mois-ci. Rien n'est parti."))?;
        return Ok((json!(chemin.to_string_lossy()), dsn.avis()));
    }
    // Ce qui reste a declarer s'ecrit, et un manque refuse ; quand rien ne reste, c'est la DPAE partie.
    match dpae(ctx, charge)? {
        Dpae::Ecrite(w) => Ok((w["fichiers"][0]["chemin"].clone(), w)),
        Dpae::Rien(phrase) => partie(ctx, charge)?.map(|c| (c, json!({}))).ok_or_else(|| refus(phrase)),
    }
}

/// CE QUI EST PARTI POUR LA DESIGNATION `{quoi, mois, etablissement?, engagements?}`, en test ou en reel, lu au journal
/// (`envois::journal::lu`) : le fichier de la derniere ligne du mois, du meme etablissement (DSN mensuelle) ou des memes
/// embauches (DPAE, quand la charge les nomme). Rien ne se fabrique.
fn partie(ctx: &Contexte, charge: &Value) -> R<Option<Value>> {
    let quoi = valeur_en_texte(requis(charge, "quoi")?);
    let mois = valeur_en_texte(requis(charge, "mois")?);
    let lignes = envois::journal::lu(&ctx.racine).map_err(plain)?.lignes;
    let t = |l: &Value, k: &str| l.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let principal = droit::bareme::nic_principal(&ctx.entreprise);
    let nic = charge.get("etablissement").filter(|n| vrai(n)).map(valeur_en_texte).unwrap_or_else(|| principal.clone());
    let paires = |v: &Value| -> BTreeSet<(String, String)> {
        v.as_array().into_iter().flatten().map(|e| (valeur_en_texte(&e["salarie"]), valeur_en_texte(&e["numero"]))).collect()
    };
    let voulues = charge.get("engagements").filter(|e| vrai(e)).map(paires);
    let ligne = lignes.iter().rev().find(|l| {
        t(l, "quoi") == quoi
            && t(l, "mois") == mois
            && if quoi == "dsn" {
                let de_la_ligne = l["detail"].get("etablissement").filter(|n| vrai(n)).map(valeur_en_texte).unwrap_or_else(|| principal.clone());
                t(l, "fichier").starts_with("DSN_") && de_la_ligne == nic
            } else {
                voulues.as_ref().is_none_or(|v| paires(&l["detail"]["embauches"]) == *v)
            }
    });
    Ok(ligne.map(|l| json!(ctx.racine.join("sorties").join(&mois).join(t(l, "fichier")).to_string_lossy())))
}

/// `deposer` : `motdepasse` est celui DU LOGICIEL, et il ne sert qu'a prouver la presence. Il arrive par le
/// corps du POST, il n'est ni journalise ni renvoye. `contournement` : le motif ECRIT d'un depot reel sans
/// repetition. `forcer` : l'annule-et-remplace voulu, apres l'avertissement que la page affiche sur
/// `DsnDejaDeposee` -- jamais par defaut.
fn deposer(ctx: &Contexte, charge: &Value) -> R<Value> {
    let quoi = requis(charge, "quoi")?;
    let (fichier, _) = designee(ctx, charge)?;
    let fichier = &fichier;
    let preuve = Preuve::de_la_charge(charge);
    let contournement = charge.get("contournement").filter(|c| vrai(c)).map(valeur_en_texte).unwrap_or_default();
    let contournement = contournement.trim().to_string();
    let reel = charge.get("reel").is_some_and(vrai);
    let fait = if etats::est(quoi, "dsn") {
        let forcer = charge.get("forcer").is_some_and(vrai);
        depot_acte::deposer_dsn(ctx, fichier, reel, Some(&preuve), "interface", forcer, &contournement)?
    } else {
        depot_acte::deposer_dpae(ctx, fichier, reel, Some(&preuve), "interface", false, &contournement)?
    };
    // LA PHRASE DU DEPOT : ou en est son test, ou ce qu'ont dit les retours du reel s'ils sont deja la (le simulateur repond
    // dans le geste) ; puis ses suites.
    let tranche = if !reel {
        let b = bilan_de_la_repetition(ctx, &json!({"quoi": quoi, "fichier": fichier}))?;
        format!("{} {}", valeur_en_texte(&b["titre"]), valeur_en_texte(&b["phrase"]))
    } else if vrai(&fait.ligne["retours"]) {
        valeur_en_texte(&suivre_depot(ctx, &json!({"quoi": quoi, "flux": fait.flux, "mois": fait.mois}))?["phrase"])
    } else {
        "Dépôt réel effectué — ses retours sont attendus.".to_string()
    };
    let phrase = std::iter::once(tranche).chain(fait.suites.iter().cloned()).collect::<Vec<_>>().join(" ");
    let mut v = fait.en_json();
    v["phrase"] = json!(phrase);
    // APRES LA DSN REELLE D'UN MOIS QUI A DES SALARIES, la boite d'avis : leurs documents partent de l'etape 4.
    if reel && etats::est(quoi, "dsn") && envois::depot::lire(&lire_octets(&resoudre(&ctx.racine, fichier)?)?).salaries > 0 {
        v["avis"] = super::encart("information", "Les documents des salariés", "Ils partent de l'étape 4, « Envoyer les documents », qui dit ce qu'il leur manque encore.");
    }
    Ok(v)
}

// ------------------------------------------------------------------ suivre-depot

/// `suivre_le_depot` : juste apres un depot REEL, relever ses retours et dire s'il est tranche. Une LECTURE.
/// `{"fini", "passe", "refuse", "incident", "phrase", "couleur"}`. Fini quand chaque embauche du depot est passee ou
/// refusee (DPAE), quand le bilan du mois est la (DSN).
fn suivre_depot(ctx: &Contexte, charge: &Value) -> R<Value> {
    let quoi = requis(charge, "quoi")?;
    let flux = charge.get("flux").filter(|f| vrai(f)).map(valeur_en_texte).unwrap_or_default();
    let mois = charge.get("mois").filter(|m| vrai(m)).map(valeur_en_texte).unwrap_or_default();
    let dpae = etats::est(quoi, "dpae");
    let (debut, fin) = releve_retours::derniere_journee();
    let releve = if dpae {
        crate::compagnie::releve::relever_retours_dpae(ctx, debut, fin, false, &flux)
    } else {
        crate::compagnie::releve::relever_retours(ctx, debut, fin, false, "production", None, "")
    };
    let incident = match releve {
        Ok(r) => releve_retours::incidents_en_phrase(&r),
        Err(e) => phrase_de(&e),
    };
    let lignes = envois::journal::lu(&ctx.racine).map(|j| j.lignes)?;
    let (passe, refuse);
    if dpae {
        let ligne = lignes.iter().rev().find(|l| l.get("quoi").and_then(Value::as_str) == Some("dpae") && l.get("flux").and_then(Value::as_str) == Some(flux.as_str()));
        let etat = etats::etat_des_embauches(&lignes)?;
        let mut par_cle: HashMap<(String, String), &Value> = HashMap::new();
        for t in etat.as_array().map(Vec::as_slice).unwrap_or(&[]) {
            if let Some(a) = t.as_array() {
                par_cle.insert((valeur_en_texte(&a[0]), valeur_en_texte(&a[1])), &a[2]);
            }
        }
        let vide = json!({});
        let embauches: Vec<&Value> = ligne
            .and_then(|l| l.get("detail"))
            .and_then(|d| d.get("embauches"))
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|e| par_cle.get(&(valeur_en_texte(e.get("salarie").unwrap_or(&Value::Null)), valeur_en_texte(e.get("numero").unwrap_or(&Value::Null)))).copied().unwrap_or(&vide)).collect())
            .unwrap_or_default();
        passe = !embauches.is_empty() && embauches.iter().all(|x| x.get("passee").is_some_and(vrai));
        refuse = embauches.iter().any(|x| x.get("refusee").is_some_and(vrai));
    } else {
        let ligne = if flux.is_empty() {
            None
        } else {
            lignes.iter().rev().find(|l| l.get("quoi").and_then(Value::as_str) == Some("dsn") && l.get("flux").and_then(Value::as_str) == Some(flux.as_str()))
        };
        let nic = ligne.and_then(|l| l.get("detail")).and_then(|d| d.get("etablissement")).filter(|e| vrai(e)).cloned().unwrap_or(Value::Null);
        let e = crate::compagnie::etat_dsn::du_mois(ctx, &mois, &nic)?;
        passe = e.0["etat"] == "passee";
        refuse = e.refusee();
    }
    // Ce que l'ecran en dit, et en quelle couleur, une fois le depot tranche.
    let nom = if dpae { "La DPAE" } else { "La DSN" };
    let (phrase, etat) = if passe {
        (format!("{} est passée — retour de l'organisme reçu.", nom), "fait")
    } else if refuse {
        (format!("{} est refusée — voir le motif dans l'étape.", nom), "refuse")
    } else {
        (format!("{} attend le retour de l'organisme.", nom), "en_attente")
    };
    Ok(json!({"fini": passe || refuse, "passe": passe, "refuse": refuse, "incident": incident,
              "phrase": phrase, "couleur": crate::routes::couleur_de_l_etat(etat)}))
}

// ------------------------------------------------------------------ bilan-repetition

/// `bilan_de_la_repetition` : ou en est le test de CE fichier -- et, s'il attend son compte rendu, aller le
/// chercher maintenant. Une LECTURE : rien ne part. Rend `journal.repetition`, plus `incident` si le releve n'a
/// pas pu se faire (le test reste ce qu'il est ; on le redira au passage suivant).
fn bilan_repetition(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mut r = bilan_de_la_repetition(ctx, charge)?;
    if etats::etat_du_test(&r) == "en_test" {
        let (debut, fin) = releve_retours::derniere_journee();
        let quoi = requis(charge, "quoi")?;
        let releve = if etats::est(quoi, "dpae") {
            let flux = r["conforme"].get("flux").filter(|f| vrai(f)).map(valeur_en_texte).unwrap_or_default();
            crate::compagnie::releve::relever_retours_dpae(ctx, debut, fin, false, &flux)
        } else {
            crate::compagnie::releve::relever_retours(ctx, debut, fin, false, "production", None, "")
        };
        let incident = match releve {
            Ok(rel) => releve_retours::incidents_en_phrase(&rel),
            Err(e) => phrase_de(&e),
        };
        r = bilan_de_la_repetition(ctx, charge)?;
        r["incident"] = json!(incident);
    }
    Ok(r)
}

/// `charge["cle"]` : `KeyError` si elle manque.
fn cle<'a>(charge: &'a Value, k: &str) -> R<&'a Value> {
    charge.get(k).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k)))
}

/// L'erreur du disque, sous sa classe (`FileNotFoundError`, `PermissionError`, `OSError`).
fn erreur_io(e: &std::io::Error, p: &Path) -> Echec {
    let classe = match (e.kind(), e.raw_os_error()) {
        (std::io::ErrorKind::NotFound, _) => "FileNotFoundError",
        (std::io::ErrorKind::PermissionDenied, _) => "PermissionError",
        (_, Some(20)) => "NotADirectoryError",
        (_, Some(21)) => "IsADirectoryError",
        _ => "OSError",
    };
    erreur(classe, socle::fichiers::panne_fichier(e, p))
}

// ------------------------------------------------------------------ les chemins

/// Un chemin POSIX normalisé : les `//` et les `.` intermédiaires s'en vont, un `/`
/// de tete reste (deux, s'il y en a exactement deux).
pub(crate) fn normaliser(t: &str) -> String {
    let tete = if t.starts_with("//") && !t.starts_with("///") {
        "//"
    } else if t.starts_with('/') {
        "/"
    } else {
        ""
    };
    let morceaux: Vec<&str> = t.split('/').filter(|m| !m.is_empty() && *m != ".").collect();
    let corps = morceaux.join("/");
    if tete.is_empty() && corps.is_empty() {
        return ".".to_string();
    }
    format!("{}{}", tete, corps)
}

/// `PurePosixPath(t).name`.
pub(crate) fn nom_du_chemin(t: &str) -> String {
    let n = normaliser(t);
    if n == "." || n.chars().all(|c| c == '/') {
        return String::new();
    }
    n.rsplit('/').next().unwrap_or("").to_string()
}

fn existe(t: &str) -> bool {
    !t.contains('\0') && Path::new(t).exists()
}

/// Un chemin tape a la main, tel quel
/// s'il existe, sinon sous les donnees, un mois nu designant sa saisie. Rend
/// le texte du chemin, normalisé.
pub(crate) fn resoudre(racine: &Path, chemin: &Value) -> R<String> {
    let vide = match chemin {
        Value::Null => true,
        Value::String(t) => t.chars().all(socle::texte::blanc),
        _ => false,
    };
    if vide {
        return Err(erreur("FileNotFoundError", "Aucun fichier indique."));
    }
    let Value::String(t) = chemin else {
        return Err(erreur("TypeError", socle::fichiers::attendu("un chemin en texte", nom_type(chemin))));
    };
    let p = normaliser(t);
    if existe(&p) || p.starts_with('/') {
        return Ok(p);
    }
    let base = normaliser(&racine.display().to_string());
    let candidat = normaliser(&format!("{}/{}", base, p));
    if existe(&candidat) {
        return Ok(candidat);
    }
    // Un mois nu (`\d{4}-\d{2}`) designe sa saisie.
    let c: Vec<char> = t.chars().collect();
    if c.len() == 7
        && c[..4].iter().all(|x| envois::retours::est_chiffre(*x))
        && c[4] == '-'
        && c[5..].iter().all(|x| envois::retours::est_chiffre(*x))
    {
        let saisie = normaliser(&format!("{}/{}/{}.json", base, engagement::stock::DOSSIER, t));
        if existe(&saisie) {
            return Ok(saisie);
        }
    }
    Ok(p)
}

/// `read_bytes`, l'erreur du disque sous sa classe.
fn empreinte_du_fichier(chemin: &str) -> R<String> {
    if chemin.contains('\0') {
        return Err(erreur("ValueError", "embedded null byte"));
    }
    let p = Path::new(chemin);
    let octets = std::fs::read(p).map_err(|e| erreur_io(&e, p))?;
    Ok(envois::journal::empreinte(&octets))
}

// ------------------------------------------------------------------ depot-apercu, la DPAE

/// Le contenu d'un fichier, en octets.
pub(crate) fn lire_octets(chemin: &str) -> R<Vec<u8>> {
    if chemin.contains('\0') {
        return Err(erreur("ValueError", "embedded null byte"));
    }
    let p = Path::new(chemin);
    std::fs::read(p).map_err(|e| erreur_io(&e, p))
}

/// Un chiffre du latin-1 : les chiffres, et les exposants `²`, `³`, `¹`.
fn chiffre_du_latin1(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '\u{b2}' | '\u{b3}' | '\u{b9}')
}

/// Un jour JJ/MM/AAAA en ISO.
fn iso_depuis_jjmmaaaa(v: &str) -> String {
    let t: Vec<char> = v.trim_matches(socle::texte::blanc).chars().collect();
    if t.len() != 8 || !t.iter().all(|c| chiffre_du_latin1(*c)) {
        return String::new();
    }
    let s = |a: usize, b: usize| t[a..b].iter().collect::<String>();
    format!("{}-{}-{}", s(4, 8), s(2, 4), s(0, 2))
}

/// `apercu_depot_dpae` : ce que le fichier DPAE declare, enregistrement par enregistrement.
pub(crate) fn apercu_depot_dpae(ctx: &Contexte, fichier: &Value) -> R<Value> {
    let chemin = resoudre(&ctx.racine, fichier)?;
    let contenu = lire_octets(&chemin)?;
    let texte: String = contenu.iter().map(|&b| b as char).collect();
    let enrs = declarations::dpae::relire(&texte);
    let m = crate::routes::etapes::Moteur::nouveau(ctx);
    let lignes_journal = m.lignes()?;
    let faites = etats::embauches_par_depot(&lignes_journal)?;
    let mut par_nir: HashMap<String, (&String, &Value)> = HashMap::new();
    for (cle, s) in &ctx.salaries {
        let nir = etats::get(s, "nir")?;
        if !vrai(nir) {
            continue;
        }
        match nir {
            Value::Array(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("list"))),
            Value::Object(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("dict"))),
            Value::String(t) => {
                par_nir.insert(t.clone(), (cle, s));
            }
            _ => {}
        }
    }
    // LE NUMERO DU CONTRAT DE CHAQUE EMBAUCHE : celui de l'engagement valide du salarie qui commence ce jour-la. C'est lui
    // qui relie la DPAE, partie ou refusee, a son engagement.
    let departs = m.departs()?;
    let mut numeros: HashMap<(String, String), String> = HashMap::new();
    // La nature de chaque embauche, par son nom court (« CDDU ») : le fichier ne dit que CDD, CDI ou CTT (Format 120).
    let mut natures: HashMap<(String, String), String> = HashMap::new();
    let mut mois_vus: Vec<String> = Vec::new();
    let vide = json!({});
    let mut lignes = Vec::new();
    for e in &enrs {
        let champ = |k: &str| e.iter().find(|(n, _)| *n == k).map(|(_, v)| v.as_str()).unwrap_or("");
        let m_e: String = iso_depuis_jjmmaaaa(champ("date_embauche")).chars().take(7).collect();
        if m_e.len() == 7 && !mois_vus.contains(&m_e) {
            for etat in engagement::etats::tels_qu_ecrits(ctx, &m_e, &departs).map_err(Echec::from)? {
                let Some(v) = etat.valide() else { continue };
                let brut = &*v.objet().brut;
                let Some(cle) = engagement::cle_de_l_engagement(ctx, &brut) else { continue };
                let id = (cle, engagement::debut_brut(&ctx.racine, &brut));
                if let Ok(nat) = droit::natures::nature(brut.get("nature").and_then(Value::as_str)) {
                    natures.insert(id.clone(), nat.libelle.clone());
                }
                if let Some(n) = brut.get("numero").and_then(Value::as_str) {
                    numeros.insert(id, n.to_string());
                }
            }
            mois_vus.push(m_e);
        }
    }
    for e in &enrs {
        let champ = |k: &str| e.iter().find(|(n, _)| *n == k).map(|(_, v)| v.as_str()).unwrap_or("");
        // Sans NIR, la DPAE declare un numero temporaire (zone a blanc) : l'embauche se reconnait a l'identite qu'elle
        // porte, nom, prenom et date de naissance (`dpae::enregistrement`).
        let identite = |s: &Value| {
            let t = |k: &str| s.get(k).map(socle::date::en_texte).unwrap_or_default().trim().to_uppercase();
            let n = s.get("date_naissance").and_then(Value::as_str).and_then(|d| socle::date::Jour::lire(d).ok()).map(|j| j.jjmmaaaa());
            t("nom") == champ("nom").trim() && t("prenom") == champ("prenom").trim() && n.as_deref() == Some(champ("date_naissance"))
        };
        let trouve = par_nir.get(champ("nir").trim()).copied().or_else(|| {
            if !champ("nir").trim().is_empty() {
                return None;
            }
            ctx.salaries.iter().find(|(_, s)| identite(s))
        });
        let (cle, s) = match trouve {
            Some((c, s)) => (Some(c), s),
            None => (None, &vide),
        };
        let debut = iso_depuis_jjmmaaaa(champ("date_embauche"));
        let nom = socle::texte::salarie_nomme(s).unwrap_or_else(|| "salarie inconnu de la configuration".to_string());
        let nom = nom.as_str();
        let type_brut = champ("type_contrat").trim_matches(socle::texte::blanc);
        let type_contrat = cle.and_then(|c| natures.get(&(c.clone(), debut.clone()))).cloned().unwrap_or_else(|| {
            let t = droit::natures::TYPES_CONTRAT_DPAE.iter().find(|(k, _)| *k == type_brut);
            t.map_or_else(|| format!("type {}", type_brut), |(_, l)| l.to_string())
        });
        let cle_json = cle.map_or(Value::Null, |c| json!(c));
        let numero = cle.and_then(|c| numeros.get(&(c.clone(), debut.clone()))).cloned();
        let deja = match &numero {
            Some(n) => faites.contient(&[cle_json, json!(n)])?,
            None => false,
        };
        lignes.push(json!({
            "salarie": cle.map_or("?", |c| c.as_str()),
            "nom": nom,
            "debut": debut,
            "numero": numero,
            "fin": iso_depuis_jjmmaaaa(champ("date_fin")),
            "type_contrat": type_contrat,
            "deja_declaree": deja,
        }));
    }
    let debuts: Vec<String> = lignes.iter().map(|l| l["debut"].as_str().unwrap_or("").to_string()).collect();
    let mois: std::collections::BTreeSet<String> =
        debuts.iter().filter(|d| !d.is_empty()).map(|d| d.chars().take(7).collect()).collect();
    let mois: Vec<String> = mois.into_iter().collect();
    let empreinte = envois::journal::empreinte(&contenu);
    let nom_fichier = nom_du_chemin(&chemin);
    let deja_depose = etats::deja_depose(&lignes_journal, "dpae", &empreinte)?;
    let repetition = etats::repetition(&lignes_journal, "dpae", &empreinte, &nom_fichier)?;
    Ok(json!({
        "description": format!("{} — {} enregistrement{}, {} octets, indicateur {}.", nom_fichier, enrs.len(), if enrs.len() > 1 { "s" } else { "" }, contenu.len(), envois::depot::indicateur(&contenu)),
        "fichier": chemin, "nom": nom_fichier,
        "enregistrements": enrs.len(), "octets": contenu.len(),
        "reel": envois::depot::est_reel(&contenu),
        "indicateur": envois::depot::indicateur(&contenu),
        "mois": mois.first().cloned().unwrap_or_default(),
        "embauches": lignes,
        "deja_depose": deja_depose,
        "repetition": repetition,
    }))
}

fn depot_apercu(ctx: &Contexte, charge: &Value) -> R<Value> {
    let quoi = cle(charge, "quoi")?;
    let (fichier, dit) = designee(ctx, charge)?;
    let mut a = if etats::est(quoi, "dsn") { crate::apercu_dsn::apercu_depot_dsn(ctx, &fichier)? } else { apercu_depot_dpae(ctx, &fichier)? };
    for k in ["phrase", "avertissements", "informations"] {
        if let Some(v) = dit.get(k) {
            a[k] = v.clone();
        }
    }
    // Ce que deux engagements disent pareil se dit une fois.
    for k in ["avertissements", "informations"] {
        if let Some(l) = a.get_mut(k).and_then(Value::as_array_mut) {
            let mut vus = Vec::new();
            l.retain(|x| if vus.contains(x) { false } else { vus.push(x.clone()); true });
        }
    }
    Ok(a)
}

/// Ou en est le test de ce qui est parti pour la designation (`partie`), ou du signalement nomme.
fn bilan_de_la_repetition(ctx: &Contexte, charge: &Value) -> R<Value> {
    let quoi = cle(charge, "quoi")?;
    let fichier = match charge.get("mois").filter(|m| vrai(m)) {
        Some(m) => partie(ctx, charge)?.ok_or_else(|| refus(format!("Rien n'est parti pour {} : le déposer d'abord en test.", valeur_en_texte(m))))?,
        None => cle(charge, "fichier")?.clone(),
    };
    let chemin = resoudre(&ctx.racine, &fichier)?;
    let empreinte = empreinte_du_fichier(&chemin)?;
    let nom = nom_du_chemin(&chemin);
    let m = crate::routes::etapes::Moteur::nouveau(ctx);
    let lignes = m.lignes()?;
    let Some(quoi) = quoi.as_str() else {
        return Err(Echec { classe: None, message: "demande illisible : il faut `quoi`".to_string() });
    };
    let mut r = etats::repetition(&lignes, quoi, &empreinte, &nom)?;
    r["incident"] = json!("");
    Ok(r)
}
