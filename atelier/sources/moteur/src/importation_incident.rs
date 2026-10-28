//! L'incident de l'import (lot E du plan d'import, `prive/journal/PLAN-IMPORT-BULLETINS.md` §2.c et §4) : ce qui part
//! chez l'editeur quand des fichiers du dossier `import/` n'ont pas pu etre repris, pour qu'il ecrive le gabarit ou le
//! libelle qui manquait.
//!
//! DEUX ROUTES, ET RIEN NE PART SANS LA SECONDE (decide par Leandre, plan §4) :
//! - `importation-incident`, `{"dossier": ...}` : ce qui PARTIRAIT, en entier, tel qu'il partirait (`texte`), ce qui en
//!   a ete enleve (`resume` : « 3 noms, 1 NIR enlevés »), et les fichiers non repris pour l'ecran (`fichiers` :
//!   `chemin`, `sort`, `raison`). `a_envoyer` : faux quand tout est repris. RIEN N'EST ECRIT, ni dans le journal des
//!   incidents (l'ouverture du logiciel envoie ce qui y attend : une proposition notee partirait sans le OK) ;
//! - `importation-incident-envoyer`, meme charge : sur le clic « Envoyer » de la personne, et seulement la, le meme
//!   message est NOTE dans le journal des incidents (`incidents::noter`) puis envoye par le canal existant
//!   (`incidents::envoyer`). `{"envoye": bool, "message": "..."}`. Un second clic ne le note pas deux fois.
//!
//! CE QUI PART : le compte rendu de chaque fichier non repris (son sort, sa raison, ceux de ses pages, le gabarit
//! reconnu, la forme de chaque page -- mots, polices, images, traits) et le TEXTE DES PAGES REFUSEES, CAVIARDE
//! (`caviardage::caviarder`) : noms, prenoms, NIR, dates de naissance, adresses, numeros Conges Spectacles, courriels,
//! telephones, IBAN, remplaces par `<NOM>`, `<NIR>`... ; les montants, les taux, les libelles restent -- c'est ce qu'il
//! faut pour ecrire un gabarit. Le caviardage connait ce que les bulletins du dossier ont dit de leurs salaries, pages
//! refusees comprises (`bulletin_lu::Lecture::partiel`), les fiches de la compagnie et celles que l'import creerait ;
//! il cherche le reste par la forme. Un fichier se nomme par son empreinte (16 caracteres), JAMAIS par son nom (un
//! « bulletin-Dupont.pdf ») ; une raison qui citerait le chemin le perd aussi. LE FICHIER D'ORIGINE NE PART JAMAIS.
//!
//! LA FORME DU CANAL : un incident de nature `logiciel` (ce que le logiciel n'a pas su faire), `ou` = `importation`,
//! un par sort (« fichier non repris : gabarit_inconnu »...) : le serveur regroupe par sort, et chaque incident a sa
//! place -- le detail (le compte rendu) tient en 2 400 caracteres chez le serveur, la trace (le texte des pages) en
//! 4 000 : ce qui depasse ne part pas, et `texte` le dit a l'endroit ou il coupe. Le journal ajoute la version du
//! logiciel, le systeme et le numero anonyme du poste (`incidents::pour_l_editeur`) ; `texte` le dit aussi.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::bulletin_lu::{Entreprise, Salarie};
use crate::caviardage::{self, Caviarde, Connus};
use droit::contexte::Contexte;
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::fichiers::vrai;
use crate::importation_bulletins::{self as ib, DossierLu};
use crate::importation_tri::Sorte;
use crate::incidents;

/// Les routes que ce module sert.
pub const ROUTES: [&str; 2] = ["importation-incident", "importation-incident-envoyer"];

/// L'endroit, dans le journal des incidents.
const OU: &str = "importation";
/// Les bornes du journal (`incidents::noter`) et du serveur : une chaine du detail, la trace, le detail entier.
const TEXTE_MAX: usize = 600;
const TRACE_MAX: usize = 4000;
const DETAIL_MAX: usize = 2400;
const COUPE: &str = "[… la suite ne part pas : la trace d'un incident tient en 4 000 caractères]";

/// Les sorts d'une page dont le texte part (une page sans texte n'en a pas).
const PAGES_A_LIRE: [&str; 4] = ["incoherent", "incomplet", "gabarit_inconnu", "plusieurs_gabarits"];

/// Une route de ce module.
pub fn route(ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    let dossier = crate::routes::importation::dossier_import(ctx, charge)?;
    let proposition = ib::lire_dossier(ctx, &dossier, |lu| Ok(proposer(ctx, &lu)))?;
    match nom {
        "importation-incident" => Ok(proposition.json()),
        _ => Ok(envoyer(&ctx.racine, &proposition)),
    }
}

/// Un incident tel qu'il sera note.
struct Message {
    genre: String,
    resume: String,
    detail: Value,
    trace: String,
}

/// Ce qui partirait.
struct Proposition {
    messages: Vec<Message>,
    /// Pour l'ecran : chaque fichier non repris, par son chemin dans le dossier.
    fichiers: Vec<Value>,
    enleve: Caviarde,
}

impl Proposition {
    fn json(&self) -> Value {
        json!({
            "a_envoyer": !self.messages.is_empty(),
            "texte": self.texte(),
            "resume": if self.messages.is_empty() { String::new() } else { self.enleve.resume() },
            "fichiers": self.fichiers,
        })
    }

    /// Le message en clair, champ par champ, tel qu'il part.
    fn texte(&self) -> String {
        if self.messages.is_empty() {
            return String::new();
        }
        let mut t = String::new();
        let n = self.messages.len();
        for (k, m) in self.messages.iter().enumerate() {
            t.push_str(&format!("== Message {}/{} : {} ==\n{}\n", k + 1, n, m.genre, m.resume));
            for f in m.detail.get("fichiers").and_then(Value::as_array).into_iter().flatten() {
                let s = |c: &str| f.get(c).map(valeur_en_texte).unwrap_or_default();
                t.push_str(&format!("- fichier {} ({}, {} octets) : {}\n", s("fichier"), s("sorte"), s("taille"), s("raison")));
                for p in f.get("pages").and_then(Value::as_array).into_iter().flatten() {
                    let s = |c: &str| p.get(c).map(valeur_en_texte).unwrap_or_default();
                    let gabarit = if s("gabarit").is_empty() { String::new() } else { format!(" [gabarit {}]", s("gabarit")) };
                    t.push_str(&format!("    page {} : {}{} -- {} ({})\n", s("page"), s("sort"), gabarit, s("raison"), s("forme")));
                }
            }
            if let Some(a) = m.detail.get("autres").and_then(Value::as_u64) {
                t.push_str(&format!("- et {} autre(s) fichier(s) du même sort, que le détail ne nomme pas (il tient en {} caractères)\n", a, DETAIL_MAX));
            }
            if !m.trace.is_empty() {
                t.push_str("Le texte des pages, caviardé :\n");
                t.push_str(&m.trace);
                if !m.trace.ends_with('\n') {
                    t.push('\n');
                }
            }
            t.push('\n');
        }
        t.push_str("Joints d'office : la version du logiciel, le système de l'ordinateur et le numéro anonyme du poste. Les fichiers eux-mêmes ne partent pas.\n");
        t
    }
}

/// `s[:n]` sur des points de code.
fn tete(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Un fichier nomme sans rien de personnel : son empreinte, et une extension courte.
fn identifiant(f: &crate::importation_tri::Fichier, rang: usize) -> String {
    let base = if f.empreinte.len() >= 16 { f.empreinte[..16].to_string() } else { format!("fichier-{}", rang + 1) };
    let ext = &f.extension;
    if !ext.is_empty() && ext.len() <= 5 && ext.chars().all(|c| c.is_ascii_alphanumeric()) {
        format!("{}.{}", base, ext)
    } else {
        base
    }
}

/// Ce que le caviardage connait : les salaries que les bulletins ont nommes (pages refusees comprises), les fiches de la
/// compagnie et celles que l'import creerait ; les entreprises, protegees.
fn connus(ctx: &Contexte, lu: &DossierLu) -> Connus {
    let mut c = Connus::default();
    let pousser = |v: &mut Vec<String>, x: Option<String>| {
        if let Some(x) = x.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) {
            if !v.contains(&x) {
                v.push(x);
            }
        }
    };
    // Une ligne d'adresse « 99999 Commune » donne le code postal et la commune ; les autres sont des lignes d'adresse.
    fn adresse(c: &mut Connus, ligne: &str) {
        let l = ligne.trim();
        let cp: String = l.chars().take(5).collect();
        if cp.len() == 5 && cp.chars().all(|x| x.is_ascii_digit()) && l[5..].starts_with(' ') {
            let commune = l[5..].trim().to_string();
            if !c.codes_postaux.contains(&cp) {
                c.codes_postaux.push(cp);
            }
            if !commune.is_empty() && !c.communes.contains(&commune) {
                c.communes.push(commune);
            }
        } else if !l.is_empty() && !c.adresses.iter().any(|a| a == l) {
            c.adresses.push(l.to_string());
        }
    }
    for s in &lu.salaries_lus {
        let Salarie { nom_complet, nom, prenom, nir, naissance, adresse: lignes, conges_spectacles, code, .. } = s;
        // Un code salarie tire du nom (« DUPO01 ») s'enleve comme un nom, ou qu'il soit ; un code de chiffres seuls
        // (« 7 ») n'est cherche que par la forme, apres « Code salarié ».
        pousser(&mut c.noms, code.clone().filter(|x| x.chars().filter(|c| c.is_alphabetic()).count() >= 2));
        pousser(&mut c.noms, nom_complet.clone());
        pousser(&mut c.noms, nom.clone());
        pousser(&mut c.prenoms, prenom.clone());
        pousser(&mut c.nirs, nir.clone());
        pousser(&mut c.naissances, naissance.map(|d| d.iso()));
        pousser(&mut c.conges_spectacles, conges_spectacles.clone());
        for l in lignes {
            adresse(&mut c, l);
        }
    }
    let fiches = ctx
        .salaries
        .iter()
        .filter(|(k, v)| !k.starts_with('_') && v.is_object())
        .map(|(_, v)| v.clone())
        .chain(lu.ap.a_creer.iter().filter(|a| a.nom == "salarie").map(|a| Value::Object(a.donnees.clone())));
    for f in fiches {
        let t = |k: &str| f.get(k).filter(|v| vrai(v)).map(valeur_en_texte);
        for k in ["nom", "nom_affichage"] {
            pousser(&mut c.noms, t(k));
        }
        pousser(&mut c.noms, t("code_salarie").filter(|x| x.chars().filter(|c| c.is_alphabetic()).count() >= 2));
        for k in ["prenom", "prenom_affichage"] {
            pousser(&mut c.prenoms, t(k));
        }
        pousser(&mut c.nirs, t("nir"));
        if let (Some(n), Some(k)) = (t("nir"), t("nir_cle")) {
            pousser(&mut c.nirs, Some(format!("{}{}", n, k)));
        }
        pousser(&mut c.naissances, t("date_naissance"));
        for k in ["adresse", "adresse_affichage"] {
            pousser(&mut c.adresses, t(k));
        }
        pousser(&mut c.codes_postaux, t("code_postal"));
        for k in ["commune", "commune_affichage", "lieu_naissance", "lieu_naissance_affichage"] {
            pousser(&mut c.communes, t(k));
        }
        pousser(&mut c.courriels, t("email"));
        pousser(&mut c.conges_spectacles, t("num_conges_spectacles"));
    }
    let e = &ctx.entreprise;
    for k in ["raison_sociale", "nom_affichage", "adresse", "adresse_affichage", "code_postal", "commune", "commune_affichage"] {
        pousser(&mut c.entreprise, e.get(k).filter(|v| vrai(v)).map(valeur_en_texte));
    }
    for Entreprise { nom, adresse: lignes, .. } in &lu.entreprises_lues {
        pousser(&mut c.entreprise, nom.clone());
        for l in lignes {
            pousser(&mut c.entreprise, Some(l.clone()));
        }
    }
    c
}

/// Le caviardage d'un texte, ce qui en est enleve ajoute au compte.
fn caviarder(texte: &str, connus: &Connus, compte: &mut BTreeMap<&'static str, usize>) -> String {
    let c = caviardage::caviarder(texte, connus);
    for (k, n) in c.enleves {
        *compte.entry(k).or_default() += n;
    }
    c.texte
}

/// Ce qui partirait : lu dans le dossier, avant que le dossier de travail (les zip ouverts) ne s'efface.
fn proposer(ctx: &Contexte, lu: &DossierLu) -> Proposition {
    let vide: Vec<Value> = Vec::new();
    let cr = lu.ap.reponse.get("tri").and_then(|t| t.get("fichiers")).and_then(Value::as_array).unwrap_or(&vide);
    let connus = connus(ctx, lu);
    let mut compte: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut fichiers: Vec<Value> = Vec::new();
    // Par sort : (le detail de chaque fichier, le texte de ses pages).
    let mut par_sort: BTreeMap<usize, (String, Vec<Value>, Vec<(String, BTreeMap<&'static str, usize>)>)> = BTreeMap::new();
    let mut textes: HashMap<usize, Vec<String>> = HashMap::new();
    for (i, (f, c)) in lu.tri.fichiers.iter().zip(cr.iter()).enumerate() {
        let sort = c.get("sort").and_then(Value::as_str).unwrap_or("");
        if !ib::non_repris(sort) {
            continue;
        }
        fichiers.push(json!({"chemin": f.chemin, "sort": sort, "raison": c.get("raison").cloned().unwrap_or(json!(""))}));
        let ident = identifiant(f, i);
        let base = f.chemin.rsplit('/').next().unwrap_or(&f.chemin).to_string();
        // Une raison ne cite jamais le chemin d'origine (il peut porter un nom) : l'identifiant le remplace.
        let propre = |r: &str, compte: &mut BTreeMap<&'static str, usize>| {
            let mut r = r.replace(&f.chemin, &ident);
            if !base.is_empty() {
                r = r.replace(&base, &ident);
            }
            tete(&caviarder(&r, &connus, compte), TEXTE_MAX)
        };
        let mut pages: Vec<Value> = Vec::new();
        let mut lire: Vec<(u64, String)> = Vec::new();
        for p in c.get("pages").and_then(Value::as_array).into_iter().flatten() {
            let n = p.get("page").and_then(Value::as_u64).unwrap_or(0);
            let ps = p.get("sort").and_then(Value::as_str).unwrap_or("");
            let forme = f
                .pages
                .get((n as usize).saturating_sub(1))
                .map(|x| format!("{} mots, {} polices, {} images, {} traits", x.mots, x.polices, x.images, x.traits))
                .unwrap_or_default();
            let mut e = Map::new();
            e.insert("page".into(), json!(n));
            e.insert("sort".into(), json!(ps));
            e.insert("raison".into(), json!(propre(p.get("raison").and_then(Value::as_str).unwrap_or(""), &mut compte)));
            if let Some(g) = p.get("gabarit").and_then(Value::as_str) {
                e.insert("gabarit".into(), json!(g));
            }
            e.insert("forme".into(), json!(forme));
            pages.push(Value::Object(e));
            if PAGES_A_LIRE.contains(&ps) {
                lire.push((n, ps.to_string()));
            }
        }
        // Les pages refusees, en texte : la fabrique les relit (le fichier est encore la ou le tri l'a mis).
        // Chaque page avec ce que son caviardage a enleve : seul compte ce qui part (`trace`).
        let mut morceaux: Vec<(String, BTreeMap<&'static str, usize>)> = Vec::new();
        if f.sorte == Sorte::PdfTexte && !lire.is_empty() {
            let pages_texte = textes
                .entry(i)
                .or_insert_with(|| fabrique_pdf::lecture::texte(&f.disque.to_string_lossy()).map(|(t, _)| t).unwrap_or_default());
            for (n, ps) in &lire {
                let Some(brut) = pages_texte.get((*n as usize).saturating_sub(1)) else { continue };
                let mut enleve = BTreeMap::new();
                let propre = caviarder(brut, &connus, &mut enleve);
                morceaux.push((format!("--- fichier {} page {} ({}) ---\n{}", ident, n, ps, propre.trim_end()), enleve));
            }
        }
        let mut d = Map::new();
        d.insert("fichier".into(), json!(ident));
        d.insert("sorte".into(), json!(f.sorte.nom()));
        d.insert("taille".into(), json!(f.taille));
        d.insert("raison".into(), json!(propre(c.get("raison").and_then(Value::as_str).unwrap_or(""), &mut compte)));
        if !pages.is_empty() {
            d.insert("pages".into(), Value::Array(pages));
        }
        let rang = ib::SORTS.iter().position(|s| *s == sort).unwrap_or(ib::SORTS.len());
        let entree = par_sort.entry(rang).or_insert_with(|| (sort.to_string(), Vec::new(), Vec::new()));
        entree.1.push(Value::Object(d));
        entree.2.extend(morceaux);
    }

    let total = lu.tri.fichiers.len();
    let mut messages = Vec::new();
    for (_, (sort, details, morceaux)) in par_sort {
        let n = details.len();
        // Le detail tient en `DETAIL_MAX` caracteres chez le serveur : les fichiers qui n'y tiennent pas sont comptes.
        let mut gardes: Vec<Value> = Vec::new();
        for d in &details {
            let essai = json!({"fichiers": gardes.iter().chain(std::iter::once(d)).collect::<Vec<_>>(), "autres": n, "total": total});
            if !gardes.is_empty() && essai.to_string().chars().count() > DETAIL_MAX {
                break;
            }
            gardes.push(d.clone());
        }
        let mut detail = json!({"fichiers": gardes, "total": total});
        if gardes_len(&detail) < n {
            detail["autres"] = json!(n - gardes_len(&detail));
        }
        messages.push(Message {
            genre: format!("fichier non repris : {}", sort),
            resume: format!(
                "Import de l'historique : {} fichier(s) sur {} non repris, « {} ». Le compte rendu de chacun, et le texte de ses pages refusées, caviardé.",
                n,
                total,
                sort.replace('_', " ")
            ),
            detail,
            trace: trace(&morceaux, &mut compte),
        });
    }
    Proposition { messages, fichiers, enleve: Caviarde { texte: String::new(), enleves: compte } }
}

fn gardes_len(detail: &Value) -> usize {
    detail.get("fichiers").and_then(Value::as_array).map_or(0, Vec::len)
}

/// Le texte des pages, borne a ce que la trace d'un incident porte ; la coupe se dit. Ce que le caviardage a enleve
/// des pages qui partent (meme en partie) s'ajoute au compte ; les pages qui ne partent pas ne comptent pas.
fn trace(morceaux: &[(String, BTreeMap<&'static str, usize>)], compte: &mut BTreeMap<&'static str, usize>) -> String {
    if morceaux.is_empty() {
        return String::new();
    }
    let garde = TRACE_MAX - COUPE.chars().count() - 1;
    let mut tout = String::new();
    let mut n = 0usize;
    for (k, (texte, enleve)) in morceaux.iter().enumerate() {
        let separe = usize::from(k > 0);
        let longueur = texte.chars().count();
        let reste_total: usize = morceaux[k..].iter().map(|m| m.0.chars().count() + 1).sum::<usize>() - 1 + separe;
        if n + reste_total <= TRACE_MAX {
            // Tout le reste tient : sans coupe.
            for (t, e) in &morceaux[k..] {
                if !tout.is_empty() {
                    tout.push('\n');
                }
                tout.push_str(t);
                for (c, x) in e {
                    *compte.entry(c).or_default() += x;
                }
            }
            return tout;
        }
        if n + separe >= garde {
            break;
        }
        if separe == 1 {
            tout.push('\n');
            n += 1;
        }
        let pris = longueur.min(garde - n);
        tout.push_str(&tete(texte, pris));
        n += pris;
        for (c, x) in enleve {
            *compte.entry(c).or_default() += x;
        }
        if pris < longueur {
            break;
        }
    }
    format!("{}\n{}", tout, COUPE)
}

/// `importation-incident-envoyer` : le message, note puis envoye. Ne leve pas : ce qui n'a pas pu se faire se dit.
fn envoyer(racine: &Path, p: &Proposition) -> Value {
    let reponse = |envoye: bool, message: &str| json!({"envoye": envoye, "message": message});
    if p.messages.is_empty() {
        return reponse(false, "Rien à envoyer : chaque fichier du dossier est repris.");
    }
    let deja = incidents::lire(racine, None, false).unwrap_or_default();
    // Ce que `noter` ecrira d'un message, pour le reconnaitre s'il y est deja (un second clic).
    let ecrit = |v: Value, limite: usize| incidents::nettoyer(&v, &mut Vec::new(), limite);
    let mut miens: Vec<(Value, Value)> = Vec::new();
    for m in &p.messages {
        let empreinte = json!(incidents::empreinte(incidents::LOGICIEL, OU, &m.genre));
        // `noter` ne garde pas une trace vide.
        let trace = if m.trace.is_empty() { Value::Null } else { ecrit(json!(m.trace), TRACE_MAX) };
        let (resume, detail) = (ecrit(json!(m.resume), TEXTE_MAX), ecrit(m.detail.clone(), TEXTE_MAX));
        let present = deja.iter().find(|l| {
            l.get("empreinte") == Some(&empreinte)
                && l.get("resume") == Some(&resume)
                && l.get("detail") == Some(&detail)
                && l.get("trace") == Some(&trace)
                && l.get("envoi").and_then(|e| e.get("etat")).and_then(Value::as_str) != Some(incidents::SANS_OBJET)
        });
        if let Some(l) = present {
            miens.push((l["empreinte"].clone(), l["quand"].clone()));
            continue;
        }
        let note = incidents::Note {
            nature: incidents::LOGICIEL,
            resume: &m.resume,
            ou: OU,
            quoi: &m.genre,
            detail: m.detail.clone(),
            trace: Some(&m.trace),
            ..Default::default()
        };
        match incidents::noter(racine, &note) {
            Some(l) => miens.push((l["empreinte"].clone(), l["quand"].clone())),
            None => return reponse(false, "Le message ne s'écrit pas dans le journal des incidents de ce poste : rien n'est parti."),
        }
    }
    if !incidents::envoi_actif(racine) {
        return reponse(
            false,
            "L'envoi à l'éditeur est coupé sur ce poste : le message est noté ici, il ne part pas. Il partira si l'envoi est rétabli.",
        );
    }
    let bilan = incidents::envoyer(racine);
    let apres = incidents::lire(racine, None, false).unwrap_or_default();
    let partis = miens.iter().all(|(e, q)| {
        apres.iter().any(|l| {
            l.get("empreinte") == Some(e)
                && l.get("quand") == Some(q)
                && l.get("envoi").and_then(|x| x.get("etat")).and_then(Value::as_str) == Some(incidents::DEPOSE)
        })
    });
    if partis {
        return reponse(true, "Le message est parti chez le développeur ; Léandre s'en occupe et vous répond.");
    }
    if bilan.get("sans_serveur") == Some(&Value::Bool(true)) {
        return reponse(false, "Le message est noté sur ce poste, mais l'adresse de l'éditeur n'y est pas réglée : il partira dès qu'elle le sera.");
    }
    let erreur = bilan.get("erreur").map(valeur_en_texte).unwrap_or_default();
    reponse(
        false,
        &format!(
            "Le message n'est pas parti{} : il est noté sur ce poste et repartira à la prochaine ouverture du logiciel.",
            if erreur.is_empty() { String::new() } else { format!(" ({})", erreur) }
        ),
    )
}
