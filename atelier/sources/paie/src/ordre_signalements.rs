//! Le numero d'ordre (S20.G00.05.004) du prochain signalement, LA PLATE-FORME FAISANT FOI (lot O1, decision de Leandre
//! du 14/10/2026 : pas de regle « commiter le journal » a la place).
//!
//! POURQUOI. Le journal d'un dossier (`journal/depots.json`) ne connait que ce que CE dossier a depose ; la plate-forme se
//! souvient de tout, par SIRET, essais compris. Cahier technique DSN 2026.1, §1.4.3.2, p. 14 : « Quand un signalement
//! d'evenement de type normal a ete accepte pour un SIRET employeur et un numero d'ordre de la declaration donne, il n'est
//! pas possible d'emettre un deuxieme signalement d'evenement de type normal avec le meme SIRET employeur et le meme
//! numero d'ordre » ; p. 136 : pour un signalement, l'ordre « ne doit pas etre remis a zero afin qu'il soit unique quel
//! que soit le mois d'emission ». Le 14/10/2026, deux normales d'essai a l'ordre 1, parties d'une copie dont le journal
//! etait vide, ont pris B1-208-10 : l'ordre 1 etait deja pris dans la base d'essai (PASSATION-O1, point 1).
//!
//! CE QUE LA PLATE-FORME DIT. Seul le bilan de nature 11 (certificat ou bilan d'anomalies) porte l'ordre, avec la nature,
//! le type, le SIREN ; ni salarie, ni contrat (`retours::declarations_du_bilan`). Les documents se purgent a TROIS MOIS
//! (guide d'implementation de l'API DSN v6.8, §8.2, p. 93, `atelier/reference/sources/gip-mds-api-dsn/`) : on releve donc
//! les trois derniers mois, et ce qui est plus ancien n'est plus que dans nos archives (`retours/`) et le journal.
//!
//! LE PLUS GRAND DES DEUX, ESSAIS ET REELS ENSEMBLE. Un signalement s'ecrit une fois et part en test puis en reel
//! (la repetition, `depot_acte::exiger_repetition`) au meme ordre ; la base d'essai et la base reelle sont distinctes
//! (archives §17.6). L'ordre doit donc etre libre dans les deux : on prend le plus grand de tous. Qu'une sequence reelle
//! saute des numeros, aucun texte ne l'interdit : p. 15, « Il ne sera pas possible d'integrer une annule et remplace de
//! rang inferieur au precedent envoi », et le rang se compare (B1-208-11, « inferieur (ou egal) au dernier ordre
//! accepte ») ; trop grand ne coute rien, trop petit est refuse (INFERENCE de ces deux textes, ecrite comme telle dans
//! PASSATION-O1). Tous SIRET de l'entreprise confondus, comme le journal le faisait : trop grand ne coute rien.
//!
//! SANS RESEAU, OU SANS MOT DE PASSE, le journal et les bilans deja archives decident, et l'avertissement le dit : jamais
//! en silence. Une compagnie fictive n'a pas de plate-forme : ses envois sont joues, son journal est la seule
//! memoire, rien n'est a dire.

use std::path::Path;

use serde_json::Value;

use droit::contexte::Contexte;
use socle::valeur::valeur_en_texte;
use socle::echec::erreur;
use socle::fichiers::vrai;
use crate::releve_retours::{self, Client};
use socle::echec::Echec;

/// Trois mois de releve : la retention des documents de la plate-forme (guide API v6.8, §8.2). 92 jours couvrent trois
/// mois civils, quels qu'ils soient.
pub const JOURS_DE_RELEVE: i64 = 92;

/// Ce que le moteur sait du dernier ordre pris, et ce qu'il doit en dire.
#[derive(Debug, Clone, Default)]
pub struct OrdreConnu {
    /// Le plus grand ordre de signalement connu, 0 si aucun : le prochain signalement part au suivant.
    pub dernier: i64,
    /// Ce que l'appelant affiche avec le fichier ecrit : la plate-forme qui n'a pas repondu, ou un ordre qu'elle
    /// connaissait et que le journal ignorait.
    pub avertissements: Vec<String>,
}

/// Le SIREN de l'entreprise : `siren`, sinon les neuf premiers chiffres du `siret`.
fn siren(ent: &Value) -> String {
    match ent.get("siren").filter(|s| vrai(s)) {
        Some(s) => valeur_en_texte(s),
        None => ent.get("siret").map(valeur_en_texte).unwrap_or_default().chars().take(9).collect(),
    }
}

/// Le plus grand ordre des signalements (natures 04, 05, 07, 08) que les bilans de nature 11 archives dans `retours/`
/// declarent pour ce SIREN, essais et reels, acceptes ou non ; et le flux qui le porte. Le flux se lit dans le nom de
/// l'archive (`AAAAMMJJ-hhmmss_nature11_<flux>.<ext>`, `transport::archiver`).
pub fn dernier_ordre_des_bilans(racine: &Path, siren: &str) -> Result<Option<(i64, String)>, Echec> {
    let dossier = racine.join("retours");
    let Ok(lecture) = std::fs::read_dir(&dossier) else { return Ok(None) };
    let mut noms: Vec<String> = lecture.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.contains("_nature11_")).collect();
    noms.sort();
    let mut dernier: Option<(i64, String)> = None;
    for nom in noms {
        let p = dossier.join(&nom);
        if !p.is_file() {
            continue;
        }
        let octets = std::fs::read(&p).map_err(|e| erreur("OSError", format!("{} : {}", p.display(), e)))?;
        let flux = nom.split_once("_nature11_").map(|(_, f)| f.rsplit_once('.').map_or(f, |(x, _)| x)).unwrap_or("").to_string();
        for d in crate::retours::declarations_du_bilan(&octets) {
            let texte = |k: &str| d.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            if !crate::depot::SIGNALEMENTS.contains(&texte("nature").as_str()) || texte("siren") != siren {
                continue;
            }
            if let Ok(o) = texte("ordre").parse::<i64>() {
                if dernier.as_ref().is_none_or(|(m, _)| o > *m) {
                    dernier = Some((o, flux.clone()));
                }
            }
        }
    }
    Ok(dernier)
}

/// Le plus grand ordre des signalements d'ESSAI que ce dossier a deposes (`detail.ordre` des lignes `dsn` en test) :
/// la base d'essai les garde, et le meme fichier part ensuite en reel. Une ligne qui ne dit pas son ordre n'en donne pas.
fn dernier_ordre_des_essais(racine: &Path) -> Result<i64, Echec> {
    let lignes = crate::journal::lire(racine, Some("dsn"), None, false).map_err(|m| erreur("ErreurOperation", m))?;
    let mut dernier = 0;
    for l in lignes.iter().filter(|l| l.get("mode").and_then(Value::as_str) == Some("test")) {
        let detail = l.get("detail").cloned().unwrap_or(Value::Null);
        let nature = detail.get("nature").map(valeur_en_texte).unwrap_or_default();
        if !crate::depot::SIGNALEMENTS.contains(&nature.as_str()) {
            continue;
        }
        let ordre = match detail.get("ordre") {
            Some(Value::Number(n)) => n.as_i64(),
            Some(Value::String(s)) => s.trim().parse::<i64>().ok(),
            _ => None,
        };
        dernier = dernier.max(ordre.unwrap_or(0));
    }
    Ok(dernier)
}

/// Le plus grand ordre de signalement connu, la plate-forme faisant foi : la releve des retours des trois derniers mois
/// (`releve_retours::relever_retours`, qui archive avant toute lecture), puis le plus grand de ce que disent les bilans
/// archives et le journal (reels, `journal::dernier_ordre_des_signalements`, qui refuse une ligne reelle sans son
/// ordre ; essais). `client` : un client donne (un test le pointe sur un faux serveur) ; sans lui, net-entreprises,
/// avec le mot de passe de consultation.
pub fn dernier_ordre_connu(ctx: &Contexte, client: Option<Client>) -> Result<OrdreConnu, Echec> {
    let reels = crate::journal::dernier_ordre_des_signalements(&ctx.racine).map_err(|m| erreur("ErreurOperation", m))?;
    let journal = reels.max(dernier_ordre_des_essais(&ctx.racine)?);
    let mut avertissements = Vec::new();

    // LA PLATE-FORME : une compagnie fictive n'en a pas ; ailleurs, ce qui empeche de la lire se dit.
    let joue = client.is_none() && crate::routes_accueil::simulation_active(&ctx.racine);
    let lue: Result<(), String> = if joue {
        Ok(())
    } else {
        let fin = releve_retours::maintenant_naif();
        match releve_retours::relever_retours(ctx, fin - JOURS_DE_RELEVE * 86_400, fin, false, "production", client, "") {
            Ok(r) if r.incidents.is_empty() => Ok(()),
            Ok(r) => Err(releve_retours::incidents_en_phrase(&r)),
            Err(e) => Err(e.message),
        }
    };

    let bilans = dernier_ordre_des_bilans(&ctx.racine, &siren(&ctx.entreprise))?;
    let (plateforme, flux) = bilans.clone().unwrap_or((0, String::new()));
    let dernier = journal.max(plateforme);
    if let Err(pourquoi) = lue {
        avertissements.push(format!(
            "La plate-forme n'a pas pu etre consultee ({}) : l'ordre {} suit le journal de ce dossier et les retours deja archives, qui ne savent que ce que ce dossier a depose et releve. Un signalement depose d'ailleurs sous le meme SIREN (un autre poste, une copie, un essai) peut avoir pris cet ordre : la plate-forme le refuserait (B1-208-10). Pour en etre sur : relever les retours (`paie retours`), puis refaire le signalement.",
            pourquoi.trim_end_matches('.'),
            dernier + 1
        ));
    }
    if plateforme > journal {
        avertissements.push(format!(
            "Ordre {} : la plate-forme connait un signalement a l'ordre {} (flux {}) que le journal de ce dossier ne connait pas (il s'arretait a {}). Elle fait foi : le numero d'ordre d'un signalement ne se prend pas deux fois (cahier technique DSN 2026.1, §1.4.3.2, p. 14).",
            dernier + 1,
            plateforme,
            flux,
            journal
        ));
    }
    Ok(OrdreConnu { dernier, avertissements })
}

/// La conduite a tenir devant un signalement refuse B1-208-10, ou `None` si ce bilan n'en est pas un. Le bilan ne dit
/// pas QUEL signalement tient deja cet ordre (PASSATION-O1, point 1) : la phrase donne les deux cas.
pub fn conduite_b1_208_10(declaration: &Value, flux: &str) -> Option<String> {
    let texte = |k: &str| declaration.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let nature = texte("nature");
    if !crate::depot::SIGNALEMENTS.contains(&nature.as_str()) {
        return None;
    }
    let codes = declaration.get("codes").and_then(Value::as_array).cloned().unwrap_or_default();
    if !codes.iter().any(|c| c.as_str() == Some("B1-208-10")) {
        return None;
    }
    let essai = if texte("essai_reel") == crate::retours::ESSAI_TEST { "essai" } else { "reel" };
    Some(format!(
        "Signalement {} de nature {} (flux {}, ordre {}) refuse B1-208-10, << il existe deja une declaration normale >> : pour un signalement, la plate-forme n'en accepte qu'un normal par SIRET et par numero d'ordre (cahier technique DSN 2026.1, §1.4.3.2, p. 14), et l'ordre {} etait deja pris -- par un depot que ce dossier ne connaissait peut-etre pas (un autre poste, une copie, un essai ; la base d'essai a sa propre memoire). Le bilan ne dit pas lequel. Si l'evenement n'est pas encore declare : refaire le signalement sans `--ordre`, il prend le suivant du plus grand ordre que la plate-forme et le journal connaissent. S'il l'est deja (un signalement accepte pour le meme salarie et le meme contrat) : il n'y a rien a renvoyer ; pour le corriger, c'est un annule-et-remplace (`--remplace`, S20.G00.05.006 = date de constitution et ordre du signalement accepte), a un ordre superieur (§1.4.3.4, p. 15).",
        essai,
        nature,
        flux,
        texte("ordre"),
        texte("ordre")
    ))
}

// ------------------------------------------------------------------ et l'ordre des mensuelles

/// Ce que les bilans archives disent des DSN MENSUELLES d'une base (essai ou reel), lu en silence (tickets #221, #222,
/// regle de Leandre du 20/10/2026 : « le compteur d'ordre lit tous les retours, en arriere-plan, sans rien montrer, y
/// compris ceux des tests faits ailleurs »).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MensuellesVues {
    /// Le plus grand ordre de mensuelle vu pour ce mois de constitution, 0 si aucun.
    pub ordre: i64,
    /// Une mensuelle du mois declare a ete ACCEPTEE dans cette base : une normale de plus y serait refusee, le depot
    /// suivant part en annule-et-remplace (cahier technique DSN, §1.4.3.2).
    pub du_mois_acceptee: bool,
}

/// L'ordre d'une mensuelle « au cours du mois courant », lu sur la plate-forme. Cahier technique DSN 2026.1, p. 136
/// (`atelier/reference/norme-dsn/texte/bloc-S20.G00.05.txt`) : le numero d'ordre « permet d'identifier le rang de
/// constitution d'une declaration au cours du mois courant. Pour une Mensuelle, ce numero est remis a zero a chaque
/// premier jour de mois civil » ; et B1-208-11 (`dsn::generer`, 10/09/2026) compare le rang « AAAAMM + ordre » au
/// « dernier ordre accepte ». La plate-forme se souvient de tout ce qui est parti sous le SIREN, d'ou qu'il vienne
/// (l'atelier, un autre poste) ; le journal du dossier ne sait que ce qu'il a envoye.
///
/// Deux lectures, sur les bilans de nature 11 archives dans `retours/` pour ce SIREN et cette base (`essai_reel`) :
/// - l'ordre de chaque mensuelle (nature 01) recue le mois `constitution` (`AAAA-MM`). Le mois de reception du bilan
///   (`date_reception`) tient lieu de mois de constitution : la date de constitution est le jour meme ou le fichier
///   s'ecrit (`dsn::generer`), et un fichier se depose ce jour-la ou plus tard ; la lecture ne peut donc que surestimer
///   l'ordre (un fichier ecrit le 31 et depose le 1er compte pour le mois suivant), jamais le sous-estimer, et trop
///   grand ne coute rien ;
/// - le rang que la plate-forme cite dans un refus (« d'ordre 20261021 inferieur (ou egal) au dernier ordre accepte
///   20261030 ») : AAAAMM + ordre, exact, quel que soit le jour du bilan.
///
/// `mois` (`AAAA-MM`) : le mois declare (S20.G00.05.005) dont on cherche une mensuelle acceptee.
pub fn dernier_ordre_des_mensuelles(racine: &Path, siren: &str, essai: bool, constitution: &str, mois: &str) -> Result<MensuellesVues, Echec> {
    let dossier = racine.join("retours");
    let Ok(lecture) = std::fs::read_dir(&dossier) else { return Ok(MensuellesVues::default()) };
    let mut noms: Vec<String> = lecture.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.contains("_nature11_")).collect();
    noms.sort();
    let base = if essai { crate::retours::ESSAI_TEST } else { "02" }; // `essai_reel` : 01 essai, 02 reel
    let aaaamm = constitution.replace('-', "");
    let cite = regex::Regex::new(r"ordre (?:accept\S* )?(\d{7,})").map_err(|e| erreur("ValueError", e.to_string()))?;
    let mut vues = MensuellesVues::default();
    for nom in noms {
        let p = dossier.join(&nom);
        if !p.is_file() {
            continue;
        }
        let octets = std::fs::read(&p).map_err(|e| erreur("OSError", format!("{} : {}", p.display(), e)))?;
        let bilan = crate::retours::bilan_d_octets(&octets);
        if bilan.is_null() {
            continue;
        }
        let champ = |k: &str| bilan.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        if champ("essai_reel") != base {
            continue;
        }
        let mut du_siren = false;
        for d in crate::retours::declarations_du_bilan(&octets) {
            let texte = |k: &str| d.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            if texte("nature") != crate::depot::MENSUELLE || texte("siren") != siren {
                continue;
            }
            du_siren = true;
            if champ("recu_le").starts_with(constitution) {
                vues.ordre = vues.ordre.max(texte("ordre").parse::<i64>().unwrap_or(0));
            }
            if champ("mois") == mois && texte("etat").eq_ignore_ascii_case("OK") {
                vues.du_mois_acceptee = true;
            }
        }
        if !du_siren {
            continue;
        }
        for c in cite.captures_iter(&String::from_utf8_lossy(&octets)) {
            if let Some(reste) = c[1].strip_prefix(aaaamm.as_str()) {
                vues.ordre = vues.ordre.max(reste.parse::<i64>().unwrap_or(0));
            }
        }
    }
    Ok(vues)
}

/// Le SIREN de l'entreprise du dossier, comme le lit `dernier_ordre_connu`.
pub(crate) fn siren_de(ctx: &Contexte) -> String {
    siren(&ctx.entreprise)
}
