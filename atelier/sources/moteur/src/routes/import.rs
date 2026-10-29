//! Les routes de l'import du passe d'une compagnie : ce qu'il ferait, rien d'ecrit (`importation-apercu`), ou en est
//! la boite aux lettres (`importation-etat`), l'import lui-meme (`importer`, `import/ecrite.rs`) et la nature d'un
//! contrat importe (`importation-nature`).
//!
//! `importation-apercu` lit la boite aux lettres `import/` entiere (`{"dossier": ...}` en nomme une autre ;
//! `import::bulletins`) : le tri, les DSN et AEM qu'il trouve (`import/dsn_aem.rs`, par `apercu_lu`), puis
//! les bulletins de paie.
//!
//! `contrats_nouveaux` : un contrat atteste que l'historique (les engagements importes) porte deja
//! -- meme salarie, memes dates, meme objet, memes heures, cachets, jours et meme brut (`str`) -- n'est
//! pas nouveau (`_cle_engagement`).

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::import::dsn_aem::{self, SalarieDeclare};
use droit::contexte::Contexte;
use envois::etats::{self, get};
use socle::echec::{erreur, Echec, R};
use socle::fichiers::vrai;
use socle::valeur::{nom_type, valeur_en_texte};



/// Les routes de ce module.
pub const ROUTES: &[&str] = &["importation-apercu", "importation-etat", "importer", crate::import::nature::ROUTE];

/// Celles qui ecrivent.
pub const ECRITURES: &[&str] = &["importer", crate::import::nature::ROUTE];

/// UNE REPONSE DE L'IMPORT, AVEC SES PASTILLES (`routes::pastille`) : celle de chaque sort
/// (`import::bulletins::pastilles_des_sorts`), celle de chaque fiche que l'import cree, complete ou laisse
/// (`fiche_importee` : creee, completee ou deja la, faite ; a creer ou a completer, a faire), celle de chaque ecart entre
/// un bulletin et une DSN (le bulletin l'emporte, a voir ; sinon note, une information).
pub(crate) fn avec_les_pastilles(mut v: Value) -> Value {
    poser_les_pastilles(&mut v);
    if let Some(o) = v.as_object_mut() {
        o.insert("pastilles".into(), crate::import::bulletins::pastilles_des_sorts());
    }
    v
}

fn poser_les_pastilles(v: &mut Value) {
    match v {
        Value::Object(o) => {
            let fiche = ["salarie", "spectacle", "entreprise"].contains(&o.get("nom").and_then(Value::as_str).unwrap_or("")) && o.contains_key("libelle");
            if fiche {
                let a_completer = o.get("completer").and_then(Value::as_object).is_some_and(|c| !c.is_empty());
                let p = match o.get("etat").and_then(Value::as_str).unwrap_or("") {
                    "completee" => super::pastille("fait", "complétée"),
                    "existante" if a_completer => super::pastille("a_faire", "à compléter"),
                    "existante" => super::pastille("fait", "existe déjà"),
                    "creee" => super::pastille("fait", "créée"),
                    _ => super::pastille("a_faire", "à créer"),
                };
                o.insert("pastille".into(), p);
            }
            if let Some(l) = o.get("le_bulletin_l_emporte").map(vrai) {
                o.insert("pastille".into(), if l { super::pastille("a_faire", "le bulletin l'emporte") } else { super::pastille("information", "noté") });
            }
            for x in o.values_mut() {
                poser_les_pastilles(x);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(poser_les_pastilles),
        _ => {}
    }
}

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        // `{"dossier": ...}` (vide ou absent : `import/` de la compagnie) : la boite aux lettres entiere, bulletins
        // compris (`import::bulletins`).
        "importation-apercu" => Ok(avec_les_pastilles(Value::Object(crate::import::bulletins::apercu_dossier(ctx, &dossier_import(ctx, charge)?)?.reponse))),
        "importation-etat" => etat_importation(&ctx.racine),
        "importer" => crate::import::ecrite::importer(ctx, charge).map(avec_les_pastilles),
        // La nature des contrats repris qu'aucun bulletin ne dit.
        crate::import::nature::ROUTE => crate::import::nature::dire(ctx, charge),
        autre => Err(Echec { classe: None, message: format!("route inconnue : {}", autre) }),
    }
}

/// Le dossier a trier : `charge.dossier`, ou `import/` a la racine de la compagnie s'il est vide ou absent.
///
/// LA BOITE AUX LETTRES DE LA COMPAGNIE SE CREE VIDE : `import/` qui n'existe pas encore est
/// cree, et l'apercu (ou l'import) n'a rien a reprendre, au lieu de refuser « dossier absent ». Un dossier DONNE
/// (`charge.dossier`) qui n'existe pas reste refuse (`import::tri::trier`).
pub(crate) fn dossier_import(ctx: &Contexte, charge: &Value) -> R<PathBuf> {
    Ok(match charge.get("dossier").filter(|v| vrai(v)) {
        None => {
            let d = ctx.racine.join("import");
            if !d.exists() {
                std::fs::create_dir_all(&d).map_err(|e| erreur("OSError", format!("{} : {}", d.display(), e)))?;
            }
            d
        }
        Some(Value::String(s)) => PathBuf::from(crate::routes::depots::normaliser(s)),
        Some(autre) => return Err(erreur("TypeError", socle::fichiers::attendu("un chemin en texte", nom_type(autre)))),
    })
}

/// Un dossier de travail temporaire du systeme, propre a cet appel (a effacer avant de rendre).
pub(crate) fn dossier_de_travail() -> PathBuf {
    let marque = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    std::env::temp_dir().join(format!("puppetpaie-tri-{}-{}", std::process::id(), marque))
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
        Some(x) => declarations::champs::titre(&valeur_en_texte(x)),
        None => String::new(),
    }
}

/// `_ecarts` : les champs DECLARES que la DSN disait autrement que la fiche.
fn ecarts(nom: &str, fiche: &Value, declare: &Map<String, Value>, mois: &str) -> R<Value> {
    let mut out = Map::new();
    for c in declarations::champs::fiche(nom)? {
        if !c.declare || !declare.contains_key(&c.cle) {
            continue;
        }
        let a = fiche.get(&c.cle).unwrap_or(&Value::Null);
        let b = &declare[&c.cle];
        // Une case vide ou « A COMPLETER » n'est pas un ecart : l'import la complete (`import::completer`).
        if crate::import::completer::vide(Some(a)) || valeur_en_texte(a) == valeur_en_texte(b) {
            continue;
        }
        // Une structure (la banque) ne differe que par ce que la DSN en declare : ses notes et autres cles restent.
        if let (Value::Object(x), Value::Object(y)) = (a, b) {
            if y.iter().all(|(k, v)| x.get(k).map(valeur_en_texte) == Some(valeur_en_texte(v))) {
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
    f.insert("nir_cle".into(), json!(declarations::champs::cle_nir(&decl.nir).unwrap_or_default()));
    f.insert("nir".into(), json!(decl.nir));
    if !decl.initiales.is_empty() {
        f.insert("initiales_contrat".into(), json!(decl.initiales));
    }
    if !decl.premier_contrat.is_empty() {
        f.insert("date_anciennete".into(), json!(decl.premier_contrat));
    }
    // Le statut de bareme se deduit du code emploi France Travail, a defaut de la PCS (354x : artistes).
    let code = f.get("code_emploi_ft").map_or(String::new(), valeur_en_texte);
    let m = droit::emplois::metier(&code)?.unwrap_or_else(|| json!({}));
    let statut = match m.get("statut").filter(|v| vrai(v)) {
        Some(s) => s.clone(),
        None => json!(if f.get("pcs_ese").map_or(String::new(), valeur_en_texte).starts_with("354") { "artiste" } else { "technicien" }),
    };
    f.insert("statut".into(), statut);
    let mut f = declarations::champs::combler_affichage("salarie", &f)?;
    f.insert("actif".into(), json!(true));
    Ok(f)
}

/// La fiche Entreprise que les DSN donnent : ce qu'elles declarent de l'entreprise, son Siret, la suite de ses licences.
fn fiche_entreprise(lecture: &dsn_aem::LectureDsn) -> R<Map<String, Value>> {
    let mut e = lecture.entreprise.clone();
    let (siren, nic) = (e.get("siren").filter(|v| vrai(v)).cloned(), e.get("nic").filter(|v| vrai(v)).cloned());
    if let (Some(s), Some(n)) = (siren, nic) {
        e.insert("siret".into(), json!(format!("{}{}", valeur_en_texte(&s), valeur_en_texte(&n))));
    }
    if !lecture.licences.is_empty() {
        let l: Vec<Value> = lecture.licences.iter().map(|(m, n)| json!({"depuis": format!("{}-01", m), "numero": n})).collect();
        e.insert("licence_spectacle".into(), Value::Array(l));
    }
    let mut e = declarations::champs::combler_affichage("entreprise", &e)?;
    if e.get("raison_sociale").is_some_and(vrai) && !e.get("nom_affichage").is_some_and(vrai) {
        let t = titre_de(e.get("raison_sociale"));
        e.insert("nom_affichage".into(), json!(t));
    }
    Ok(e)
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
    /// Les fiches existantes que l'import completerait (`import::completer`).
    pub(crate) a_completer: Vec<crate::import::completer::ACompleter>,
    pub(crate) contrats: Vec<dsn_aem::ContratAtteste>,
    /// Les versements declares par les DSN, rattaches a une fiche (ou `<a creer>`).
    pub(crate) paies: Vec<Value>,
    /// Les contrats que les bulletins attestent (`import::bulletins`), contrats d'engagements importes
    /// pretes a ecrire, chacune avec le NIR qui la rattache (`salarie` vaut `<a creer>` tant que la fiche n'existe pas).
    pub(crate) engagements_bulletins: Vec<(String, Value)>,
}

/// LE NOM QUE LA PERSONNE A DONNE A LA COMPAGNIE (« Ajouter une compagnie ») reste le sien : la fiche que l'import cree
/// garde son nom et sa raison sociale, que les pieces ecrivent en capitales (S21.G00.06.003) ; changer le nom d'une
/// fiche demande une date d'effet que l'import ne devine pas.
pub(crate) fn garder_le_nom(ctx: &Contexte, e: &mut Map<String, Value>) {
    for k in ["nom_affichage", "raison_sociale"] {
        if let Some(v) = ctx.entreprise.get(k).filter(|v| vrai(v) && !valeur_en_texte(v).trim().is_empty()) {
            e.insert(k.into(), v.clone());
        }
    }
}

/// La cle `_cle_engagement` : (salarie, debut, fin, numero_objet, heures, cachets, jours, str(brut)).
pub(crate) type CleEngagement = (String, String, String, String, i128, i128, i128, String);

/// `deja` : les contrats que les engagements importes attestent deja (`engagement::importe::contrats_lus`). Une valeur
/// qui n'est pas un texte ne peut egaler un texte : l'entree ne retrouvera aucun contrat.
pub(crate) fn cles_historique(ctx: &Contexte) -> R<HashSet<CleEngagement>> {
    let engagements = Value::Array(engagement::importe::contrats_lus(&ctx.racine).map_err(Echec::from)?);
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
            nombres[i] = entier_de(e.get(*k).unwrap_or(&json!(0)))?;
        }
        let texte = |v: Option<&Value>| match v {
            Some(Value::String(t)) => Some(t.clone()),
            _ => None,
        };
        let objet = match e.get("numero_objet") {
            None => Some(String::new()),
            autre => texte(autre),
        };
        let brut = e.get("brut").map_or("0".to_string(), valeur_en_texte);
        if let (Some(a), Some(b), Some(c), Some(d)) = (texte(e.get("salarie")), texte(e.get("debut")), texte(e.get("fin")), objet) {
            deja.insert((a, b, c, d, nombres[0], nombres[1], nombres[2], brut));
        }
    }
    Ok(deja)
}

/// Les paies versees des engagements importes, une ligne par mois (`engagement::importe::paies_lues`).
pub(crate) fn paies_historique(ctx: &Contexte) -> R<Vec<Value>> {
    engagement::importe::paies_lues(&ctx.racine).map_err(Echec::from)
}

/// La cle d'une paie de l'historique : (mois, salarie, contrat).
pub(crate) fn cle_paie(x: &Value) -> R<(String, String, String)> {
    let k = |c: &str| -> R<String> { x.get(c).map(valeur_en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(c))) };
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
        Some(d) => dsn_aem::lire_dsn(d),
        None => Default::default(),
    };
    let mut avertissements: Vec<String> = lecture.illisibles.iter().map(|f| format!("DSN illisible, ignoree : {}", f)).collect();
    // LA FICHE NE PORTE QU'UN ETABLISSEMENT (celui de son NIC, `champs-base.json`, `etablissements`) : des DSN qui en
    // declarent plusieurs, ou dont le siege n'est pas l'etablissement declare, le disent.
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
    let mut a_completer: Vec<crate::import::completer::ACompleter> = Vec::new();
    let origine_dsn = |mois: &str| json!({"source": "dsn", "mois": mois});
    if !lecture.entreprise.is_empty() {
        let mut e = fiche_entreprise(&lecture)?;
        garder_le_nom(ctx, &mut e);
        if !ctx.entreprise.get("siren").is_some_and(|s| vrai(s) && !valeur_en_texte(s).trim().is_empty()) {
            let anomalies = crate::routes::fiches::verifier_comme_a_l_ecriture("entreprise", &e)?;
            let libelle = e.get("raison_sociale").map_or("?".to_string(), valeur_en_texte);
            a_creer_liste.push(ACreer { nom: "entreprise", rang: None, donnees: e.clone() });
            entreprise = fiche_importee("entreprise", None, &libelle, "a_creer", json!({}), anomalies, Value::Object(e));
            a_creer += 1;
        } else {
            let libelle = ctx.entreprise.get("raison_sociale").map_or(String::new(), valeur_en_texte);
            let ec = ecarts("entreprise", &ctx.entreprise, &lecture.entreprise, &lecture.dernier_mois)?;
            entreprise = fiche_importee("entreprise", None, &libelle, "existante", ec, Vec::new(), json!({}));
            let champs = crate::import::completer::champs_a_completer("entreprise", &ctx.entreprise, &e)?;
            crate::import::completer::noter(&mut entreprise, &champs, origine_dsn(&lecture.dernier_mois));
            crate::import::completer::ajouter(&mut a_completer, "entreprise", None, &champs);
        }
    }

    // 2. Les salaries, par NIR.
    let mut par_nir: HashMap<String, String> = HashMap::new();
    for (cle, s) in &ctx.salaries {
        if let Some(n) = s.get("nir").filter(|v| vrai(v)) {
            par_nir.insert(valeur_en_texte(n), cle.clone());
        }
    }
    let mut ordre: Vec<&SalarieDeclare> = lecture.salaries.iter().collect();
    ordre.sort_by(|a, b| a.mois.cmp(&b.mois));
    let mut salaries = Vec::new();
    let mut cles_a_venir: HashSet<String> = HashSet::new();
    for decl in ordre {
        let libelle = format!(
            "{} {}",
            decl.fiche.get("nom").map_or(String::new(), valeur_en_texte),
            titre_de(decl.fiche.get("prenom"))
        );
        if let Some(cle) = par_nir.get(&decl.nir) {
            let ec = ecarts("salarie", &ctx.salaries[cle], &decl.fiche, &decl.mois)?;
            let mut entree = fiche_importee("salarie", Some(cle), &libelle, "existante", ec, Vec::new(), json!({}));
            let champs = crate::import::completer::champs_a_completer("salarie", &ctx.salaries[cle], &fiche_salarie(decl)?)?;
            crate::import::completer::noter(&mut entree, &champs, origine_dsn(&decl.mois));
            crate::import::completer::ajouter(&mut a_completer, "salarie", Some(cle), &champs);
            salaries.push(entree);
            continue;
        }
        let donnees = fiche_salarie(decl)?;
        let anomalies = crate::routes::fiches::verifier_comme_a_l_ecriture("salarie", &donnees)?;
        a_creer_liste.push(ACreer { nom: "salarie", rang: Some(salaries.len()), donnees: donnees.clone() });
        salaries.push(fiche_importee("salarie", None, &libelle, "a_creer", json!({}), anomalies, Value::Object(donnees)));
        cles_a_venir.insert(decl.nir.clone());
        a_creer += 1;
    }

    cles_a_venir.extend(a_venir.iter().filter(|n| !par_nir.contains_key(*n)).cloned());

    // 3. Les spectacles, par numero d'objet -- ceux des DSN et ceux des AEM.
    let (contrats, illisibles) = match &aem {
        Some(d) => dsn_aem::lire_aem(d)?,
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
        let n = sp.get("numero_objet").map_or(String::new(), valeur_en_texte).to_uppercase();
        connus.insert(n, cle.clone());
    }
    objets.sort_by(|a, b| a.premier.cmp(&b.premier));
    let seuil = format!("{:04}-{:02}", aujourdhui.an - 1, aujourdhui.mois);
    let mut spectacles = Vec::new();
    for o in &objets {
        if let Some(cle) = connus.get(&o.numero.to_uppercase()) {
            let titre = ctx.spectacles[cle].get("titre").map_or("?".to_string(), valeur_en_texte);
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
        let anomalies = crate::routes::fiches::verifier_comme_a_l_ecriture("spectacle", &donnees)?;
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
                let k = (nir.clone(), valeur_en_texte(&texte("debut")), valeur_en_texte(&texte("fin")));
                json!(objets_aem.get(&k).cloned().unwrap_or_default())
            }
        };
        paies.push(json!({
            "mois": mois, "salarie": cle.cloned().unwrap_or_else(|| "<a creer>".to_string()), "nir": nir,
            "contrat": contrat, "numero_objet": objet,
            "debut": texte("debut"), "fin": texte("fin"), "quotite": texte("quotite"),
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
    Ok(Apercu { reponse, a_creer: a_creer_liste, a_completer, contrats, paies, engagements_bulletins: Vec::new() })
}

/// `int(x)` d'une valeur JSON : un entier, un reel, un texte qui se lit comme un entier.
fn entier_de(v: &Value) -> R<i128> {
    match v {
        Value::Number(n) => Ok(n.as_i64().map(i128::from).unwrap_or_else(|| n.as_f64().unwrap_or(0.0) as i128)),
        Value::Bool(b) => Ok(i128::from(*b)),
        Value::String(s) => socle::num::entier_lu(s)
            .ok_or_else(|| erreur("ValueError", format!("nombre entier illisible : {}", socle::fichiers::repr(s)))),
        autre => Err(erreur(
            "TypeError",
            socle::fichiers::attendu("un nombre entier", nom_type(autre)),
        )),
    }
}

/// Le nombre de valeurs distinctes (`len({...})`) : `1`, `1.0` et `True` ne font
/// qu'une ; une liste ou un dictionnaire ne se hache pas.
fn distincts(engagements: &[&Value], cle: &str) -> R<usize> {
    let mut vus: HashSet<String> = HashSet::new();
    for e in engagements {
        let v = get(e, cle)?;
        if !vrai(v) {
            continue;
        }
        vus.insert(match v {
            Value::String(t) => format!("s{}", t),
            Value::Array(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("list"))),
            Value::Object(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("dict"))),
            autre => format!("n{:?}", socle::valeur::nombre(autre).unwrap_or(0.0)),
        });
    }
    Ok(vus.len())
}

/// `etat_importation` : ce qui a deja ete repris, d'apres les engagements importes, pour que la page le dise :
/// - `importe_le`, `contrats`, `salaries`, `objets`, `du`, `au` : les contrats attestes ;
/// - `dossier` : la boite aux lettres `import/` de la compagnie, que la page preremplit ;
/// - `reprises` : chaque source reprise -- `DSN` (les versements declares par une DSN ; `declarations` : une DSN par
///   mois declare), `AEM` (les contrats attestes par une AEM), `bulletins` (les paies qu'un bulletin a ecrites ou
///   completees, les contrats qu'il atteste, les PDF copies dans `historique/bulletins/`) --, avec combien, de quand a
///   quand, et le jour de la reprise ;
/// - `editeur` : ce qui attend l'editeur -- les incidents et les signalements partis (ou a partir) de ce poste qui
///   parlent de l'import, sans valeur personnelle (ils ont ete caviardes avant d'etre ecrits) ; `attend_editeur` en
///   compte ceux qui n'ont pas de reponse.
///
/// Ces ajouts ne levent jamais : une ligne qui n'a pas la forme attendue n'est pas comptee.
fn etat_importation(racine: &Path) -> R<Value> {
    let contrats = engagement::importe::contrats_lus(racine).map_err(Echec::from)?;
    let paies = engagement::importe::paies_lues(racine).map_err(Echec::from)?;
    let engagements: Vec<&Value> = contrats.iter().collect();
    let dernier = |k: &str| contrats.iter().chain(paies.iter()).filter_map(|x| x.get(k).filter(|v| vrai(v)).map(valeur_en_texte)).max().unwrap_or_default();
    let dates = |cle: &str| -> R<Vec<String>> {
        let mut v = Vec::new();
        for e in &engagements {
            let x = get(e, cle)?;
            if vrai(x) {
                v.push(valeur_en_texte(x));
            }
        }
        v.sort();
        Ok(v)
    };
    let debuts = dates("debut")?;
    let fins = dates("fin")?;
    let editeur = attend_l_editeur(racine);
    let attend = editeur.iter().filter(|x| !x.get("repondu").is_some_and(vrai)).count();
    Ok(json!({
        "importe_le": dernier("importe_le"),
        "contrats": engagements.len(),
        "salaries": distincts(&engagements, "salarie")?,
        "objets": distincts(&engagements, "numero_objet")?,
        "du": debuts.first().cloned().unwrap_or_default(),
        "au": fins.last().cloned().unwrap_or_default(),
        "dossier": racine.join("import").display().to_string(),
        "reprises": reprises_faites(racine, &engagements, &paies),
        "editeur": editeur,
        // La pastille de l'historique repris.
        "pastille_repris": super::pastille("fait", "fait"),
        "attend_editeur": attend,
        // Les contrats repris dont la nature reste a dire, par salarie.
        "a_dire": crate::import::nature::a_dire(racine),
        // Les permanents dont le contrat continue et dont la fiche ne dit pas les conges a la reprise.
        "conges_a_dire": crate::import::nature::conges_a_dire(racine),
    }))
}

/// Une source reprise : combien, de quand a quand, quel jour.
#[derive(Default)]
struct Reprise {
    nombre: usize,
    du: Option<String>,
    au: Option<String>,
    le: Option<String>,
}

/// `reprises` de `importation-etat` : DSN, AEM, bulletins, dans cet ordre, celles qui ont repris quelque chose.
fn reprises_faites(racine: &Path, engagements: &[&Value], paies: &[Value]) -> Vec<Value> {
    let le_de = |o: &Map<String, Value>| o.get("importe_le").filter(|v| v.as_str().is_some_and(|s| !s.is_empty())).cloned();
    let (mut aem, mut contrats_bulletins) = (Reprise::default(), Reprise::default());
    for e in engagements {
        let Some(o) = e.as_object() else { continue };
        let le = le_de(o);
        let cible = if o.get("atteste_par").and_then(Value::as_str) == Some("bulletin") { &mut contrats_bulletins } else { &mut aem };
        cible.voir(o.get("debut"), o.get("fin"), le.as_ref());
    }
    let (mut dsn, mut bulletins) = (Reprise::default(), Reprise::default());
    // Les DSN reprises : une par mois declare (la phrase de l'etat dit des declarations, pas des versements).
    let mut mois_dsn: BTreeSet<String> = BTreeSet::new();
    {
        for x in paies {
            let Some(o) = x.as_object() else { continue };
            let le = le_de(o);
            let le = le.as_ref();
            let par = o.get("declare_par").and_then(Value::as_str).unwrap_or("");
            // Une paie declaree par une DSN et completee par un bulletin compte des deux cotes : elle a les deux sources.
            if par != "bulletin" {
                dsn.voir(o.get("mois"), o.get("mois"), le);
                if let Some(m) = o.get("mois").and_then(Value::as_str).filter(|m| !m.is_empty()) {
                    mois_dsn.insert(m.to_string());
                }
            }
            if par == "bulletin" || o.get("bulletin").is_some_and(vrai) {
                bulletins.voir(o.get("mois"), o.get("mois"), le);
            }
        }
    }
    let copies = std::fs::read_dir(racine.join("historique").join("bulletins"))
        .map(|d| d.flatten().filter(|e| e.path().is_file()).count())
        .unwrap_or(0);
    let mut out = Vec::new();
    let base = |source: &str, quoi: &str, r: &Reprise| {
        json!({"source": source, "quoi": quoi, "nombre": r.nombre, "du": r.du.clone().unwrap_or_default(),
               "au": r.au.clone().unwrap_or_default(), "le": r.le.clone().unwrap_or_default()})
    };
    if dsn.nombre > 0 {
        let mut d = base("DSN", "versements déclarés", &dsn);
        d["declarations"] = json!(mois_dsn.len());
        out.push(d);
    }
    if aem.nombre > 0 {
        out.push(base("AEM", "contrats attestés", &aem));
    }
    if bulletins.nombre > 0 || contrats_bulletins.nombre > 0 || copies > 0 {
        let mut b = base("bulletins", "bulletins de paie", &bulletins);
        if b["le"] == json!("") {
            b["le"] = json!(contrats_bulletins.le.clone().unwrap_or_default());
        }
        b["contrats"] = json!(contrats_bulletins.nombre);
        b["copies"] = json!(copies);
        out.push(b);
    }
    out
}

/// `editeur` de `importation-etat` : les incidents (`journal/incidents.json`) et les signalements
/// (`journal/signalements.json`) de ce poste qui parlent de l'import -- un incident de nature `refus` n'en est pas : la
/// phrase a ete dite a l'ecran. Rien que ce qu'ils portent deja (caviarde).
fn attend_l_editeur(racine: &Path) -> Vec<Value> {
    let parle_d_import = |t: &str| {
        let t = t.to_lowercase();
        t.contains("import") || t.contains("repris")
    };
    let texte = |v: &Value, cle: &str| v.get(cle).and_then(Value::as_str).unwrap_or("").to_string();
    let mut out = Vec::new();
    for l in crate::compagnie::incidents::lire(racine, None, false).unwrap_or_default() {
        let (ou_, genre, resume) = (texte(&l, "ou"), texte(&l, "genre"), texte(&l, "resume"));
        // Les ecarts de calcul de l'import sont l'etude de l'editeur (`import::ecarts`) : rien n'y attend la compagnie.
        if ou_ == crate::import::ecarts::OU {
            continue;
        }
        // Un refus (un dossier mal tape, un export absent) se dit a l'ecran ; il n'attend pas l'editeur.
        if l.get("quoi").and_then(Value::as_str) == Some(crate::compagnie::incidents::REFUS) || !parle_d_import(&format!("{} {}", ou_, genre)) {
            continue;
        }
        let etat = l.get("envoi").map_or(String::new(), |e| texte(e, "etat"));
        if etat == crate::compagnie::incidents::SANS_OBJET {
            continue;
        }
        out.push(json!({
            "canal": "incident", "quand": texte(&l, "quand"),
            "resume": if resume.is_empty() { genre } else { resume },
            "parti": etat == crate::compagnie::incidents::DEPOSE, "repondu": false,
        }));
    }
    for s in crate::compagnie::problemes::signalements(racine) {
        let debut = texte(&s, "debut");
        if !parle_d_import(&debut) {
            continue;
        }
        out.push(json!({
            "canal": "signalement", "quand": texte(&s, "quand"), "resume": debut,
            "numero": s.get("numero").cloned().unwrap_or(Value::Null),
            "parti": s.get("etat").and_then(Value::as_str) == Some("arrive"), "repondu": s.get("reponse").is_some_and(vrai),
        }));
    }
    // Sa pastille : la reponse recue, faite ; le reste attend l'editeur, rien que la compagnie ait a faire.
    for x in out.iter_mut() {
        x["pastille"] = if x["repondu"] == true {
            super::pastille("fait", "réponse reçue")
        } else if x["parti"] == true {
            super::pastille("information", "attend l'éditeur")
        } else {
            super::pastille("information", "pas encore parti")
        };
    }
    out
}

impl Reprise {
    /// Une ligne de plus : ses dates (un texte non vide), et le jour ou elle a ete reprise.
    fn voir(&mut self, debut: Option<&Value>, fin: Option<&Value>, le: Option<&Value>) {
        let texte = |v: Option<&Value>| v.and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string);
        self.nombre += 1;
        if let Some(d) = texte(debut) {
            if self.du.as_ref().is_none_or(|x| &d < x) {
                self.du = Some(d);
            }
        }
        if let Some(f) = texte(fin) {
            if self.au.as_ref().is_none_or(|x| &f > x) {
                self.au = Some(f);
            }
        }
        if let Some(l) = texte(le) {
            if self.le.as_ref().is_none_or(|x| &l > x) {
                self.le = Some(l);
            }
        }
    }
}
