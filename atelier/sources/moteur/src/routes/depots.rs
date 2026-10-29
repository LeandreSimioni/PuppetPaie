//! Les routes des declarations : produire la DPAE (`dpae`), montrer et deposer la DSN du mois (`depot-apercu`,
//! `dsn-a-deposer`, `deposer`), suivre un depot (`suivre-depot`), le bilan d'une repetition (`bilan-repetition`), et
//! preparer le filet DPAE (`filet-preparer`).
//!
//! Les formats sont ailleurs (`dpae`, la DSN de `declarations`, la fabrique de PDF) : ce module enchaine -- charger,
//! choisir, refuser, nommer, ecrire, et passer a `envois` ce qui part. Le jour est celui du contexte
//! (`PUPPETPAIE_AUJOURDHUI`) ; l'horodatage d'un fichier partiel de DPAE est la minute de l'horloge, a l'heure de Paris.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::compagnie::depot_acte::{self, phrase_de, Preuve};
use declarations::dpae::{Chemins, Embauche, Parametres};
use droit::contexte::Contexte;
use engagement::calcul::Paie;
use envois::etats;
use envois::releve_retours;
use socle::date::{en_texte, repr_texte, Jour};
use socle::echec::{erreur, Echec, R};
use socle::fichiers::{dossier_du_code, vrai};
use socle::valeur::{nom_type, valeur_en_texte};



/// Les routes de ce module.
pub const ROUTES: &[&str] = &["dpae", "depot-apercu", "deposer", "suivre-depot", "bilan-repetition", "filet-preparer", "dsn-a-deposer"];

/// Celles qui ecrivent.
pub const ECRITURES: &[&str] = &["dpae", "deposer", "suivre-depot", "bilan-repetition", "filet-preparer", "dsn-a-deposer"];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "dpae" => dpae(ctx, charge),
        "depot-apercu" => depot_apercu(ctx, charge),
        "deposer" => deposer(ctx, charge),
        "suivre-depot" => suivre_depot(ctx, charge),
        "bilan-repetition" => bilan_repetition(ctx, charge),
        "filet-preparer" => filet_preparer(ctx, charge),
        "dsn-a-deposer" => crate::compagnie::depart::dsn_a_deposer(ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
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

/// `datetime.now()` a l'heure de Paris, en `AAAA-MM-JJTHH:MM:SS`.
fn maintenant_iso() -> String {
    socle::paris::iso_secondes(maintenant())
}

fn mois_en_toutes_lettres(mois: &str) -> R<(String, String)> {
    let an = mois.get(..4).ok_or_else(|| erreur("ValueError", "mois illisible"))?;
    let rang: usize = mois
        .get(5..7)
        .and_then(|m| m.parse().ok())
        .ok_or_else(|| erreur("ValueError", format!("nombre entier illisible : {}", repr_texte(mois.get(5..7).unwrap_or("")))))?;
    let nom = socle::date::nom_du_mois(rang as i64, socle::date::Graphie::Titre).map_err(|e| erreur("IndexError", e))?;
    let annee: i64 = an.parse().map_err(|_| erreur("ValueError", format!("nombre entier illisible : {}", repr_texte(an))))?;
    Ok((format!("{}{}", nom, annee), format!("{} {}", nom, annee)))
}






fn ecrire_texte(chemin: &Path, texte: &str) -> R<()> {
    if let Some(d) = chemin.parent() {
        std::fs::create_dir_all(d).map_err(|e| plain(format!("{} : {}", d.display(), e)))?;
    }
    std::fs::write(chemin, texte).map_err(|e| plain(format!("{} : {}", chemin.display(), e)))
}

// ------------------------------------------------------------------ dpae

/// La phrase qu'on lit en face d'une embauche deja declaree (`libelle_embauche_declaree`).
fn libelle_embauche_declaree(cle: &str, debut: &str, e: &Value) -> R<String> {
    let quand = {
        let t = e.get("quand").filter(|q| vrai(q)).map(valeur_en_texte).unwrap_or_default();
        let c: Vec<char> = t.chars().collect();
        if c.len() < 10 || c[4] != '-' || c[7] != '-' {
            t
        } else {
            let tr = |a: usize, b: usize| c[a..b].iter().collect::<String>();
            let jour = format!("{}/{}/{}", tr(8, 10), tr(5, 7), tr(0, 4));
            if c.len() >= 16 {
                format!("{} a {}", jour, tr(11, 16))
            } else {
                jour
            }
        }
    };
    let debut_lisible = documents::bulletin::jj_mm_aaaa(&json!(debut)).map_err(|m| erreur("ValueError", m))?;
    if e.get("refusee").map_or(false, vrai) {
        let mut vus: Vec<String> = Vec::new();
        for a in e.get("anomalies").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
            let t = valeur_en_texte(a);
            if !vus.contains(&t) {
                vus.push(t);
            }
        }
        let motif = if vus.is_empty() { "motif non lu".to_string() } else { vus.join(" ; ") };
        return Ok(format!("{} au {} : REFUSEE le {} -- {} -- a corriger et a renvoyer", cle, debut_lisible, quand, motif));
    }
    let suite = if e.get("passee").map_or(false, vrai) {
        "certificat de conformite recu : la DPAE est passee, rien a faire de notre cote"
    } else {
        "retour pas encore arrive : on attend, on ne renvoie pas"
    };
    Ok(format!("{} au {} : declaree le {} -- {}", cle, debut_lisible, quand, suite))
}

/// Les noms `DPAE_<etiquette>_AAAAMMJJ-HHMM.txt` d'un dossier, du plus grand au plus petit
/// (`sorted(sortie.glob("DPAE_%s_????????-????.txt" % etiq), reverse=True)`).
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

/// Ce qu'une production d'amorcage a laisse : les fichiers ecrits.
struct Amorcage {
    fichiers: Vec<PathBuf>,
    /// Qui est demande : (cle du salarie, ce que la situation de son taux dit).
    lignes: Vec<(String, String)>,
}

/// Un fichier de signalement par individu dont le taux manque (le travail est
/// celui de `amorcage_demande`, que `moteur pas --demander` generalise). L'echec (quel qu'il soit) est celui que la DPAE ecarte.
fn produire_amorcage(ctx: &Contexte, mois: &str, retenues: &[Paie], constitution: Jour, ecrits: &mut Vec<PathBuf>) -> Result<Amorcage, String> {
    let demande = crate::compagnie::amorcage_demande::Demande {
        motif: declarations::amorcage::EMBAUCHE,
        mois,
        salaries: None,
        envoi_test: false,
        forcer: false,
        constitution,
    };
    let lot = crate::compagnie::amorcage_demande::produire(ctx, retenues, &demande, ecrits)?;
    Ok(Amorcage { fichiers: lot.fichiers, lignes: lot.lignes.iter().map(|l| (l.salarie.clone(), l.libelle.clone())).collect() })
}

/// `{"nom", "chemin", "octets"}` d'un fichier (`sortie_json.fichier`).
fn fichier_json(f: &Path) -> Value {
    let octets = std::fs::metadata(f).ok().map(|m| m.len());
    json!({"nom": f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), "chemin": f.to_string_lossy(), "octets": octets})
}

/// La DPAE, due AVANT le premier jour travaille : tout le mois, ou les engagements choisis (`engagements` : une liste de
/// `{salarie, debut}`) dans un fichier horodate a cote du complet. `forcer` : reecrire meme une embauche deja declaree (un
/// depot rejete, a refaire).
pub(crate) fn dpae(ctx: &Contexte, charge: &Value) -> R<Value> {
    dpae_detail(ctx, charge).map(|(v, _, _)| v)
}

/// `dpae`, avec ce que la reponse JSON ne porte pas et que la ligne de commande dit : qui est demande dans le signalement
/// d amorcage (`None` : il n y en a pas) et, sinon, pourquoi il est ecarte.
pub(crate) fn dpae_detail(ctx: &Contexte, charge: &Value) -> R<(Value, Option<Vec<(String, String)>>, String)> {
    let forcer = charge.get("forcer").is_some_and(vrai);
    let mois_demande = texte_requis(charge, "mois")?;
    let heure: Value = charge.get("heure").cloned().unwrap_or_else(|| json!("0800"));
    let selection: Option<&Vec<Value>> = match charge.get("engagements") {
        None | Some(Value::Null) => None,
        Some(Value::Array(a)) => Some(a),
        Some(autre) => return Err(erreur("TypeError", socle::fichiers::attendu("une liste", socle::valeur::nom_type(autre)))),
    };

    // Les engagements valides du mois (rien ne part d'un engagement en saisie), vus comme des embauches, sans relire leur
    // paie.
    let data = engagement::etats::validees_du_mois(ctx, mois_demande).map_err(plain)?.map(|(d, _)| d)
        .ok_or_else(|| erreur("FileNotFoundError", format!("Aucune saisie pour {}.", mois_demande)))?;
    let paies: Vec<Paie> = {
        let engs = engagement::engagements_de_la_saisie(ctx, &data).map_err(plain)?;
        engs.iter().map(|e| engagement::embauche(ctx, e)).collect::<Result<_, _>>().map_err(plain)?
    };
    let mois = data.get("mois").map(en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("mois")))?;
    let (etiq, libelle) = mois_en_toutes_lettres(&mois)?;
    let sortie = ctx.racine.join("sorties").join(&mois);
    let aujourdhui = ctx.aujourdhui;
    let cle_de = |p: &Paie| -> (String, String) { (p.salarie.get("_cle").map(valeur_en_texte).unwrap_or_default(), p.engagement.debut.iso()) };

    // Un contrat en cours depuis un mois precedent n'est pas une embauche de ce mois.
    let mut en_cours: Vec<&Paie> = Vec::new();
    let mut premiers: Vec<&Paie> = Vec::new();
    for p in &paies {
        if p.engagement.premier_mois().map_err(plain)? {
            premiers.push(p);
        } else {
            en_cours.push(p);
        }
    }
    let mut retenues: Vec<&Paie> = premiers;
    let mut marque = String::new();
    if let Some(sel) = selection {
        let mut voulues: BTreeSet<(String, String)> = BTreeSet::new();
        for x in sel {
            match x {
                Value::Object(_) => {
                    let g = |k: &str| x.get(k).map(en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k)));
                    voulues.insert((g("salarie")?, g("debut")?));
                }
                Value::Array(a) if a.len() == 2 => {
                    voulues.insert((en_texte(&a[0]), en_texte(&a[1])));
                }
                autre => return Err(erreur("TypeError", socle::fichiers::attendu("une liste", socle::valeur::nom_type(autre)))),
            }
        }
        retenues = paies.iter().filter(|p| voulues.contains(&cle_de(p))).collect();
        let trouvees: BTreeSet<(String, String)> = retenues.iter().map(|p| cle_de(p)).collect();
        let anciennes: BTreeSet<(String, String)> = en_cours.iter().map(|p| cle_de(p)).collect();
        let joint = |s: BTreeSet<&(String, String)>| s.into_iter().map(|(a, b)| format!("{} au {}", a, b)).collect::<Vec<_>>().join(", ");
        let deja_parties: BTreeSet<&(String, String)> = voulues.intersection(&anciennes).collect();
        if !deja_parties.is_empty() {
            return Err(refus(format!(
                "Contrat en cours depuis un mois precedent, sa DPAE est deja partie avec son premier mois : {}. Rien n'est ecrit.",
                joint(deja_parties)
            )));
        }
        let absentes: BTreeSet<&(String, String)> = voulues.difference(&trouvees).collect();
        if !absentes.is_empty() {
            return Err(refus(format!("Ces embauches ne sont pas dans la saisie de {} : {}. Rien n'est ecrit.", mois, joint(absentes))));
        }
        if retenues.is_empty() {
            return Err(refus("Aucune embauche retenue : il n'y a rien a declarer."));
        }
        let now = maintenant_iso();
        marque = format!("_{}{}{}-{}{}", &now[0..4], &now[5..7], &now[8..10], &now[11..13], &now[14..16]);
    }
    if retenues.is_empty() {
        return Err(refus(format!(
            "Aucune embauche en {} : {}. Rien n'est ecrit.",
            libelle,
            if en_cours.is_empty() {
                "la saisie est vide"
            } else {
                "les contrats du mois sont en cours depuis un mois precedent, leur DPAE est partie avec lui"
            }
        )));
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
                let (c, d) = cle_de(&paies[i]);
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

    // PAS DEUX FOIS : une embauche deja partie en DPAE reelle ne se reecrit pas.
    let lignes_journal = etats::charger(&ctx.racine)?;
    let etat = etats::etat_des_embauches(&lignes_journal)?;
    let mut deja: HashMap<(String, String), Value> = HashMap::new();
    for entree in etat.as_array().map(Vec::as_slice).unwrap_or(&[]) {
        if let [Value::String(c), Value::String(d), info] = entree.as_array().map(Vec::as_slice).unwrap_or(&[]) {
            deja.insert((c.clone(), d.clone()), info.clone());
        }
    }
    let mut bloquees: Vec<(String, String)> = Vec::new();
    for p in &retenues {
        let k = cle_de(p);
        if let Some(info) = deja.get(&k) {
            if !info.get("refusee").map_or(false, vrai) && !bloquees.contains(&k) {
                bloquees.push(k);
            }
        }
    }
    if !bloquees.is_empty() && !forcer {
        let mut phrases = Vec::new();
        for (c, d) in &bloquees {
            phrases.push(libelle_embauche_declaree(c, d, &deja[&(c.clone(), d.clone())])?);
        }
        return Err(refus(format!("Deja declaree(s), rien n'est ecrit :\n  {}", phrases.join("\n  "))));
    }

    // LE MEME CONTENU, LE MEME FICHIER : une selection deja ecrite se reprend telle quelle.
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
    if !marque.is_empty() {
        let h = horodatage(&maintenant_iso());
        let voulu = declarations::dpae::contenu(&embauches, &parametres_de(&h, &heure_texte, &code, &ent, &chemins)).map_err(plain)?;
        let voulu = latin1(&voulu);
        let mut pareils = Vec::new();
        for x in fichiers_partiels(&sortie, &etiq) {
            let contenu = std::fs::read(&x).map_err(|e| erreur("OSError", format!("{} : {}", x.display(), e)))?;
            if declarations::dpae::meme_declaration(&latin1(&contenu), &voulu) {
                pareils.push(x);
            }
        }
        let mut repetes = Vec::new();
        for x in &pareils {
            let empreinte = envois::journal::empreinte_fichier(x).map_err(plain)?;
            let nom = x.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let r = etats::repetition(&lignes_journal, etats::DPAE, &empreinte, &nom)?;
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
            let h = horodatage(&maintenant_iso());
            declarations::dpae::ecrire(&embauches, &cible, &parametres_de(&h, &heure_texte, &code, &ent, &chemins), true).map_err(plain)?;
            crate::compagnie::depart::noter_piece_ecrite(&ctx.racine, &crate::compagnie::depart::cle_de_piece(&cible), None)?;
            let pour_recap: Vec<Embauche> = retenues.iter().map(|p| embauche_dpae(p, true)).collect();
            ecrire_texte(&recap, &declarations::dpae::recapitulatif(&pour_recap, &heure).map_err(plain)?)?;
            cible
        }
    };

    let mut lignes = Vec::new();
    for p in &retenues {
        let reste = p.engagement.debut.moins(&aujourdhui);
        let (etat, libelle_ligne) = if reste < 0 {
            ("retard", "EN RETARD (embauche passee)".to_string())
        } else if reste == 0 {
            ("aujourdhui", "AUJOURD'HUI - a deposer avant le debut du travail".to_string())
        } else {
            ("ok", format!("dans {} jour(s) - delai tenu", reste))
        };
        let nom = p.salarie.get("nom").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("nom")))?;
        let prenom = p.salarie.get("prenom_affichage").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("prenom_affichage")))?;
        lignes.push(json!({
            "salarie": format!("{} {}", valeur_en_texte(nom), valeur_en_texte(prenom)),
            "debut": p.engagement.debut.iso(),
            "heure": heure,
            "etat": etat,
            "libelle": libelle_ligne,
            "jours_restants": reste,
        }));
    }
    let retard = lignes.iter().any(|l| l["etat"] == "retard");

    let nom_f = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let nom_recap = recap.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut etapes = vec![json!({
        "cle": "depot",
        "titre": "Depot",
        "detail": format!(
            "Deposer {}.zip sur net-entreprises.fr, rubrique DPAE, AVANT le debut du contrat. Pour la saisie en ligne, recopier {}. Le moteur depose lui-meme : en test, sans rien demander ; en reel, avec le mot de passe du logiciel, tape par l'utilisateur au moment du depot. Le geste : dans la fenetre, l'action « Envoyer les DPAE… » du mois (le test d'abord, puis l'envoi du fichier teste).",
            nom_f, nom_recap
        ),
        "commande": Value::Null,
    })];

    // LE SIGNALEMENT D'AMORCAGE, DANS LE MEME GESTE : il ne fait jamais echouer la DPAE ; ce qui
    // l'empeche est ecarte, et les fichiers deja ecrits restent.
    let retenues_owned: Vec<Paie> = retenues.iter().map(|p| (*p).clone()).collect();
    let mut ecrits = Vec::new();
    let (amorce, ecarte) = match produire_amorcage(ctx, &mois, &retenues_owned, aujourdhui, &mut ecrits) {
        Ok(a) => (Some(a), String::new()),
        Err(m) => (None, m),
    };
    let _ = ecrits;
    let mut fichiers: Vec<Value> = vec![fichier_json(&f), fichier_json(&PathBuf::from(format!("{}.zip", f.to_string_lossy()))), fichier_json(&recap)];
    if let Some(a) = &amorce {
        etapes.push(json!({
            "cle": "amorcage",
            "titre": "Demander les taux de PAS",
            "detail": format!(
                "Deposer aussi {} signalement(s) d'amorcage (nature 08), un par salarie : ils demandent a la DGFiP le taux de prelevement a la source de ces embauches, qui repond sous cinq jours ouvres. Meme site, meme mot de passe, rubrique DSN. Le geste : le depot depuis la fenetre (boite « Deposer la DSN », le test d'abord). Sans eux, la premiere paie partira sur la grille de l'article 204 H, qui preleve le plus souvent PLUS que le taux du salarie.",
                a.fichiers.len()
            ),
            "commande": Value::Null,
        }));
        fichiers.extend(a.fichiers.iter().map(|p| fichier_json(p)));
    }
    let json = json!({
        "mois": mois,
        "libelle": libelle,
        "dossier": sortie.to_string_lossy(),
        "retard": retard,
        "lignes": lignes,
        "fichiers": fichiers,
        "etapes": etapes,
    });
    let amorce_lignes = amorce.as_ref().map(|a| a.lignes.clone());
    Ok((json, amorce_lignes, ecarte))
}

/// Les parametres d'une ecriture de DPAE.
fn parametres_de<'a>(horodatage: &'a str, heure: &str, code: &str, ent: &'a Value, chemins: &'a Chemins) -> Parametres<'a> {
    Parametres { horodatage, heure_embauche: heure.to_string(), code_urssaf: json!(code), ent: Some(ent), chemins }
}

/// `AAAA-MM-JJTHH:MM:SS` -> `JJMMAAAAHHMM` (`strftime("%d%m%Y%H%M")`).
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
            let dsn = crate::compagnie::depart::fabriquer_dsn(ctx, &valeur_en_texte(mois), "interface")?;
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
        crate::compagnie::releve::relever_retours_dpae(ctx, debut, fin, false, &flux)
    } else {
        crate::compagnie::releve::relever_retours(ctx, debut, fin, false, "production", None, "")
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
    let mut r = crate::routes::depots::bilan_de_la_repetition(ctx, charge)?;
    if vrai(&r["conforme"]) && etats::est(&r["compte_rendu"], "attendu") {
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
        r = crate::routes::depots::bilan_de_la_repetition(ctx, charge)?;
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

/// Un passage -- tester ce qui doit l'etre, armer, desarmer. Rend `{"testees",
/// "armes", "desarmes", "incidents", "occupe"}`. Ne leve jamais pour une panne : elle devient un incident, et le
/// passage suivant reessaie. `tester` (vrai par defaut) ; `qui` (`filet` par defaut) ; `maintenant`
/// (`AAAA-MM-JJTHH:MM`, que l'appelant peut fixer).
pub(crate) fn filet_preparer(ctx: &Contexte, charge: &Value) -> R<Value> {
    filet_preparer_avec(ctx, charge, None)
}

/// `filet_preparer` avec, au besoin, le client Urssaf que l'appelant donne pour le test automatique des DPAE (la preuve de
/// l'ouverture, lot I2b : un client pointe sur un serveur local). Un client donne n'est jamais confronte au coffre.
pub(crate) fn filet_preparer_avec(ctx: &Contexte, charge: &Value, mut client: Option<&mut envois::releve_retours::Client>) -> R<Value> {
    let tester = charge.get("tester").map_or(true, vrai);
    let qui = charge.get("qui").filter(|q| vrai(q)).map(valeur_en_texte).unwrap_or_else(|| "filet".into());
    let maintenant_iso = match charge.get("maintenant").filter(|m| vrai(m)).map(valeur_en_texte) {
        Some(m) => m,
        None => socle::paris::iso_de_naif(socle::paris::maintenant_naif())[..16].to_string(),
    };
    let mut bilan_testees: Vec<Value> = Vec::new();
    let mut bilan_incidents: Vec<Value> = Vec::new();
    let suivies = crate::compagnie::filet::suivies(ctx)?;
    let mut demande = Map::new();
    demande.insert("maintenant".into(), json!(maintenant_iso));
    if tester {
        let etat = crate::compagnie::filet::lire_etat(&ctx.racine)?;
        if let Some(tests) = tester_les_dpae(ctx, &etat, &suivies, &maintenant_iso, &qui, &mut bilan_testees, &mut bilan_incidents, client.as_deref_mut())? {
            demande.insert("tests".into(), tests);
            demande.insert(
                "vivantes".into(),
                Value::Array(suivies.iter().map(|e| json!(cle_du_test(&valeur_en_texte(&e["salarie"]), &valeur_en_texte(&e["debut"])))).collect()),
            );
        }
    }
    let champ = |k: &str| demande.get(k).cloned().unwrap_or(Value::Null);
    match crate::compagnie::filet::passage(ctx, &champ("tests"), &champ("vivantes"), &champ("maintenant")) {
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
    mut client: Option<&mut envois::releve_retours::Client>,
) -> R<Option<Value>> {
    // La copie de developpement ne teste pas d'elle-meme ; sans le mot de passe Urssaf, rien ne part. LA
    // COMPAGNIE FICTIVE, elle, teste : son depot est joue.
    // Un client donne (la preuve) tient lieu du mot de passe : le coffre n'est pas confronte.
    if !crate::compagnie::bridage::branchement(&ctx.racine).joue() {
        let copie = ctx.racine.join("COPIE-DE-DEVELOPPEMENT.json").is_file();
        let dispo = client.is_some() || crate::compagnie::coffre::Coffre::du_poste(&ctx.racine, "").disponible("urssaf").map_err(crate::routes::vers_route)?;
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
            let r = crate::routes::depots::dpae(ctx, &json!({"mois": mois, "engagements": selection}))?;
            let f = r["fichiers"][0]["chemin"].as_str().unwrap_or("").to_string();
            // Le fichier est note des qu'il est ecrit : un depot qui echoue garde sa trace (`fichier = r.fichiers[0]`).
            *fichier = Some(f.clone());
            let p = std::path::Path::new(&f);
            let octets = std::fs::read(p).map_err(|x| erreur("OSError", format!("{} : {}", p.display(), x)))?;
            let nom = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let lignes = etats::charger(&ctx.racine)?;
            let deja = etats::repetition(&lignes, "dpae", &envois::journal::empreinte(&octets), &nom)?;
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

// ------------------------------------------------------------------ les documents du mois




// ------------------------------------------------------------------ les chemins

/// `str(PurePosixPath(t))` : les `//` et les `.` intermediaires s'en vont, un `/`
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
/// le texte du chemin, ecrit comme `str(Path)`.
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
        return Err(erreur("TypeError", format!("expected str, bytes or os.PathLike object, not {}", nom_type(chemin))));
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

// ------------------------------------------------------------------ bilan-repetition


// ------------------------------------------------------------------ depot-apercu, la DPAE

/// `Path.read_bytes()`.
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
    let mut m = crate::routes::etapes::Moteur::nouveau(ctx);
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
    let vide = json!({});
    let mut lignes = Vec::new();
    for e in &enrs {
        let champ = |k: &str| e.iter().find(|(n, _)| *n == k).map(|(_, v)| v.as_str()).unwrap_or("");
        let trouve = par_nir.get(champ("nir"));
        let (cle, s) = match trouve {
            Some((c, s)) => (Some(*c), *s),
            None => (None, &vide),
        };
        let debut = iso_depuis_jjmmaaaa(champ("date_embauche"));
        let nom_s = etats::getd(s, "nom", etats::texte_vide())?;
        let prenom = etats::ou(etats::get(s, "prenom_affichage")?, etats::getd(s, "prenom", etats::texte_vide())?);
        let nom = format!("{} {}", socle::valeur::valeur_en_texte(nom_s), socle::valeur::valeur_en_texte(prenom));
        let nom = nom.trim_matches(socle::texte::blanc);
        let nom = if nom.is_empty() { "salarie inconnu de la configuration" } else { nom };
        let type_brut = champ("type_contrat").trim_matches(socle::texte::blanc);
        let type_contrat = match type_brut {
            "1" => "CDD".to_string(),
            "2" => "CDI".to_string(),
            "3" => "CTT".to_string(),
            _ => format!("type {}", champ("type_contrat")),
        };
        let cle_json = cle.map_or(Value::Null, |c| json!(c));
        let deja = faites.contient(&[cle_json, json!(debut)])?;
        lignes.push(json!({
            "salarie": cle.map_or("?", |c| c.as_str()),
            "nom": nom,
            "debut": debut,
            "fin": iso_depuis_jjmmaaaa(champ("date_fin")),
            "type_contrat": type_contrat,
            "heure": champ("heure_embauche"),
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
        "fichier": chemin, "nom": nom_fichier,
        "enregistrements": enrs.len(), "octets": contenu.len(),
        "reel": envois::depot::est_reel(&contenu),
        "indicateur": envois::depot::indicateur(&contenu),
        "mois": mois.first().cloned().unwrap_or_default(),
        "tous_les_mois": mois,
        "embauches": lignes,
        "deja_depose": deja_depose,
        "repetition": repetition,
    }))
}

fn depot_apercu(ctx: &Contexte, charge: &Value) -> R<Value> {
    let quoi = cle(charge, "quoi")?;
    let fichier = cle(charge, "fichier")?;
    if etats::est(quoi, "dsn") {
        return crate::apercu_dsn::apercu_depot_dsn(ctx, fichier);
    }
    apercu_depot_dpae(ctx, fichier)
}

pub(crate) fn bilan_de_la_repetition(ctx: &Contexte, charge: &Value) -> R<Value> {
    let quoi = cle(charge, "quoi")?;
    let fichier = cle(charge, "fichier")?;
    let chemin = resoudre(&ctx.racine, fichier)?;
    let empreinte = empreinte_du_fichier(&chemin)?;
    let nom = nom_du_chemin(&chemin);
    let mut m = crate::routes::etapes::Moteur::nouveau(ctx);
    let lignes = m.lignes()?;
    let Some(quoi) = quoi.as_str() else {
        return Err(Echec { classe: None, message: "demande illisible : il faut `quoi`".to_string() });
    };
    let mut r = etats::repetition(&lignes, quoi, &empreinte, &nom)?;
    r["incident"] = json!("");
    Ok(r)
}
