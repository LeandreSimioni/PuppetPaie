//! `importation-apercu` : ce que l'import d'un export CoolPaie ferait. Rien n'est ecrit.
//!
//! C'est le Python de `operations_importation.apercu_importation` (et `_fiche_salarie`,
//! `_fiche_spectacle`, `_fiche_entreprise`, `_ecarts`, `_paies_declarees`, `lire_historique`,
//! `lire_paies_declarees`) et de `sortie_json.importation`, porte a l'identique : meme JSON, memes
//! refus. La lecture de l'export (DSN et AEM) est dans `importation.rs`.
//!
//! `{"dossier": ...}` : la boite aux lettres `import/` entiere (`importation_bulletins`) : le tri, les DSN et AEM
//! qu'il trouve passees par `apercu_lu` (le meme code que pour l'export), puis les bulletins de paie.
//!
//! `contrats_nouveaux` : un contrat atteste que l'historique (`historique/engagements.json`) porte deja
//! -- meme salarie, memes dates, meme objet, memes heures, cachets, jours et meme brut (`str`) -- n'est
//! pas nouveau (`_cle_engagement`).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use crate::etats;
use socle::valeur::py_str;
use socle::valeur::nom_type;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::vrai;
use crate::importation::{self, SalarieDeclare};
use socle::echec::Echec;

/// Les routes que ce module sert.
pub const ROUTES: [&str; 2] = ["importation-apercu", "importation-tri"];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        // `{"dossier": ...}` (vide : `import/` de la compagnie) : la boite aux lettres entiere, bulletins compris
        // (`importation_bulletins`) ; sans `dossier`, l'export de CoolPaie comme avant.
        "importation-apercu" if charge.get("dossier").is_some() => {
            if charge.get("export").is_some_and(vrai) {
                return Err(erreur("ErreurOperation", "L'apercu lit un export (`export`) ou un dossier (`dossier`), pas les deux."));
            }
            Ok(Value::Object(crate::importation_bulletins::apercu_dossier(ctx, &dossier_import(ctx, charge)?)?.reponse))
        }
        "importation-apercu" => {
            let export = export_de(ctx, charge)?;
            Ok(Value::Object(apercu_importation(ctx, &export, ctx.aujourdhui)?.reponse))
        }
        "importation-tri" => tri(ctx, charge),
        autre => Err(Echec { classe: None, message: format!("route inconnue : {}", autre) }),
    }
}

/// `importation-tri` : le tri de la boite aux lettres (`importation_tri`), `{"dossier": "..."}` ; sans dossier,
/// `import/` a la racine de la compagnie. Rien n'est ecrit dans les donnees de la compagnie.
///
/// LE DOSSIER DE TRAVAIL (ou s'ouvrent les zip) EST UN DOSSIER TEMPORAIRE DU SYSTEME, propre a cet appel, efface
/// avant de rendre : sous `import/.travail`, il se retrouverait dans la sauvegarde et dans le depot de la
/// compagnie, en double des archives qu'il copie, et garderait d'un tri a l'autre ce qu'une archive retiree
/// avait rendu. Le tri rend du JSON, pas des chemins a relire : un lot qui lira ces fichiers trie dans son
/// propre appel, et les lit avant d'effacer.
fn tri(ctx: &Contexte, charge: &Value) -> R<Value> {
    let dossier = dossier_import(ctx, charge)?;
    let travail = dossier_de_travail();
    let rendu = crate::importation_tri::trier(&dossier, &travail).map(|t| t.json());
    let _ = std::fs::remove_dir_all(&travail);
    rendu
}

/// Le dossier a trier : `charge.dossier`, ou `import/` a la racine de la compagnie s'il est vide ou absent.
///
/// LA BOITE AUX LETTRES DE LA COMPAGNIE SE CREE VIDE (Leandre, 10/10/2026) : `import/` qui n'existe pas encore est
/// cree, et l'apercu (ou l'import) n'a rien a reprendre, au lieu de refuser « dossier absent ». Un dossier DONNE
/// (`charge.dossier`) qui n'existe pas reste refuse (`importation_tri::trier`).
pub(crate) fn dossier_import(ctx: &Contexte, charge: &Value) -> R<PathBuf> {
    Ok(match charge.get("dossier").filter(|v| vrai(v)) {
        None => {
            let d = ctx.racine.join("import");
            if !d.exists() {
                std::fs::create_dir_all(&d).map_err(|e| erreur("OSError", format!("{} : {}", d.display(), e)))?;
            }
            d
        }
        Some(Value::String(s)) => PathBuf::from(crate::routes_apercus::normaliser(s)),
        Some(autre) => return Err(erreur("TypeError", format!("expected str, bytes or os.PathLike object, not {}", nom_type(autre)))),
    })
}

/// Un dossier de travail temporaire du systeme, propre a cet appel (a effacer avant de rendre).
pub(crate) fn dossier_de_travail() -> PathBuf {
    let marque = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    std::env::temp_dir().join(format!("puppetpaie-tri-{}-{}", std::process::id(), marque))
}

/// `contexte.trouver_export` : le dossier de CoolPaie, remonte de dossier en dossier.
pub(crate) fn trouver_export(racine: &Path) -> PathBuf {
    let parent = racine.parent();
    let grand = parent.and_then(Path::parent);
    for base in [Some(racine), parent, grand].into_iter().flatten() {
        let p = base.join("export");
        if p.is_dir() {
            return p;
        }
    }
    let copie = racine.join("export-coolpaie");
    if copie.is_dir() {
        return copie;
    }
    racine.join("export")
}

/// `brut(valeur)` : un dictionnaire perd ses cles qui commencent par `_`.
pub(crate) fn brut(v: &Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(o.iter().filter(|(k, _)| !k.starts_with('_')).map(|(k, x)| (k.clone(), brut(x))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(brut).collect()),
        autre => autre.clone(),
    }
}

/// `str.title()` d'un texte, `""` s'il manque.
pub(crate) fn titre_de(v: Option<&Value>) -> String {
    match v.filter(|x| vrai(x)) {
        Some(x) => crate::champs::titre(&py_str(x)),
        None => String::new(),
    }
}

/// `_ecarts` : les champs DECLARES que la DSN disait autrement que la fiche.
fn ecarts(nom: &str, fiche: &Value, declare: &Map<String, Value>, mois: &str) -> R<Value> {
    let mut out = Map::new();
    for c in crate::champs::fiche(nom)? {
        if !c.declare || !declare.contains_key(&c.cle) {
            continue;
        }
        let a = fiche.get(&c.cle).unwrap_or(&Value::Null);
        let b = &declare[&c.cle];
        // Une case vide ou « A COMPLETER » n'est pas un ecart : l'import la complete (`importation_completer`).
        if crate::importation_completer::vide(Some(a)) || py_str(a) == py_str(b) {
            continue;
        }
        // Une structure (la banque) ne differe que par ce que la DSN en declare : ses notes et autres cles restent.
        if let (Value::Object(x), Value::Object(y)) = (a, b) {
            if y.iter().all(|(k, v)| x.get(k).map(py_str) == Some(py_str(v))) {
                continue;
            }
        }
        out.insert(c.cle.clone(), json!({"fiche": a, "dsn": b, "mois": mois}));
    }
    Ok(Value::Object(out))
}

/// Une fiche que l'import cree, laisse, ou sur laquelle il a a redire.
pub(crate) fn fiche_importee(nom: &str, cle: Option<&str>, libelle: &str, etat: &str, ecarts: Value, anomalies: Vec<Value>, donnees: Value) -> Value {
    json!({"nom": nom, "cle": cle, "libelle": libelle, "etat": etat, "ecarts": brut(&ecarts), "anomalies": brut(&Value::Array(anomalies)), "donnees": brut(&donnees)})
}

/// `_fiche_salarie` : ce qu'une fiche neuve recoit d'une DSN.
fn fiche_salarie(decl: &SalarieDeclare) -> R<Map<String, Value>> {
    let mut f = decl.fiche.clone();
    f.insert("nir_cle".into(), json!(crate::champs::cle_nir(&decl.nir).unwrap_or_default()));
    f.insert("nir".into(), json!(decl.nir));
    if !decl.initiales.is_empty() {
        f.insert("initiales_contrat".into(), json!(decl.initiales));
    }
    if !decl.premier_contrat.is_empty() {
        f.insert("date_anciennete".into(), json!(decl.premier_contrat));
    }
    // Le statut de bareme se deduit du code emploi France Travail, a defaut de la PCS (354x : artistes).
    let code = f.get("code_emploi_ft").map_or(String::new(), py_str);
    let m = droit::emplois::metier(&code)?.unwrap_or_else(|| json!({}));
    let statut = match m.get("statut").filter(|v| vrai(v)) {
        Some(s) => s.clone(),
        None => json!(if f.get("pcs_ese").map_or(String::new(), py_str).starts_with("354") { "artiste" } else { "technicien" }),
    };
    f.insert("statut".into(), statut);
    let mut f = crate::champs::combler_affichage("salarie", &f)?;
    f.insert("actif".into(), json!(true));
    Ok(f)
}

/// La fiche Entreprise que les DSN donnent : ce qu'elles declarent de l'entreprise, son Siret, la suite de ses licences.
fn fiche_entreprise(lecture: &importation::LectureDsn) -> R<Map<String, Value>> {
    let mut e = lecture.entreprise.clone();
    let (siren, nic) = (e.get("siren").filter(|v| vrai(v)).cloned(), e.get("nic").filter(|v| vrai(v)).cloned());
    if let (Some(s), Some(n)) = (siren, nic) {
        e.insert("siret".into(), json!(format!("{}{}", py_str(&s), py_str(&n))));
    }
    if !lecture.licences.is_empty() {
        let l: Vec<Value> = lecture.licences.iter().map(|(m, n)| json!({"depuis": format!("{}-01", m), "numero": n})).collect();
        e.insert("licence_spectacle".into(), Value::Array(l));
    }
    let mut e = crate::champs::combler_affichage("entreprise", &e)?;
    if e.get("raison_sociale").is_some_and(vrai) && !e.get("nom_affichage").is_some_and(vrai) {
        let t = titre_de(e.get("raison_sociale"));
        e.insert("nom_affichage".into(), json!(t));
    }
    Ok(e)
}

/// Le dossier de l'export demandé : `charge.export`, ou celui qu'on trouve a cote des donnees.
pub(crate) fn export_de(ctx: &Contexte, charge: &Value) -> R<PathBuf> {
    let export_v = charge.get("export").filter(|v| vrai(v));
    Ok(match export_v {
        None => trouver_export(&ctx.racine),
        Some(Value::String(s)) => PathBuf::from(crate::routes_apercus::normaliser(s)),
        Some(autre) => {
            return Err(erreur("TypeError", format!("expected str, bytes or os.PathLike object, not {}", nom_type(autre))));
        }
    })
}

/// Une fiche que l'import creerait : ou elle va dans la reponse, et ce qu'elle porte (tel quel, les cles
/// `_vu_dans` comprises, que la reponse ne montre pas).
pub(crate) struct ACreer {
    pub(crate) nom: &'static str,
    /// Le rang dans `salaries` ou `spectacles` de la reponse ; rien pour l'entreprise.
    pub(crate) rang: Option<usize>,
    pub(crate) donnees: Map<String, Value>,
}

/// Ce que l'import ferait : la reponse (`sortie_json.importation`), et ce dont l'ecriture a besoin.
pub(crate) struct Apercu {
    pub(crate) reponse: Map<String, Value>,
    pub(crate) a_creer: Vec<ACreer>,
    /// Les fiches existantes que l'import completerait (`importation_completer`).
    pub(crate) a_completer: Vec<crate::importation_completer::ACompleter>,
    pub(crate) contrats: Vec<importation::ContratAtteste>,
    /// `imp._paies` : les versements declares par les DSN, rattaches a une fiche (ou `<a creer>`).
    pub(crate) paies: Vec<Value>,
    /// Les contrats que les bulletins attestent (`importation_bulletins`), lignes de `historique/engagements.json`
    /// pretes a ecrire, chacune avec le NIR qui la rattache (`salarie` vaut `<a creer>` tant que la fiche n'existe pas).
    pub(crate) engagements_bulletins: Vec<(String, Value)>,
    pub(crate) source: String,
}

/// `apercu_importation(ctx, dossier, aujourdhui)`.
///
/// UNE DSN OU UNE AEM D'UNE AUTRE ENTREPRISE N'EST PAS LUE (lot G1) : `importation::garder_export` les ecarte devant le
/// Siren de la compagnie (`importation::siren_de_reference`, le meme que pour les bulletins) ; l'apercu lit alors une
/// copie de l'export sans elles (un dossier de travail, efface avant de rendre), et chacune est un avertissement.
pub(crate) fn apercu_importation(ctx: &Contexte, export: &Path, aujourdhui: socle::date::Jour) -> R<Apercu> {
    let export: PathBuf = export.to_path_buf();
    let (dsn, aem) = importation::dossiers(&export);
    if dsn.is_none() && aem.is_none() {
        return Err(erreur(
            "ErreurOperation",
            format!(
                "Aucune DSN ni AEM sous {}. Le dossier attendu est l'export des déclarations d'avant, avec ses sous-dossiers `DSN/` et `AEM_EDI/`.",
                export.display()
            ),
        ));
    }
    let garde = importation::garder_export(&ctx.entreprise, dsn.as_deref(), aem.as_deref());
    let source = export.display().to_string();
    let mut ap = if garde.ecartes.is_empty() {
        apercu_lu(ctx, &source, dsn.as_deref(), aem.as_deref(), aujourdhui, &HashSet::new())?
    } else {
        let travail = dossier_de_travail();
        let rendu = (|| -> R<Apercu> {
            let ecartes: HashSet<&PathBuf> = garde.ecartes.iter().map(|(f, _)| f).collect();
            let mut dossiers: [Option<PathBuf>; 2] = [None, None];
            for (k, (sous, fichiers)) in [
                ("DSN", dsn.as_deref().map(importation::fichiers_dsn).unwrap_or_default()),
                ("AEM_EDI", aem.as_deref().map(importation::fichiers_aem).unwrap_or_default()),
            ]
            .into_iter()
            .enumerate()
            {
                let d = travail.join(sous);
                std::fs::create_dir_all(&d).map_err(|e| erreur("OSError", format!("{} : {}", d.display(), e)))?;
                for f in fichiers.iter().filter(|f| !ecartes.contains(f)) {
                    let nom = f.file_name().unwrap_or_default();
                    std::fs::copy(f, d.join(nom)).map_err(|e| erreur("OSError", format!("{} : {}", f.display(), e)))?;
                }
                dossiers[k] = Some(d);
            }
            apercu_lu(ctx, &source, dossiers[0].as_deref(), dossiers[1].as_deref(), aujourdhui, &HashSet::new())
        })();
        let _ = std::fs::remove_dir_all(&travail);
        rendu?
    };
    if let Some(Value::Array(a)) = ap.reponse.get_mut("avertissements") {
        a.extend(garde.avertissements(true, true).into_iter().map(Value::String));
    }
    Ok(ap)
}

/// La cle `_cle_engagement` : (salarie, debut, fin, numero_objet, heures, cachets, jours, str(brut)).
pub(crate) type CleEngagement = (String, String, String, String, i128, i128, i128, String);

/// `deja = {_cle_engagement(e) ...}` : les contrats que `historique/engagements.json` porte deja. Une valeur qui
/// n'est pas un texte ne peut egaler un texte : l'entree ne retrouvera aucun contrat.
pub(crate) fn cles_historique(ctx: &Contexte) -> R<HashSet<CleEngagement>> {
    let historique = crate::routes_etats::lire_json_ou_vide(&ctx.racine.join("historique").join("engagements.json"))?;
    // `lire_historique` : `data.setdefault("engagements", [])` leve sur ce qui n'est pas un objet.
    if !historique.is_object() {
        return Err(erreur("AttributeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(&historique))));
    }
    let engagements = historique.get("engagements").cloned().unwrap_or_else(|| json!([]));
    let mut deja: HashSet<CleEngagement> = HashSet::new();
    for e in etats::items(&engagements)? {
        // `_cle_engagement(e)` : les cles qu'elle lit doivent y etre, et les nombres se lire.
        for k in ["salarie", "debut", "fin"] {
            if e.get(k).is_none() {
                return Err(erreur("KeyError", socle::fichiers::absent(k)));
            }
        }
        let mut nombres = [0i128; 3];
        for (i, k) in ["heures", "cachets", "jours"].iter().enumerate() {
            nombres[i] = entier_py(e.get(*k).unwrap_or(&json!(0)))?;
        }
        let texte = |v: Option<&Value>| match v {
            Some(Value::String(t)) => Some(t.clone()),
            _ => None,
        };
        let objet = match e.get("numero_objet") {
            None => Some(String::new()),
            autre => texte(autre),
        };
        let brut = e.get("brut").map_or("0".to_string(), py_str);
        if let (Some(a), Some(b), Some(c), Some(d)) = (texte(e.get("salarie")), texte(e.get("debut")), texte(e.get("fin")), objet) {
            deja.insert((a, b, c, d, nombres[0], nombres[1], nombres[2], brut));
        }
    }
    Ok(deja)
}

/// Les lignes de `historique/paies.json` telles qu'elles sont (`lire_paies_declarees` : meme `setdefault`).
pub(crate) fn paies_historique(ctx: &Contexte) -> R<Vec<Value>> {
    let fichier_paies = crate::routes_etats::lire_json_ou_vide(&ctx.racine.join("historique").join("paies.json"))?;
    if !fichier_paies.is_object() {
        return Err(erreur("AttributeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(&fichier_paies))));
    }
    let liste_paies = fichier_paies.get("paies").cloned().unwrap_or_else(|| json!([]));
    Ok(etats::items(&liste_paies)?.into_iter().map(|c| c.into_owned()).collect())
}

/// La cle d'une paie de l'historique : (mois, salarie, contrat).
pub(crate) fn cle_paie(x: &Value) -> R<(String, String, String)> {
    let k = |c: &str| -> R<String> { x.get(c).map(py_str).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(c))) };
    Ok((k("mois")?, k("salarie")?, k("contrat")?))
}

/// L'apercu de ce qu'un dossier de DSN et un dossier d'AEM donneraient (l'un ou l'autre peut manquer : rien n'en
/// vient). `a_venir` : des NIR dont une fiche sera creee d'ailleurs (les bulletins) -- un contrat atteste qui les
/// porte n'est pas « sans fiche ».
pub(crate) fn apercu_lu(
    ctx: &Contexte,
    source: &str,
    dsn: Option<&Path>,
    aem: Option<&Path>,
    aujourdhui: socle::date::Jour,
    a_venir: &HashSet<String>,
) -> R<Apercu> {
    let lecture = match &dsn {
        Some(d) => importation::lire_dsn(d),
        None => Default::default(),
    };
    let mut avertissements: Vec<String> = lecture.illisibles.iter().map(|f| format!("DSN illisible, ignoree : {}", f)).collect();
    // LA FICHE NE PORTE QU'UN ETABLISSEMENT (celui de son NIC, `champs-base.json`, `etablissements`) : des DSN qui en
    // declarent plusieurs, ou dont le siege n'est pas l'etablissement declare, le disent (lot I2, 3b).
    if lecture.etablissements.len() > 1 {
        avertissements.push(format!(
            "les DSN déclarent plusieurs établissements (S21.G00.11.001 : {}) : la fiche Entreprise n'en porte qu'un, celui de son NIC ; le moteur ne déclare que celui-là",
            lecture.etablissements.iter().cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    if lecture.nics_siege.iter().any(|n| !lecture.etablissements.contains(n)) && !lecture.etablissements.is_empty() {
        avertissements.push(format!(
            "le siège de l'entreprise (S21.G00.06.002 : {}) n'est pas l'établissement déclaré (S21.G00.11.001) : la fiche Entreprise porte l'établissement, et le moteur déclare son NIC aux deux places",
            lecture.nics_siege.iter().cloned().collect::<Vec<_>>().join(", ")
        ));
    }

    // 1. L'entreprise : seulement si la fiche est vide.
    let mut entreprise = Value::Null;
    let mut a_creer = 0usize;
    let mut a_creer_liste: Vec<ACreer> = Vec::new();
    let mut a_completer: Vec<crate::importation_completer::ACompleter> = Vec::new();
    let origine_dsn = |mois: &str| json!({"source": "dsn", "mois": mois});
    if !lecture.entreprise.is_empty() {
        let e = fiche_entreprise(&lecture)?;
        if !ctx.entreprise.get("siren").is_some_and(|s| vrai(s) && !py_str(s).trim().is_empty()) {
            let anomalies = crate::routes_fiches::verifier_comme_a_l_ecriture("entreprise", &e)?;
            let libelle = e.get("raison_sociale").map_or("?".to_string(), py_str);
            a_creer_liste.push(ACreer { nom: "entreprise", rang: None, donnees: e.clone() });
            entreprise = fiche_importee("entreprise", None, &libelle, "a_creer", json!({}), anomalies, Value::Object(e));
            a_creer += 1;
        } else {
            let libelle = ctx.entreprise.get("raison_sociale").map_or(String::new(), py_str);
            let ec = ecarts("entreprise", &ctx.entreprise, &lecture.entreprise, &lecture.dernier_mois)?;
            entreprise = fiche_importee("entreprise", None, &libelle, "existante", ec, Vec::new(), json!({}));
            let champs = crate::importation_completer::champs_a_completer("entreprise", &ctx.entreprise, &e)?;
            crate::importation_completer::noter(&mut entreprise, &champs, origine_dsn(&lecture.dernier_mois));
            crate::importation_completer::ajouter(&mut a_completer, "entreprise", None, &champs);
        }
    }

    // 2. Les salaries, par NIR.
    let mut par_nir: HashMap<String, String> = HashMap::new();
    for (cle, s) in &ctx.salaries {
        if let Some(n) = s.get("nir").filter(|v| vrai(v)) {
            par_nir.insert(py_str(n), cle.clone());
        }
    }
    let mut ordre: Vec<&SalarieDeclare> = lecture.salaries.iter().collect();
    ordre.sort_by(|a, b| a.mois.cmp(&b.mois));
    let mut salaries = Vec::new();
    let mut cles_a_venir: HashSet<String> = HashSet::new();
    for decl in ordre {
        let libelle = format!(
            "{} {}",
            decl.fiche.get("nom").map_or(String::new(), py_str),
            titre_de(decl.fiche.get("prenom"))
        );
        if let Some(cle) = par_nir.get(&decl.nir) {
            let ec = ecarts("salarie", &ctx.salaries[cle], &decl.fiche, &decl.mois)?;
            let mut entree = fiche_importee("salarie", Some(cle), &libelle, "existante", ec, Vec::new(), json!({}));
            let champs = crate::importation_completer::champs_a_completer("salarie", &ctx.salaries[cle], &fiche_salarie(decl)?)?;
            crate::importation_completer::noter(&mut entree, &champs, origine_dsn(&decl.mois));
            crate::importation_completer::ajouter(&mut a_completer, "salarie", Some(cle), &champs);
            salaries.push(entree);
            continue;
        }
        let donnees = fiche_salarie(decl)?;
        let anomalies = crate::routes_fiches::verifier_comme_a_l_ecriture("salarie", &donnees)?;
        a_creer_liste.push(ACreer { nom: "salarie", rang: Some(salaries.len()), donnees: donnees.clone() });
        salaries.push(fiche_importee("salarie", None, &libelle, "a_creer", json!({}), anomalies, Value::Object(donnees)));
        cles_a_venir.insert(decl.nir.clone());
        a_creer += 1;
    }

    cles_a_venir.extend(a_venir.iter().filter(|n| !par_nir.contains_key(*n)).cloned());

    // 3. Les spectacles, par numero d'objet -- ceux des DSN et ceux des AEM.
    let (contrats, illisibles) = match &aem {
        Some(d) => importation::lire_aem(d)?,
        None => (Vec::new(), Vec::new()),
    };
    avertissements.extend(illisibles.iter().map(|f| format!("AEM illisible, ignoree : {}", f)));
    let n_aem = contrats.iter().map(|c| c.fichier.clone()).collect::<HashSet<_>>().len();
    struct Objet {
        numero: String,
        premier: String,
        dernier: String,
        contrats: i64,
        aem: bool,
    }
    let mut objets: Vec<Objet> = lecture
        .objets
        .iter()
        .map(|o| Objet { numero: o.numero.clone(), premier: o.premier_mois.clone(), dernier: o.dernier_mois.clone(), contrats: o.contrats, aem: o.source == "AEM" })
        .collect();
    for c in &contrats {
        if c.numero_objet.is_empty() {
            continue;
        }
        let m: String = c.fin.iso().chars().take(7).collect();
        let i = match objets.iter().position(|o| o.numero == c.numero_objet) {
            Some(i) => i,
            None => {
                objets.push(Objet { numero: c.numero_objet.clone(), premier: m.clone(), dernier: m.clone(), contrats: 0, aem: true });
                objets.len() - 1
            }
        };
        if objets[i].aem {
            if m > objets[i].dernier {
                objets[i].dernier = m;
            }
            objets[i].contrats += 1;
        }
    }
    let mut connus: HashMap<String, String> = HashMap::new();
    for (cle, sp) in &ctx.spectacles {
        let n = sp.get("numero_objet").map_or(String::new(), py_str).to_uppercase();
        connus.insert(n, cle.clone());
    }
    objets.sort_by(|a, b| a.premier.cmp(&b.premier));
    let seuil = format!("{:04}-{:02}", aujourdhui.an - 1, aujourdhui.mois);
    let mut spectacles = Vec::new();
    for o in &objets {
        if let Some(cle) = connus.get(&o.numero.to_uppercase()) {
            let titre = ctx.spectacles[cle].get("titre").map_or("?".to_string(), py_str);
            spectacles.push(fiche_importee("spectacle", Some(cle), &format!("{} ({})", titre, o.numero), "existante", json!({}), Vec::new(), json!({})));
            continue;
        }
        let source = if o.aem { "AEM" } else { "DSN" };
        let mut donnees = Map::new();
        donnees.insert("titre".into(), json!("A COMPLETER"));
        donnees.insert("numero_objet".into(), json!(o.numero));
        donnees.insert("alias".into(), json!([]));
        donnees.insert("actif".into(), json!(o.dernier >= seuil));
        donnees.insert("_vu_dans".into(), json!(format!("{} de {} a {} ({} contrat(s))", source, o.premier, o.dernier, o.contrats)));
        let anomalies = crate::routes_fiches::verifier_comme_a_l_ecriture("spectacle", &donnees)?;
        a_creer_liste.push(ACreer { nom: "spectacle", rang: Some(spectacles.len()), donnees: donnees.clone() });
        spectacles.push(fiche_importee(
            "spectacle",
            None,
            &format!("numéro d'objet {}, sans titre (déclarations de {} à {})", o.numero, o.premier, o.dernier),
            "a_creer",
            json!({}),
            anomalies,
            Value::Object(donnees),
        ));
        a_creer += 1;
    }

    // 4. L'historique : les contrats attestes, rattaches par NIR aux fiches -- existantes ou a creer.
    let deja = cles_historique(ctx)?;
    let mut contrats_nouveaux = 0usize;
    let mut sans_fiche = 0usize;
    for c in &contrats {
        if !par_nir.contains_key(&c.nir) && !cles_a_venir.contains(&c.nir) {
            sans_fiche += 1;
            continue;
        }
        let cle = par_nir.get(&c.nir).cloned().unwrap_or_else(|| "<a creer>".to_string());
        if !deja.contains(&(cle, c.debut.iso(), c.fin.iso(), c.numero_objet.clone(), c.heures, c.cachets, c.jours, c.brut.clone())) {
            contrats_nouveaux += 1;
        }
    }
    let contrats_du = contrats.iter().map(|c| c.debut).min().map(|d| d.iso()).unwrap_or_default();
    let contrats_au = contrats.iter().map(|c| c.fin).max().map(|d| d.iso()).unwrap_or_default();
    if sans_fiche > 0 {
        avertissements.push(format!(
            "{} contrat(s) atteste(s) portent un NIR qu'aucune DSN ne declare : ils ne seront pas rattaches a une fiche, donc pas comptes dans les droits. Creer la fiche a la main, puis reimporter.",
            sans_fiche
        ));
    }

    // 5. Les paies declarees par les DSN : ce que chaque versement a porte.
    let deja_paies: HashSet<(String, String, String)> = paies_historique(ctx)?.iter().map(cle_paie).collect::<R<_>>()?;
    let mut paies_nouvelles = 0usize;
    for ((mois, nir, contrat), _) in &lecture.paies {
        let cle = par_nir.get(nir);
        if cle.is_none() && !cles_a_venir.contains(nir) {
            continue;
        }
        let salarie = cle.cloned().unwrap_or_else(|| "<a creer>".to_string());
        if !deja_paies.contains(&(mois.clone(), salarie, contrat.clone())) {
            paies_nouvelles += 1;
        }
    }
    let rien_a_faire = a_creer == 0 && a_completer.is_empty() && contrats_nouveaux == 0 && paies_nouvelles == 0;
    // `_paies_declarees` : DEUX SOURCES POUR UNE LIGNE -- les montants viennent de la DSN, le numero d'objet peut
    // venir de l'AEM (rapprochee sur salarie, debut, fin).
    let objets_aem: HashMap<(String, String, String), String> = contrats
        .iter()
        .filter(|c| !c.numero_objet.is_empty())
        .map(|c| ((c.nir.clone(), c.debut.iso(), c.fin.iso()), c.numero_objet.clone()))
        .collect();
    let mut paies: Vec<Value> = Vec::new();
    for ((mois, nir, contrat), v) in &lecture.paies {
        let cle = par_nir.get(nir);
        if cle.is_none() && !cles_a_venir.contains(nir) {
            continue;
        }
        let texte = |k: &str| v.get(k).cloned().unwrap_or_else(|| json!(""));
        let objet = match v.get("objet").filter(|x| vrai(x)) {
            Some(o) => o.clone(),
            None => {
                let k = (nir.clone(), py_str(&texte("debut")), py_str(&texte("fin")));
                json!(objets_aem.get(&k).cloned().unwrap_or_default())
            }
        };
        paies.push(json!({
            "mois": mois, "salarie": cle.cloned().unwrap_or_else(|| "<a creer>".to_string()), "nir": nir,
            "contrat": contrat, "numero_objet": objet,
            "debut": texte("debut"), "fin": texte("fin"),
            "brut": texte("brut"), "net_imposable": texte("net_imposable"),
            // 50.004 est le net verse, pas le net a payer : la DSN ne declare pas celui-ci, un bulletin le donnera.
            "net_verse": texte("net_verse"), "net_a_payer": "", "pas": texte("pas"),
            "verse_le": texte("verse_le"),
            "declare_par": "DSN", "depose_le": texte("depot"),
            "fichier": texte("fichier"),
        }));
    }
    let Value::Object(reponse) = json!({
        "source": source,
        "dsn": lecture.fichiers,
        "aem": n_aem,
        "premier_mois": lecture.premier_mois,
        "dernier_mois": lecture.dernier_mois,
        "entreprise": entreprise,
        "salaries": salaries,
        "spectacles": spectacles,
        "contrats": contrats.len(),
        "contrats_nouveaux": contrats_nouveaux,
        "contrats_du": contrats_du,
        "contrats_au": contrats_au,
        "contrats_sans_fiche": sans_fiche,
        "avertissements": avertissements,
        "rien_a_faire": rien_a_faire,
        "ecrit": false,
        "fichiers": [],
        "copie": {},
    }) else {
        unreachable!("un objet")
    };
    Ok(Apercu { reponse, a_creer: a_creer_liste, a_completer, contrats, paies, engagements_bulletins: Vec::new(), source: source.to_string() })
}

/// `int(x)` d'une valeur JSON : un entier, un reel, un texte qui se lit comme un entier.
fn entier_py(v: &Value) -> R<i128> {
    match v {
        Value::Number(n) => Ok(n.as_i64().map(i128::from).unwrap_or_else(|| n.as_f64().unwrap_or(0.0) as i128)),
        Value::Bool(b) => Ok(i128::from(*b)),
        Value::String(s) => crate::apercu_dsn::int_py(s)
            .ok_or_else(|| erreur("ValueError", format!("nombre entier illisible : {}", socle::fichiers::repr(s)))),
        autre => Err(erreur(
            "TypeError",
            socle::fichiers::attendu("un nombre entier", nom_type(autre)),
        )),
    }
}
