

//! Les routes qui DEPOSENT et SUIVENT :
//!
//! - `deposer` : une DSN ou une DPAE, en test ou en reel (`depot_acte`) -- la preuve du mot de passe du logiciel,
//!   la repetition exigee avant le reel, le journal, les incidents, la trace sur les arrets, le desarmement du
//!   filet ; la compagnie fictive joue le depot. La DSN du mois se nomme par son mois : elle se fabrique alors depuis
//!   l'engagement valide (`depart::fabriquer_dsn`) ;
//! - `dsn-a-deposer` : la DSN du mois, fabriquee pour etre deposee (`depart::fabriquer_dsn`) ;
//! - `suivre-depot` : juste apres un depot REEL, relever ses retours et dire s'il est tranche ;
//! - `bilan-repetition` : ou en est le test de CE fichier, et -- s'il attend son compte rendu -- aller le
//!   chercher maintenant (`releve_retours`) ;
//! - `filet-preparer` : un passage du filet DPAE (tester une DPAE par jour d'embauche, armer, desarmer).
//!
//! Tout garde-fou vit ICI, dans le moteur : la page n'en decide aucun. Les fils d'arriere-plan (le filet apres
//! `dsn-a-deposer` et `demander-signature`) restent des lanceurs, au branchement : le moteur n'a pas de fil.

use std::collections::HashMap;

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use crate::depot_acte::{self, phrase_de, Preuve};
use crate::etats;
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::vrai;
use crate::releve_retours;
use socle::echec::Echec;

/// Les routes que ce module sert.
pub const ROUTES: [&str; 5] = ["deposer", "suivre-depot", "bilan-repetition", "filet-preparer", "dsn-a-deposer"];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "deposer" => deposer(ctx, charge),
        "suivre-depot" => suivre_depot(ctx, charge),
        "bilan-repetition" => bilan_repetition(ctx, charge),
        "filet-preparer" => filet_preparer(ctx, charge),
        "dsn-a-deposer" => crate::depart::dsn_a_deposer(ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

/// `charge["cle"]` : `KeyError` si elle manque.
fn requis<'a>(charge: &'a Value, cle: &str) -> R<&'a Value> {
    charge.get(cle).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle)))
}

// ------------------------------------------------------------------------ deposer

/// `deposer` : `motdepasse` est celui DU LOGICIEL, et il ne sert qu'a prouver la presence. Il arrive par le
/// corps du POST, il n'est ni journalise ni renvoye. `contournement` : le motif ECRIT d'un depot reel sans
/// repetition. `forcer` : l'annule-et-remplace voulu, apres l'avertissement que la page affiche sur
/// `DsnDejaDeposee` -- jamais par defaut.
fn deposer(ctx: &Contexte, charge: &Value) -> R<Value> {
    let quoi = requis(charge, "quoi")?;
    // LA DSN DU MOIS SE NOMME PAR SON MOIS : elle se fabrique ici, depuis l'engagement valide, et c'est elle qui part
    // (celle de l'etablissement `etablissement`, le principal sinon).
    let fabriquee;
    let fichier = match (charge.get("fichier").filter(|f| vrai(f)), charge.get("mois").filter(|m| vrai(m))) {
        (None, Some(mois)) if etats::est(quoi, "dsn") => {
            let dsn = crate::depart::fabriquer_dsn(ctx, &valeur_en_texte(mois), "interface")?;
            let nic = charge.get("etablissement").filter(|n| vrai(n)).map(valeur_en_texte);
            let chemin = match nic {
                Some(n) => dsn.fichiers.iter().find(|f| f.file_name().is_some_and(|x| x.to_string_lossy().contains(&format!("_{}", n)))),
                None => dsn.fichiers.first(),
            };
            fabriquee = json!(chemin.ok_or_else(|| erreur("ErreurOperation", "Aucune DSN de cet etablissement ce mois-ci. Rien n'est parti."))?.to_string_lossy());
            &fabriquee
        }
        _ => requis(charge, "fichier")?,
    };
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
    Ok(fait.en_json())
}

// ------------------------------------------------------------------ suivre-depot

/// `suivre_le_depot` : juste apres un depot REEL, relever ses retours et dire s'il est tranche. Une LECTURE.
/// `{"fini", "passe", "refuse", "incident"}`. Fini quand chaque embauche du depot est passee ou refusee (DPAE),
/// quand le bilan du mois est la (DSN).
fn suivre_depot(ctx: &Contexte, charge: &Value) -> R<Value> {
    let quoi = requis(charge, "quoi")?;
    let flux = charge.get("flux").filter(|f| vrai(f)).map(valeur_en_texte).unwrap_or_default();
    let mois = charge.get("mois").filter(|m| vrai(m)).map(valeur_en_texte).unwrap_or_default();
    let dpae = etats::est(quoi, "dpae");
    let (debut, fin) = releve_retours::derniere_journee();
    let releve = if dpae {
        releve_retours::relever_retours_dpae(ctx, debut, fin, false, &flux)
    } else {
        releve_retours::relever_retours(ctx, debut, fin, false, "production", None, "")
    };
    let incident = match releve {
        Ok(r) => releve_retours::incidents_en_phrase(&r),
        Err(e) => phrase_de(&e),
    };
    let lignes = etats::charger(&ctx.racine)?;
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
            .map(|a| a.iter().map(|e| par_cle.get(&(valeur_en_texte(e.get("salarie").unwrap_or(&Value::Null)), valeur_en_texte(e.get("debut").unwrap_or(&Value::Null)))).copied().unwrap_or(&vide)).collect())
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
        let e = etats::etat_dsn_du_mois(&lignes, &mois, &nic, &etats::nic_principal(&ctx.entreprise))?;
        passe = e.get("passee").is_some_and(vrai);
        refuse = e.get("refusee").is_some_and(vrai);
    }
    Ok(json!({"fini": passe || refuse, "passe": passe, "refuse": refuse, "incident": incident}))
}

// ------------------------------------------------------------------ bilan-repetition

/// `bilan_de_la_repetition` : ou en est le test de CE fichier -- et, s'il attend son compte rendu, aller le
/// chercher maintenant. Une LECTURE : rien ne part. Rend `journal.repetition`, plus `incident` si le releve n'a
/// pas pu se faire (le test reste ce qu'il est ; on le redira au passage suivant).
fn bilan_repetition(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mut r = crate::routes_apercus::bilan_repetition(ctx, charge)?;
    if vrai(&r["conforme"]) && etats::est(&r["compte_rendu"], "attendu") {
        let (debut, fin) = releve_retours::derniere_journee();
        let quoi = requis(charge, "quoi")?;
        let releve = if etats::est(quoi, "dpae") {
            let flux = r["conforme"].get("flux").filter(|f| vrai(f)).map(valeur_en_texte).unwrap_or_default();
            releve_retours::relever_retours_dpae(ctx, debut, fin, false, &flux)
        } else {
            releve_retours::relever_retours(ctx, debut, fin, false, "production", None, "")
        };
        let incident = match releve {
            Ok(rel) => releve_retours::incidents_en_phrase(&rel),
            Err(e) => phrase_de(&e),
        };
        r = crate::routes_apercus::bilan_repetition(ctx, charge)?;
        r["incident"] = json!(incident);
    }
    Ok(r)
}

// ------------------------------------------------------------------ filet-preparer

/// Un test automatique de DPAE se retente au plus tot six heures apres un echec.
const REESSAI_SECONDES: i64 = 6 * 3600;

fn cle_du_test(salarie: &str, debut: &str) -> String {
    format!("{}|{}", salarie, debut)
}

/// `operations_filet.preparer` : un passage -- tester ce qui doit l'etre, armer, desarmer. Rend `{"testees",
/// "armes", "desarmes", "incidents", "occupe"}`. Ne leve jamais pour une panne : elle devient un incident, et le
/// passage suivant reessaie. `tester` (vrai par defaut) ; `qui` (`filet` par defaut) ; `maintenant`
/// (`AAAA-MM-JJTHH:MM`, que l'appelant peut fixer).
pub(crate) fn filet_preparer(ctx: &Contexte, charge: &Value) -> R<Value> {
    filet_preparer_avec(ctx, charge, None)
}

/// `filet_preparer` avec, au besoin, le client Urssaf que l'appelant donne pour le test automatique des DPAE (la preuve de
/// l'ouverture, lot I2b : un client pointe sur un serveur local). Un client donne n'est jamais confronte au coffre.
pub(crate) fn filet_preparer_avec(ctx: &Contexte, charge: &Value, mut client: Option<&mut crate::releve_retours::Client>) -> R<Value> {
    let tester = charge.get("tester").map_or(true, vrai);
    let qui = charge.get("qui").filter(|q| vrai(q)).map(valeur_en_texte).unwrap_or_else(|| "filet".into());
    let maintenant_iso = match charge.get("maintenant").filter(|m| vrai(m)).map(valeur_en_texte) {
        Some(m) => m,
        None => socle::paris::iso_de_naif(socle::paris::maintenant_naif())[..16].to_string(),
    };
    let mut bilan_testees: Vec<Value> = Vec::new();
    let mut bilan_incidents: Vec<Value> = Vec::new();
    let suivies = crate::filet::suivies(ctx)?;
    let mut demande = Map::new();
    demande.insert("maintenant".into(), json!(maintenant_iso));
    if tester {
        let etat = crate::filet::lire_etat(&ctx.racine)?;
        if let Some(tests) = tester_les_dpae(ctx, &etat, &suivies, &maintenant_iso, &qui, &mut bilan_testees, &mut bilan_incidents, client.as_deref_mut())? {
            demande.insert("tests".into(), tests);
            demande.insert(
                "vivantes".into(),
                Value::Array(suivies.iter().map(|e| json!(cle_du_test(&valeur_en_texte(&e["salarie"]), &valeur_en_texte(&e["debut"])))).collect()),
            );
        }
    }
    let champ = |k: &str| demande.get(k).cloned().unwrap_or(Value::Null);
    match crate::filet::passage(ctx, &champ("tests"), &champ("vivantes"), &champ("maintenant")) {
        Ok(passage) => {
            if let Some(i) = passage.get("incidents").and_then(Value::as_array) {
                bilan_incidents.extend(i.iter().cloned());
            }
            Ok(json!({
                "testees": bilan_testees,
                "armes": passage.get("armes").cloned().unwrap_or_else(|| json!([])),
                "desarmes": passage.get("desarmes").cloned().unwrap_or_else(|| json!([])),
                "incidents": bilan_incidents,
                "occupe": false,
            }))
        }
        Err(e) if e.message.contains("autre passage") => {
            Ok(json!({"testees": bilan_testees, "armes": [], "desarmes": [], "incidents": bilan_incidents, "occupe": true}))
        }
        Err(e) => Err(e),
    }
}

/// `_tester` : LE TEST AUTOMATIQUE -- une DPAE par jour d'embauche, ecrite puis deposee en TEST. Le fichier est
/// celui que le depot reel reprendra : `dpae` rend le meme fichier pour la meme selection, et le reel exige
/// justement la repetition de CE contenu. Rend les tests faits (cle -> entree), ou `None` quand rien ne se
/// teste sur ce poste.
#[allow(clippy::too_many_arguments)]
fn tester_les_dpae(
    ctx: &Contexte,
    etat: &Value,
    suivies: &[Value],
    maintenant_iso: &str,
    qui: &str,
    testees: &mut Vec<Value>,
    incidents: &mut Vec<Value>,
    mut client: Option<&mut crate::releve_retours::Client>,
) -> R<Option<Value>> {
    // La copie de developpement ne teste pas d'elle-meme ; sans le mot de passe Urssaf, rien ne part. LA
    // COMPAGNIE FICTIVE, elle, teste (27/09/2026) : son depot est joue.
    // Un client donne (la preuve) tient lieu du mot de passe : le coffre n'est pas confronte.
    if !crate::routes_accueil::simulation_active(&ctx.racine) {
        let copie = ctx.racine.join("COPIE-DE-DEVELOPPEMENT.json").is_file();
        let dispo = client.is_some() || crate::coffre::Coffre::du_poste(&ctx.racine, "").disponible("urssaf").map_err(crate::routes_etats::vers_route)?;
        if copie || !dispo {
            return Ok(None);
        }
    }
    let aujourdhui = ctx.aujourdhui.iso();
    let maintenant = socle::paris::naif_de_iso(maintenant_iso).unwrap_or(0);
    // (mois, debut) -> les embauches de ce jour, dans l'ordre ; `sorted` apres.
    let mut a_tester: Vec<((String, String), Vec<&Value>)> = Vec::new();
    let deja = etat.get("tests").cloned().unwrap_or_else(|| json!({}));
    for e in suivies {
        let (salarie, debut) = (valeur_en_texte(&e["salarie"]), valeur_en_texte(&e["debut"]));
        if !vrai(&e["contrat_parti"]) || debut < aujourdhui {
            continue;
        }
        if let Some(t) = deja.get(cle_du_test(&salarie, &debut)).filter(|t| vrai(t)) {
            if t.get("empreinte") == e.get("empreinte") {
                if !t.get("erreur").is_some_and(vrai) {
                    continue; // teste, et rien n'a change
                }
                let essai = t.get("essai_le").filter(|x| vrai(x)).map(valeur_en_texte).and_then(|x| socle::paris::naif_de_iso(&x)).unwrap_or_else(|| socle::paris::naif_de_iso("2000-01-01T00:00").unwrap_or(0));
                if maintenant - essai < REESSAI_SECONDES {
                    continue;
                }
            }
        }
        let k = (valeur_en_texte(&e["mois"]), debut);
        match a_tester.iter_mut().find(|(c, _)| *c == k) {
            Some((_, g)) => g.push(e),
            None => a_tester.push((k, vec![e])),
        }
    }
    a_tester.sort_by(|a, b| a.0.cmp(&b.0));

    let mut tests = Map::new();
    for ((mois, debut), groupe) in a_tester {
        let selection: Vec<Value> = groupe.iter().map(|e| json!({"salarie": e["salarie"], "debut": e["debut"]})).collect();
        let mut fichier: Option<String> = None;
        let mut erreur_texte = String::new();
        let mut essai = |fichier: &mut Option<String>| -> R<()> {
            let r = crate::routes_actes_h1::dpae(ctx, &json!({"mois": mois, "paies": selection}))?;
            let f = r["fichiers"][0]["chemin"].as_str().unwrap_or("").to_string();
            // Le fichier est note des qu'il est ecrit : un depot qui echoue garde sa trace (`fichier = r.fichiers[0]`).
            *fichier = Some(f.clone());
            let p = std::path::Path::new(&f);
            let octets = std::fs::read(p).map_err(|x| erreur("OSError", format!("{} : {}", p.display(), x)))?;
            let nom = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let lignes = etats::charger(&ctx.racine)?;
            let deja = etats::repetition(&lignes, "dpae", &crate::journal::empreinte(&octets), &nom)?;
            if !vrai(&deja["conforme"]) {
                depot_acte::deposer_dpae_avec(ctx, &json!(f), false, None, qui, false, "", client.as_deref_mut())?;
            }
            Ok(())
        };
        match essai(&mut fichier) {
            Ok(()) => {}
            // Partie entre-temps : rien a tester.
            Err(e) if e.message.starts_with("Deja declaree(s)") => continue,
            Err(e) => {
                let phrase = phrase_de(&e);
                let premiere = phrase.lines().next().unwrap_or("").chars().take(300).collect::<String>();
                erreur_texte = if phrase.is_empty() { e.classe.unwrap_or("Exception").to_string() } else { premiere };
                incidents.push(json!(format!("Test automatique de la DPAE du {} : {}", jj_mm_aaaa(&debut), erreur_texte)));
            }
        }
        for e in &groupe {
            tests.insert(
                cle_du_test(&valeur_en_texte(&e["salarie"]), &valeur_en_texte(&e["debut"])),
                json!({
                    "empreinte": e["empreinte"], "mois": mois, "fichier": fichier.clone().unwrap_or_default(),
                    "essai_le": maintenant_iso, "erreur": erreur_texte,
                }),
            );
        }
        if erreur_texte.is_empty() {
            testees.extend(selection.iter().cloned());
        }
    }
    Ok(Some(Value::Object(tests)))
}

/// `util.jj_mm_aaaa` d'un `AAAA-MM-JJ`.
fn jj_mm_aaaa(iso: &str) -> String {
    let c: Vec<char> = iso.chars().collect();
    if c.len() >= 10 {
        format!("{}/{}/{}", c[8..10].iter().collect::<String>(), c[5..7].iter().collect::<String>(), c[..4].iter().collect::<String>())
    } else {
        iso.to_string()
    }
}
