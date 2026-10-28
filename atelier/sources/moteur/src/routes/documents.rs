//! Les routes des documents du mois : ce que la boite montre avant l'envoi (`envoi-apercu`), les envoyer par courriel
//! (`envoyer-documents`) ou les imprimer (`documents-imprimer`) ; l'un et l'autre sont un depart (bulletins et
//! certificats numerotes, une ligne au journal, la paie figee). Et ce que les envois partagent avec `contrats` et
//! `archives` : la boite qui envoie (`client_courriel`), la preuve (`preuve_de`), les lectures de la charge.
//!
//! La seule ecriture de l'apercu : le rangement des AER revenues (`ranger_aer`) dans `sorties/<mois>/envoi/`.

use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::depot_acte::{verifier_preuve, Preuve};
use crate::routes::etapes::Moteur;
use crate::routes::qui_de;
use droit::contexte::Contexte;
use envois::courriel::{BoiteSimulee, Courriel};
use envois::envoi;
use envois::journal;
use envois::ligne_d_envoi::{echec_d_envoi, noter_echec_d_envoi, noter_envoi};
use socle::date::jour_iso;
use socle::echec::{erreur, Echec};
use socle::fichiers::{nom_du, vrai};
use socle::valeur::valeur_en_texte;



pub(crate) type R<T> = Result<T, Echec>;

/// De quoi ouvrir la boite : appele UNE fois par envoi. Les tests passent un faux client du trait `Courriel`, ou la
/// boite simulee ; une route ouvre la vraie (`client_courriel`).
pub type Ouvrir<'a> = &'a mut dyn FnMut() -> R<Box<dyn Courriel>>;

/// Les routes de ce module.
pub const ROUTES: &[&str] = &["envoi-apercu", "envoyer-documents", "documents-imprimer"];

/// Celles qui ecrivent.
pub const ECRITURES: &[&str] = &["envoyer-documents", "documents-imprimer"];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "envoi-apercu" => envoi_apercu(ctx, charge),
        "envoyer-documents" => envoyer_documents(ctx, charge, &mut || client_courriel(ctx)),
        "documents-imprimer" => documents_imprimer(ctx, charge, &qui_de(charge)),
        autre => Err(Echec { classe: None, message: format!("route inconnue : {}", autre) }),
    }
}

// ------------------------------------------------------------------ les outils

/// `charge["cle"]` : `KeyError` si elle manque.
pub(super) fn cle<'a>(charge: &'a Value, k: &str) -> R<&'a Value> {
    charge.get(k).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k)))
}

/// `Path.glob("<prefixe>*<suffixe>")` sur un dossier : y a-t-il un nom qui s'y accorde ? Un dossier
/// qui n'en est pas un, ou qu'on ne peut lire, ne rend rien.
pub(crate) fn un_nom_commence_et_finit(dossier: &Path, prefixe: &str, suffixe: &str) -> bool {
    let Ok(lecture) = std::fs::read_dir(dossier) else { return false };
    lecture.flatten().any(|e| {
        let n = e.file_name().to_string_lossy().into_owned();
        n.len() >= prefixe.len() + suffixe.len() && n.starts_with(prefixe) && n.ends_with(suffixe)
    })
}

pub(super) fn nul() -> &'static Value {
    static V: Value = Value::Null;
    &V
}

// ------------------------------------------------------------------ signature-apercu



/// `d["cle"]` sur une fiche : `KeyError` si elle manque.
pub(super) fn get_requis<'a>(d: &'a Value, k: &str) -> R<&'a Value> {
    d.get(k).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k)))
}

// ------------------------------------------------------------------ avenant-apercu

/// `jj_mm_aaaa(x)` : `JJ/MM/AAAA`, `ValueError` si `x` ne se lit pas comme une date.
pub(super) fn jj_mm_aaaa(v: &Value) -> R<String> {
    Ok(socle::date::d(v).map_err(|m| erreur("ValueError", m))?.jj_mm_aaaa())
}




// ------------------------------------------------------------------ envoi-apercu

/// `int(texte)` d'un morceau de `mois` : `ValueError` sinon.
fn entier(t: &str) -> R<i64> {
    let s = t.trim();
    let (neg, chiffres) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    if chiffres.is_empty() || !chiffres.chars().all(|c| c.is_ascii_digit()) {
        return Err(erreur("ValueError", format!("nombre entier illisible : {}", socle::fichiers::repr(t))));
    }
    let n: i64 = chiffres.parse().unwrap_or(i64::MAX);
    Ok(if neg { -n } else { n })
}

/// `(int(mois[:4]), int(mois[5:7]))`.
fn an_et_mois(mois: &str) -> R<(i64, i64)> {
    let tranche = |a: usize, b: usize| -> String { mois.chars().skip(a).take(b - a).collect() };
    Ok((entier(&tranche(0, 4))?, entier(&tranche(5, 7))?))
}

/// Le nom du mois `mo` (`date::nom_du_mois`) ; hors 1-12, `IndexError` (l'indice negatif du Python lisait « 00 »
/// decembre : il se refuse).
fn nom_du_mois(mo: i64, g: socle::date::Graphie) -> R<&'static str> {
    socle::date::nom_du_mois(mo, g).map_err(|e| erreur("IndexError", e))
}

/// `envoi.etiquette` : `2026-08` -> `Aout2026`.
fn etiquette(mois: &str) -> R<String> {
    let (an, mo) = an_et_mois(mois)?;
    Ok(format!("{}{}", nom_du_mois(mo, socle::date::Graphie::Titre)?, an))
}

/// `envoi.nom_fichier` : `AER_<cle>_<etiquette>.pdf`.
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

/// `_attestations_archivees` : (periode, qui) -> les attestations du DERNIER retour qui le porte, lues
/// dans les retours 46, dans l'ordre de l'archive. `mois` : seulement cette periode.
fn attestations_archivees(ctx: &Contexte, mois: Option<&str>, seulement: Option<&[std::path::PathBuf]>) -> R<HashMap<(String, String), Vec<Attestation>>> {
    let dossier = ctx.racine.join("retours");
    // `fichiers` : seulement ces archives (la releve range ce qu'elle vient de telecharger).
    let mut fichiers: Vec<String> = match seulement {
        Some(liste) => liste.iter().map(|f| f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).collect(),
        None => match std::fs::read_dir(&dossier) {
            Ok(l) => l.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.contains("_nature46_")).collect(),
            Err(_) => Vec::new(),
        },
    };
    fichiers.sort();
    let index: Vec<Value> = ctx
        .salaries
        .iter()
        .filter(|(_, v)| v.is_object())
        .map(|(cle, v)| {
            let champ = |c: &str| v.get(c).map_or("".to_string(), valeur_en_texte);
            json!([cle, format!("{} {}", champ("nom"), champ("prenom"))])
        })
        .collect();
    let mut vues: HashMap<(String, String), Vec<Attestation>> = HashMap::new();
    for nom in fichiers {
        let chemin = dossier.join(&nom);
        let octets = crate::routes::depots::lire_octets(&chemin.to_string_lossy())?;
        let atts = envois::retours::aer_spectacle(&octets)?;
        let liste = atts.as_array().cloned().unwrap_or_default();
        let cles = if liste.is_empty() {
            Value::Array(Vec::new())
        } else {
            let quis: Vec<Value> = liste.iter().map(|a| a["qui"].clone()).collect();
            envois::retours::rattacher(&quis, &index)
        };
        let mut de_ce_retour: HashMap<(String, String), Vec<Attestation>> = HashMap::new();
        for (a, c) in liste.iter().zip(cles.as_array().map(Vec::as_slice).unwrap_or(&[])) {
            let periode = valeur_en_texte(&a["periode"]);
            if mois.is_some_and(|m| !m.is_empty()) && Some(periode.as_str()) != mois {
                continue;
            }
            let qui = valeur_en_texte(&a["qui"]);
            let donnees = socle::base64::lire(a["donnees"].as_str().unwrap_or("")).map_err(Echec::from)?;
            de_ce_retour.entry((periode, qui.clone())).or_default().push(Attestation {
                nom_archive: valeur_en_texte(&a["nom_archive"]),
                qui,
                donnees,
                cle: c.as_str().map(str::to_string),
            });
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
            Some(cle) => dossier.join(nom_fichier("AER", cle, &etiquette(mois)?)).exists(),
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
    documents::ecrire_documents::paies_aux_numeros_declares(ctx, periode, None).map(|t| t.1).unwrap_or_default()
}

/// `mois_suivant("2026-12")` = `"2027-01"`.
fn mois_suivant(m: &str) -> String {
    let (a, mo): (i32, u32) = (m.get(..4).and_then(|x| x.parse().ok()).unwrap_or(0), m.get(5..7).and_then(|x| x.parse().ok()).unwrap_or(12));
    if mo >= 12 {
        format!("{:04}-01", a + 1)
    } else {
        format!("{:04}-{:02}", a, mo + 1)
    }
}

/// Toutes les periodes que l'AER de `cle` pour `periode` doit porter (`aer::ecarts`) : chaque contrat dont l'AER est due
/// ce mois-ci (`aer::due`), mois par mois depuis son debut, chacun lu dans la paie de son mois. `None` : la saisie de la
/// periode n'est pas dans le logiciel (un mois d'avant lui) -- rien a quoi comparer.
fn periodes_attendues(ctx: &Contexte, par_mois: &mut HashMap<String, Vec<engagement::calcul::Paie>>, periode: &str, cle: &str) -> R<Option<Vec<documents::aer::PaieAer>>> {
    if !engagement::stock::a_une_saisie(&ctx.racine, periode).unwrap_or(false) {
        return Ok(None);
    }
    let mut du_mois = |m: &str| -> Vec<engagement::calcul::Paie> {
        par_mois.entry(m.to_string()).or_insert_with(|| paies_de_la_periode(ctx, m)).clone()
    };
    let de_lui = |p: &engagement::calcul::Paie| p.salarie.get("_cle").map(valeur_en_texte).as_deref() == Some(cle);
    let mut out = Vec::new();
    for p in du_mois(periode).iter().filter(|p| de_lui(p)) {
        let e = &p.engagement;
        if !documents::aer::due(e).map_err(Echec::from)? {
            continue;
        }
        let numero = e.numero.clone().unwrap_or_default();
        let mut m = e.debut.iso()[..7].to_string();
        while m.as_str() < periode {
            for q in du_mois(&m).iter().filter(|q| de_lui(q) && q.engagement.numero.as_deref() == Some(numero.as_str()) && !q.engagement.sans_paie()) {
                out.push(documents::aer::PaieAer::du_mois(&q.engagement).map_err(Echec::from)?);
            }
            m = mois_suivant(&m);
        }
        if !e.sans_paie() {
            out.push(documents::aer::PaieAer::du_mois(e).map_err(Echec::from)?);
        }
    }
    Ok(Some(out))
}

/// Un dossier temporaire, retire quand on a fini.
struct Temporaire(std::path::PathBuf);

impl Temporaire {
    fn nouveau() -> R<Temporaire> {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!("assemblage-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
        std::fs::create_dir_all(&p).map_err(|e| erreur("OSError", e.to_string()))?;
        Ok(Temporaire(p))
    }
}

impl Drop for Temporaire {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `fabrique.assembler` : les PDF mis a la suite.
fn assembler(pieces: &[Vec<u8>]) -> R<Vec<u8>> {
    let tmp = Temporaire::nouveau()?;
    let mut liste = Vec::new();
    for (i, octets) in pieces.iter().enumerate() {
        let f = tmp.0.join(format!("piece{}.pdf", i));
        std::fs::write(&f, octets).map_err(|e| erreur("OSError", e.to_string()))?;
        liste.push(json!({"fichier": f.to_string_lossy()}));
    }
    let description = json!({"format": 1, "pieces": liste});
    fabrique_pdf::assemblage::assembler(&description.to_string()).map_err(|r| Echec::from(r.0))
}

/// `chemin_identite("cachet")` : le tampon pose, quelle que soit son extension (png, jpg, svg).
fn cachet_pose(ent: &Value) -> Option<String> {
    let dossier = documents::bulletin::gabarits_de(ent)?.join("identite");
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
        phrases.push(format!("Attestation France Travail de {} : aucun salarie ne porte ce nom. Deposee dans {} ; verifier la fiche, puis `moteur aer`.", qui, refuge));
    }
    for (cle, periode, ecarts) in &fait.refusees {
        phrases.push(format!("Attestation France Travail de {} ({}) NON SIGNEE : elle ne dit pas ce que la paie dit ({}). La correction se fait sur la DSN, pas sur le PDF.", cle, periode, ecarts.join("; ")));
    }
    Ok(phrases)
}

/// `ranger_aer` du Python, structure : le rangement et ce qu'il a laisse. `forcer` : signer malgre un ecart (`aer --forcer`).
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
            let refuge = ctx.racine.join("retours").join("aer-a-ranger");
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
        let signature = documents::bulletin::identite_de(&ctx.entreprise_et_gabarits(), "signature").map(|f| f.to_string_lossy().into_owned());
        let cachet = cachet_pose(&ctx.entreprise_et_gabarits());
        let mut signees: Vec<Vec<u8>> = Vec::new();
        for att in groupe {
            let a = Authentification {
                ent: &ctx.entreprise,
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
        let donnees = if signees.len() == 1 { signees.remove(0) } else { assembler(&signees)? };
        let cible = ctx.racine.join("sorties").join(periode).join("envoi");
        std::fs::create_dir_all(&cible).map_err(|e| erreur("OSError", e.to_string()))?;
        let chemin = cible.join(nom_fichier("AER", &cle, &etiquette(periode)?));
        std::fs::write(&chemin, donnees).map_err(|e| erreur("OSError", e.to_string()))?;
        fait.ecrits.push((periode.clone(), cle, chemin));
    }
    Ok(fait)
}

/// Une AER Cinema Spectacle est-elle due a l'un des salaries du mois (`aer::due`) ? Sans elle, France Travail n'en
/// publie aucune (pas de retour 46) : le mois d'un permanent seul n'a rien a attendre.
fn aer_attendue(paies: &[engagement::calcul::Paie]) -> R<bool> {
    for p in paies {
        if documents::aer::due(&p.engagement).map_err(Echec::from)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `documents_prets` : peut-on envoyer les documents du mois ? Le moteur repond, et dit pourquoi.
fn documents_prets(m: &mut Moteur, ctx: &Contexte, mois: &str, paies: &[engagement::calcul::Paie]) -> R<Map<String, Value>> {
    let dossier = ctx.racine.join("sorties").join(mois).join("envoi");
    // LES DOCUMENTS PARTENT DES QU'UN ENGAGEMENT EST VALIDE : ils se fabriquent a leur envoi, depuis sa paie ecrite.
    let mut empechements: Vec<String> = Vec::new();
    if paies.is_empty() {
        empechements.push("Aucun engagement valide ce mois-ci : rien ne part d'un engagement en saisie.".to_string());
    }
    // L'AER NE SE FABRIQUE PAS : France Travail la renvoie apres la DSN, et elle part avec les documents quand elle est la.
    let mut informations: Vec<String> = Vec::new();
    let e = m.dsn_du_mois(mois)?;
    if vrai(&e) && e.get("passee").is_some_and(vrai) && !e.get("aer_recue").is_some_and(vrai) && aer_attendue(paies)? {
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
    let mut r = Map::new();
    r.insert("pret".into(), json!(empechements.is_empty()));
    r.insert("empechements".into(), json!(empechements));
    r.insert("informations".into(), json!(informations));
    r.insert("boite_liee".into(), json!(crate::routes::ouverture::boite_liee(ctx)?));
    r.insert("dossier".into(), json!(dossier.to_string_lossy()));
    Ok(r)
}

/// Un message prepare pour un salarie : ce que l'ecran montre avant l'envoi.
struct Message {
    cle: String,
    nom: String,
    destinataire: String,
    sujet: String,
    pieces: Vec<std::path::PathBuf>,
}

impl Message {
    /// `Message.taille` : l'octet de chaque piece qui existe.
    fn taille(&self) -> u64 {
        self.pieces.iter().filter_map(|p| std::fs::metadata(p).ok()).map(|m| m.len()).sum()
    }
}

/// `envoi.preparer` : un message par salarie, a partir des pieces individuelles deja ecrites. Le texte
/// du corps, que l'apercu ne montre pas, n'est pas compose : seuls ses refus (`KeyError` d'une fiche
/// sans nom d'affichage ou sans signataire) sont gardes.
fn preparer(ctx: &Contexte, paies: &[engagement::calcul::Paie], mois: &str, dossier: &Path, renvois: &HashMap<String, envois::envoi::Renvoi>) -> R<Vec<Message>> {
    const PIECES: [(&str, &str, &str); 4] = [
        ("bulletin", "Bulletin", "bulletin"),
        ("rectificatif", "BulletinRectificatif", "bulletin rectificatif"),
        ("certificat_cs", "CertificatCS", "certificat Congés Spectacles"),
        ("aer", "AER", "attestation France Travail"),
    ];
    let (an, mo) = an_et_mois(mois)?;
    let etiq = etiquette(mois)?;
    let mut salaries: Vec<Value> = paies.iter().map(|p| p.salarie.clone()).collect();
    // Celui qui n'a AUCUNE paie ce mois-ci mais un bulletin rectificatif d'un mois precedent.
    let prefixe = "BulletinRectificatif_";
    let suffixe = format!("_{}.pdf", etiq);
    let mut rectifs: Vec<String> = match std::fs::read_dir(dossier) {
        Ok(l) => l
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.len() >= prefixe.len() + suffixe.len() && n.starts_with(prefixe) && n.ends_with(&suffixe))
            .collect(),
        Err(_) => Vec::new(),
    };
    rectifs.sort();
    for f in rectifs {
        let cle: String = f.chars().skip(prefixe.chars().count()).take(f.chars().count() - prefixe.chars().count() - suffixe.chars().count()).collect();
        let connues: Vec<String> = salaries.iter().map(|s| s.get("_cle").map_or("None".to_string(), valeur_en_texte)).collect();
        if !connues.contains(&cle) {
            let mut fiche = ctx.salarie(&cle).map_err(|m| erreur("KeyError", m))?;
            if let Some(o) = fiche.as_object_mut() {
                o.insert("_cle".into(), Value::String(cle.clone()));
            }
            salaries.push(fiche);
        }
    }
    let mut vus: Vec<String> = Vec::new();
    let mut messages = Vec::new();
    for s in &salaries {
        let cle = valeur_en_texte(get_requis(s, "_cle")?);
        if vus.contains(&cle) {
            continue;
        }
        vus.push(cle.clone());
        let (mut pieces, mut courts) = (Vec::new(), Vec::new());
        for (_, prefixe, court) in PIECES {
            let chemin = dossier.join(nom_fichier(prefixe, &cle, &etiq));
            if chemin.exists() {
                pieces.push(chemin);
                courts.push(court);
            }
        }
        // Ce que le corps du message lit : `ent["nom_affichage"]`, `ent["signataire"]`.
        let ent = &ctx.entreprise;
        get_requis(ent, "nom_affichage")?;
        let sg = get_requis(ent, "signataire")?;
        if !sg.get("prenom_affichage").is_some_and(vrai) {
            get_requis(sg, "prenom")?;
        }
        let sujet = format!(
            "{} {} — {}{}",
            capitaliser(nom_du_mois(mo, socle::date::Graphie::Accentuee)?),
            an,
            if renvois.contains_key(&cle) { "documents corrigés : " } else { "" },
            match courts.len() {
                0 => "documents".to_string(),
                1 => courts[0].to_string(),
                n => format!("{} et {}", courts[..n - 1].join(", "), courts[n - 1]),
            }
        );
        messages.push(Message {
            nom: format!("{} {}", valeur_en_texte(get_requis(s, "prenom_affichage")?), valeur_en_texte(get_requis(s, "nom")?)),
            // Une fiche sans courriel se montre : ses documents s'impriment ; c'est l'envoi par courriel qui la refuse.
            destinataire: s.get("email").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default().to_lowercase(),
            cle,
            sujet,
            pieces,
        });
    }
    Ok(messages)
}

/// `str.capitalize()`.
fn capitaliser(t: &str) -> String {
    let mut c = t.chars();
    match c.next() {
        Some(p) => p.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
        None => String::new(),
    }
}

/// Ce qui peut partir de `mois` (`engagement::etats::validees_du_mois` : la saisie des engagements valides et leurs paies
/// ecrites), numerote. L'envoi des documents (`envoyer_documents`) prepare ses messages sur les memes paies que l'apercu.
pub(crate) fn charger_le_mois(ctx: &Contexte, mois: &str) -> R<(Value, Vec<engagement::calcul::Paie>)> {
    let Some((data, mut paies)) = engagement::etats::validees_du_mois(ctx, mois)? else {
        return Ok((json!({ "mois": mois, "engagements": [] }), Vec::new()));
    };
    let annee: i32 = mois.chars().take(4).collect::<String>().parse().map_err(|_| erreur("ValueError", format!("mois illisible : {}", socle::fichiers::repr(mois))))?;
    documents::ecrire_documents::numeroter(ctx, &mut paies, annee)?;
    Ok((data, paies))
}

/// `apercu_envoi` : ce que chaque salarie recevrait, sans rien envoyer.
fn envoi_apercu(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois_v = cle(charge, "mois")?;
    let mois = valeur_en_texte(mois_v);
    // LE RATTRAPAGE : une AER relevee avant que la releve ne sache la ranger se range ici.
    if !aer_manquantes(ctx, &mois)?.is_empty() {
        let _ = ranger_aer(ctx, Some(&mois), None)?;
    }
    // Ce qui peut partir du mois, numerote.
    let (data, paies) = charger_le_mois(ctx, &mois)?;
    let mut m = Moteur::nouveau(ctx);
    let prets = documents_prets(&mut m, ctx, &mois, &paies)?;
    let dossier = ctx.racine.join("sorties").join(&mois).join("envoi");
    // A QUI ILS PARTENT (ticket #240) : le meme choix que l'envoi (`crate::routes::documents::choisir`).
    let choix = crate::routes::documents::choisir(ctx, &mois, &paies)?;
    // CE QUI PARTIRA, FABRIQUE TEL QUEL pour etre montre : leurs numeros ne s.ecrivent qu.a l.envoi (`envoyer_documents`).
    let messages = if prets.get("pret").is_some_and(vrai) {
        let mut cles: Vec<String> = Vec::new();
        for p in &paies {
            let c = p.salarie.get("_cle").map(valeur_en_texte).unwrap_or_default();
            if !cles.contains(&c) {
                cles.push(c);
            }
        }
        let cles = choix.garder(cles, |c| c.as_str());
        crate::depart::fabriquer_documents(ctx, &mois, &cles, "interface")?;
        preparer(ctx, &paies, &mois, &dossier, &choix.renvois)?
    } else {
        Vec::new()
    };
    let tous_a_jour = messages.iter().all(|x| choix.a_jour.contains(&x.cle));
    let renvoi_voulu = !messages.is_empty() && tous_a_jour;
    let messages = choix.garder(messages, |x| x.cle.as_str());
    let (mut expediteur, mut avertissements) = (String::new(), Vec::<String>::new());
    if prets.get("boite_liee").is_some_and(vrai) {
        // `preparer_envoi` : la saisie rechargee, les messages refaits, le compte de la boite.
        let mois_saisie = data.get("mois").map(socle::date::en_texte).ok_or_else(|| socle::fichiers::absent("mois"))?;
        let dossier = ctx.racine.join("sorties").join(&mois_saisie).join("envoi");
        if dossier.exists() {
            preparer(ctx, &paies, &mois_saisie, &dossier, &choix.renvois)?;
            expediteur = compte_de_la_boite(ctx)?.to_lowercase();
            let sg = get_requis(&ctx.entreprise, "signataire")?;
            let signataire = sg.get("email").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default().to_lowercase();
            if !signataire.is_empty() && signataire != expediteur {
                avertissements.push(format!(
                    "Les messages partiront de {} (la boite posee), pas de {} (le courriel du signataire, fiche Entreprise).",
                    expediteur, signataire
                ));
            }
        }
    }
    let lignes = m.lignes()?;
    // Ce qui est deja parti : la meme lecture que la case Documents de l'engagement (`etapes::documents_partis`).
    let partis = crate::routes::etapes::documents_partis(ctx, &mois, &m.etats_du_mois(&mois)?, &lignes)?;
    let deja = messages.iter().filter_map(|x| partis.get(&x.cle)).filter_map(|p| p["quand"].as_str()).max().map(|q| json!({"quand": q}));
    let mut r = Map::new();
    r.insert("mois".into(), mois_v.clone());
    r.insert("expediteur".into(), json!(expediteur));
    r.insert("avertissements".into(), json!(avertissements));
    r.insert(
        "messages".into(),
        Value::Array(
            messages
                .iter()
                .map(|x| {
                    let renvoi = choix.renvois.get(&x.cle);
                    json!({"cle": x.cle, "nom": x.nom, "destinataire": x.destinataire, "sujet": x.sujet,
                           "pieces": x.pieces.iter().map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).collect::<Vec<_>>(),
                           "octets": x.taille(),
                           // Un renvoi : le jour du premier envoi, et ce que le courriel dira avoir change.
                           "renvoi": renvoi.is_some(),
                           "envoye_le": renvoi.map(|r| r.envoye_le.clone()),
                           "changements": renvoi.map(|r| if r.changements.is_empty() {
                               vec![envois::envoi::RIEN_DANS_LE_RESUME.trim_start_matches([' ', '-']).to_string()]
                           } else {
                               r.changements.iter().map(|l| l.trim_start().trim_start_matches("- ").to_string()).collect()
                           }).unwrap_or_default()})
                })
                .collect(),
        ),
    );
    // « Deja envoye » ne se dit que d'un renvoi voulu, a l'identique : tous ont deja leurs documents, et leur paie n'a
    // pas change. Un renvoi de documents corriges ou un premier envoi n'envoie rien une seconde fois.
    r.insert("deja".into(), if tous_a_jour { json!(deja) } else { Value::Null });
    let renvois = messages.iter().filter(|x| choix.renvois.contains_key(&x.cle)).count();
    let n = messages.len();
    r.insert("renvois".into(), json!(renvois));
    r.insert(
        "bouton".into(),
        json!(if renvois > 0 && renvois == n {
            format!("Renvoyer les documents corrigés à {} salarié{}", n, if n > 1 { "s" } else { "" })
        } else if renvois > 0 {
            format!("Envoyer à {} salarié{} ({} corrigé{} à renvoyer)", n, if n > 1 { "s" } else { "" }, renvois, if renvois > 1 { "s" } else { "" })
        } else if renvoi_voulu {
            format!("Renvoyer à {} salarié{}", n, if n > 1 { "s" } else { "" })
        } else {
            format!("Envoyer à {} salarié{}", n, if n > 1 { "s" } else { "" })
        }),
    );
    r.extend(prets);
    Ok(Value::Object(r))
}

/// `client.compte` : l'adresse de la boite qui enverrait. Celle de la compagnie fictive est le courriel du
/// signataire ; sinon l'adresse posee avec le mot de passe (jamais le mot de passe).
fn compte_de_la_boite(ctx: &Contexte) -> R<String> {
    if crate::bridage::branchement(&ctx.racine).joue() {
        let sg = ctx.entreprise.get("signataire").filter(|v| vrai(v));
        let mail = sg.and_then(|s| s.get("email")).filter(|v| vrai(v)).map(valeur_en_texte);
        return Ok(mail.unwrap_or_else(|| "fictive@puppetpaie.invalid".to_string()));
    }
    let coffre = crate::coffre::Coffre::du_poste(&ctx.racine, "");
    let vers = |x: crate::coffre::Echec| Echec { classe: Some(x.classe), message: x.message };
    let a = match std::env::var("COURRIEL_COMPTE").ok().filter(|v| !v.is_empty()) {
        Some(a) => a,
        None => coffre.compte("courriel").map_err(vers)?,
    };
    Ok(a.trim().to_string())
}

// ------------------------------------------------------------------ les contrats (`envois::envois_signature`)

/// Les elements d'une liste de la charge, ou rien : `charge.get(cle)` quand il est vrai.
pub(super) fn liste_de<'a>(charge: &'a Value, cle: &str) -> R<Option<&'a Vec<Value>>> {
    match charge.get(cle).filter(|v| vrai(v)) {
        None => Ok(None),
        Some(Value::Array(a)) => Ok(Some(a)),
        Some(autre) => Err(erreur("TypeError", format!("`{}` : une liste est attendue, pas {}", cle, socle::valeur::nom_type(autre)))),
    }
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
        autre => Err(erreur("TypeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(autre)))),
    }
}

/// `d.get(cle) or defaut` en texte : la valeur quand elle est vraie.
pub(crate) fn vrai_ou(d: &Value, cle: &str) -> Option<String> {
    d.get(cle).filter(|v| vrai(v)).map(valeur_en_texte)
}

/// `charge[cle]` en texte (`KeyError` si elle manque).
pub(crate) fn charge_texte(charge: &Value, cle: &str) -> R<String> {
    Ok(valeur_en_texte(requis(charge, cle)?))
}

/// `Preuve(motdepasse=charge.get("motdepasse"))`.
pub(crate) fn preuve_de(charge: &Value) -> Preuve {
    Preuve::de_la_charge(charge)
}

/// La boite qui envoie, selon le branchement de la compagnie (`bridage::branchement`) : le simulateur ecrit les messages
/// sans les envoyer ; le message refuse, avant la preuve et avant toute ligne au journal des depots ; le monde prend la
/// vraie boite, et sans mot de passe d'application au coffre, il n'y en a pas (`BoiteAbsente`).
pub(crate) fn client_courriel(ctx: &Contexte) -> R<Box<dyn Courriel>> {
    let branchement = crate::bridage::branchement(&ctx.racine);
    branchement.ouvrir()?;
    if branchement.joue() {
        return Ok(Box::new(BoiteSimulee::du_contexte(ctx)));
    }
    if !crate::boite::disponible(&ctx.racine) {
        return Err(erreur(
            "BoiteAbsente",
            "Aucune boite courriel : poser le mot de passe d'application de la boite (Configuration > Mots de passe, ou `moteur coffre poser courriel --compte adresse@...`).",
        ));
    }
    let client = crate::boite::client_du_contexte(ctx).map_err(|e| erreur("ErreurCourriel", e.0))?;
    Ok(Box::new(client))
}

/// Les identifiants de la saisie : `expediteur`, la boite qui envoie, en minuscules.
pub(super) fn expediteur_de(client: &dyn Courriel) -> String {
    client.compte().to_lowercase()
}

// ------------------------------------------------------------------ envoyer-documents

/// Envoie a chaque salarie ses documents du mois. Irreversible.
fn envoyer_documents(ctx: &Contexte, charge: &Value, ouvrir: Ouvrir) -> R<Value> {
    let mois = charge_texte(charge, "mois")?;
    let prepare = preparer_envoi(ctx, &mois, salaries_choisis(charge)?.as_deref(), ouvrir)?;
    expedier_envoi(ctx, prepare, &mois, &preuve_de(charge), &qui_de(charge))
}

/// `envoyer_documents(ctx, mois, preuve=..., qui=...)` : la meme operation, `qui` et la preuve donnes (l'horloge de
/// la compagnie fictive passe `horloge` et une boite muette).
pub(crate) fn envoyer_documents_de(ctx: &Contexte, mois: &str, preuve: &Preuve, qui: &str, ouvrir: Ouvrir) -> R<Value> {
    let prepare = preparer_envoi(ctx, mois, None, ouvrir)?;
    expedier_envoi(ctx, prepare, mois, preuve, qui)
}

/// Les salaries que la charge choisit (`salaries`), ou tous quand elle n'en dit rien.
fn salaries_choisis(charge: &Value) -> R<Option<Vec<String>>> {
    Ok(liste_de(charge, "salaries")?.map(|l| l.iter().map(valeur_en_texte).collect()))
}

/// Les pieces d'un salarie, dans l'ordre ou elles s'impriment : bulletin, bulletin rectificatif, certificat Conges
/// Spectacles, attestation France Travail (celles qui sont ecrites).
const PIECES_A_IMPRIMER: [&str; 4] = ["Bulletin", "BulletinRectificatif", "CertificatCS", "AER"];

/// La route `documents-imprimer` : les documents du mois des salaries choisis (`salaries`, tous sinon), fabriques comme
/// pour l'envoi par courriel (`depart::fabriquer_documents`), reunis en un seul PDF a imprimer, salarie par salarie, et
/// remis en main propre. L'IMPRESSION EST UN DEPART : ce qui est remis au salarie est parti. Les numeros des bulletins et
/// des certificats s'ecrivent dans les paies, le journal note `documents` (`main_propre`) avec la version de chaque
/// engagement, et la paie est figee comme par le courriel. Rien ne quitte le poste : aucun mot de passe.
pub(crate) fn documents_imprimer(ctx: &Contexte, charge: &Value, qui: &str) -> R<Value> {
    let mois = charge_texte(charge, "mois")?;
    let seulement = salaries_choisis(charge)?;
    let (data, paies) = crate::routes::documents::charger_le_mois(ctx, &mois)?;
    if paies.is_empty() {
        return Err(erreur(
            "AucunDocument",
            format!("{} : aucun engagement valide, aucun document a imprimer. Rien ne part d'un engagement en saisie.", engagement::libelle_mois(&mois)),
        ));
    }
    let mois_saisie = data.get("mois").map(socle::date::en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("mois")))?;
    let mut cles: Vec<String> = Vec::new();
    let mut noms: HashMap<String, String> = HashMap::new();
    for p in &paies {
        let cle = p.salarie.get("_cle").map(valeur_en_texte).unwrap_or_default();
        if cles.contains(&cle) || seulement.as_ref().is_some_and(|s| !s.contains(&cle)) {
            continue;
        }
        let prenom = vrai_ou(&p.salarie, "prenom_affichage").or_else(|| vrai_ou(&p.salarie, "prenom")).unwrap_or_default();
        noms.insert(cle.clone(), format!("{} {}", prenom, vrai_ou(&p.salarie, "nom").unwrap_or_default()).trim().to_string());
        cles.push(cle);
    }
    if cles.is_empty() {
        return Err(refus("Aucun salarié coché. Rien n'est imprimé."));
    }
    let choix = choisir(ctx, &mois, &paies)?;
    let documents = crate::depart::fabriquer_documents(ctx, &mois_saisie, &cles, qui)?;
    let dossier = ctx.racine.join("sorties").join(&mois_saisie).join("envoi");
    let etiq = socle::date::etiquette_du_mois(&mois_saisie).map_err(|m| erreur("ValueError", m))?;
    let mut pieces: Vec<std::path::PathBuf> = Vec::new();
    let mut salaries: Vec<Value> = Vec::new();
    for cle in &cles {
        let siennes: Vec<std::path::PathBuf> =
            PIECES_A_IMPRIMER.iter().map(|prefixe| dossier.join(format!("{}_{}_{}.pdf", prefixe, cle, etiq))).filter(|f| f.is_file()).collect();
        salaries.push(json!({
            "cle": cle, "nom": noms.get(cle), "pieces": siennes.iter().map(|p| nom_du(p)).collect::<Vec<_>>(),
            "resume": choix.resumes.get(cle), "renvoi": choix.renvois.contains_key(cle),
        }));
        pieces.extend(siennes);
    }
    if pieces.is_empty() {
        return Err(refus(format!("Aucun document écrit pour {} : rien à imprimer.", engagement::libelle_mois(&mois))));
    }
    let versions = journal::versions_du_mois(ctx, &mois, |cle, _, _| cles.iter().any(|c| c == cle)).map_err(Echec::from)?;
    let impression = envois::envois_signature::reunir_pour_imprimer(&dossier, "documents", &etiq, &pieces)?;
    let avertissements = documents.avertissements.clone();
    documents.engager(ctx, &mois_saisie)?;
    let detail = json!({"salaries": salaries, "main_propre": true});
    let reponse = format!("documents de {} salarie(s) imprimes pour une remise en main propre", cles.len());
    let ligne = noter_envoi(ctx, "documents", Some(&mois), qui, true, "", &reponse, Some(&detail), Some(&versions))?;
    Ok(json!({"impression": nom_du(&impression), "salaries": cles, "avertissements": avertissements, "ligne": ligne}))
}

/// Les messages, le compte verifie -- rien n'est parti.
pub(crate) struct EnvoiPrepare {
    pub(crate) mois_saisie: String,
    pub(crate) messages: Vec<envoi::Message>,
    pub(crate) expediteur: String,
    pub(crate) avertissements: Vec<String>,
    /// A qui les documents partent, et pourquoi (`choisir`) : le journal en garde l'empreinte et le resume.
    pub(crate) choix: Choix,
    /// Les documents fabriques pour ceux a qui ils partent ; leurs numeros s.ecrivent quand ils sont partis.
    documents: crate::depart::DocumentsFabriques,
    client: Box<dyn Courriel>,
    /// La duree de la preparation (lecture du mois, messages, boite), en millisecondes : le journal la garde.
    preparation_ms: u128,
}
/// A QUI LES DOCUMENTS DU MOIS PARTENT. Chaque salarie du mois est dans l'un de trois cas, lus dans le journal
/// (`crate::routes::etapes::documents_partis` : ce que l'engagement sait de ce qui est parti) et dans ses engagements :
///
/// * ses documents ne sont jamais partis : ils partent, avec le premier courriel ;
/// * ils sont partis, et la paie d'un de ses engagements a change depuis la version sur laquelle ils sont partis
///   (`engagement::etats::a_refaire`, un bulletin rectificatif) : ils repartent refaits, avec le courriel qui dit ce qui
///   a change (`envoi::CORPS_RENVOI`, `envoi::ce_qui_a_change` sur le resume note a l'envoi) ;
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
    pub(crate) resumes: HashMap<String, Value>,
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

/// Les salaries de `mois` dont un engagement a un bulletin a refaire : la paie de ce mois a change depuis la version sur
/// laquelle ses documents sont partis (`engagement::etats::a_refaire`).
pub(crate) fn bulletins_a_refaire(ctx: &Contexte, mois: &str) -> R<std::collections::HashSet<String>> {
    let departs = envois::journal::departs(&ctx.racine)?;
    let mut out = std::collections::HashSet::new();
    for e in engagement::etats::du_mois(ctx, mois)? {
        let Some(v) = e.valide() else { continue };
        let refaire = engagement::etats::a_refaire(ctx, v, &departs)
            .into_iter()
            .any(|a| matches!(a, engagement::etats::ARefaire::Bulletin { mois: ref m, .. } if m == mois));
        if refaire {
            if let Some(c) = engagement::cle_de_l_engagement(ctx, &Value::Object(v.objet().brut.clone())) {
                out.insert(c);
            }
        }
    }
    Ok(out)
}

/// Le choix, pour le mois `mois`, sur ses paies validees (`crate::routes::documents::charger_le_mois`).
pub(crate) fn choisir(ctx: &Contexte, mois: &str, paies: &[engagement::calcul::Paie]) -> R<Choix> {
    let lignes = envois::etats::charger(&ctx.racine)?;
    let partis = crate::routes::etapes::documents_partis(ctx, mois, &engagement::etats::du_mois(ctx, mois).map_err(Echec::from)?, &lignes)?;
    let cles: Vec<String> = paies.iter().map(|p| p.salarie.get("_cle").map(valeur_en_texte).unwrap_or_default()).collect();
    let mut resumes: HashMap<String, Value> = HashMap::new();
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
        if !changes.contains(cle) {
            choix.a_jour.insert(cle.clone());
            continue;
        }
        let quand = parti.get("quand").map(valeur_en_texte).unwrap_or_default();
        let envoye_le = jour_iso(quand.get(..10).unwrap_or("")).map(|j| j.jj_mm_aaaa()).unwrap_or(quand);
        let ancien = parti.get("resume").cloned().unwrap_or(Value::Null);
        let changements = envoi::ce_qui_a_change(&ancien, choix.resumes.get(cle).unwrap_or(&Value::Null));
        let declaration = declaration.as_ref().map(|d| d(cle)).unwrap_or_default();
        choix.renvois.insert(cle.clone(), envoi::Renvoi { envoye_le, changements, declaration });
    }
    Ok(choix)
}


/// LA CORRECTION D'UN MOIS DONT L'ANNULE-ET-REMPLACE EST PASSE : le courriel de renvoi ajoute une phrase, la correction
/// sera declaree avec la paie d'un mois suivant. La DSN deposee ne se refait plus ; la correction part dans la DSN qui la
/// porte (`crate::regularisation::corrections_du_mois` : celle d'un mois suivant deja partie, ou la DSN du mois
/// courant). `None` : le mois n'en est pas la (pas de DSN deposee, ou son annule-et-remplace encore admis).
#[allow(clippy::type_complexity)]
fn declaration_de_la_correction(ctx: &Contexte, mois: &str) -> R<Option<Box<dyn Fn(&str) -> String>>> {
    let mut m = crate::routes::etapes::Moteur::nouveau(ctx);
    let etat = crate::regularisation::etat_mois(&mut m, ctx, &json!(mois))?;
    if !vrai(&etat["regularisable"]) {
        return Ok(None);
    }
    let courant = valeur_en_texte(&etat["mois_courant"]);
    let (a_porter, portees) = crate::regularisation::corrections_du_mois(ctx, mois)?;
    let mut porteurs: HashMap<String, String> = HashMap::new();
    for c in &portees {
        porteurs.insert(valeur_en_texte(&c["salarie"]), valeur_en_texte(&c["portee_par"]));
    }
    for c in &a_porter {
        porteurs.insert(valeur_en_texte(&c["salarie"]), courant.clone());
    }
    let de = crate::regularisation::de_mois_en_clair(mois);
    Ok(Some(Box::new(move |cle: &str| match porteurs.get(cle) {
        Some(p) => format!(
            "Ta paie {} était déjà déclarée : la correction sera déclarée avec la paie {}.\n\n",
            de,
            crate::regularisation::de_mois_en_clair(p)
        ),
        None => format!("Ta paie {} était déjà déclarée : la correction sera déclarée avec une paie suivante.\n\n", de),
    })))
}

/// La saisie du mois, les messages, puis la boite -- une boite absente se voit
/// avant la preuve. Rien n'est parti.
pub(crate) fn preparer_envoi(ctx: &Contexte, mois: &str, seulement: Option<&[String]>, ouvrir: Ouvrir) -> R<EnvoiPrepare> {
    let debut = std::time::Instant::now();
    let (data, paies) = crate::routes::documents::charger_le_mois(ctx, mois)?;
    if paies.is_empty() {
        return Err(erreur(
            "AucunDocument",
            format!("{} : aucun engagement valide, aucun document a envoyer. Rien ne part d'un engagement en saisie.", engagement::libelle_mois(mois)),
        ));
    }
    let mois_saisie = data.get("mois").map(socle::date::en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("mois")))?;
    let dossier = ctx.racine.join("sorties").join(&mois_saisie).join("envoi");
    let fiche = |cle: &str| ctx.salarie(cle).unwrap_or_else(|_| json!({}));
    let choix = choisir(ctx, mois, &paies)?;
    // LES DOCUMENTS SE FABRIQUENT ICI, pour ceux a qui ils partent : depuis les paies ecrites, a leurs numeros.
    let mut cles: Vec<String> = Vec::new();
    for p in &paies {
        let cle = p.salarie.get("_cle").map(valeur_en_texte).unwrap_or_default();
        if !cles.contains(&cle) {
            cles.push(cle);
        }
    }
    let cles: Vec<String> = choix.garder(cles, |c| c.as_str()).into_iter().filter(|c| seulement.is_none_or(|s| s.contains(c))).collect();
    if cles.is_empty() {
        return Err(refus("Aucun salarié coché. Rien n'est parti."));
    }
    let documents = crate::depart::fabriquer_documents(ctx, &mois_saisie, &cles, "interface")?;
    let messages = envoi::preparer_avec(&paies, &ctx.entreprise, &mois_saisie, &dossier, Some(&fiche), &choix.renvois).map_err(|m| refus_de_preparation(&m))?;
    let messages: Vec<envoi::Message> = choix.garder(messages, |m| m.cle.as_str()).into_iter().filter(|m| cles.contains(&m.cle)).collect();
    let client = ouvrir()?;
    let expediteur = expediteur_de(client.as_ref());
    let mut avertissements: Vec<String> = documents.avertissements.clone();
    let signataire = requis(&ctx.entreprise, "signataire")?.get("email").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default().to_lowercase();
    if !signataire.is_empty() && signataire != expediteur {
        avertissements.push(format!(
            "Les messages partiront de {} (la boite posee), pas de {} (le courriel du signataire, fiche Entreprise).",
            expediteur, signataire
        ));
    }
    Ok(EnvoiPrepare { mois_saisie, messages, expediteur, avertissements, choix, documents, client, preparation_ms: socle::temps::duree(debut).as_millis() })
}

/// Ce qui suit la preparation : la PREUVE (le mot de passe du logiciel), l'envoi, le journal. `qui` : la ligne du journal.
pub(crate) fn expedier_envoi(ctx: &Contexte, prepare: EnvoiPrepare, mois: &str, preuve: &Preuve, qui: &str) -> R<Value> {
    let EnvoiPrepare { mois_saisie, messages, expediteur, avertissements, choix, documents, mut client, preparation_ms } = prepare;
    let a_qui = format!("{} salarie(s) : {}", messages.len(), messages.iter().map(|m| m.nom.as_str()).collect::<Vec<_>>().join(", "));
    // LA VERSION SUR LAQUELLE ILS PARTENT, engagement par engagement des salaries servis.
    let servis: Vec<&str> = messages.iter().map(|m| m.cle.as_str()).collect();
    let versions = journal::versions_du_mois(ctx, mois, |cle, _, _| servis.contains(&cle)).map_err(Echec::from)?;
    // LA DUREE DE CHAQUE ETAPE (08/10/2026 : un envoi de deux courriels a pris quatre minutes, sans qu'on sache ou).
    let t_preuve = std::time::Instant::now();
    verifier_preuve(ctx, Some(preuve), mois, "envoi des documents du mois aux salaries", &a_qui)?;
    let preuve_ms = socle::temps::duree(t_preuve).as_millis();
    let t_envoi = std::time::Instant::now();
    let envoye = envoi::envoyer_suivi(&messages, &expediteur, client.as_mut(), None, None);
    let envoi_ms = socle::temps::duree(t_envoi).as_millis();
    // Ce que chaque salarie servi a recu : ses pieces, le resume de sa paie (ce qui dira, si elle change, ce qui a change),
    // et si c'etait un renvoi.
    let salaries = |servis: &[envoi::Message]| -> Value {
        json!(servis.iter().map(|m| json!({
            "cle": m.cle, "nom": m.nom, "pieces": m.pieces.iter().map(|p| nom_du(p)).collect::<Vec<_>>(),
            "resume": choix.resumes.get(&m.cle),
            "renvoi": choix.renvois.contains_key(&m.cle),
        })).collect::<Vec<_>>())
    };
    let durees = json!({"preparation": preparation_ms as u64, "mot_de_passe": preuve_ms as u64, "envoi": envoi_ms as u64});
    let partis = match envoye {
        Ok(p) => p,
        Err(e) => {
            // CE QUI A ATTEINT LE SALARIE EST PARTI : les messages partent dans l'ordre, les premiers sont servis. Leurs
            // numeros, et seulement les leurs, s'ecrivent d'abord ; la ligne porte ensuite la version de leurs
            // engagements, et seulement les leurs.
            let servis = &messages[..e.partis.len().min(messages.len())];
            let mut versions_servies = Value::Array(Vec::new());
            if !servis.is_empty() {
                let cles: Vec<&str> = servis.iter().map(|m| m.cle.as_str()).collect();
                documents.engager_servis(ctx, &mois_saisie, &cles)?;
                versions_servies = json!(versions.as_array().into_iter().flatten().filter(|v| v["salarie"].as_str().is_some_and(|c| cles.contains(&c))).collect::<Vec<_>>());
            }
            let detail = json!({"salaries": salaries(servis), "expediteur": expediteur, "durees_ms": durees});
            noter_echec_d_envoi(ctx, "documents", Some(mois), qui, &e.partis, &e.echoue, &e.message, Some(detail), Some(&versions_servies))?;
            return Err(echec_d_envoi(e.message));
        }
    };
    // LES DOCUMENTS SONT PARTIS : leurs numeros s'ecrivent dans les paies des engagements, puis la ligne du journal dit
    // qu'ils sont partis, et sur quelle version.
    documents.engager(ctx, &mois_saisie)?;
    let detail = json!({"salaries": salaries(&messages), "expediteur": expediteur, "durees_ms": durees});
    let reponse = format!("{} message(s) envoye(s)", partis.len());
    let ligne = noter_envoi(ctx, "documents", Some(mois), qui, true, &partis.join(", "), &reponse, Some(&detail), Some(&versions))?;
    Ok(json!({
        "partis": partis,
        "messages": messages.len(),
        "expediteur": expediteur,
        "avertissements": avertissements,
        "ligne": ligne,
    }))
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

// ------------------------------------------------------------------ dossier-envoyer





// ------------------------------------------------------------------ casc-envoyer



