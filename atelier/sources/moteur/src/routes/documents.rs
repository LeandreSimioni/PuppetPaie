//! Les routes des documents du mois : ce que la boite montre avant l'envoi (`envoi-apercu`), les envoyer par courriel
//! (`envoyer-documents`) ou les imprimer (`documents-imprimer`) ; l'un et l'autre sont un depart (bulletins et
//! certificats numerotes, une ligne au journal, la paie figee). Et ce que les envois partagent avec `contrats` et
//! `archives` : la boite qui envoie (`client_courriel`), les lectures de la charge.
//!
//! L'apercu ne fait que lire : il nomme les pieces dues a chacun, que l'envoi fabrique.
//!
//! Chaque charge et chaque reponse est une forme declaree ici, lue une fois (`routes::lire_charge`) ; l'horloge d'une
//! compagnie fictive appelle les memes fonctions, sur les memes formes.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::compagnie::depot_acte::{verifier_preuve, Preuve};
use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus, ILLISIBLE, MANQUE, MOIS, OPERATION};
use crate::routes::catalogue::Route;
use crate::routes::etapes::{ChargeMois, Moteur};
use crate::routes::{en_json, lire_charge, Pastille, INTERFACE};
use droit::contexte::Contexte;
use envois::courriel::{BoiteSimulee, Courriel};
use envois::envoi;
use envois::journal;
use envois::ligne_d_envoi::{echec_d_envoi, noter, noter_echec_d_envoi, par_courriel};
use socle::date::jour_iso;
use socle::echec::{erreur, Echec};
use socle::fichiers::nom_du;
use socle::mois::Mois;
use documents::valeur::valeur_en_texte;
use crate::routes::catalogue::{Ecran, Place};

pub(crate) type R<T> = Result<T, Echec>;

/// De quoi ouvrir la boite : appele UNE fois par envoi. Les tests passent un faux client du trait `Courriel`, ou la
/// boite simulee ; une route ouvre la vraie (`client_courriel`).
pub type Ouvrir<'a> = &'a mut dyn FnMut() -> R<Box<dyn Courriel>>;

const SALARIES: Champ = Champ { nom: "salaries", libelle: Affiche::new("Salariés"), forme: Forme::Liste, sens: "seulement ceux-ci (les clés de leurs fiches) ; tous sinon", avance: false };

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Paie);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::EnvoiApercu,
        geste: "Montrer les documents du mois de chaque salarié, et à qui ils partiraient.",
        charge: &[MOIS],
        exemple: r#"{"mois":"2026-11"}"#,
        rend: "pret, empechements, informations, avertissements, boite_liee, expediteur, messages (cle, nom, destinataire, pieces, renvoi, changements), deja",
        refus: &[MANQUE, ILLISIBLE],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::EnvoyerDocuments,
        geste: "Envoyer à chaque salarié ses documents du mois par courriel ; la paie se fige.",
        charge: &[MOIS, SALARIES],
        exemple: r#"{"mois":"2026-11"}"#,
        rend: "phrase, partis (les salariés servis), avertissements (ceux sans courriel, la boîte qui envoie)",
        refus: &[
            Refus { classe: "PreuveInsuffisante", corriger: "le mot de passe du logiciel, tapé par un humain au moment de l'envoi ; rien n'est écrit" },
            Refus { classe: "AucunDocument", corriger: "valider un engagement du mois" },
            Refus { classe: "BoiteAbsente", corriger: "poser la boîte courriel (Configuration, Mots de passe), ou imprimer" },
            OPERATION,
        ],
        acte: Acte::Engage,
        avance: false,
        phrase: 300,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::DocumentsImprimer,
        geste: "Imprimer les documents du mois, à remettre en main propre ; la paie se fige.",
        charge: &[MOIS, SALARIES],
        exemple: r#"{"mois":"2026-11","salaries":["SVOBODA"]}"#,
        rend: "impression (le PDF, dans sorties/<mois>/) et son chemin, salaries, avertissements",
        refus: &[Refus { classe: "AucunDocument", corriger: "valider un engagement du mois" }, OPERATION],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::PiecePartie,
        geste: "Dire où est une pièce partie d'un mois, refaite à l'identique si elle manque.",
        charge: &[MOIS, Champ { nom: "fichier", libelle: Affiche::new("Pièce"), forme: Forme::Texte, sens: "le nom de la pièce, tel que le journal le dit", avance: false }],
        exemple: r#"{"mois":"2026-11","fichier":"DPAE_Novembre2026.txt"}"#,
        rend: "chemin (relatif à la compagnie)",
        refus: &[MANQUE, OPERATION],
        acte: Acte::Lit,
        avance: true,
        phrase: 0,
        ecran: Ecran::Dessine(Place::Paie),
    },
];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: Route, charge: &Value) -> R<Value> {
    match nom {
        Route::EnvoiApercu => en_json(envoi_apercu(ctx, lire_charge::<ChargeMois>(charge)?.mois)?),
        Route::EnvoyerDocuments => en_json(envoyer_documents(ctx, lire_charge(charge)?, &mut || client_courriel(ctx))?),
        Route::DocumentsImprimer => en_json(documents_imprimer(ctx, lire_charge(charge)?, INTERFACE)?),
        Route::PiecePartie => en_json(piece_partie(ctx, lire_charge(charge)?)?),
        autre => Err(Echec { classe: None, message: format!("route inconnue : {}", autre) }),
    }
}

// ------------------------------------------------------------------ les formes

/// La charge de `documents-imprimer` (et de l'horloge d'une compagnie fictive, pour ceux qui n'ont pas de courriel) :
/// le mois, les salaries choisis (aucun : tous).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeDocumentsImprimer {
    pub(crate) mois: Mois,
    #[serde(default)]
    pub(crate) salaries: Vec<String>,
}

/// Ce que rend `documents-imprimer` : sa phrase, le PDF a imprimer et son chemin, les salaries servis, ce que la fabrique
/// a dit (aussi en `phrases`).
#[derive(Debug, Serialize)]
pub(crate) struct DocumentsImprimes {
    phrase: String,
    impression: String,
    chemin: String,
    salaries: Vec<String>,
    avertissements: Vec<String>,
    phrases: Vec<String>,
}

/// Ce que rend `envoyer-documents` : sa phrase, les adresses servies, ce qui n'est pas parti (pas de courriel) et la
/// boite qui envoie.
#[derive(Debug, Serialize)]
pub(crate) struct DocumentsEnvoyes {
    phrase: String,
    partis: Vec<String>,
    avertissements: Vec<String>,
}

/// La charge de `piece-partie` : le mois, et la piece telle que le journal la nomme.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargePiecePartie {
    mois: Mois,
    fichier: String,
}

/// Ce que rend `piece-partie` : le chemin de la piece, relatif a la compagnie.
#[derive(Debug, Serialize)]
struct PiecePartie {
    phrase: &'static str,
    chemin: String,
}

/// Un message de l'apercu : a qui, quelles pieces (leur nom de fichier, et leur sorte, ce que l'ecran en dit), et, pour un
/// renvoi de documents corriges, sa pastille et ce qui a change.
#[derive(Debug, Serialize)]
struct MessageVu {
    cle: String,
    nom: String,
    destinataire: String,
    pieces: Vec<String>,
    sortes: Vec<&'static str>,
    renvoi: bool,
    pastille: Option<Pastille>,
    changements: Vec<String>,
}

/// Le dernier depart des documents, quand tous sont deja partis.
#[derive(Debug, Serialize)]
struct DejaPartis {
    quand: String,
    quand_dit: String,
}

/// Ce que rend `envoi-apercu`.
#[derive(Debug, Serialize)]
struct ApercuEnvoi {
    phrase: String,
    expediteur: String,
    expediteur_dit: String,
    avertissements: Vec<String>,
    messages: Vec<MessageVu>,
    deja: Option<DejaPartis>,
    pret: bool,
    empechements: Vec<String>,
    informations: Vec<String>,
    boite_liee: bool,
    phrases: Vec<String>,
}

// ------------------------------------------------------------------ piece-partie

/// `piece-partie` : le chemin de la piece partie, refaite si son fichier manque (`depart::piece_partie`), relatif a la
/// compagnie (`sorties/<mois>/[envoi/]<fichier>`).
fn piece_partie(ctx: &Contexte, c: ChargePiecePartie) -> R<PiecePartie> {
    let f = crate::compagnie::depart::piece_partie(ctx, &c.mois.to_string(), &c.fichier)?;
    let chemin = f.strip_prefix(&ctx.racine).unwrap_or(&f).to_string_lossy().replace('\\', "/");
    Ok(PiecePartie { phrase: "La pièce partie, telle qu'elle est partie.", chemin })
}

// ------------------------------------------------------------------ les outils

/// Y a-t-il dans le dossier un nom qui commence par `prefixe` et finit par `suffixe` ? Un dossier
/// qui n'en est pas un, ou qu'on ne peut lire, ne rend rien.
pub(crate) fn un_nom_commence_et_finit(dossier: &Path, prefixe: &str, suffixe: &str) -> bool {
    let Ok(lecture) = std::fs::read_dir(dossier) else { return false };
    lecture.flatten().any(|e| {
        let n = e.file_name().to_string_lossy().into_owned();
        n.len() >= prefixe.len() + suffixe.len() && n.starts_with(prefixe) && n.ends_with(suffixe)
    })
}


// ------------------------------------------------------------------ signature-apercu


// ------------------------------------------------------------------ envoi-apercu

/// `AER_<cle>_<etiquette>.pdf`.
fn nom_fichier(prefixe: &str, cle: &str, etiq: &str) -> String {
    format!("{}_{}_{}.pdf", prefixe, cle, etiq)
}

/// Une attestation d'un retour de nature 46, rattachee ou non a une fiche.
struct Attestation {
    nom_archive: String,
    qui: String,
    donnees: Vec<u8>,
    cle: Option<String>,
}

/// (periode, qui) -> les attestations du DERNIER retour qui le porte, lues
/// dans les retours 46, dans l'ordre de l'archive. `mois` : seulement cette periode.
fn attestations_archivees(ctx: &Contexte, mois: Option<&str>, seulement: Option<&[std::path::PathBuf]>) -> R<HashMap<(String, String), Vec<Attestation>>> {
    let dossier = ctx.racine.join(envois::retours::RETOURS);
    // `fichiers` : seulement ces archives (la releve range ce qu'elle vient de telecharger).
    let mut fichiers: Vec<String> = match seulement {
        Some(liste) => liste.iter().map(|f| f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).collect(),
        None => match std::fs::read_dir(&dossier) {
            Ok(l) => l.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.contains("_nature46_")).collect(),
            Err(_) => Vec::new(),
        },
    };
    fichiers.sort();
    let index: Vec<(String, String)> = ctx.salaries.iter().map(|(cle, v)| (cle.to_string(), format!("{} {}", v.nom.texte(), v.prenom.texte()))).collect();
    let mut vues: HashMap<(String, String), Vec<Attestation>> = HashMap::new();
    for nom in fichiers {
        let chemin = dossier.join(&nom);
        let octets = crate::routes::depots::lire_octets(&chemin.to_string_lossy())?;
        let liste = envois::retours::aer_spectacle(&octets)?;
        let cles = envois::retours::rattacher(&liste.iter().map(|a| a.qui.as_str()).collect::<Vec<_>>(), &index);
        let mut de_ce_retour: HashMap<(String, String), Vec<Attestation>> = HashMap::new();
        for (a, cle) in liste.into_iter().zip(cles) {
            if mois.is_some_and(|m| !m.is_empty()) && Some(a.periode.as_str()) != mois {
                continue;
            }
            de_ce_retour.entry((a.periode, a.qui.clone())).or_default().push(Attestation { nom_archive: a.nom_archive, qui: a.qui, donnees: a.donnees, cle });
        }
        vues.extend(de_ce_retour);
    }
    Ok(vues)
}

/// `aer_manquantes` : les salaries dont France Travail a renvoye l'AER du mois, mais dont le PDF signe
/// n'est pas dans `sorties/<mois>/envoi/`.
fn aer_manquantes(ctx: &Contexte, mois: &str) -> R<Vec<String>> {
    let dossier = ctx.racine.join("sorties").join(mois).join("envoi");
    let mut out: Vec<String> = Vec::new();
    for groupe in attestations_archivees(ctx, Some(mois), None)?.values() {
        let Some(att) = groupe.first() else { continue };
        let present = match att.cle.as_deref().filter(|c| !c.is_empty()) {
            Some(cle) => dossier.join(nom_fichier("AER", cle, &crate::routes::dossier_salarie::etiquette(mois)?)).exists(),
            None => false,
        };
        if !present {
            let nom = att.cle.clone().filter(|c| !c.is_empty()).unwrap_or_else(|| att.qui.clone());
            if !out.contains(&nom) {
                out.push(nom);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Les paies de `periode`, sous les numeros de contrat que la DSN a DECLARES (regle 4 : un numero se lit --
/// `ecrire_documents::paies_aux_numeros_declares`), sans rien dire de ce qui les fait echouer (vide).
fn paies_de_la_periode(ctx: &Contexte, periode: &str) -> Vec<engagement::calcul::Paie> {
    let Ok(departs) = envois::journal::lu(&ctx.racine).map(|j| j.departs) else { return Vec::new() };
    documents::ecrire_documents::paies_aux_numeros_declares(ctx, periode, None, &departs).map(|t| t.1).unwrap_or_default()
}

/// Toutes les periodes que l'AER de `cle` pour `periode` doit porter (`documents::aer::periodes_attendues`). `None` : la
/// saisie de la periode n'est pas dans le logiciel (un mois d'avant lui) -- rien a quoi comparer.
fn periodes_attendues(ctx: &Contexte, par_mois: &mut HashMap<String, Vec<engagement::calcul::Paie>>, periode: &str, cle: &str) -> R<Option<Vec<documents::aer::PaieAer>>> {
    if !engagement::stock::a_une_saisie(&ctx.racine, periode).unwrap_or(false) {
        return Ok(None);
    }
    let mut du_mois = |m: &str| -> Vec<engagement::calcul::Paie> { par_mois.entry(m.to_string()).or_insert_with(|| paies_de_la_periode(ctx, m)).clone() };
    documents::aer::periodes_attendues(periode, cle, &mut du_mois).map(Some).map_err(Echec::from)
}

/// `chemin_identite("cachet")` : le tampon pose, quelle que soit son extension (png, jpg, svg).
fn cachet_pose(gabarits: &Path) -> Option<String> {
    let dossier = gabarits.join("identite");
    [".png", ".jpg", ".svg"].iter().map(|e| dossier.join(format!("cachet{}", e))).find(|f| f.exists()).map(|f| f.to_string_lossy().into_owned())
}

/// `RangementAer` : ce que le rangement des AER a fait -- les PDF ecrits `(periode, cle, chemin)`, les attestations sans salarie
/// deposees a part `(qui, chemin)`, les refusees `(cle, periode, ecarts)`.
#[derive(Default)]
pub(crate) struct RangementAer {
    pub ecrits: Vec<(String, String, std::path::PathBuf)>,
    pub a_ranger: Vec<(String, std::path::PathBuf)>,
    pub refusees: Vec<(String, String, Vec<String>)>,
}

/// `ranger_aer(ctx, mois=mois)` : signe et range les AER deja archivees la ou l'envoi ira les chercher -- en phrases, pour la releve
/// et l'ouverture (qui ne signent jamais malgre un ecart).
pub(crate) fn ranger_aer(ctx: &Contexte, mois: Option<&str>, fichiers: Option<&[std::path::PathBuf]>) -> R<Vec<String>> {
    let fait = ranger_aer_detail(ctx, mois, fichiers, false)?;
    let mut phrases: Vec<String> = Vec::new();
    // `RangementAer.a_faire()` : d'abord les attestations sans salarie, puis les refusees (la paie ne les confirme pas).
    for (qui, chemin) in &fait.a_ranger {
        let refuge = chemin.parent().map(|p| p.display().to_string()).unwrap_or_default();
        phrases.push(format!("Attestation France Travail de {} : aucun salarie ne porte ce nom. Deposee dans {} ; verifier la fiche.", qui, refuge));
    }
    for (cle, periode, ecarts) in &fait.refusees {
        phrases.push(format!("Attestation France Travail de {} ({}) NON SIGNEE : elle ne dit pas ce que la paie dit ({}). La correction se fait sur la DSN, pas sur le PDF.", cle, periode, ecarts.join("; ")));
    }
    Ok(phrases)
}

/// Le rangement des AER revenues et ce qu'il a laisse. `forcer` : signer malgre un ecart (`aer --forcer`).
pub(crate) fn ranger_aer_detail(ctx: &Contexte, mois: Option<&str>, fichiers: Option<&[std::path::PathBuf]>, forcer: bool) -> R<RangementAer> {
    use documents::aer::{authentifier, Authentification, Echec as AerEchec};
    let vues = attestations_archivees(ctx, mois, fichiers)?;
    let mut par_mois: HashMap<String, Vec<engagement::calcul::Paie>> = HashMap::new();
    let mut cles: Vec<&(String, String)> = vues.keys().collect();
    cles.sort();
    // `RangementAer.a_faire()` : d'abord les attestations sans salarie, puis les refusees (la paie ne les confirme pas).
    let mut fait = RangementAer::default();
    for k in cles {
        let (periode, _) = k;
        let groupe = &vues[k];
        let cle = groupe[0].cle.clone().filter(|c| !c.is_empty());
        let Some(cle) = cle.filter(|_| !periode.is_empty()) else {
            // On ne devine pas : un nom que le rapprochement ne reconnait pas est peut-etre un salarie archive.
            let refuge = ctx.racine.join(envois::retours::RETOURS).join("aer-a-ranger");
            std::fs::create_dir_all(&refuge).map_err(|e| erreur("OSError", e.to_string()))?;
            for att in groupe {
                let nom = att.nom_archive.rsplit('/').next().unwrap_or("").to_string();
                std::fs::write(refuge.join(nom), &att.donnees).map_err(|e| erreur("OSError", e.to_string()))?;
                fait.a_ranger.push((att.qui.clone(), refuge.join(att.nom_archive.rsplit('/').next().unwrap_or(""))));
            }
            continue;
        };
        // LA VERIFICATION SE FAIT SUR TOUTES LES ATTESTATIONS DU SALARIE A LA FOIS : au-dela de cinq periodes, France
        // Travail en edite plusieurs, et chacune ne porte qu'une partie des periodes attendues.
        let mut imprime: Vec<documents::aer::Ligne> = Vec::new();
        for att in groupe {
            let champs = documents::aer::champs(&att.donnees).map_err(|e| match e {
                AerEchec::Fabrique(m) | AerEchec::Autre(m) => Echec::from(m),
                AerEchec::Ecarts(e) => Echec::from(e.join("; ")),
            })?;
            imprime.extend(documents::aer::lire_quantites(&champs).map_err(Echec::from)?);
        }
        let ecarts = match periodes_attendues(ctx, &mut par_mois, periode, &cle)? {
            Some(attendues) => documents::aer::ecarts(&imprime, &attendues),
            None => Vec::new(),
        };
        if !ecarts.is_empty() && !forcer {
            // Une seule fausse et rien ne part.
            fait.refusees.push((cle, periode.clone(), ecarts));
            continue;
        }
        let signature = documents::bulletin::identite_de(&ctx.gabarits(), "signature").map(|f| f.to_string_lossy().into_owned());
        let cachet = cachet_pose(&ctx.gabarits());
        let ent = &ctx.entreprise;
        let mut signees: Vec<Vec<u8>> = Vec::new();
        for att in groupe {
            let a = Authentification {
                ent,
                aujourdhui: ctx.aujourdhui,
                signature: signature.as_deref(),
                // Deja verifiee ci-dessus, sur l'ensemble des attestations du salarie.
                paies: None,
                forcer,
                cachet_fichier: cachet.as_deref(),
            };
            match authentifier(&att.donnees, &a) {
                Ok(octets) => signees.push(octets),
                Err(AerEchec::Ecarts(e)) => return Err(Echec::from(e.join("; "))),
                Err(AerEchec::Fabrique(m)) | Err(AerEchec::Autre(m)) => return Err(Echec::from(m)),
            }
        }
        let donnees = if signees.len() == 1 { signees.remove(0) } else { documents::fabrique::fusion_octets(&signees, &std::env::temp_dir()).map_err(Echec::from)? };
        let cible = ctx.racine.join("sorties").join(periode).join("envoi");
        std::fs::create_dir_all(&cible).map_err(|e| erreur("OSError", e.to_string()))?;
        let chemin = cible.join(nom_fichier("AER", &cle, &crate::routes::dossier_salarie::etiquette(periode)?));
        std::fs::write(&chemin, donnees).map_err(|e| erreur("OSError", e.to_string()))?;
        fait.ecrits.push((periode.clone(), cle, chemin));
    }
    Ok(fait)
}

/// Ce qui est du a chaque salarie du mois et ne se fabrique pas ici : l'AER Cinema Spectacle (`aer::due`), que France
/// Travail publie apres la DSN (retour 46), et l'attestation employeur de droit commun (`aer::attestation_fctu`), qu'elle
/// edite apres le signalement de fin de contrat. Par salarie : ce que son courriel dit absent.
fn dues_a_part(paies: &[engagement::calcul::Paie]) -> R<HashMap<String, Vec<&'static str>>> {
    let mut out: HashMap<String, Vec<&'static str>> = HashMap::new();
    for p in paies {
        let siennes = out.entry(p.salarie.id.to_string()).or_default();
        if documents::aer::due(p).map_err(Echec::from)? && !siennes.contains(&"aer") {
            siennes.push("aer");
        }
        if documents::aer::attestation_fctu(&p.engagement).map_err(Echec::from)? && !siennes.contains(&"attestation_fctu") {
            siennes.push("attestation_fctu");
        }
    }
    Ok(out)
}

/// Peut-on envoyer les documents du mois ? Ce qui l'empeche, ce qui est a savoir, et si une boite est posee.
struct Prets {
    empechements: Vec<String>,
    informations: Vec<String>,
    boite_liee: bool,
}

/// `documents_prets` : peut-on envoyer les documents du mois ? Le moteur repond, et dit pourquoi.
fn documents_prets(ctx: &Contexte, mois: &str, paies: &[engagement::calcul::Paie]) -> R<Prets> {
    // LES DOCUMENTS PARTENT DES QU'UN ENGAGEMENT EST VALIDE : ils se fabriquent a leur envoi, depuis sa paie ecrite.
    let mut empechements: Vec<String> = Vec::new();
    if paies.is_empty() {
        empechements.push("Aucun engagement valide ce mois-ci : rien ne part d'un engagement en saisie.".to_string());
    }
    // L'AER NE SE FABRIQUE PAS : France Travail la renvoie apres la DSN, et elle part avec les documents quand elle est la.
    let mut informations: Vec<String> = Vec::new();
    if crate::compagnie::etat_dsn::du_mois(ctx, Mois::lire(mois)?, None)?.aer_attendue() && dues_a_part(paies)?.values().any(|v| v.contains(&"aer")) {
        informations.push(
            "L'attestation employeur (AER) de France Travail n'est pas encore revenue : France Travail l'édite à partir de la DSN, et chaque relevé des retours la cherche. Elle partira avec un envoi suivant."
                .to_string(),
        );
    }
    let manquent = aer_manquantes(ctx, mois)?;
    if !manquent.is_empty() {
        informations.push(format!(
            "L'attestation employeur (AER) est revenue de France Travail, mais elle n'est pas signee pour {} : la paie ne confirme pas ce qu'elle affiche, ou le nom n'est pas reconnu. Voir les messages de l'ouverture.",
            manquent.join(", ")
        ));
    }
    Ok(Prets { empechements, informations, boite_liee: crate::routes::ouverture::boite_liee(ctx)? })
}

/// Un message prepare pour un salarie : ce que l'ecran montre avant l'envoi.
struct Message {
    cle: String,
    nom: String,
    destinataire: String,
    pieces: Vec<std::path::PathBuf>,
}

/// Un message par salarie, et les pieces qui lui sont DUES, nommees sans etre fabriquees : le bulletin d'une paie qui a des
/// cachets, le certificat Conges Spectacles d'un contrat qui finit (`certificats_dus`), l'AER que France Travail a rendue
/// et qui est rangee. Le corps n'est pas compose : seuls ses refus (`KeyError` d'une fiche entreprise sans nom
/// d'affichage ou sans signataire) sont gardes.
fn preparer(ctx: &Contexte, paies: &[engagement::calcul::Paie], mois: &str, dossier: &Path) -> R<Vec<Message>> {
    let etiq = crate::routes::dossier_salarie::etiquette(mois)?;
    let ent = &ctx.entreprise;
    let requis = |c: &droit::fiche::Case<String>, k: &str| if c.est_absente() { Err(erreur("KeyError", socle::fichiers::absent(k))) } else { Ok(()) };
    requis(&ent.nom_affichage, "nom_affichage")?;
    let sg = ent.signataire.dite().ok_or_else(|| erreur("KeyError", socle::fichiers::absent("signataire")))?;
    if sg.prenom_affichage.remplie().is_none() {
        requis(&sg.prenom, "prenom")?;
    }
    let (certificats, _) = crate::routes::dossier_salarie::certificats_dus(ctx, paies)?;
    let mut messages: Vec<Message> = Vec::new();
    for s in paies.iter().map(|p| &p.salarie) {
        let cle = s.id.to_string();
        if messages.iter().any(|m| m.cle == cle) {
            continue;
        }
        let de_lui = |p: &&engagement::calcul::Paie| p.salarie.id == cle.as_str();
        let mut pieces = Vec::new();
        if paies.iter().filter(de_lui).any(|p| !p.engagement.sans_paie()) {
            pieces.push(dossier.join(nom_fichier("Bulletin", &cle, &etiq)));
        }
        if certificats.iter().any(|(c, _, pc)| *c == cle && pc.is_some()) {
            pieces.push(dossier.join(nom_fichier("CertificatCS", &cle, &etiq)));
        }
        let aer = dossier.join(nom_fichier("AER", &cle, &etiq));
        if aer.exists() {
            pieces.push(aer);
        }
        messages.push(Message {
            nom: ctx.nomme(&cle),
            // Une fiche sans courriel se montre : ses documents s'impriment ; c'est l'envoi par courriel qui la refuse.
            destinataire: s.email.texte().to_lowercase(),
            cle,
            pieces,
        });
    }
    Ok(messages)
}

/// Ce qui peut partir de `mois` (`engagement::etats::validees_du_mois` : la saisie des engagements valides et leurs paies
/// ecrites), numerote. L'envoi des documents (`envoyer_documents`) prepare ses messages sur les memes paies que l'apercu.
pub(crate) fn charger_le_mois(ctx: &Contexte, mois: &str) -> R<Vec<engagement::calcul::Paie>> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let Some((_, mut paies)) = engagement::etats::validees_du_mois(ctx, mois, &departs)? else {
        return Ok(Vec::new());
    };
    documents::ecrire_documents::numeroter(ctx, &mut paies, Mois::lire(mois)?.annee(), &departs)?;
    Ok(paies)
}

/// `apercu_envoi` : ce que chaque salarie recevrait, sans rien envoyer.
fn envoi_apercu(ctx: &Contexte, le_mois: Mois) -> R<ApercuEnvoi> {
    let mois = le_mois.to_string();
    // Ce qui peut partir du mois, numerote.
    let paies = charger_le_mois(ctx, &mois)?;
    let m = Moteur::nouveau(ctx);
    let prets = documents_prets(ctx, &mois, &paies)?;
    let pret = prets.empechements.is_empty();
    let dossier = ctx.racine.join("sorties").join(&mois).join("envoi");
    // A QUI ILS PARTENT : le meme choix que l'envoi (`choisir`).
    let choix = choisir(ctx, &mois, &paies)?;
    // CE QUI PARTIRA, nomme sans etre fabrique : les documents s'ecrivent, numerotes, a l'envoi (`envoyer_documents`).
    let messages = if pret { preparer(ctx, &paies, &mois, &dossier)? } else { Vec::new() };
    let tous_a_jour = messages.iter().all(|x| choix.a_jour.contains(&x.cle));
    let messages = choix.garder(messages, |x| x.cle.as_str());
    let (mut expediteur, mut avertissements) = (String::new(), Vec::<String>::new());
    if prets.boite_liee {
        // La boite qui enverrait, quand des documents sont dus.
        if !messages.is_empty() {
            expediteur = compte_de_la_boite(ctx)?.to_lowercase();
            let sg = ctx.entreprise.signataire.dite().ok_or_else(|| erreur("KeyError", socle::fichiers::absent("signataire")))?;
            let signataire = sg.email.texte().to_lowercase();
            if !signataire.is_empty() && signataire != expediteur {
                avertissements.push(format!(
                    "Les messages partiront de {} (la boite posee), pas de {} (le courriel du signataire, fiche Entreprise).",
                    expediteur, signataire
                ));
            }
        }
        let sans: Vec<String> = messages.iter().filter(|x| x.destinataire.is_empty()).map(|x| nom_de_la_paie(&paies, &x.cle)).collect();
        if !sans.is_empty() {
            avertissements.push(sans_courriel_dit(&sans.join(", ")));
        }
    }
    let lignes = m.lignes()?;
    // Ce qui est deja parti : la meme lecture que la case Documents de l'engagement (`etapes::documents_partis`).
    let partis = crate::routes::etapes::documents_partis(ctx, &mois, &m.etats_du_mois(le_mois)?, &lignes)?;
    // « Deja envoye » ne se dit que d'un renvoi voulu, a l'identique : tous ont deja leurs documents, et leur paie n'a
    // pas change. Un renvoi de documents corriges ou un premier envoi n'envoie rien une seconde fois.
    let deja = messages
        .iter()
        .filter_map(|x| partis.get(&x.cle))
        .map(|p| p.quand.as_str())
        .max()
        .filter(|_| tous_a_jour)
        .map(|q| DejaPartis { quand: q.to_string(), quand_dit: format!("Le {}.", super::quand(q)) });
    let messages: Vec<MessageVu> = messages
        .into_iter()
        .map(|x| {
            let renvoi = choix.renvois.get(&x.cle);
            MessageVu {
                pieces: x.pieces.iter().map(|p| nom_du(p)).collect(),
                // Ce que l'ecran en dit : la sorte de chaque piece, pas son nom de fichier.
                sortes: x.pieces.iter().map(|p| crate::historique_mois::sorte_de(&nom_du(p))).collect(),
                renvoi: renvoi.is_some(),
                // Un renvoi : des documents corriges, qui remplacent ceux deja partis (a faire partir).
                pastille: renvoi.map(|r| Pastille::de("a_faire", &format!("documents corrigés — remplacent ceux du {}", r.envoye_le))),
                changements: renvoi
                    .map(|r| {
                        if r.changements.is_empty() {
                            vec![envois::envoi::RIEN_DANS_LE_RESUME.trim_start_matches([' ', '-']).to_string()]
                        } else {
                            r.changements.iter().map(|l| l.trim_start().trim_start_matches("- ").to_string()).collect()
                        }
                    })
                    .unwrap_or_default(),
                cle: x.cle,
                nom: x.nom,
                destinataire: x.destinataire,
            }
        })
        .collect();
    let mut phrases: Vec<String> = messages.iter().map(|x| format!("{} ({}) : {}", x.nom, x.destinataire, x.sortes.join(", "))).collect();
    phrases.extend(prets.empechements.iter().chain(&avertissements).chain(&prets.informations).cloned());
    Ok(ApercuEnvoi {
        phrase: format!("Documents à envoyer par courriel, de {} : {}.", expediteur, super::compte(messages.len(), "message", "messages")),
        expediteur_dit: format!("Courriels depuis {}", expediteur),
        expediteur,
        avertissements,
        messages,
        deja,
        pret,
        empechements: prets.empechements,
        informations: prets.informations,
        boite_liee: prets.boite_liee,
        phrases,
    })
}

/// L'adresse de la boite qui enverrait. Celle de la compagnie fictive est le courriel du
/// signataire ; sinon l'adresse posee avec le mot de passe (jamais le mot de passe).
fn compte_de_la_boite(ctx: &Contexte) -> R<String> {
    if crate::compagnie::bridage::branchement(&ctx.racine).joue() {
        let mail = ctx.entreprise.signataire.dite().and_then(|s| s.email.remplie()).map(String::from);
        return Ok(mail.unwrap_or_else(|| "fictive@puppetpaie.invalid".to_string()));
    }
    let coffre = crate::compagnie::coffre::Coffre::du_poste(&ctx.racine, "");
    let vers = |x: crate::compagnie::coffre::Echec| Echec { classe: Some(x.classe), message: x.message };
    let a = match std::env::var("COURRIEL_COMPTE").ok().filter(|v| !v.is_empty()) {
        Some(a) => a,
        None => coffre.compte("courriel").map_err(vers)?,
    };
    Ok(a.trim().to_string())
}

// ------------------------------------------------------------------ ce que toutes les routes partagent

/// `ErreurOperation` : un refus ordinaire, avec sa phrase.
pub(crate) fn refus(message: impl Into<String>) -> Echec {
    erreur("ErreurOperation", message)
}

/// `d[cle]` : `KeyError` si elle manque.
pub(crate) fn requis<'a>(d: &'a Value, cle: &str) -> R<&'a Value> {
    match d {
        Value::Object(o) => o.get(cle).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle))),
        autre => Err(erreur("TypeError", socle::fichiers::attendu("un objet", documents::valeur::nom_type(autre)))),
    }
}

/// La boite qui envoie, selon le branchement de la compagnie (`bridage::branchement`) : le simulateur ecrit les messages
/// sans les envoyer ; le message refuse, avant la preuve et avant toute ligne au journal des depots ; le monde prend la
/// vraie boite, et sans mot de passe d'application au coffre, il n'y en a pas (`BoiteAbsente`).
pub(crate) fn client_courriel(ctx: &Contexte) -> R<Box<dyn Courriel>> {
    let branchement = crate::compagnie::bridage::branchement(&ctx.racine);
    branchement.ouvrir()?;
    if branchement.joue() {
        return Ok(Box::new(BoiteSimulee::du_contexte(ctx)));
    }
    if !crate::compagnie::boite::disponible(&ctx.racine) {
        return Err(erreur(
            "BoiteAbsente",
            socle::echec::cle("Aucune boîte courriel : poser le mot de passe d'application de la boîte dans Configuration › Mots de passe.", "courriel"),
        ));
    }
    let client = crate::compagnie::boite::client_du_contexte(ctx).map_err(|e| erreur("ErreurCourriel", e.0))?;
    Ok(Box::new(client))
}

/// Les identifiants de la saisie : `expediteur`, la boite qui envoie, en minuscules.
pub(super) fn expediteur_de(client: &dyn Courriel) -> String {
    client.compte().to_lowercase()
}

// ------------------------------------------------------------------ envoyer-documents

/// La charge de `envoyer-documents` : le mois, les salaries choisis (tous sinon), et le mot de passe du logiciel tape
/// pour l'envoi.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeEnvoyerDocuments {
    mois: Mois,
    #[serde(default)]
    salaries: Vec<String>,
    #[serde(default)]
    motdepasse: Option<String>,
}

/// Envoie a chaque salarie ses documents du mois. Irreversible.
fn envoyer_documents(ctx: &Contexte, c: ChargeEnvoyerDocuments, ouvrir: Ouvrir) -> R<DocumentsEnvoyes> {
    let mois = c.mois.to_string();
    let preuve = Preuve::du_mot_de_passe(c.motdepasse);
    let seulement = Some(c.salaries).filter(|l| !l.is_empty());
    let prepare = preparer_envoi(ctx, &mois, seulement.as_deref(), Some(&preuve), ouvrir)?;
    expedier_envoi(ctx, prepare, &mois, &preuve, INTERFACE)
}

/// `envoyer_documents(ctx, mois, preuve=..., qui=...)` : la meme operation, `qui` et la preuve donnes (l'horloge de
/// la compagnie fictive passe `horloge` et une boite muette).
pub(crate) fn envoyer_documents_de(ctx: &Contexte, mois: &str, preuve: &Preuve, qui: &str, ouvrir: Ouvrir) -> R<DocumentsEnvoyes> {
    let prepare = preparer_envoi(ctx, mois, None, Some(preuve), ouvrir)?;
    expedier_envoi(ctx, prepare, mois, preuve, qui)
}

/// Les pieces d'un salarie, dans l'ordre ou elles s'impriment : bulletin, certificat Conges Spectacles, attestation
/// France Travail (celles qui sont ecrites).
const PIECES_A_IMPRIMER: [&str; 3] = ["Bulletin", "CertificatCS", "AER"];

/// La route `documents-imprimer` : les documents du mois des salaries choisis (`salaries`, tous sinon), fabriques comme
/// pour l'envoi par courriel (`depart::fabriquer_documents`), reunis en un seul PDF a imprimer, salarie par salarie, et
/// remis en main propre. L'IMPRESSION EST UN DEPART : ce qui est remis au salarie est parti. Les numeros des bulletins et
/// des certificats s'ecrivent dans les paies, le journal note `documents` (`main_propre`) avec la version de chaque
/// engagement, et la paie est figee comme par le courriel. Rien ne quitte le poste : aucun mot de passe.
pub(crate) fn documents_imprimer(ctx: &Contexte, c: ChargeDocumentsImprimer, qui: &str) -> R<DocumentsImprimes> {
    let mois = c.mois.to_string();
    let paies = charger_le_mois(ctx, &mois)?;
    if paies.is_empty() {
        return Err(erreur(
            "AucunDocument",
            format!("{} : aucun engagement valide, aucun document a imprimer. Rien ne part d'un engagement en saisie.", Mois::en_clair(&mois)),
        ));
    }
    let mut cles: Vec<String> = Vec::new();
    let mut noms: HashMap<String, String> = HashMap::new();
    for p in &paies {
        let cle = p.salarie.id.to_string();
        if cles.contains(&cle) || (!c.salaries.is_empty() && !c.salaries.contains(&cle)) {
            continue;
        }
        noms.insert(cle.clone(), p.salarie.nomme().unwrap_or_else(|| (&cle).to_string()));
        cles.push(cle);
    }
    if cles.is_empty() {
        return Err(refus("Aucun salarié coché. Rien n'est imprimé."));
    }
    let choix = choisir(ctx, &mois, &paies)?;
    let documents = crate::compagnie::depart::fabriquer_documents(ctx, &mois, &cles, qui)?;
    let dossier = ctx.racine.join("sorties").join(&mois).join("envoi");
    let etiq = c.mois.etiquette();
    let mut pieces: Vec<std::path::PathBuf> = Vec::new();
    let mut salaries: Vec<journal::Servi> = Vec::new();
    for cle in &cles {
        let siennes: Vec<std::path::PathBuf> =
            PIECES_A_IMPRIMER.iter().map(|prefixe| dossier.join(format!("{}_{}_{}.pdf", prefixe, cle, etiq))).filter(|f| f.is_file()).collect();
        salaries.push(journal::Servi {
            cle: droit::contexte::IdSalarie::lire(cle)?,
            nom: noms.get(cle).cloned(),
            pieces: siennes.iter().map(|p| nom_du(p)).collect(),
            resume: choix.resumes.get(cle).cloned(),
            renvoi: choix.renvois.contains_key(cle),
        });
        pieces.extend(siennes);
    }
    if pieces.is_empty() {
        return Err(refus(format!("Aucun document écrit pour {} : rien à imprimer.", Mois::en_clair(&mois))));
    }
    let versions = journal::versions_du_mois(ctx, &mois, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?, |cle, _, _| cles.iter().any(|c| c == cle)).map_err(Echec::from)?;
    let impression = crate::compagnie::signature::reunir_pour_imprimer(&dossier, "documents", &etiq, &pieces)?;
    let avertissements = documents.avertissements.clone();
    documents.engager(ctx, &mois)?;
    let detail = journal::DetailDocuments { salaries, main_propre: true, expediteur: None, durees_ms: None, partis: None, echec_a: None };
    let reponse = format!("documents de {} salarie(s) imprimes pour une remise en main propre", cles.len());
    noter(ctx, journal::Ligne::Documents(par_courriel(Some(c.mois), qui, true, "", &reponse, detail, Some(versions))))?;
    let qui_les_recoit: Vec<String> = cles.iter().map(|cle| nom_a_l_ecran(ctx, cle, || noms.get(cle).cloned().unwrap_or_else(|| cle.clone()))).collect();
    Ok(DocumentsImprimes {
        phrase: format!("Documents {} imprimés pour {}, à remettre en main propre.", socle::texte::de(&Mois::en_clair(&mois)), qui_les_recoit.join(", ")),
        impression: nom_du(&impression),
        chemin: impression.to_string_lossy().into_owned(),
        salaries: cles,
        phrases: avertissements.clone(),
        avertissements,
    })
}

/// Le nom du salarie `cle` tel que l'ecran le montre (`routes::nom_a_l_ecran`), lu de sa fiche ; `sinon` quand la fiche
/// n'est plus la.
fn nom_a_l_ecran(ctx: &Contexte, cle: &str, sinon: impl FnOnce() -> String) -> String {
    ctx.salaries.get(cle).map(|s| super::nom_a_l_ecran(s)).unwrap_or_else(sinon)
}

/// Les messages, le compte verifie -- rien n'est parti.
pub(crate) struct EnvoiPrepare {
    pub(crate) messages: Vec<envoi::Message>,
    pub(crate) expediteur: String,
    pub(crate) avertissements: Vec<String>,
    /// A qui les documents partent, et pourquoi (`choisir`) : le journal en garde l'empreinte et le resume.
    pub(crate) choix: Choix,
    /// Les documents fabriques pour ceux a qui ils partent ; leurs numeros s.ecrivent quand ils sont partis.
    documents: crate::compagnie::depart::DocumentsFabriques,
    client: Box<dyn Courriel>,
    /// La preuve deja verifiee avant la fabrique (`preparer_envoi` avec une preuve) : l.expedition ne la redemande pas.
    preuve: Option<Preuve>,
    /// La duree de la preparation (lecture du mois, messages, boite), en millisecondes : le journal la garde.
    preparation_ms: u128,
}
/// A QUI LES DOCUMENTS DU MOIS PARTENT. Chaque salarie du mois est dans l'un de trois cas, lus dans le journal
/// (`crate::routes::etapes::documents_partis` : ce que l'engagement sait de ce qui est parti) et dans ses engagements :
///
/// * ses documents ne sont jamais partis : ils partent, avec le premier courriel ;
/// * ils sont partis, et la paie d'un de ses engagements a change depuis la version sur laquelle ils sont partis
///   (`engagement::etats::a_refaire`, un bulletin rectificatif) : ils repartent refaits, avec le courriel qui dit ce qui
///   a change (`envoi::CORPS_RENVOI`, `envoi::ce_qui_a_change` : ce que dit la frise, puis le brut et le net) ;
/// * ils sont partis et sa paie n'a pas bouge : ils ne repartent pas.
///
/// Quand TOUS sont a jour, l'envoi est un renvoi voulu, a l'identique : tous repartent, avec le premier courriel.
#[derive(Default)]
pub(crate) struct Choix {
    /// Ceux dont la paie a change depuis l'envoi de leurs documents : le jour de l'envoi, ce qui a change.
    pub(crate) renvois: HashMap<String, envoi::Renvoi>,
    /// Ceux dont les documents sont partis et dont la paie n'a pas change.
    pub(crate) a_jour: std::collections::HashSet<String>,
    /// Le resume de la paie de chaque salarie du mois, a noter au journal avec l'envoi.
    pub(crate) resumes: HashMap<String, envoi::ResumeDePaie>,
}

impl Choix {
    /// Ce qui part, parmi les messages de tout le mois : ceux qui ne sont pas a jour ; s'ils le sont tous, tous.
    pub(crate) fn garder<T>(&self, messages: Vec<T>, cle: impl Fn(&T) -> &str) -> Vec<T> {
        if messages.iter().all(|m| self.a_jour.contains(cle(m))) {
            return messages;
        }
        messages.into_iter().filter(|m| !self.a_jour.contains(cle(m))).collect()
    }
}

/// Les salaries de `mois` dont un engagement a un bulletin a refaire (la paie de ce mois a change depuis la version sur
/// laquelle ses documents sont partis, `engagement::etats::a_refaire`), et ce qui a change dans leurs engagements depuis
/// cette version (`engagement::versions::depuis`, ce que dit la frise) ; un salarie a plusieurs engagements nomme
/// chacun par son contrat.
pub(crate) fn bulletins_a_refaire(ctx: &Contexte, mois: &str) -> R<HashMap<String, Vec<String>>> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let mut par_salarie: HashMap<String, Vec<(String, Vec<String>)>> = HashMap::new();
    for e in engagement::etats::du_mois(ctx, mois, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?)? {
        let Some(v) = e.valide() else { continue };
        let depuis = engagement::etats::a_refaire(ctx, v, &departs).into_iter().find_map(|a| match a {
            engagement::etats::ARefaire::Bulletin { mois: m, depuis } if m == mois => Some(depuis),
            _ => None,
        });
        let Some(depuis) = depuis else { continue };
        let brut = &*v.objet().brut;
        if let Some(c) = engagement::cle_de_l_engagement(ctx, &brut) {
            let contrat = brut.saisie.carte.numero.clone().unwrap_or_default();
            par_salarie.entry(c.into()).or_default().push((contrat, engagement::versions::depuis(ctx, v, depuis)));
        }
    }
    Ok(par_salarie
        .into_iter()
        .map(|(c, engs)| {
            let plusieurs = engs.len() > 1;
            let faits = engs
                .into_iter()
                .flat_map(|(contrat, faits)| faits.into_iter().map(move |f| if plusieurs { format!("contrat n° {} : {}", contrat, f) } else { f }))
                .collect();
            (c, faits)
        })
        .collect())
}

/// Le choix, pour le mois `mois`, sur ses paies validees (`crate::routes::documents::charger_le_mois`).
pub(crate) fn choisir(ctx: &Contexte, mois: &str, paies: &[engagement::calcul::Paie]) -> R<Choix> {
    let lignes = envois::journal::lu(&ctx.racine).map(|j| j.lignes)?;
    let partis = crate::routes::etapes::documents_partis(ctx, mois, &engagement::etats::du_mois(ctx, mois, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?).map_err(Echec::from)?, &lignes)?;
    let cles: Vec<String> = paies.iter().map(|p| p.salarie.id.to_string()).collect();
    let mut resumes: HashMap<String, envoi::ResumeDePaie> = HashMap::new();
    for cle in &cles {
        if resumes.contains_key(cle) {
            continue;
        }
        let siennes: Vec<&engagement::calcul::Paie> = paies.iter().zip(&cles).filter(|(_, c)| *c == cle).map(|(p, _)| p).collect();
        resumes.insert(cle.clone(), envoi::resume_de(&siennes).map_err(Echec::from)?);
    }
    let changes = bulletins_a_refaire(ctx, mois)?;
    let mut choix = Choix { resumes, ..Choix::default() };
    let declaration = declaration_de_la_correction(ctx, mois)?;
    for (cle, parti) in &partis {
        let Some(faits) = changes.get(cle) else {
            choix.a_jour.insert(cle.clone());
            continue;
        };
        let quand = parti.quand.clone();
        let envoye_le = jour_iso(quand.get(..10).unwrap_or("")).map(|j| j.jj_mm_aaaa()).unwrap_or(quand);
        let changements = envoi::ce_qui_a_change(faits, parti.resume.as_ref(), choix.resumes.get(cle));
        let declaration = declaration.as_ref().map(|d| d(cle)).unwrap_or_default();
        choix.renvois.insert(cle.clone(), envoi::Renvoi { envoye_le, changements, declaration });
    }
    Ok(choix)
}

/// LA CORRECTION D'UN MOIS DONT L'ANNULE-ET-REMPLACE EST PASSE : le courriel de renvoi ajoute une phrase, la correction
/// sera declaree avec la paie d'un mois suivant. La DSN du mois ne se refait pas (`se_corrige_ailleurs` : sa DSN est
/// partie et l'annule-et-remplace n'est plus admis, ou c'est celle de l'ancien logiciel) ; une correction deja portee
/// par une DSN partie se lit dans ce qui est parti (`corrections_du_mois`), une correction a porter part dans la DSN
/// qui porte le mois (`crate::regularisation::dsn_qui_porte`). `None` : le mois se corrige dans sa propre DSN.
#[allow(clippy::type_complexity)]
fn declaration_de_la_correction(ctx: &Contexte, mois: &str) -> R<Option<Box<dyn Fn(&str) -> String>>> {
    let departs = journal::lu(&ctx.racine).map(|j| j.departs)?;
    if !declarations::produire_regularisations::se_corrige_ailleurs(ctx, &departs, mois)? {
        return Ok(None);
    }
    let porteur = crate::regularisation::dsn_qui_porte(ctx, mois)?;
    let (a_porter, portees) = crate::regularisation::corrections_du_mois(ctx, mois)?;
    let mut porteurs: HashMap<String, String> = HashMap::new();
    for c in &portees {
        porteurs.insert(valeur_en_texte(&c["salarie"]), valeur_en_texte(&c["portee_par"]));
    }
    for c in &a_porter {
        porteurs.remove(&valeur_en_texte(&c["salarie"]));
    }
    let de = crate::regularisation::de_mois_en_clair(mois);
    Ok(Some(Box::new(move |cle: &str| {
        format!(
            "Ta paie {} était déjà déclarée : la correction sera déclarée avec la paie {}.\n\n",
            de,
            crate::regularisation::de_mois_en_clair(porteurs.get(cle).unwrap_or(&porteur))
        )
    })))
}

/// La saisie du mois, la boite, la preuve (`preuve` : un envoi ; `None` : ce qui partirait, sans rien envoyer), puis les
/// documents et les messages. Une boite absente se voit avant la preuve ; une preuve refusee n.ecrit rien. Rien n.est parti.
pub(crate) fn preparer_envoi(ctx: &Contexte, mois: &str, seulement: Option<&[String]>, preuve: Option<&Preuve>, ouvrir: Ouvrir) -> R<EnvoiPrepare> {
    let debut = std::time::Instant::now();
    let paies = charger_le_mois(ctx, mois)?;
    if paies.is_empty() {
        return Err(erreur(
            "AucunDocument",
            format!("{} : aucun engagement validé, aucun document à envoyer. Rien ne part d'un engagement en saisie.", socle::mois::Mois::en_clair(mois)),
        ));
    }
    let dossier = ctx.racine.join("sorties").join(mois).join("envoi");
    let choix = choisir(ctx, mois, &paies)?;
    // LES DOCUMENTS SE FABRIQUENT ICI, pour ceux a qui ils partent : depuis les paies ecrites, a leurs numeros.
    let mut cles: Vec<String> = Vec::new();
    for p in &paies {
        let cle = p.salarie.id.to_string();
        if !cles.contains(&cle) {
            cles.push(cle);
        }
    }
    let cles: Vec<String> = choix.garder(cles, |c| c.as_str()).into_iter().filter(|c| seulement.is_none_or(|s| s.contains(c))).collect();
    if cles.is_empty() {
        return Err(refus("Aucun salarié coché. Rien n'est parti."));
    }
    // UN SALARIE SANS COURRIEL n'empeche pas les autres : ses documents ne partent pas par courriel, ils s'impriment
    // (`documents-imprimer`) ; les autres partent.
    let (cles, sans): (Vec<String>, Vec<String>) = cles.into_iter().partition(|c| a_un_courriel(&paies, c));
    let noms_sans = sans.iter().map(|c| nom_de_la_paie(&paies, c)).collect::<Vec<_>>().join(", ");
    if cles.is_empty() {
        return Err(refus(format!(
            "Pas de courriel dans la fiche de {} : ses documents ne partent pas par courriel. Imprimez-les pour les remettre en main propre, ou saisissez son courriel dans sa fiche. Rien n'est parti.",
            noms_sans
        )));
    }
    // La boite, puis LA PREUVE, AVANT TOUTE FABRIQUE : un mot de passe refuse n'ecrit rien.
    let client = ouvrir()?;
    let expediteur = expediteur_de(client.as_ref());
    let preuve = match preuve {
        Some(p) => {
            let a_qui = format!("{} salarie(s) : {}", cles.len(), cles.iter().map(|c| nom_de_la_paie(&paies, c)).collect::<Vec<_>>().join(", "));
            Some(Preuve { donnee: Some(verifier_preuve(ctx, Some(p), mois, "envoi des documents du mois aux salaries", &a_qui)?), ..Preuve::default() })
        }
        None => None,
    };
    let documents = crate::compagnie::depart::fabriquer_documents(ctx, mois, &cles, "interface")?;
    let paies_servies: Vec<engagement::calcul::Paie> = paies.iter().filter(|p| cles.contains(&p.salarie.id.to_string())).cloned().collect();
    let messages = envoi::preparer_avec(&paies_servies, &ctx.entreprise, mois, &dossier, &choix.renvois, &dues_a_part(&paies_servies)?).map_err(|m| refus_de_preparation(&m))?;
    let messages: Vec<envoi::Message> = choix.garder(messages, |m| m.cle.as_str()).into_iter().filter(|m| cles.contains(&m.cle)).collect();
    let mut avertissements: Vec<String> = documents.avertissements.clone();
    if !sans.is_empty() {
        avertissements.push(sans_courriel_dit(&noms_sans));
    }
    let signataire = ctx.entreprise.signataire.dite().ok_or_else(|| erreur("KeyError", socle::fichiers::absent("signataire")))?.email.texte().to_lowercase();
    if !signataire.is_empty() && signataire != expediteur {
        avertissements.push(format!(
            "Les messages partiront de {} (la boite posee), pas de {} (le courriel du signataire, fiche Entreprise).",
            expediteur, signataire
        ));
    }
    Ok(EnvoiPrepare { messages, expediteur, avertissements, choix, documents, client, preuve, preparation_ms: socle::temps::duree(debut).as_millis() })
}

/// Ce qui suit la preparation : la PREUVE (le mot de passe du logiciel), l'envoi, le journal. `qui` : la ligne du journal.
pub(crate) fn expedier_envoi(ctx: &Contexte, prepare: EnvoiPrepare, mois: &str, preuve: &Preuve, qui: &str) -> R<DocumentsEnvoyes> {
    let EnvoiPrepare { messages, expediteur, avertissements, choix, documents, mut client, preuve: verifiee, preparation_ms } = prepare;
    let preuve = verifiee.as_ref().unwrap_or(preuve);
    let a_qui = format!("{} salarie(s) : {}", messages.len(), messages.iter().map(|m| m.nom.as_str()).collect::<Vec<_>>().join(", "));
    // LA VERSION SUR LAQUELLE ILS PARTENT, engagement par engagement des salaries servis.
    let servis: Vec<&str> = messages.iter().map(|m| m.cle.as_str()).collect();
    let versions = journal::versions_du_mois(ctx, mois, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?, |cle, _, _| servis.contains(&cle)).map_err(Echec::from)?;
    // LA DUREE DE CHAQUE ETAPE (le journal dit ou va le temps d'un envoi).
    let t_preuve = std::time::Instant::now();
    verifier_preuve(ctx, Some(preuve), mois, "envoi des documents du mois aux salaries", &a_qui)?;
    let preuve_ms = socle::temps::duree(t_preuve).as_millis();
    let t_envoi = std::time::Instant::now();
    let envoye = envoi::envoyer_suivi(&messages, &expediteur, client.as_mut(), None, None);
    let envoi_ms = socle::temps::duree(t_envoi).as_millis();
    // Ce que chaque salarie servi a recu : ses pieces, le resume de sa paie (ce qui dira, si elle change, ce qui a change),
    // et si c'etait un renvoi.
    let salaries = |servis: &[envoi::Message]| -> R<Vec<journal::Servi>> {
        servis
            .iter()
            .map(|m| {
                Ok(journal::Servi {
                    cle: droit::contexte::IdSalarie::lire(&m.cle)?,
                    nom: Some(m.nom.clone()),
                    pieces: m.pieces.iter().map(|p| nom_du(p)).collect(),
                    resume: choix.resumes.get(&m.cle).cloned(),
                    renvoi: choix.renvois.contains_key(&m.cle),
                })
            })
            .collect()
    };
    let durees = journal::Durees { preparation: preparation_ms as u64, mot_de_passe: preuve_ms as u64, envoi: envoi_ms as u64 };
    let le_mois = Some(socle::mois::Mois::lire(mois)?);
    let detail = |servis: &[envoi::Message]| -> R<journal::DetailDocuments> {
        Ok(journal::DetailDocuments { salaries: salaries(servis)?, main_propre: false, expediteur: Some(expediteur.clone()), durees_ms: Some(durees.clone()), partis: None, echec_a: None })
    };
    let partis = match envoye {
        Ok(p) => p,
        Err(e) => {
            // CE QUI A ATTEINT LE SALARIE EST PARTI : les messages partent dans l'ordre, les premiers sont servis. Leurs
            // numeros, et seulement les leurs, s'ecrivent d'abord ; la ligne porte ensuite la version de leurs
            // engagements, et seulement les leurs.
            let servis = &messages[..e.partis.len().min(messages.len())];
            let mut versions_servies = Vec::new();
            if !servis.is_empty() {
                let cles: Vec<&str> = servis.iter().map(|m| m.cle.as_str()).collect();
                documents.engager_servis(ctx, mois, &cles)?;
                versions_servies = versions.iter().filter(|v| cles.contains(&v.salarie.as_str())).cloned().collect();
            }
            noter_echec_d_envoi(ctx, journal::Ligne::Documents, le_mois, qui, &e.partis, &e.echoue, &e.message, detail(servis)?, Some(versions_servies))?;
            return Err(echec_d_envoi(e.message));
        }
    };
    // LES DOCUMENTS SONT PARTIS : leurs numeros s'ecrivent dans les paies des engagements, puis la ligne du journal dit
    // qu'ils sont partis, et sur quelle version.
    documents.engager(ctx, mois)?;
    let reponse = format!("{} message(s) envoye(s)", partis.len());
    noter(ctx, journal::Ligne::Documents(par_courriel(le_mois, qui, true, &partis.join(", "), &reponse, detail(&messages)?, Some(versions))))?;
    let noms: Vec<String> = messages.iter().map(|m| nom_a_l_ecran(ctx, &m.cle, || m.nom.clone())).collect();
    Ok(DocumentsEnvoyes {
        phrase: format!("Documents {} envoyés à {} : {}.", socle::texte::de(&Mois::en_clair(mois)), crate::routes::compte(partis.len(), "salarié", "salariés"), noms.join(", ")),
        partis,
        avertissements,
    })
}

/// La fiche du salarie `cle`, telle que ses paies du mois la portent, a-t-elle un courriel ?
fn a_un_courriel(paies: &[engagement::calcul::Paie], cle: &str) -> bool {
    paies.iter().any(|p| p.salarie.id == cle && !p.salarie.email.texte().is_empty())
}

/// « NOM Prenom » du salarie `cle`, lu dans ses paies du mois.
fn nom_de_la_paie(paies: &[engagement::calcul::Paie], cle: &str) -> String {
    paies
        .iter()
        .find(|p| p.salarie.id == cle)
        .map(|p| {
            p.salarie.nomme().unwrap_or_else(|| (cle).to_string())
        })
        .unwrap_or_else(|| cle.to_string())
}

/// Ce que l'envoi dit de ceux qui n'ont pas de courriel : leurs documents ne sont pas partis, ils s'impriment.
fn sans_courriel_dit(noms: &str) -> String {
    format!(
        "Pas de courriel dans la fiche de {} : ses documents ne partent pas par courriel. Imprimez-les pour les remettre en main propre, ou saisissez son courriel dans sa fiche.",
        noms
    )
}

/// Ceux a qui les documents du mois sont a faire partir (`choisir`) et qui n'ont pas de courriel dans leur fiche : leurs
/// documents s'impriment. Leur cle, et leur nom.
pub(crate) fn sans_courriel(ctx: &Contexte, mois: &str) -> R<Vec<(String, String)>> {
    let paies = charger_le_mois(ctx, mois)?;
    let mut cles: Vec<String> = Vec::new();
    for p in &paies {
        let c = p.salarie.id.to_string();
        if !cles.contains(&c) {
            cles.push(c);
        }
    }
    let choix = choisir(ctx, mois, &paies)?;
    Ok(cles.into_iter().filter(|c| !choix.a_jour.contains(c) && !a_un_courriel(&paies, c)).map(|c| { let nom = nom_de_la_paie(&paies, &c); (c, nom) }).collect())
}

/// Le refus de `envoi.preparer` : une fiche sans adresse est un `ValueError`, une cle absente un `KeyError`.
fn refus_de_preparation(m: &str) -> Echec {
    if m.starts_with("Pas d'adresse e-mail") {
        erreur("ValueError", m)
    } else if m.starts_with("champ ") && m.ends_with(" absent") {
        erreur("KeyError", m)
    } else {
        Echec::from(m.to_string())
    }
}

