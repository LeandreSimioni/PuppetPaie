//! DEPOSER UNE DSN OU UNE DPAE, du cote de la compagnie : tout ce que `moteur` verifie avant que le depot parte
//! (`envois::depot_acte`), et les suites qu'il fait ensuite (`suites::faire`).
//!
//! DEPOSER EST L'UN DES RARES GESTES DU LOGICIEL QUI ENGAGENT L'EMPLOYEUR -- jusqu'a des ordres de
//! telepaiement reellement preleves. L'ordre est donc toujours le meme, et c'est le moteur qui le tient (jamais
//! la page) : ouvrir la porte du branchement ; lire le fichier ; verifier que le mode demande colle a ce qu'il code (un
//! fichier `02` depose << pour voir >> EST une declaration) ; exiger la preuve (le mot de passe du logiciel, retape au
//! moment de l'acte) ; exiger la REPETITION (ce contenu exact est deja passe en test, ou un motif ECRIT) ; seulement
//! alors chercher le secret de l'organisme ; deposer ; NOTER, quoi qu'il arrive -- succes comme echec.
//!
//! Le secret de l'organisme ne sort pas d'ici que pour le client (`releve_retours::Client`) : il n'est ni
//! journalise, ni affiche, ni renvoye. Un depot de test ne part jamais avec la nature `02` ni l'indicateur
//! `120` : la bascule se fait en memoire (`depot::basculer_*`), jamais sur le disque.
//!
//! Le SIMULATEUR (`bridage::branchement`) joue le depot -- un flux `FICTIF-...` -- et rien ne quitte le poste ;
//! le reste (repetition, journal) se deroule comme en vrai.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use droit::contexte::Contexte;
use envois::depot::{self, Apercu};
use envois::etats;
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::vrai;
use envois::releve_retours::Client;
use envois::suites::Suites;

pub use envois::depot_acte::{phrase_de, DepotFait};
use socle::echec::Echec;

/// Une erreur qui porte une donnee de plus pour la page (`etat`, `manques`...) : sa phrase et la donnee
/// voyagent ensemble dans le message, que `routes::commande` redresse en `{"erreur", "classe", <cle>}`.
pub(crate) fn avec_donnee(classe: &'static str, message: impl Into<String>, cle: &str, donnee: Value) -> Echec {
    Echec { classe: Some(classe), message: json!({"message": message.into(), cle: donnee}).to_string() }
}

/// `Preuve` : ce qui prouve qu'un humain veut CET acte, MAINTENANT. Le mot de passe DU LOGICIEL, retape.
#[derive(Default, Clone)]
pub struct Preuve {
    pub motdepasse: Option<String>,
    pub mois_recopie: Option<String>,
}

impl Preuve {
    pub fn de_la_charge(charge: &Value) -> Preuve {
        Preuve { motdepasse: charge.get("motdepasse").filter(|m| vrai(m)).map(valeur_en_texte), mois_recopie: None }
    }
}

// -------------------------------------------------------------------- la preuve

/// `_cle` lit une version en entiers separes par des points (illisible : `(0,)`).
fn cle_de_version(v: &str) -> Vec<i64> {
    let lu: Option<Vec<i64>> = v.trim().split('.').map(|x| x.trim().parse::<i64>().ok()).collect();
    lu.unwrap_or_else(|| vec![0])
}

/// La raison pour laquelle une version telechargee et pas encore appliquee est
/// indispensable avant de declarer -- ou `""`.
/// La version prete est celle de l'INSTALLATION du contexte (`<installation>/logiciel/donnees` = `ctx.code`).
pub(crate) fn exigee_en_attente(ctx: &Contexte) -> R<String> {
    let code = ctx.code.parent().map(Path::to_path_buf).unwrap_or_default();
    let Some(p) = crate::routes::ouverture::prete(&code.join(".."))? else { return Ok(String::new()) };
    let exigee = p.get("exigee").filter(|e| vrai(e)).map(valeur_en_texte).unwrap_or_default();
    let version = p.get("version").map(valeur_en_texte).unwrap_or_default();
    if exigee.is_empty() || cle_de_version(&version) <= cle_de_version(&crate::routes::ouverture::courante(&code)?) {
        return Ok(String::new());
    }
    Ok(format!(
        "la version {} est indispensable avant de declarer ({}) : fermez et rouvrez PuppetPaie, elle s'appliquera au demarrage",
        version, exigee
    ))
}

/// La forme de preuve retenue, ou un refus. Aucun acte sans elle.
/// LA REGLE : LE MOT DE PASSE DU LOGICIEL, toujours -- le mois recopie seul n'a jamais prouve qu'un humain
/// etait la. Un booleen « confirme=True » venu du front ne prouve rien du tout.
///
/// DEUX FACONS DE LE PROUVER, et aucune autre : le mot de passe que la fenetre ou la page a fait taper et qui
/// arrive dans la charge (`Preuve`) ; sinon LE MOTEUR LE DEMANDE LUI-MEME (`demande::autoriser_acte` : fenetre, terminal,
/// ou refus). Ni la seance (`garder_ouverture`), ni `PUPPETPAIE_MOTDEPASSE`, ni le mot de passe retenu pour ouvrir le
/// coffre ne prouvent quoi que ce soit : ils ouvrent, ils n'autorisent pas. `quoi` et `a_qui` disent a l'humain ce qui
/// va partir ; `mois_attendu` est le mois que la preuve recopiee devrait porter.
pub(crate) fn verifier_preuve(ctx: &Contexte, preuve: Option<&Preuve>, mois_attendu: &str, quoi: &str, a_qui: &str) -> R<String> {
    // Le binaire de l'ATELIER refuse tout acte reel, avant toute autre question (`atelier::redirection`). Un depot reel, un envoi,
    // une signature passent tous par ici.
    #[cfg(feature = "atelier")]
    return refus_atelier(ctx, preuve, mois_attendu, quoi, a_qui);
    #[cfg(not(feature = "atelier"))]
    verifier_preuve_du_logiciel(ctx, preuve, mois_attendu, quoi, a_qui)
}

#[cfg(feature = "atelier")]
fn refus_atelier(ctx: &Contexte, _preuve: Option<&Preuve>, _mois_attendu: &str, _quoi: &str, _a_qui: &str) -> R<String> {
    // UNE COMPAGNIE FICTIVE AUX ENVOIS SIMULES : son « acte reel » est joue (`FICTIF-…`), rien ne part -- comme dans le
    // binaire livre (`verifier_preuve_du_logiciel`). Le banc d'essai en vit : l'horloge y depose la DSN et la DPAE
    // « en reel ». Toute autre compagnie : refuse, mot de passe ou non (la commande `signatures` fait de meme).
    if crate::compagnie::bridage::branchement(&ctx.racine).joue() {
        return Ok("fictive".into());
    }
    Err(erreur("ActeReelRefuse", crate::atelier::redirection::REFUS_ACTE_REEL.to_string()))
}

#[cfg_attr(feature = "atelier", allow(dead_code))]
fn verifier_preuve_du_logiciel(ctx: &Contexte, preuve: Option<&Preuve>, mois_attendu: &str, quoi: &str, a_qui: &str) -> R<String> {
    // LA COMPAGNIE FICTIVE : rien ne part, il n'y a rien a prouver.
    if crate::compagnie::bridage::branchement(&ctx.racine).joue() {
        return Ok("fictive".into());
    }
    // Une version EXIGEE attend d'etre appliquee : l'ancienne ferait une declaration fausse.
    let presse = exigee_en_attente(ctx)?;
    if !presse.is_empty() {
        return Err(erreur("VersionExigee", format!("Rien n'est parti : {}.", presse)));
    }
    let coffre = crate::compagnie::coffre::Coffre::du_poste(&ctx.racine, "");
    let vers = crate::routes::vers_route;
    if !coffre.ouverture_exigee().map_err(vers)? {
        return Err(erreur(
            "PreuveInsuffisante",
            "Aucun mot de passe du logiciel n'est pose : rien ne peut partir en reel. Le poser d'abord -- la fenetre PuppetPaie le demande a sa premiere ouverture, ou `moteur coffre ouverture`. Rien n'est parti.",
        ));
    }
    let Some(preuve) = preuve.filter(|p| p.motdepasse.as_deref().is_some_and(|m| !m.is_empty())) else {
        // Rien n'a ete tape pour cette charge : c'est le moteur qui demande, a un humain, ou il refuse.
        let acte = crate::compagnie::demande::Acte::nouveau(quoi, mois_attendu, a_qui);
        return match crate::compagnie::demande::autoriser_acte(&ctx.racine, &acte) {
            crate::compagnie::demande::Verdict::Accepte => Ok("mot de passe du logiciel".into()),
            crate::compagnie::demande::Verdict::Refuse(phrase) => Err(erreur("PreuveInsuffisante", phrase)),
        };
    };
    if let Some(m) = preuve.mois_recopie.as_deref().filter(|m| !m.is_empty()) {
        if m.trim() != mois_attendu {
            return Err(erreur("PreuveInsuffisante", format!("Mois confirme '{}' alors que le fichier declare '{}'. Rien n'est parti.", m.trim(), mois_attendu)));
        }
    }
    if !coffre.verifier_ouverture(preuve.motdepasse.as_deref().unwrap_or("")).map_err(vers)? {
        return Err(erreur("PreuveInsuffisante", "Mot de passe du logiciel refuse. Rien n'est parti."));
    }
    Ok("mot de passe du logiciel".into())
}

// ------------------------------------------------------------ le secret de l'organisme

/// `_motdepasse_organisme` : le secret de l'organisme -- l'environnement d'abord (quelqu'un l'a tape dans une
/// fenetre, il vaut confirmation), puis le coffre. Ce qui autorise a y puiser pour un depot REEL, c'est la
/// preuve verifiee juste avant par l'appelant. Ce qui SORT d'ici est un secret : il va directement au client.
pub(crate) fn motdepasse_organisme(ctx: &Contexte, service: &str, ouverture: Option<&str>) -> R<Option<String>> {
    let coffre = crate::compagnie::coffre::Coffre::du_poste(&ctx.racine, "");
    let vers = crate::routes::vers_route;
    if !coffre.disponible(service).map_err(vers)? {
        return Err(erreur(
            "ErreurOperation",
            format!("Mot de passe {} absent : ni dans l'environnement, ni au coffre.\n  moteur coffre poser {}", service, service),
        ));
    }
    coffre.lire(&json!(service), &json!(crate::compagnie::coffre::DEPOT), true, ouverture).map_err(vers)
}

// -------------------------------------------------------- la repetition avant l'irreversible

/// `_pourquoi_rate` : ce qui a fait echouer un depot de test, en une phrase.
pub(crate) fn pourquoi_rate(ligne: &Value) -> String {
    let mut anomalies: Vec<String> = Vec::new();
    for r in ligne.get("retours").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
        if r.get("verdict").and_then(Value::as_str) == Some("KO") {
            for a in r.get("anomalies").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
                let t = valeur_en_texte(a);
                if !anomalies.contains(&t) {
                    anomalies.push(t);
                }
            }
        }
    }
    if !anomalies.is_empty() {
        return anomalies.join(" ; ");
    }
    let rep: String = ligne.get("reponse").filter(|r| vrai(r)).map(valeur_en_texte).unwrap_or_default().trim().chars().take(200).collect();
    if rep.is_empty() {
        "motif non lu".into()
    } else {
        rep
    }
}

fn quand_a(v: &Value) -> String {
    etats::quand_lisible(v).replace(" à ", " a ")
}

/// `_phrase_repetition` : le refus, ecrit pour qu'on sache quoi faire dans la minute. Un test qui a RATE se
/// corrige, un test PERIME se refait, une absence de test se fait.
fn phrase_repetition(ctx: &Contexte, quoi: &str, chemin: &Path, etat: &Value) -> String {
    let acte = if quoi == "dsn" { "DSN" } else { "DPAE" };
    // LE CHEMIN DOIT SE RECOPIER TEL QUEL : « sorties/2026-08/... » marche dans les deux cas.
    let ou = chemin.strip_prefix(&ctx.racine).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| chemin.to_string_lossy().into_owned());
    let verbe = if quoi == "dsn" { "depot-dsn" } else { "deposer-dpae" };
    let commande = format!("paie {} {} --test", verbe, ou);
    let dernier = |cle: &str| etat.get(cle).and_then(Value::as_array).and_then(|a| a.last()).cloned();
    let situation = if vrai(&etat["conforme"]) {
        format!(
            "Ce contenu est parti en test le {}, et son compte rendu de conformite n'est pas encore arrive :\nl'accuse d'enregistrement dit que l'envoi est recu, pas qu'il est conforme. Attendre le bilan\n(en general dans les dix minutes), relever les retours, puis envoyer le reel.",
            quand_a(&etat["conforme"]["quand"])
        )
    } else if let Some(l) = dernier("ratees") {
        format!(
            "Ce contenu a ete essaye en test le {}, et l'essai a ECHOUE : {}.\nC'est cela qu'il faut corriger : le meme refus attend en reel.",
            quand_a(&l["quand"]),
            pourquoi_rate(&l)
        )
    } else if let Some(l) = dernier("perimees") {
        format!(
            "Un fichier du meme nom a bien ete essaye en test le {}, mais son\ncontenu a CHANGE depuis ({} teste, {} aujourd'hui) : un octet qui\nbouge rend le test caduc.",
            quand_a(&l["quand"]),
            l.get("empreinte").filter(|e| vrai(e)).map(valeur_en_texte).unwrap_or_else(|| "?".into()),
            valeur_en_texte(&etat["empreinte"])
        )
    } else {
        "Ce contenu n'a jamais ete depose en test.".to_string()
    };
    format!(
        "Depot {} REEL refuse : aucune repetition reussie de ce contenu-la. Rien n'est parti.\n{}\n\nFaire la repetition avec CE fichier-ci -- la bascule en envoi de test se fait\nen memoire, le fichier sur le disque n'est pas modifie :\n  {}\n\nSi le service de test est indisponible et que l'echeance ne peut pas attendre,\nle dire en toutes lettres -- le motif est ecrit au journal des depots :\n  ... --reel --sans-repetition \"ce qui empechait le test\"",
        acte, situation, commande
    )
}

/// `_exiger_repetition` : leve tant que ce contenu n'a pas ete repete ; rend ce qu'il faut noter. LA SORTIE
/// DE SECOURS EST VOULUE, et elle est tracee : un motif ECRIT (dix caracteres au moins) passe outre ; ce
/// qu'on refuse, c'est le contournement SILENCIEUX.
fn exiger_repetition(ctx: &Contexte, lignes: &[Value], quoi: &str, chemin: &Path, contournement: &str) -> R<Value> {
    let octets = std::fs::read(chemin).map_err(|e| erreur("OSError", format!("{} : {}", chemin.display(), e)))?;
    let nom = chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let etat = etats::repetition(lignes, quoi, &envois::journal::empreinte(&octets), &nom)?;
    // UN REEL NE PART QUE SUR LE BILAN LU DE SON TEST : l'accuse d'enregistrement dit
    // « envoi recu », pas « conforme » (regle 5). Pour une DSN, c'est le bilan de nature 11 du test, qui arrive « en
    // general dans les dix minutes » (la reponse du point de depot). Pour une DPAE, le bilan 11 ou le certificat 41 :
    // celui d'un test revient bel et bien (constate sur des DPAE de test : le certificat 41 les dit enregistrees,
    // code 99).
    if vrai(&etat["conforme"]) && etats::est(&etat["compte_rendu"], "ok") {
        let l = &etat["conforme"];
        return Ok(json!({
            "test": l.get("quand").cloned().unwrap_or(Value::Null),
            "flux": if vrai(&l["flux"]) { l["flux"].clone() } else { json!("") },
            "empreinte": etat["empreinte"], "compte_rendu": etat["compte_rendu"],
        }));
    }
    let raison = contournement.trim();
    if !raison.is_empty() {
        if raison.chars().count() < 10 {
            return Err(erreur(
                "ErreurOperation",
                "Deposer sans repetition demande un motif ECRIT, pas un mot : ce qui empechait le test, en une phrase. C'est ce qu'on relira le jour ou ce depot posera question. Rien n'est parti.",
            ));
        }
        return Ok(json!({"contournement": raison, "empreinte": etat["empreinte"]}));
    }
    Err(avec_donnee("RepetitionManquante", phrase_repetition(ctx, quoi, chemin, &etat), "etat", etat))
}

/// LES ENGAGEMENTS QU'UN SIGNALEMENT COUVRE : un signalement porte un individu, un contrat, un evenement
/// (`signalement`) ; l'engagement du mois de son evenement (`evenement`, AAAA-MM-JJ) dont il cite le numero
/// (S21.G00.40.009), a sa version (`journal::versions_du_mois`).
fn versions_du_signalement(ctx: &Contexte, evenement: &str, octets: &[u8]) -> R<Value> {
    let Some(mois) = evenement.get(..7) else { return Ok(json!([])) };
    let texte = String::from_utf8_lossy(octets);
    let numeros: Vec<String> =
        texte.lines().filter_map(|l| l.strip_prefix("S21.G00.40.009,")).map(|v| v.trim().trim_matches('\'').to_string()).collect();
    envois::journal::versions_du_mois(ctx, mois, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?, |_, numero, _| numeros.iter().any(|n| n == numero)).map_err(Echec::from)
}

/// La derniere porte -- une copie de developpement ne declare rien en reel.
fn refuser_si_copie(ctx: &Contexte, quoi: &str) -> R<()> {
    if ctx.racine.join("COPIE-DE-DEVELOPPEMENT.json").is_file() {
        return Err(erreur(
            "ErreurOperation",
            format!("Copie de developpement : {} ne part jamais d'ici. Les essais se font en test ; pour le faire en vrai, ouvrir la vraie compagnie.", quoi),
        ));
    }
    Ok(())
}

// ------------------------------------------------------------------------ la DSN

/// `ordre_d_essai_du_fichier` : l'ordre auquel un essai part : le calcule, jamais plus bas que celui que le fichier porte
/// deja. `None` reste `None`.
fn ordre_d_essai_du_fichier(calcule: Option<i64>, chemin: &Path) -> R<Option<i64>> {
    let Some(c) = calcule else { return Ok(None) };
    let octets = std::fs::read(chemin).map_err(|e| erreur("OSError", format!("{} : {}", chemin.display(), e)))?;
    Ok(Some(rang_dans_le_fichier(&octets).map_or(c, |m| c.max(m))))
}

/// Le mois de constitution (`AAAA-MM`) d'une date S20.G00.05.007 (`JJMMAAAA`) ; illisible, celui d'aujourd'hui (la
/// date de constitution est le jour ou le fichier s'ecrit, `dsn::generer`).
fn mois_de_constitution(jjmmaaaa: &str, aujourdhui: &socle::date::Jour) -> String {
    let c: Vec<char> = jjmmaaaa.trim().chars().collect();
    if c.len() == 8 && c.iter().all(char::is_ascii_digit) {
        return format!("{}-{}", c[4..8].iter().collect::<String>(), c[2..4].iter().collect::<String>());
    }
    aujourdhui.iso().chars().take(7).collect()
}

/// `re.search(rb"S20\.G00\.05\.004,'(\d+)'", octets)`.
fn rang_dans_le_fichier(octets: &[u8]) -> Option<i64> {
    let motif = b"S20.G00.05.004,'";
    let mut i = 0;
    while i + motif.len() <= octets.len() {
        if &octets[i..i + motif.len()] == motif {
            let chiffres: Vec<u8> = octets[i + motif.len()..].iter().take_while(|b| b.is_ascii_digit()).copied().collect();
            if !chiffres.is_empty() && octets.get(i + motif.len() + chiffres.len()) == Some(&b'\'') {
                return String::from_utf8_lossy(&chiffres).parse().ok();
            }
        }
        i += 1;
    }
    None
}

/// `prochain_ordre_de_test` : l'ordre a donner au prochain essai de ce mois, ou `None` pour le premier. Les
/// suivants partent en annule-et-remplace, a un ordre plus grand que tout ce que la base de test a pu retenir :
/// le plus grand des ordres deja poses par nous, de ceux des fichiers essayes encore sur le disque, du nombre
/// d'essais, et de ce que les bilans archives disent de la base d'essai pour le mois de constitution du fichier
/// (`ordre_signalements::dernier_ordre_des_mensuelles` : les essais faits AILLEURS sous le meme SIREN, l'atelier ou un
/// autre poste, comptent pour l'ordre sans s'afficher), plus un. Une mensuelle du mois deja
/// acceptee dans la base d'essai, meme venue d'ailleurs, fait partir l'essai en annule-et-remplace. Trop grand ne coute
/// rien ; trop petit, c'est B1-208-11. `constitution` : le mois de constitution du fichier (`AAAA-MM`, S20.G00.05.007).
fn prochain_ordre_de_test(ctx: &Contexte, mois: &str, nic: &Value, constitution: &str) -> R<Option<i64>> {
    let du_mois = envois::journal::lire(&ctx.racine, Some("dsn"), Some(mois), false).map_err(|m| erreur("OSError", m))?;
    let essais: Vec<Value> = etats::de_l_etablissement(&du_mois, nic, &etats::nic_principal(&ctx.entreprise))?
        .into_iter()
        .filter(|l| l.get("mode").and_then(Value::as_str) == Some("test"))
        .collect();
    let vues = envois::ordre_signalements::dernier_ordre_des_mensuelles(&ctx.racine, &envois::ordre_signalements::siren_de(ctx), true, constitution, mois)?;
    if essais.is_empty() && !vues.du_mois_acceptee {
        return Ok(None);
    }
    let mut connus: Vec<i64> = vec![essais.len() as i64, vues.ordre];
    for l in &essais {
        let d = l.get("detail").cloned().unwrap_or(Value::Null);
        let o = d.get("ordre_test").filter(|o| vrai(o)).map(valeur_en_texte).unwrap_or_default();
        if !o.is_empty() && o.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(n) = o.parse() {
                connus.push(n);
            }
        }
        if let Some(nom) = l.get("fichier").filter(|f| vrai(f)).map(valeur_en_texte) {
            let p = ctx.racine.join("sorties").join(mois).join(nom);
            if p.exists() {
                if let Some(n) = std::fs::read(&p).ok().and_then(|o| rang_dans_le_fichier(&o)) {
                    connus.push(n);
                }
            }
        }
    }
    Ok(connus.into_iter().max().map(|m| m + 1))
}

/// Le garde-fou du depot REEL d'une mensuelle, ecrit depuis la
/// norme. `Ok(true)` : un annule-et-remplace en regle (rien a forcer) ; `Ok(false)` : une premiere DSN ou une
/// normale (`DsnDejaDeposee` tranche) ; un refus sinon.
fn verifier_depot_remplacement(ctx: &Contexte, mois: &str, a: &Apercu, nic: &Value) -> R<bool> {
    let mut m = crate::routes::etapes::Moteur::nouveau(ctx);
    let faits = m.depots_reels(mois, nic)?;
    let dernier = m.dernier_ordre(mois, &faits)?;
    if !["03", "05"].contains(&a.champ("type_declaration")) {
        return Ok(false);
    }
    // CE QUE LA PLATE-FORME A VU EN REEL au mois de constitution du fichier, lu en silence dans les bilans archives
    // (`ordre_signalements::dernier_ordre_des_mensuelles`) : une mensuelle reelle envoyee d'un autre poste compte, comme
    // a la production (`depart::forme_a_produire`).
    let constitution = mois_de_constitution(a.champ("constitution"), &ctx.aujourdhui);
    let vues = envois::ordre_signalements::dernier_ordre_des_mensuelles(&ctx.racine, &envois::ordre_signalements::siren_de(ctx), false, &constitution, mois)?;
    let dernier = match dernier {
        Some(d) => d.max(vues.ordre),
        None if vues.du_mois_acceptee || vues.ordre > 0 => vues.ordre,
        None => return Ok(false),
    };
    m.refuser_si_hors_delai(mois)?;
    let ordre: i64 = a.champ("ordre").trim().parse().unwrap_or(0);
    if ordre <= dernier {
        return Err(erreur(
            "OrdreTropPetit",
            format!(
                "Cet annule-et-remplace porte l'ordre {}, et la derniere DSN reelle connue pour {} (journal et bilans de la plate-forme) l'ordre {} : la plate-forme n'integre pas un rang inferieur ou egal (cahier technique DSN, p. 15 ; B1-208-11). Refaire la DSN du mois : elle partira a l'ordre {}. Rien n'est parti.",
                ordre,
                mois,
                dernier,
                dernier + 1
            ),
        ));
    }
    Ok(true)
}

/// Depose une DSN. La seule sortie du moteur qui declare quelque chose.
///
/// L'ORDRE COMPTE : 1. lire le fichier ; 2. verifier que le mode demande colle a la nature codee dedans ;
/// 3. exiger la preuve (reel) ; 4. exiger la REPETITION (reel, `contournement` pour la sortie de secours) ;
/// 5. seulement alors chercher le secret de l'organisme ; 6. deposer ; 7. NOTER, quoi qu'il arrive.
pub fn deposer_dsn(
    ctx: &Contexte,
    fichier: &Value,
    reel: bool,
    preuve: Option<&Preuve>,
    qui: &str,
    forcer: bool,
    contournement: &str,
) -> R<DepotFait> {
    deposer_dsn_dans(ctx, fichier, reel, preuve, qui, "production", forcer, contournement, None)
}

/// `deposer_dsn` avec l'ENVIRONNEMENT du point de depot (`production` ou `editeur`). La
/// plate-forme de l'editeur est une plate-forme d'ESSAI : un depot REEL n'y a pas de sens et reste refuse ici, car le journal le
/// noterait comme une declaration partie.
///
/// `ordre_d_essai` (`moteur depot-dsn --test --ordre N`) : l'ordre au-dessous duquel un ESSAI de mensuelle ne part pas,
/// donne par qui sait ce que la base d'essai a deja retenu pour ce SIRET et ce mois (un autre poste, une compagnie d'essai, le
/// « dernier ordre accepte » d'un B1-208-11). L'essai part alors en annule-et-remplace a cet ordre, ou plus haut si le journal
/// en sait davantage. Refuse pour un depot reel et pour un signalement (leur ordre est celui du fichier, lu sur la
/// plate-forme a son ecriture : `ordre_signalements`).
#[allow(clippy::too_many_arguments)]
pub fn deposer_dsn_dans(
    ctx: &Contexte,
    fichier: &Value,
    reel: bool,
    preuve: Option<&Preuve>,
    qui: &str,
    environnement: &str,
    forcer: bool,
    contournement: &str,
    ordre_d_essai: Option<i64>,
) -> R<DepotFait> {
    let branchement = crate::compagnie::bridage::branchement(&ctx.racine);
    branchement.ouvrir()?;
    if reel && environnement != "production" {
        return Err(erreur(
            "ErreurOperation",
            format!("L'environnement '{}' est la plate-forme d'essai de l'editeur : rien n'y est declare. Un depot REEL ne part que sur 'production' ; pour essayer, utiliser --test. Rien n'est parti.", environnement),
        ));
    }
    let chemin = PathBuf::from(crate::routes::depots::resoudre(&ctx.racine, fichier)?);
    let octets = crate::routes::depots::lire_octets(&chemin.to_string_lossy())?;
    let apercu = depot::lire(&octets);
    // 2. le mode contre le fichier ; `presence_prouvee` : c'est ICI qu'on la prouve, jamais le transport.
    depot::verifier(&apercu, reel, None, true)?;
    let mois = apercu.mois_lisible();
    if let Some(n) = ordre_d_essai {
        if reel || apercu.est_signalement() || n < 1 {
            return Err(erreur(
                "ErreurOperation",
                "Un ordre d'essai (--ordre) ne vaut que pour le depot de TEST d'une DSN mensuelle, et c'est un entier positif : un depot reel part a l'ordre du fichier, un signalement a celui que la plate-forme a donne a son ecriture. Rien n'est parti.",
            ));
        }
    }
    // UNE COMPAGNIE D'ESSAI ne depose que ce qu'elle a ecrit sous SON identite : les DSN copiees de la fictive portent le
    // SIREN de la fictive, que le compte net-entreprises ne declare pas. Le refus ne cite aucun des deux numeros.
    if crate::compagnie::essai::est_essai(&ctx.racine) && apercu.champ("siren") != valeur_en_texte(ctx.entreprise.get("siren").unwrap_or(&Value::Null)) {
        return Err(erreur(
            "ErreurOperation",
            "Compagnie d'essai : ce fichier ne porte pas le SIREN de la compagnie (il a ete ecrit sous une autre identite : celle de la compagnie fictive, avant le montage, ou une identite d'avant). Deposer ici la DSN du mois (`moteur depot-dsn AAAA-MM`), qui s'y fabrique. Rien n'est parti.",
        ));
    }
    let nic: Value = if apercu.champ("etablissement").is_empty() { Value::Null } else { json!(apercu.champ("etablissement")) };
    let lignes = envois::journal::lu(&ctx.racine).map(|j| j.lignes)?;

    let (mut forme, mut repetition) = (String::new(), Value::Null);
    // UN SIGNALEMENT REEL PART A UN ORDRE QUE PERSONNE N'A ENCORE PRIS : la sequence ne se remet jamais a zero
    // (cahier technique DSN, p. 136), et la plate-forme n'accepte pas un second signalement normal au meme ordre
    // (p. 14). Deux signalements ecrits avant que le premier parte portent le meme : le second se refait. Refuse
    // avant la preuve : aucun mot de passe n'y change rien.
    if reel && apercu.est_signalement() {
        let dernier = envois::journal::dernier_ordre_des_signalements(&ctx.racine).map_err(|m| erreur("ErreurOperation", m))?;
        let ordre: i64 = apercu.champ("ordre").trim().parse().unwrap_or(0);
        if ordre <= dernier {
            return Err(erreur(
                "ErreurOperation",
                format!(
                    "Ce signalement porte l'ordre {}, et un signalement reel est deja parti a l'ordre {} : le numero d'ordre d'un signalement ne se remet jamais a zero et ne se prend pas deux fois (cahier technique DSN, p. 136 et 14). Refaire le signalement : il partira a l'ordre {}. Rien n'est parti.",
                    ordre,
                    dernier,
                    dernier + 1
                ),
            ));
        }
    }
    if reel {
        forme = verifier_preuve(ctx, preuve, &apercu.confirmation_attendue(), "depot REEL d'une DSN", "net-entreprises (la declaration part a l'Urssaf et aux organismes)")?;
        // PAS DEUX FOIS SANS LE VOULOIR. Une DSN reelle deja partie pour ce mois, et pas refusee, ne repart pas.
        // UN SIGNALEMENT N'EST PAS CONCERNE : plusieurs peuvent partir le meme mois.
        if !apercu.est_signalement() {
            let etat_mois = crate::import::mois::dsn_avant(ctx, &mois, &nic)?;
            // L'ANNULE-ET-REMPLACE EN REGLE PART SANS QU'ON FORCE RIEN (ses gardes passent toujours).
            let remplace = verifier_depot_remplacement(ctx, &mois, &apercu, &nic)?;
            if matches!(etat_mois["etat"].as_str(), Some("passee" | "attente")) && !remplace && !forcer {
                return Err(avec_donnee(
                    "DsnDejaDeposee",
                    format!(
                        "{} : {}. Rien n'est parti. Ce fichier est une declaration NORMALE, et la plate-forme n'en accepte pas une seconde pour le meme mois (cahier technique DSN §1.4.3.2) : pour corriger, refaire la DSN du mois -- elle sortira en annule-et-remplace.",
                        mois,
                        valeur_en_texte(&etat_mois["phrase"])
                    ),
                    "etat",
                    etat_mois,
                ));
            }
        }
        // LA PAIE A CHANGE DEPUIS L'ECRITURE : le fichier est peut-etre conforme a son test, il ne
        // declare plus la paie.
        let change = if apercu.est_signalement() {
            None
        } else {
            crate::routes::etapes::Moteur::nouveau(ctx).paie_changee_depuis_l_ecriture(&mois, &chemin)?
        };
        if let Some(c) = change {
            return Err(erreur(
                "ErreurOperation",
                format!(
                    "{} : {}. Rien n'est parti. Reecrire le mois, deposer la nouvelle DSN en test, puis l'envoyer.",
                    chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                    c
                ),
            ));
        }
        // Et la repetition, en DERNIER des garde-fous.
        repetition = exiger_repetition(ctx, &lignes, "dsn", &chemin, contournement)?;
    }

    let joue = branchement.joue();
    let mut mdp: Option<String> = None;
    if !joue {
        mdp = motdepasse_organisme(ctx, "net-entreprises", preuve.and_then(|p| p.motdepasse.as_deref()))?;
    }
    if reel && !joue {
        refuser_si_copie(ctx, "une DSN reelle")?;
    }

    // UN ESSAI DE PLUS SUR UN MOIS DEJA ESSAYE part en annule-et-remplace, a l'ordre suivant : la base de test
    // garde les essais. JAMAIS PLUS BAS QUE L'ORDRE DU FICHIER.
    let ordre_test = if reel || apercu.est_signalement() { None } else { prochain_ordre_de_test(ctx, &mois, &nic, &mois_de_constitution(apercu.champ("constitution"), &ctx.aujourdhui))? };
    let ordre_test = ordre_test.max(ordre_d_essai);
    let ordre_test = ordre_d_essai_du_fichier(ordre_test, &chemin)?;

    // LA PAIE DECLAREE : la version de chaque engagement que la DSN emporte -- pour la mensuelle, ceux du mois qui relevent
    // de son etablissement (S21.G00.11.001 ; une paie qui change ensuite appelle un annule-et-remplace,
    // `engagement::etats::a_refaire`) ; pour un signalement, celui dont il cite le contrat (S21.G00.40.009) dans le mois
    // de son evenement.
    let versions = if apercu.est_signalement() {
        versions_du_signalement(ctx, &mois, &octets)?
    } else {
        let nic = Some(apercu.champ("etablissement")).filter(|n| !n.is_empty()).map(String::from).unwrap_or_else(|| droit::bareme::nic_principal(&ctx.entreprise));
        let mut v = envois::journal::versions_du_mois(ctx, &mois, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?, |_, _, e| crate::routes::etapes::nic_de_l_engagement_ecrit(ctx, &mois, e).as_deref() == Some(nic.as_str()))
            .map_err(Echec::from)?;
        // Et la version presente de l'engagement de chaque mois passe dont la DSN porte la correction.
        if let Value::Array(a) = &mut v {
            a.extend(declarations::produire_regularisations::versions_portees(ctx, &mois, &nic, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?)?);
        }
        v
    };
    let d = envois::depot_acte::Depot { chemin: &chemin, octets: &octets, mois: &mois, reel, qui, preuve: &forme, repetition: &repetition, motdepasse: mdp, versions: &versions };
    let mut suites = Suites::default();
    let fait = envois::depot_acte::deposer_dsn(ctx, branchement, &d, &apercu, environnement, ordre_test, &mut suites);
    crate::compagnie::suites::faire(ctx, suites)?;
    fait
}

// ----------------------------------------------------------------------- la DPAE

/// Depose une DPAE. Meme ordre, memes regles que la DSN. La DPAE
/// declare des EMBAUCHES, et le journal retient lesquelles (salarie, premier jour).
pub fn deposer_dpae(
    ctx: &Contexte,
    fichier: &Value,
    reel: bool,
    preuve: Option<&Preuve>,
    qui: &str,
    majuscules: bool,
    contournement: &str,
) -> R<DepotFait> {
    deposer_dpae_avec(ctx, fichier, reel, preuve, qui, majuscules, contournement, None)
}

/// `deposer_dpae` avec, au besoin, le client Urssaf que l'appelant donne a la place du vrai (la preuve : un client pointe sur
/// un serveur local). Un client donne n'est jamais confronte au coffre, et ses identifiants sont verifies comme ceux
/// d'un client construit ici. Sans client, rien ne change.
#[allow(clippy::too_many_arguments)]
pub fn deposer_dpae_avec(
    ctx: &Contexte,
    fichier: &Value,
    reel: bool,
    preuve: Option<&Preuve>,
    qui: &str,
    majuscules: bool,
    contournement: &str,
    client_donne: Option<&mut Client>,
) -> R<DepotFait> {
    let branchement = crate::compagnie::bridage::branchement(&ctx.racine);
    branchement.ouvrir()?;
    let chemin = PathBuf::from(crate::routes::depots::resoudre(&ctx.racine, fichier)?);
    let apercu = crate::routes::depots::apercu_depot_dpae(ctx, &json!(chemin.to_string_lossy()))?;
    let octets = crate::routes::depots::lire_octets(&chemin.to_string_lossy())?;
    let mois = valeur_en_texte(&apercu["mois"]);

    if reel && !vrai(&apercu["reel"]) {
        return Err(erreur(
            "ErreurOperation",
            format!(
                "Le fichier porte l'indicateur {} (test) et vous demandez un depot REEL : rien ne serait declare. Reproduire le fichier sans --test.",
                valeur_en_texte(&apercu["indicateur"])
            ),
        ));
    }

    let (mut forme, mut repetition) = (String::new(), Value::Null);
    if reel {
        forme = verifier_preuve(ctx, preuve, &mois, "depot REEL d'une DPAE", "l'Urssaf (declaration d'embauche)")?;
        // LA REPETITION, apres la preuve : le fichier sur le disque porte toujours l'indicateur reel, et c'est
        // NOUS qui le basculons en test juste en dessous.
        let lignes = envois::journal::lu(&ctx.racine).map(|j| j.lignes)?;
        repetition = exiger_repetition(ctx, &lignes, "dpae", &chemin, contournement)?;
    }

    let mut contenu = octets.clone();
    if !reel {
        contenu = depot::basculer_dpae(&contenu, depot::LONGUEUR_DPAE)?;
    }

    let joue = branchement.joue();
    let mut client: Option<Client> = None;
    let mut donne = client_donne;
    if !joue {
        let soucis = match donne.as_deref() {
            Some(c) => c.verifier_identifiants()?,
            None => {
                let mdp = motdepasse_organisme(ctx, "urssaf", preuve.and_then(|p| p.motdepasse.as_deref()))?;
                let c = Client::dpae(ctx, majuscules, mdp);
                let soucis = c.verifier_identifiants()?;
                client = Some(c);
                soucis
            }
        };
        if !soucis.is_empty() {
            return Err(erreur("ErreurOperation", format!("Ces points feraient echouer la connexion MtoM. Rien n'est envoye :\n  {}", soucis.join("\n  "))));
        }
        if depot::est_reel(&contenu) {
            refuser_si_copie(ctx, "une DPAE reelle")?;
        }
    }

    let embauches: Vec<Value> = apercu["embauches"]
        .as_array()
        .map(|a| a.iter().map(|l| json!({"salarie": l["salarie"], "debut": l["debut"], "fin": l["fin"]})).collect())
        .unwrap_or_default();
    // La version de chaque engagement dont l'embauche part : une date d'embauche qui change ensuite le dit.
    let declare = |cle: &str, e: &Value| embauches.iter().any(|x| x["salarie"].as_str() == Some(cle) && x["debut"].as_str() == Some(engagement::debut_brut(e).as_str()));
    let versions = envois::journal::versions_du_mois(ctx, &mois, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?, |cle, _, e| declare(cle, e)).map_err(Echec::from)?;
    let d = envois::depot_acte::Depot { chemin: &chemin, octets: &octets, mois: &mois, reel, qui, preuve: &forme, repetition: &repetition, motdepasse: None, versions: &versions };
    let c = match donne.as_deref_mut() {
        Some(c) => Some(c),
        None => client.as_mut(),
    };
    let mut suites = Suites::default();
    let fait = envois::depot_acte::deposer_dpae(ctx, branchement, &d, &contenu, embauches, c, apercu["deja_depose"].clone(), &mut suites);
    crate::compagnie::suites::faire(ctx, suites)?;
    fait
}
