//! `paie retours [--dpae] [--flux IDFLUX] [--depuis JJ/MM/AAAA] [--jusqu-a JJ/MM/AAAA] [--environnement production|editeur]
//! [--tout] [--appliquer]` : recuperer les retours des organismes. Le releve lui-meme est celui du moteur
//! (`crate::releve::relever_retours`) ; ici ne reste que ce
//! qui s'affiche -- et le traitement du taux de PAS, qui demande un oeil humain.
//!
//! ATTENTION : cette commande va au RESEAU (net-entreprises, ou le service DPAE de l'Urssaf avec `--dpae`). Sur une
//! compagnie fictive, les retours sont joues sans reseau. Une lecture : on archive sans rien demander, mais on
//! n'ECRIT JAMAIS un taux de PAS dans la configuration sans `--appliquer`.
//!
//! LE MOT DE PASSE. Si le coffre est ferme (ni la cle du poste, ni une seance), `Cadre::ouvrir_le_coffre` le demande par
//! `demande::ouvrir_seance` : fenetre (J3), terminal, ou refus avec sa phrase. `--fenetre` est accepte et ne change rien :
//! le moteur choisit lui-meme.

use serde_json::{json, Value};

use super::analyse::{Args, Opt, Spec};
use super::rendu::{champ, debut as debut_de, texte};
use super::Cadre;
use socle::date::Jour;
use envois::releve_retours::{self, Client, Releve};
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "retours",
    positionnels: &[],
    options: &[
        Opt::drapeau("fenetre"),
        Opt::valeur("flux"),
        Opt::drapeau("dpae"),
        Opt::valeur("depuis"),
        Opt::valeur("jusqu-a"),
        Opt::choix("environnement", &["production", "editeur"], Some("production")),
        Opt::drapeau("tout"),
        Opt::drapeau("appliquer"),
        Opt::drapeau("json"),
    ],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) => refus_reseau_ou_plantage(cadre, e),
    }
}

/// Le reseau injoignable ou coupe, ou le coffre qui ne s'ouvre pas : une phrase et un code, pas un plantage.
pub(crate) fn refus_reseau_ou_plantage(cadre: &Cadre, e: Echec) -> i32 {
    if matches!(e.classe, Some("URLError" | "TimeoutError" | "RemoteDisconnected" | "IncompleteRead" | "HTTPException")) {
        dire!("Consultation interrompue : le reseau est injoignable ou a ete coupe ({}).\nRien n'est perdu : relancer la commande quand la connexion est revenue.", e.message);
        return 1;
    }
    // Le coffre ferme et la seance refusee : « rien ici ne l'ouvre » est une phrase pour l'utilisateur, pas un defaut du logiciel.
    // Code 2, comme « le mot de passe n'est nulle part ».
    if e.classe == Some("ErreurCoffre") {
        dire!("{}", e.message);
        return 2;
    }
    cadre.plantage(e)
}

pub(crate) fn ko(m: String) -> Echec {
    Echec { classe: None, message: m }
}

const JOUR: i64 = 86_400;

fn secondes(j: &Jour) -> i64 {
    (j.ordinal() - 719_163) * JOUR
}

/// `_fenetre` : la plage de recherche, en heures locales naives. Par defaut, les 30 derniers jours.
pub(crate) fn fenetre(args: &Args) -> Result<(i64, i64), Echec> {
    let fin = match args.texte("jusqu-a") {
        Some(j) => secondes(&Jour::lire(j).map_err(|m| Echec { classe: Some("ValueError"), message: m })?) + 23 * 3600 + 59 * 60 + 59,
        None => socle::paris::maintenant_naif(),
    };
    let debut = match args.texte("depuis") {
        Some(j) => secondes(&Jour::lire(j).map_err(|m| Echec { classe: Some("ValueError"), message: m })?),
        None => fin - 30 * JOUR,
    };
    Ok((debut, fin))
}

/// Le jour d'une heure locale naive, en `JJ/MM/AAAA`.
pub(crate) fn jour_de(t: i64) -> String {
    Jour::depuis_jours_unix(t.div_euclid(JOUR)).map(|j| j.jj_mm_aaaa()).unwrap_or_default()
}

/// `_date_lisible` : `20260915...` -> `15/09/2026`.
fn date_lisible(h: Option<&Value>) -> String {
    let s = h.filter(|v| socle::fichiers::vrai(v)).map(texte).unwrap_or_default();
    let c: Vec<char> = s.chars().collect();
    if c.len() < 8 {
        return "?".into();
    }
    format!("{}{}/{}{}/{}", c[6], c[7], c[4], c[5], c[..4].iter().collect::<String>())
}

fn nature_de(r: &Value) -> String {
    r.get("nature").map(texte).unwrap_or_else(|| "None".into())
}

const SANS_LISTAGE_PAR_DECLARANT: &str = "
Le listage PAR DECLARANT rend 404 sur le service DPAE : cette adresse etait
deduite de l'API DSN, et l'Urssaf ne documente que le listage PAR FLUX.

  paie retours --dpae --flux IDFLUX

L'idflux est celui que le SI DPAE rend au deposant. Pour une DPAE deposee par
un autre logiciel, il est du cote de ce logiciel ou du tableau de bord Urssaf,
pas ici.
A defaut, la reprise se declare a la main, datee et annotee.

Et le temps compte : les retours DPAE sont purges au bout de TROIS MOIS
(guide NAT/5492, contraintes d'exploitation). Passe ce delai, il n'y a plus
rien a relever.
";

const NATURES_DPAE: [(&str, &str); 4] = [
    ("10", "avis de rejet du depot"),
    ("11", "certificat de conformite / bilan d'anomalies"),
    ("20", "bilan d'identification des salaries"),
    ("41", "certificat de conformite des DPAE"),
];

fn trier_par_publication(trouves: &[Value]) -> Vec<Value> {
    let mut v = trouves.to_vec();
    v.sort_by_key(|x| x.get("publication").filter(|p| socle::fichiers::vrai(p)).map(texte).unwrap_or_default());
    v
}

/// Le canal DPAE. `client` : un client donne (un test le pointe sur un faux serveur local) ; sans lui, le vrai.
pub fn retours_dpae(cadre: &Cadre, args: &Args, client: Option<Client>) -> Result<i32, Echec> {
    let (debut, fin) = fenetre(args)?;
    let flux = args.texte("flux").unwrap_or("");
    let r: Releve = match client {
        Some(c) if flux.is_empty() => crate::releve::relever_retours(&cadre.ctx, debut, fin, args.drapeau("tout"), "production", Some(c), "dpae")?,
        _ => crate::releve::relever_retours_dpae(&cadre.ctx, debut, fin, args.drapeau("tout"), flux)?,
    };
    for i in &r.incidents {
        dire!("{}", i);
    }
    if r.trouves.is_empty() {
        if !flux.is_empty() {
            dire!("Aucun retour pour le flux {}.", flux);
        } else {
            dire!("Aucun retour DPAE sur la periode {} -> {}.", jour_de(debut), jour_de(fin));
            dire!("{}", SANS_LISTAGE_PAR_DECLARANT);
        }
        return Ok(if r.incidents.is_empty() { 0 } else { 1 });
    }
    dire!("{} retour(s) DPAE, {} nouveau(x) :\n", r.trouves.len(), r.nouveaux.len());
    for x in trier_par_publication(&r.trouves) {
        let n = nature_de(&x);
        let libelle = NATURES_DPAE.iter().find(|(c, _)| *c == n).map(|(_, l)| *l).unwrap_or("nature inconnue de ce moteur");
        let publication = x.get("publication").filter(|p| socle::fichiers::vrai(p)).map(texte).unwrap_or_default();
        dire!("  {}  nature {:<3} {:<45} flux {}", debut_de(&publication, 14), n, libelle, champ(&x, "flux"));
    }
    if !r.archives.is_empty() {
        let dossier = envois::transport::dossier_archive(&cadre.racine, "dpae").map_err(|v| ko(texte(&v)))?;
        dire!("\n{} fichier(s) archives dans {}", r.archives.len(), dossier.display());
    }
    Ok(0)
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    if args.drapeau("dpae") {
        cadre.ouvrir_le_coffre();
        return retours_dpae(cadre, args, None);
    }
    // La cle de ce poste peut ne plus ouvrir le coffre (poste d'un agent, sauvegarde restauree) : le mot de passe du
    // logiciel l'ouvre, le temps de la releve.
    cadre.ouvrir_le_coffre();
    let mdp = crate::releve::mot_de_passe_de_consultation(&cadre.ctx, "net-entreprises")?;
    if mdp.is_none() {
        dire!(
            "\nLe mot de passe net-entreprises n'est nulle part : ni dans l'environnement,\nni dans le coffre.\n\nLe poser une fois pour toutes -- chiffre par le compte Windows, jamais\naffiche, et c'est lui qui permet au logiciel de relever les retours seul :\n\n      paie coffre poser net-entreprises\n\nOu, pour cette fenetre seulement, sans rien ecrire :\n\n      $env:{} = (Read-Host \"mot de passe net-entreprises\")\n      paie retours\n",
            releve_retours::VARIABLE_MOTDEPASSE
        );
        return Ok(2);
    }
    derouler(cadre, args, None)
}

/// Le releve des retours DSN et son compte rendu. `client` : un client donne (un test le pointe sur un faux serveur local) ;
/// sans lui, le vrai net-entreprises, avec le mot de passe du coffre.
pub fn derouler(cadre: &Cadre, args: &Args, client: Option<Client>) -> Result<i32, Echec> {
    let environnement = args.texte("environnement").unwrap_or("production");
    let (debut, fin) = fenetre(args)?;
    dire!("Consultation des retours {}, du {} au {}.", environnement, jour_de(debut), jour_de(fin));
    let releve = crate::releve::relever_retours(&cadre.ctx, debut, fin, args.drapeau("tout"), environnement, client, "")?;
    if !releve.session_jusqu_a.is_empty() {
        dire!("Session ouverte (valide jusqu'a {}).\n", releve.session_jusqu_a);
    }
    for incident in &releve.incidents {
        dire!("  {}", incident);
    }
    if releve.trouves.is_empty() {
        if !releve.incidents.is_empty() {
            return Ok(1);
        }
        dire!("Aucun retour sur la periode. Elargir avec --depuis JJ/MM/AAAA.");
        return Ok(0);
    }
    dire!("{} retour(s) sur la periode, dont {} nouveau(x).\n", releve.trouves.len(), releve.nouveaux.len());
    for r in trier_par_publication(&releve.trouves) {
        let marque = if releve.nouveaux.contains(&r) { "NEUF" } else { "    " };
        dire!(
            "  {} {}  nature {:<4} {:<8} {}",
            marque,
            date_lisible(r.get("publication")),
            nature_de(&r),
            champ(&r, "statut"),
            envois::retours::libelle_nature(&nature_de(&r))
        );
    }
    for quoi in &releve.a_faire {
        dire!("\n  A FAIRE : {}", quoi);
    }
    if !releve.archives.is_empty() {
        let dossier = envois::transport::dossier_archive(&cadre.racine, "").map_err(|v| ko(texte(&v)))?;
        dire!("\n{} fichier(s) archive(s) dans {} :", releve.archives.len(), dossier.display());
        for a in &releve.archives {
            dire!("  {}", a.chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
        }
    }

    // ----------------------------------------------- France Travail, spectacle
    let spectacle: Vec<_> = releve.archives.iter().filter(|a| matches!(nature_de(&a.retour).as_str(), envois::retours::NATURE_CRM_SPECTACLE | envois::retours::NATURE_AER_SPECTACLE)).collect();
    if !spectacle.is_empty() {
        dire!("\nFrance Travail, cinema-spectacle :");
    }
    for a in spectacle {
        let nom = a.chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if nature_de(&a.retour) == envois::retours::NATURE_CRM_SPECTACLE {
            dire!("  compte-rendu metier recu ({}) : nos contrats spectacle ont ete pris en compte.", nom);
            continue;
        }
        let att: Vec<Value> = envois::retours::aer_spectacle(&a.contenu).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
        if att.is_empty() {
            dire!("  {} : archive illisible. Le fichier brut est garde, on peut recommencer.", nom);
            continue;
        }
        let mut periodes: Vec<String> = att.iter().filter_map(|x| x.get("periode").filter(|p| socle::fichiers::vrai(p)).map(texte)).collect();
        periodes.sort();
        periodes.dedup();
        dire!("  {} attestation(s) AER, periode {} ({})", att.len(), if periodes.is_empty() { "non lue".to_string() } else { periodes.join(", ") }, nom);
        dire!("    les ranger pour l'envoi :  paie aer");
    }

    // ------------------------------------------------------------ taux de PAS
    let crm: Vec<_> = releve.archives.iter().filter(|a| nature_de(&a.retour) == envois::retours::NATURE_TAUX_PAS).collect();
    if crm.is_empty() {
        dire!("\nAucun compte-rendu DGFiP de nature 94 (taux du PAS) dans ce lot.\nC'est le retour le plus tardif de la chaine : il arrive apres l'accuse, le\ncertificat de conformite et les bilans. Relancer quelques jours apres un depot.\n");
        return Ok(0);
    }
    traiter_crm_pas(cadre, args, &crm)
}

fn traiter_crm_pas(cadre: &Cadre, args: &Args, crm: &[&releve_retours::RetourArchive]) -> Result<i32, Echec> {
    let nirs: Vec<Value> = cadre
        .ctx
        .salaries
        .iter()
        .filter(|(_, s)| s.get("nir").is_some_and(socle::fichiers::vrai))
        .map(|(cle, s)| json!([cle, s["nir"]]))
        .collect();
    let mut tous: Vec<Value> = Vec::new();
    let mut illisible = false;
    for a in crm {
        let nom = a.chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        // Un CRM illisible : une phrase, le fichier brut reste archive, code 1.
        let res = match envois::retours::taux_pas_d_octets(&a.contenu) {
            Ok(r) => r,
            Err(e) => {
                dire!("\nCRM DGFiP du {} ({})", date_lisible(a.retour.get("publication")), nom);
                dire!("  Ce fichier est illisible ({}).\n  Le fichier brut est garde tel quel, on peut recommencer.", e.message);
                illisible = true;
                continue;
            }
        };
        dire!("\nCRM DGFiP du {} ({})", date_lisible(a.retour.get("publication")), nom);
        let releves: Vec<Value> = res.get("releves").and_then(Value::as_array).cloned().unwrap_or_default();
        if releves.is_empty() {
            dire!(
                "  Aucun taux reconnu dans ce fichier.\n\n  Le format harmonise des retours n'a jamais ete confronte a un vrai CRM 94 :\n  c'est attendu, et sans gravite. Le fichier est archive tel quel. M'envoyer\n  {} et j'ajuste l'extraction dessus.",
                nom
            );
            continue;
        }
        dire!("  Reconnu par : {}", champ(&res, "strategie"));
        let rr = envois::retours::releves_vers_salaries(&releves, &nirs);
        for o in rr.get("orphelins").and_then(Value::as_array).cloned().unwrap_or_default() {
            dire!("  NIR non rattache a une fiche salarie : {}", champ(&o, "nir"));
        }
        let rattaches: Vec<Value> = rr.get("rattaches").and_then(Value::as_array).cloned().unwrap_or_default();
        for x in &rattaches {
            dire!(
                "  {:<12} taux {}  type {} ({})  identifiant {}",
                champ(x, "salarie"),
                champ(x, "taux"),
                champ(x, "type"),
                super::pas::libelle_type(&champ(x, "type")),
                champ(x, "identifiant")
            );
        }
        let production = a.retour.get("production").filter(|p| socle::fichiers::vrai(p)).map(texte).unwrap_or_default();
        for x in rattaches {
            let mut m = x.as_object().cloned().unwrap_or_default();
            m.insert("periode".into(), json!(debut_de(&production, 6)));
            m.insert("mis_a_disposition".into(), a.retour.get("publication").cloned().unwrap_or(Value::Null));
            tous.push(Value::Object(m));
        }
    }
    let code = i32::from(illisible);
    if tous.is_empty() {
        return Ok(code);
    }
    if !args.drapeau("appliquer") {
        dire!("\nRien n'a ete ecrit. Verifier les valeurs ci-dessus, puis :\n    paie retours --appliquer\n");
        return Ok(code);
    }
    let chgts = super::pas::appliquer(&cadre.ctx, &tous, "CRM DGFiP nature 94")?;
    if !chgts.is_empty() {
        dire!("\nconfig/pas.json mis a jour :");
        for c in chgts {
            dire!("  {}", c);
        }
    } else {
        dire!("\nconfig/pas.json inchange : les valeurs recues sont deja celles connues.");
    }
    Ok(code)
}
