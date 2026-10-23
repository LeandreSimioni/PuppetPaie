//! `moteur/avenant.py` (la comparaison du contrat parti a la paie d'aujourd'hui) et
//! `operations_documents.ecart_au_contrat_signe` : ce que la pastille « Contrat » d'une paie dit
//! quand le contrat parti a la signature ne dit plus la paie (« article 1 (Objet et nature du
//! contrat), article 4 (Rémunération) »).
//!
//! L'ANCIEN TEXTE SE LIT DANS LE CONTRAT PARTI, pas dans un souvenir. Le PDF envoye a la signature
//! est ce que le salarie a signe : on en tire le texte, on le decoupe au numero du contrat, et
//! chaque paragraphe des articles du modele y est cherche tel que la paie d'aujourd'hui l'ecrirait.
//! Un paragraphe qui s'y lit a l'identique n'a pas change ; les autres sont retrouves dans le PDF
//! par leur partie fixe (le texte du modele, les marqueurs laisses libres). Cela vaut pour un
//! contrat parti avant qu'on note ses clauses au journal (avant le 30/09/2026).
//!
//! Ici, seule la lecture : l'apercu et l'ecriture de l'avenant restent a faire. Le texte de
//! l'avenant n'est pas ici non plus : il vit dans `avenant-cddu.modele`, une donnee, comme celui
//! du contrat.
//!
//! SANS CALCUL TANT QU'ON PEUT : `ecart_au_contrat_signe` regarde d'abord tout ce qui ne demande
//! pas la paie (le delai, le contrat parti ce mois-la, le PDF, le modele, le texte du PDF), puis
//! seulement prend les montants dans la paie calculee relue (`calcul_du_mois`).
//!
//! Rend "" dans tous les cas ou le Python rend "" -- et la ou il leve (la pastille n'invente pas).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use fabrique_pdf::modele::{self, Modele, Valeurs};
use serde_json::{json, Value};

use crate::bulletin::{convention_du_contrat, fmt_eur};
use crate::contexte::{telle_qu_au, Contexte};
use crate::date::{d, en_texte, Jour};
use crate::emplois::{emploi_de, EmploiEngagement};
use crate::etats::{accepte, est, get, getd, items, objet_vide, ou, py_str, CONTRAT_RECU, LISTE_VIDE, SIGNATURE};
use crate::retours::Echec;
use crate::fichiers::vrai;
use crate::nfkc::nfkc;
use crate::num::{dec, Resultat};

/// `contrat.MODELE.name`.
const MODELE: &str = "contrat-cddu.modele";

/// Ce qui remplace une valeur qui n'est pas un texte : le Python ne la refuse que si le modele s'en
/// sert (`re.sub` ne sait pas poser autre chose qu'une chaine). Une phrase qui la porte est un refus.
const INVALIDE: &str = "\u{0}\u{0}VALEUR-SANS-TEXTE\u{0}\u{0}";

// ------------------------------------------------------ le texte d'un PDF

use crate::retours::blanc_py as est_blanc;

/// `avenant._normaliser` : ce qui se compare. Ligatures defaites (NFKC : « ﬀ » -> « ff », espaces
/// insecables -> espaces), une coupure apres un trait d'union recollee, les blancs ramenes a un seul.
pub fn normaliser(texte: &str) -> String {
    let t: Vec<char> = nfkc(texte).chars().collect();
    let mut recolle = String::with_capacity(t.len());
    let mut i = 0;
    while i < t.len() {
        if t[i] == '-' && i + 2 < t.len() && t[i + 1] == '\n' && !est_blanc(t[i + 2]) {
            recolle.push('-');
            i += 2;
        } else {
            recolle.push(t[i]);
            i += 1;
        }
    }
    let mut sortie = String::with_capacity(recolle.len());
    let mut dans_blanc = false;
    for c in recolle.chars() {
        if est_blanc(c) {
            if !dans_blanc {
                sortie.push(' ');
            }
            dans_blanc = true;
        } else {
            sortie.push(c);
            dans_blanc = false;
        }
    }
    sortie.trim_matches(' ').to_string()
}

type CleTexte = (String, u128);

fn textes() -> &'static Mutex<HashMap<CleTexte, String>> {
    static TEXTES: OnceLock<Mutex<HashMap<CleTexte, String>>> = OnceLock::new();
    TEXTES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// `avenant.texte_du_pdf` : le texte du PDF, garde en memoire tant que le fichier ne change pas
/// (la pastille de chaque contrat le relit a chaque affichage). Le texte des pages est celui de la
/// fabrique (`paie pdf --texte`), lu dans le meme processus.
pub fn texte_du_pdf(chemin: &Path) -> Resultat<String> {
    let mtime = std::fs::metadata(chemin)
        .and_then(|m| m.modified())
        .map_err(|e| format!("{} : {}", chemin.display(), e))?
        .duration_since(std::time::UNIX_EPOCH)
        .map(|t| t.as_nanos())
        .unwrap_or(0);
    let cle = (chemin.to_string_lossy().into_owned(), mtime);
    if let Some(t) = textes().lock().map_err(|e| e.to_string())?.get(&cle) {
        return Ok(t.clone());
    }
    // LE TEXTE GARDE SUR LE DISQUE (Leandre, 08/10/2026 : « quand je vais en avant et en arriere, ca met autant de
    // temps »). Chaque route est un processus neuf : la memoire ci-dessus ne survit pas a la requete, et la pastille de
    // chaque contrat relisait son PDF (un quart de seconde chacun) a chaque changement de mois, deux fois. Le texte se
    // range a cote du PDF, dans `.textes/`, sous un nom qui porte l'empreinte de son CONTENU : un PDF refait n'est plus
    // lu sous l'ancien texte, et la meme compagnie refaite (le rejeu d'un banc, #236) a les memes fichiers -- la date du
    // fichier, a la nanoseconde, n'est celle d'aucun rejeu. Une ecriture impossible ne change rien au resultat.
    let empreinte = crate::journal::empreinte_fichier(chemin).unwrap_or_default();
    let nom = chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let range = chemin.parent().map(|d| d.join(".textes").join(format!("{}.{}.txt", nom, empreinte)));
    if let Some(t) = range.as_ref().and_then(|r| std::fs::read_to_string(r).ok()) {
        textes().lock().map_err(|e| e.to_string())?.insert(cle, t.clone());
        return Ok(t);
    }
    let (pages, _avertissements) = fabrique_pdf::lecture::texte(&cle.0).map_err(|r| r.0)?;
    let texte = normaliser(&pages.join("\n"));
    if let Some(r) = &range {
        if let Some(d) = r.parent() {
            // Les textes d'avant de ce meme PDF partent : un seul texte par fichier.
            if let Ok(liste) = std::fs::read_dir(d) {
                for e in liste.flatten() {
                    let n = e.file_name().to_string_lossy().into_owned();
                    if n.starts_with(&format!("{}.", nom)) && n.ends_with(".txt") {
                        let _ = std::fs::remove_file(e.path());
                    }
                }
            }
            let _ = std::fs::create_dir_all(d).and_then(|_| std::fs::write(r, &texte));
        }
    }
    textes().lock().map_err(|e| e.to_string())?.insert(cle, texte.clone());
    Ok(texte)
}

// ------------------------------------------------------ le modele en motif

/// Un morceau du motif d'un paragraphe : l'expression de `avenant._motif`, sans `re`.
pub enum Elt {
    /// `" ?"` : un espace, ou rien.
    Espace,
    /// Le texte fixe du modele, tel quel.
    Fixe(Vec<char>),
    /// `(.*?)` : un marqueur laisse libre.
    Libre,
    /// `(.*?)(?= Article \d+ — | Fait en |$)` : le dernier marqueur d'un paragraphe qui s'acheve sur lui.
    LibreJusqu,
}

/// Le texte du modele coupe sur ses marqueurs : `[fixe, cle, fixe, cle, ..., fixe]`
/// (`_MARQUEUR.split`, dont le groupe garde la cle).
fn separer(texte: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (mut dernier, mut i) = (0, 0);
    while let Some(k) = texte[i..].find("{{") {
        let debut = i + k;
        match modele::marqueur_en(texte, debut) {
            Some((cle, fin)) => {
                out.push(texte[dernier..debut].to_string());
                out.push(cle.to_string());
                dernier = fin;
                i = fin;
            }
            None => i = debut + 1,
        }
    }
    out.push(texte[dernier..].to_string());
    out
}

/// `avenant._motif` : un paragraphe du modele en motif, le texte fixe tel quel, chaque marqueur
/// libre. Le gras (**) ne s'imprime pas, il disparait.
pub fn motif(texte_modele: &str) -> Vec<Elt> {
    let morceaux = separer(&texte_modele.replace("**", ""));
    let dernier_fixe_non_vide = morceaux.last().map_or(false, |m| !m.trim_matches(est_blanc).is_empty());
    let mut out = Vec::new();
    for (i, m) in morceaux.iter().enumerate() {
        if i % 2 == 1 {
            out.push(if i + 2 < morceaux.len() || dernier_fixe_non_vide { Elt::Libre } else { Elt::LibreJusqu });
        } else {
            let fixe = normaliser(m);
            if !fixe.is_empty() {
                if m.chars().next().map_or(false, est_blanc) {
                    out.push(Elt::Espace);
                }
                out.push(Elt::Fixe(fixe.chars().collect()));
                if m.chars().last().map_or(false, est_blanc) {
                    out.push(Elt::Espace);
                }
            } else if !m.is_empty() {
                out.push(Elt::Espace);
            }
        }
    }
    out
}

/// Le lookahead de `LibreJusqu` en `q` : le titre d'un article suivant, la ligne « Fait en », ou la fin.
fn borne_en(seg: &[char], q: usize) -> bool {
    if q == seg.len() {
        return true;
    }
    let est = |t: &str, de: usize| {
        let c: Vec<char> = t.chars().collect();
        seg.len() >= de + c.len() && seg[de..de + c.len()] == c[..]
    };
    if est(" Fait en ", q) {
        return true;
    }
    if !est(" Article ", q) {
        return false;
    }
    let mut p = q + " Article ".chars().count();
    let debut = p;
    while p < seg.len() && seg[p].is_ascii_digit() {
        p += 1;
    }
    p > debut && est(" — ", p)
}

/// Pour chaque element du motif (et la fin, apres le dernier), les positions du segment d'ou le
/// reste du motif tient : `t[k][p]`. On le calcule de la fin du motif vers son debut, sans retour
/// arriere.
fn tables(motif: &[Elt], seg: &[char]) -> Vec<Vec<bool>> {
    let n = seg.len();
    let mut t = vec![vec![false; n + 1]; motif.len() + 1];
    t[motif.len()] = vec![true; n + 1];
    for (k, e) in motif.iter().enumerate().rev() {
        let suiv = t[k + 1].clone();
        let cur = &mut t[k];
        match e {
            Elt::Espace => {
                for p in 0..=n {
                    cur[p] = suiv[p] || (p < n && seg[p] == ' ' && suiv[p + 1]);
                }
            }
            Elt::Fixe(l) => {
                for p in 0..=n {
                    cur[p] = p + l.len() <= n && seg[p..p + l.len()] == l[..] && suiv[p + l.len()];
                }
            }
            Elt::Libre => {
                let mut atteint = false;
                for p in (0..=n).rev() {
                    atteint = atteint || suiv[p];
                    cur[p] = atteint;
                }
            }
            Elt::LibreJusqu => {
                let mut atteint = false;
                for p in (0..=n).rev() {
                    atteint = atteint || (suiv[p] && borne_en(seg, p));
                    cur[p] = atteint;
                }
            }
        }
    }
    t
}

/// `motif.search(segment)` : le motif se retrouve-t-il quelque part dans le segment ?
pub fn retrouve(motif: &[Elt], seg: &[char]) -> bool {
    tables(motif, seg)[0].iter().any(|&b| b)
}

/// `motif.search(segment).span()` : le premier endroit ou le motif s'accorde, le plus a gauche, et
/// pour chaque morceau le choix que `re` fait d'abord (l'espace facultatif pris s'il laisse le reste
/// s'accorder, le marqueur libre le plus court).
pub fn trouve(motif: &[Elt], seg: &[char]) -> Option<(usize, usize)> {
    let t = tables(motif, seg);
    let debut = (0..=seg.len()).find(|&p| t[0][p])?;
    let mut p = debut;
    for (k, e) in motif.iter().enumerate() {
        p = match e {
            Elt::Espace => {
                if p < seg.len() && seg[p] == ' ' && t[k + 1][p + 1] {
                    p + 1
                } else {
                    p
                }
            }
            Elt::Fixe(l) => p + l.len(),
            Elt::Libre => (p..=seg.len()).find(|&q| t[k + 1][q])?,
            Elt::LibreJusqu => (p..=seg.len()).find(|&q| borne_en(seg, q) && t[k + 1][q])?,
        };
    }
    Some((debut, p))
}

// ------------------------------------------------------ les valeurs du modele

fn requis<'a>(v: &'a Value, cle: &str) -> Resultat<&'a Value> {
    v.get(cle).ok_or_else(|| format!("le modele n'a pas de valeur {}", cle))
}

/// Une valeur posee telle quelle dans les marqueurs : un texte, ou de quoi refuser si le modele s'en sert.
fn texte_ou_invalide(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        _ => INVALIDE.to_string(),
    }
}

/// `contrat._aff` : forme lisible si elle existe, forme administrative sinon.
fn aff<'a>(fiche: &'a Value, cle: &str) -> Resultat<&'a Value> {
    match fiche.get(format!("{}_affichage", cle)) {
        Some(v) if vrai(v) => Ok(v),
        _ => requis(fiche, cle),
    }
}

/// `contrat._de` : « d' » devant une voyelle ou un h, « de » sinon.
fn de_devant(mot: &str) -> &'static str {
    let t: String = mot.chars().take(1).flat_map(char::to_lowercase).collect();
    if t.is_empty() || "aeiouyhâàéèêîôûœ".contains(&t) {
        "d'"
    } else {
        "de "
    }
}

/// `contrat._et` : « a, b et c ».
fn et(morceaux: &[String]) -> String {
    match morceaux.len() {
        0 => String::new(),
        1 => morceaux[0].clone(),
        n => format!("{} et {}", morceaux[..n - 1].join(", "), morceaux[n - 1]),
    }
}

fn jj_mm_aaaa(iso: &str) -> Resultat<String> {
    Ok(Jour::lire(iso)?.jj_mm_aaaa())
}

/// Ce que `contrat.marqueurs` lit de l'engagement : la forme canonique du moteur
/// (`oracle._engagement`, dans la paie calculee).
struct Eng {
    debut: String,
    fin: Option<String>,
    mois: String,
    numero: Option<String>,
    dernier_mois: bool,
    dates_travail: Vec<String>,
    planning: Vec<String>,
    lieux: BTreeMap<String, String>,
    planning_lieux: BTreeMap<String, String>,
    lieu: String,
    date_paiement: Option<String>,
    cachets: String,
    /// `None` : un contrat sans cachet dont ni l'engagement ni le mois n'ecrivent de taux (lot F1).
    taux_cachet: Option<String>,
}

impl Eng {
    fn lire(e: &Value) -> Resultat<Eng> {
        let texte = |c: &str| e.get(c).and_then(Value::as_str).map(str::to_string);
        let requis_texte = |c: &str| texte(c).ok_or_else(|| format!("engagement sans `{}`", c));
        let liste = |c: &str| -> Resultat<Vec<String>> {
            e.get(c)
                .and_then(Value::as_array)
                .ok_or_else(|| format!("engagement sans `{}`", c))?
                .iter()
                .map(|x| x.as_str().map(str::to_string).ok_or_else(|| format!("`{}` : une date est attendue", c)))
                .collect()
        };
        let carte = |c: &str| -> Resultat<BTreeMap<String, String>> {
            Ok(e.get(c)
                .and_then(Value::as_object)
                .ok_or_else(|| format!("engagement sans `{}`", c))?
                .iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                .collect())
        };
        Ok(Eng {
            debut: requis_texte("debut")?,
            fin: texte("fin"),
            mois: requis_texte("mois")?,
            numero: texte("numero"),
            dernier_mois: e.get("dernier_mois").and_then(Value::as_bool).ok_or("engagement sans `dernier_mois`")?,
            dates_travail: liste("dates_travail")?,
            planning: liste("planning")?,
            lieux: carte("lieux")?,
            planning_lieux: carte("planning_lieux")?,
            lieu: requis_texte("lieu")?,
            date_paiement: texte("date_paiement"),
            cachets: requis_texte("cachets")?,
            taux_cachet: texte("taux_cachet"),
        })
    }

    /// `Engagement.dates_du_contrat` : les jours du mois, puis le planning.
    fn dates_du_contrat(&self) -> Vec<&String> {
        self.dates_travail.iter().chain(self.planning.iter()).collect()
    }

    /// `Engagement.lieu_de` : la date l'emporte, l'engagement supplee.
    fn lieu_de(&self, jour: &str) -> &str {
        [self.lieux.get(jour), self.planning_lieux.get(jour)]
            .into_iter()
            .flatten()
            .find(|l| !l.is_empty())
            .map_or(self.lieu.as_str(), String::as_str)
    }

    /// `Engagement.lieux_joues` : dans l'ordre des dates, sans repetition.
    fn lieux_joues(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for j in self.dates_du_contrat() {
            let ou = self.lieu_de(j);
            if !ou.is_empty() && !out.contains(&ou) {
                out.push(ou);
            }
        }
        out
    }

    /// `Engagement.dates_par_lieu()` : [(lieu, [dates])] dans l'ordre des dates, sans retrier.
    fn dates_par_lieu(&self) -> Vec<(&str, Vec<&String>)> {
        let mut out: Vec<(&str, Vec<&String>)> = Vec::new();
        for j in self.dates_du_contrat() {
            let ou = self.lieu_de(j);
            match out.last_mut() {
                Some((dernier, dates)) if *dernier == ou => dates.push(j),
                _ => out.push((ou, vec![j])),
            }
        }
        out
    }
}

/// « 2 cachets à 130,00 € » : une ligne de remuneration telle que le contrat la cite.
fn ligne_citee(l: &Value) -> Resultat<String> {
    let designation = requis(l, "designation")?.as_str().ok_or("designation : un texte est attendu")?;
    // La designation ouvre une phrase : « 2 Cachets » y devient « 2 cachets ». On ne touche
    // qu'a la premiere majuscule.
    let mut texte = String::new();
    let mut fait = false;
    for c in designation.chars() {
        if !fait && c.is_uppercase() {
            texte.extend(c.to_lowercase());
            fait = true;
        } else {
            texte.push(c);
        }
    }
    Ok(format!("{} à {} €", texte, fmt_eur(dec(requis(l, "taux")?)?)))
}

/// La ligne fait-elle le brut ? (`soumis` de la rubrique ; une paie qui ne le dit pas, si.)
fn fait_le_brut(l: &Value) -> bool {
    l.get("soumis").and_then(Value::as_bool).unwrap_or(true)
}

/// `contrat._remuneration_detaillee` : « 2 cachets à 130,00 € », ou « 1 journée de travail à
/// 200,00 € et 6 h de masterclass à 40,00 € ». Le modele la dit « brut » : elle ne cite que les
/// lignes qui font le brut ; les autres (les defraiements...) sont dans `remuneration_hors_brut`.
fn remuneration_detaillee(remuneration: &[Value], e: &Eng) -> Resultat<String> {
    let mut morceaux = Vec::new();
    for l in remuneration.iter().filter(|l| fait_le_brut(l)) {
        morceaux.push(ligne_citee(l)?);
    }
    let mut phrase = et(&morceaux);
    if !e.dernier_mois {
        let nom = crate::date::nom_du_mois(mois_de(&e.mois)? as i64, crate::date::Graphie::Accentuee)?;
        let quand = format!("{} {}", nom, an_de(&e.mois)?);
        if !e.planning.is_empty() {
            let n = e.planning.len();
            phrase += &format!(
                " pour {} et, pour {} du planning, payée{} dans {} mois, le même montant",
                quand,
                if n > 1 { format!("les {} dates suivantes", n) } else { "la date suivante".to_string() },
                if n > 1 { "s" } else { "" },
                if n > 1 { "leur" } else { "son" }
            );
        } else {
            phrase += &format!(" pour {} et, pour les dates suivantes du planning, payées dans leur mois, le même montant", quand);
        }
    }
    Ok(phrase)
}

/// Ce que le contrat verse EN PLUS du brut -- les lignes non soumises qui s'ajoutent (defraiements,
/// indemnites hors assiette), convenues comme le reste : « , auxquels s'ajoutent 3 défraiements au
/// forfait à 20,20 € hors brut ». Vide sans elles : la phrase du modele reste alors mot pour mot
/// celle d'avant. Une ligne qui retranche (un acompte) n'est pas une clause du contrat.
fn remuneration_hors_brut(remuneration: &[Value]) -> Resultat<String> {
    let mut morceaux = Vec::new();
    for l in remuneration.iter().filter(|l| !fait_le_brut(l)) {
        if dec(requis(l, "montant")?)? > crate::num::Decimal::ZERO {
            morceaux.push(ligne_citee(l)?);
        }
    }
    Ok(if morceaux.is_empty() { String::new() } else { format!(", auxquels s'ajoutent {} hors brut", et(&morceaux)) })
}

/// Le numero du mois, de 1 a 12 : « 00 » se refuse (`date::numero_du_mois`, lot F1 ; il se lisait janvier).
fn mois_de(mois: &str) -> Resultat<usize> {
    crate::date::numero_du_mois(mois).map(|n| n as usize)
}

fn an_de(mois: &str) -> Resultat<i32> {
    mois.get(..4).and_then(|m| m.parse().ok()).ok_or_else(|| format!("mois illisible : {}", mois))
}

/// `contrat._periode_remuneration` : « pour le mois d'octobre 2026 » quand le contrat se poursuit.
fn periode_remuneration(e: &Eng) -> Resultat<String> {
    if e.dernier_mois {
        return Ok(String::new());
    }
    let nom = crate::date::nom_du_mois(mois_de(&e.mois)? as i64, crate::date::Graphie::Accentuee)?;
    let de = if nom.chars().next().map_or(false, |c| "aeiouyâàéèêîôû".contains(c)) { "d'" } else { "de " };
    Ok(format!(" pour le mois {}{} {}", de, nom, an_de(&e.mois)?))
}

/// `contrat._dates_travail` : les jours joues, et OU chacun se joue quand ils different.
fn dates_travail(e: &Eng) -> Resultat<String> {
    let mut texte = if e.lieux_joues().len() < 2 {
        let v: Resultat<Vec<String>> = e.dates_du_contrat().into_iter().map(|j| jj_mm_aaaa(j)).collect();
        v?.join(", ")
    } else {
        let mut groupes = Vec::new();
        for (ou, jours) in e.dates_par_lieu() {
            let v: Resultat<Vec<String>> = jours.into_iter().map(|j| jj_mm_aaaa(j)).collect();
            let dates = et(&v?);
            groupes.push(if ou.is_empty() { dates } else { format!("{} à {}", dates, ou) });
        }
        groupes.join(", ")
    };
    if !e.dernier_mois && e.planning.is_empty() {
        let fin = e.fin.as_deref().ok_or("contrat sans fin")?;
        texte += &format!(", puis les dates du planning jusqu'au {}", jj_mm_aaaa(fin)?);
    }
    Ok(texte)
}

/// `contrat._lieu_du_contrat` : l'article « Lieu de travail », qui suit ce que les dates ont dit.
fn lieu_du_contrat(e: &Eng) -> String {
    let lieux = e.lieux_joues();
    match lieux.len() {
        0 => "au lieu de la représentation".to_string(),
        1 => format!("à {}", lieux[0]),
        _ => "aux lieux indiqués ci-dessus".to_string(),
    }
}

/// `contrat.marqueurs` : les valeurs offertes au modele. `remuneration`, `brut` et `net` sont ceux
/// de la paie calculee ; `sal` la fiche du salarie telle qu'elle vaut pour cet engagement.
fn marqueurs(ent: &Value, sal: &Value, sp: &Value, e: &Eng, remuneration: &[Value], brut: &Value, net: &Value) -> Resultat<Valeurs> {
    let (idcc, convention) = convention_du_contrat(ent, sp, sal)?;
    let sig = requis(ent, "signataire")?;
    let licence = match requis(ent, "licence_spectacle")? {
        Value::String(s) => s.clone(),
        autre => return Err(format!("licence_spectacle non resolue : elle vaut {}", en_texte(autre))),
    };
    let mut m = Valeurs::new();
    let mut poser = |k: &str, v: String| {
        m.insert(k.to_string(), v);
    };
    poser("employeur.raison_sociale", texte_ou_invalide(requis(ent, "raison_sociale")?));
    poser("employeur.adresse", texte_ou_invalide(aff(ent, "adresse")?));
    poser("employeur.code_postal", texte_ou_invalide(requis(ent, "code_postal")?));
    poser("employeur.commune", texte_ou_invalide(aff(ent, "commune")?));
    poser("employeur.siret", texte_ou_invalide(requis(ent, "siret")?));
    poser("employeur.naf", texte_ou_invalide(requis(ent, "naf")?));
    poser("employeur.licence", licence);
    poser("employeur.idcc", idcc);
    poser("employeur.convention", convention);
    // Le numero sous lequel la compagnie est affiliee a la Caisse : son numero d'adherent (`conges_spectacles::adherent`).
    poser("employeur.congesspectacles", crate::conges_spectacles::adherent(ent)?);
    poser(
        "employeur.signataire",
        format!("{} {}", en_texte(aff(sig, "prenom")?), en_texte(aff(sig, "nom")?)),
    );
    poser("employeur.signataire_fonction", texte_ou_invalide(requis(sig, "fonction")?));
    poser("salarie.nom", texte_ou_invalide(requis(sal, "nom")?));
    poser("salarie.prenom", texte_ou_invalide(aff(sal, "prenom")?));
    poser(
        "salarie.nom_complet",
        format!("{} {}", en_texte(aff(sal, "prenom")?), en_texte(requis(sal, "nom")?)),
    );
    poser("salarie.date_naissance", d(requis(sal, "date_naissance")?)?.jj_mm_aaaa());
    poser("salarie.lieu_naissance", texte_ou_invalide(aff(sal, "lieu_naissance")?));
    poser("salarie.adresse", texte_ou_invalide(aff(sal, "adresse")?));
    poser("salarie.code_postal", texte_ou_invalide(requis(sal, "code_postal")?));
    poser("salarie.commune", texte_ou_invalide(aff(sal, "commune")?));
    // Le numero technique temporaire n'a rien a faire ici : un salarie pas encore immatricule lit
    // ce qui est vrai.
    poser(
        "salarie.nir",
        if sal.get("nir").map_or(false, vrai) {
            format!("{}{}", en_texte(requis(sal, "nir")?), en_texte(requis(sal, "nir_cle")?))
        } else {
            "en cours d'immatriculation".to_string()
        },
    );
    let emploi = aff(sal, "emploi")?;
    let emploi_texte = emploi.as_str().ok_or("l'emploi n'est pas un texte")?;
    poser("salarie.emploi", emploi_texte.to_lowercase());
    poser("salarie.qualite_de", de_devant(emploi_texte).to_string());
    // Un numero que le moteur ne porte pas (contrat pas encore numerote) n'est pas devine : le
    // Python le prendrait au compteur.
    poser(
        "contrat.numero",
        match e.numero.as_deref() {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => INVALIDE.to_string(),
        },
    );
    poser("contrat.debut", jj_mm_aaaa(&e.debut)?);
    poser("contrat.fin", jj_mm_aaaa(e.fin.as_deref().ok_or("contrat sans fin")?)?);
    poser("contrat.dates_travail", dates_travail(e)?);
    poser("contrat.lieu", lieu_du_contrat(e));
    let paiement = match e.date_paiement.as_deref() {
        Some(p) if !p.is_empty() => p,
        _ => e.fin.as_deref().ok_or("contrat sans fin")?,
    };
    poser("contrat.date_paiement", jj_mm_aaaa(paiement)?);
    poser("remuneration.detail", remuneration_detaillee(remuneration, e)?);
    poser("remuneration.periode", periode_remuneration(e)?);
    poser("remuneration.hors_brut", remuneration_hors_brut(remuneration)?);
    poser("remuneration.cachets", e.cachets.clone());
    // Sans taux de cachet, pas de marqueur : un modele qui le cite se refuse (`contrat::refuser_ce_que_le_modele_tairait`).
    if let Some(t) = &e.taux_cachet {
        poser("remuneration.taux_cachet", fmt_eur(dec(&Value::String(t.clone()))?));
    }
    poser("remuneration.brut", fmt_eur(dec(brut)?));
    poser("remuneration.net_estime", fmt_eur(dec(net)?));
    poser("spectacle.titre", texte_ou_invalide(requis(sp, "titre")?));
    poser("spectacle.numero_objet", texte_ou_invalide(requis(sp, "numero_objet")?));
    Ok(m)
}

/// `contrat.texte_brut` : le paragraphe tel qu'il se LIT sur le contrat imprime.
fn texte_brut(texte: &str, vals: &Valeurs) -> Resultat<String> {
    let t = modele::texte_brut(texte, vals).map_err(|r| r.0)?;
    if t.contains(INVALIDE) {
        return Err("une valeur du modele n'est pas un texte".to_string());
    }
    Ok(t)
}

// ------------------------------------------------- ce que l'avenant modifie

/// Le refus de `avenant.modifications` : `AvenantImpossible` (l'ancien texte ne se retrouve pas),
/// ou tout autre refus (une valeur du modele qui n'est pas un texte, un modele illisible).
#[derive(Debug)]
pub enum ErrAvenant {
    Impossible(String),
    Autre(String),
}

impl From<String> for ErrAvenant {
    fn from(m: String) -> Self {
        ErrAvenant::Autre(m)
    }
}

impl From<&str> for ErrAvenant {
    fn from(m: &str) -> Self {
        ErrAvenant::Autre(m.to_string())
    }
}

impl ErrAvenant {
    pub fn message(self) -> String {
        match self {
            ErrAvenant::Impossible(m) | ErrAvenant::Autre(m) => m,
        }
    }
}

/// `avenant.segment_du_contrat` : le texte d'UN contrat dans un PDF qui peut en porter plusieurs
/// (un par page) : de son sous-titre (« Contrat n° ... ») au titre du suivant.
fn segment_du_contrat(texte: &str, m: &Modele, vals: &Valeurs, numero: &str) -> Result<String, ErrAvenant> {
    let mut avec_numero = vals.clone();
    avec_numero.insert("contrat.numero".to_string(), numero.to_string());
    let sous = normaliser(&texte_brut(&m.soustitre, &avec_numero)?);
    let debut = if sous.is_empty() {
        0
    } else {
        texte
            .find(&sous)
            .ok_or_else(|| ErrAvenant::Impossible(format!("Le contrat n° {} ne se trouve pas dans le fichier envoyé.", numero)))?
    };
    let titre = normaliser(&texte_brut(&m.titre, vals)?);
    let depart = debut + sous.len();
    let fin = if titre.is_empty() { None } else { texte[depart..].find(&titre).map(|i| depart + i) };
    let fin = match fin {
        Some(f) if f > 0 => f,
        _ => texte.len(),
    };
    Ok(texte[debut..fin].to_string())
}

/// Un article du contrat qui a change depuis le PDF parti : son numero, son titre, et pour chaque
/// paragraphe change l'ancien texte, le nouveau et son rang dans l'article.
pub struct Modification {
    pub numero: usize,
    pub article: String,
    pub avant: Vec<Value>,
    pub apres: Vec<String>,
    pub blocs: Vec<usize>,
}

/// `avenant.modifications` : les articles du contrat qui ont change depuis le PDF parti, dans
/// l'ordre du modele. Un refus quand un paragraphe change mais que son ancien texte ne se retrouve
/// pas (le modele a change depuis l'envoi : `AvenantImpossible`).
/// `deja` : {(article, bloc): texte} -- ce que les avenants deja ecrits ont ecrit ; c'est lui, et non
/// le PDF d'origine, qui fait foi pour ces paragraphes-la.
pub fn modifications(
    m: &Modele,
    vals: &Valeurs,
    texte_pdf: &str,
    numero_parti: &str,
    deja: &HashMap<(i64, i64), Value>,
) -> Result<Vec<Modification>, ErrAvenant> {
    let segment = segment_du_contrat(texte_pdf, m, vals, numero_parti)?;
    let seg: Vec<char> = segment.chars().collect();
    let mut out = Vec::new();
    for (n, (titre, corps)) in m.articles.iter().enumerate() {
        let n = n + 1;
        let (mut avant, mut apres, mut blocs): (Vec<Value>, Vec<String>, Vec<usize>) = (Vec::new(), Vec::new(), Vec::new());
        for (i, bloc) in corps.iter().enumerate() {
            let neuf = normaliser(&texte_brut(bloc, vals)?);
            if let Some(ancien) = deja.get(&(n as i64, i as i64)) {
                let texte = ancien.as_str().ok_or("un ancien texte n'est pas un texte")?;
                if normaliser(texte) != neuf {
                    avant.push(ancien.clone());
                    apres.push(neuf);
                    blocs.push(i);
                }
                continue;
            }
            if segment.contains(&neuf) {
                continue;
            }
            let Some((d, f)) = trouve(&motif(bloc), &seg) else {
                return Err(ErrAvenant::Impossible(format!(
                    "L'article {} « {} » du contrat parti ne se retrouve pas dans le modèle de contrat d'aujourd'hui (le modèle a changé depuis l'envoi ?). L'avenant doit citer l'ancien texte : faire plutôt un contrat qui annule et remplace, ou rédiger l'avenant à la main.",
                    n,
                    texte_brut(titre, vals)?
                )));
            };
            let trouve_ici: String = seg[d..f].iter().collect();
            avant.push(Value::String(trouve_ici.trim_matches(est_blanc).to_string()));
            apres.push(neuf);
            blocs.push(i);
        }
        if !avant.is_empty() {
            out.push(Modification { numero: n, article: texte_brut(titre, vals)?, avant, apres, blocs });
        }
    }
    Ok(out)
}

/// `modifications`, reduit a ce que la pastille dit : « article 4 (Rémunération) » pour chaque
/// article change, dans l'ordre du modele.
fn articles_modifies(
    m: &Modele,
    vals: &Valeurs,
    texte_pdf: &str,
    numero_parti: &str,
    deja: &HashMap<(i64, i64), Value>,
) -> Resultat<Vec<(usize, String)>> {
    modifications(m, vals, texte_pdf, numero_parti, deja)
        .map(|v| v.into_iter().map(|x| (x.numero, x.article)).collect())
        .map_err(ErrAvenant::message)
}

// ------------------------------------------------------ le journal

/// Le dernier contrat parti a la signature pour cette paie (`operations_documents.contrat_parti`) :
/// le mois, le numero, le fichier, la date d'envoi et, s'il est revenu, celle de la signature.
/// Un avenant n'en est pas un.
pub(crate) struct Parti {
    pub(crate) mois: Value,
    pub(crate) numero: Value,
    pub(crate) fichier: Value,
    pub(crate) quand: Value,
    pub(crate) signe: Value,
}

impl Parti {
    /// `(signe or quand or "")[:10]` : le jour qui date le contrat dans un avenant.
    pub(crate) fn date(&self) -> String {
        let t = if vrai(&self.signe) {
            en_texte(&self.signe)
        } else if vrai(&self.quand) {
            en_texte(&self.quand)
        } else {
            String::new()
        };
        t.chars().take(10).collect()
    }
}

/// `operations_documents.contrat_parti`. `numero` : le numero SOUS LEQUEL LE JOURNAL parle du contrat (Retours Windows, B,
/// suite : deux contrats du meme salarie qui commencent le meme jour sont deux contrats) ; `None` : tous ceux de ce salarie a
/// ce debut (le dernier l'emporte), comme avant.
pub(crate) fn contrat_parti(lignes: &[Value], salarie: &str, debut: &str, numero: Option<&str>) -> Result<Option<Parti>, Echec> {
    let du_contrat = |l: &Value| -> Result<bool, Echec> {
        Ok(numero.map_or(true, |n| py_str(&crate::etats::numero_de_cle(get(l, "numero").unwrap_or(&Value::Null))) == n))
    };
    let mut parti: Option<Parti> = None;
    for l in lignes {
        if !accepte(l)? {
            continue;
        }
        let d = ou(get(l, "detail")?, objet_vide());
        let quoi = get(l, "quoi")?;
        if est(quoi, SIGNATURE) {
            for c in items(getd(d, "contrats", &LISTE_VIDE)?)? {
                if !(est(get(&c, "salarie")?, salarie) && est(get(&c, "debut")?, debut) && du_contrat(&c)?) {
                    continue;
                }
                if est(get(&c, "acte")?, "avenant") {
                    continue;
                }
                parti = Some(Parti {
                    mois: ou(get(&c, "mois")?, get(l, "mois")?).clone(),
                    numero: get(&c, "numero")?.clone(),
                    fichier: get(&c, "fichier")?.clone(),
                    quand: get(l, "quand")?.clone(),
                    signe: Value::Null,
                });
            }
        } else if est(quoi, CONTRAT_RECU) {
            if let Some(p) = parti.as_mut() {
                if vrai(get(d, "recu")?) && est(get(d, "salarie")?, salarie) && est(get(d, "debut")?, debut) && du_contrat(d)? && p.signe.is_null() {
                    p.signe = get(l, "quand")?.clone();
                }
            }
        }
    }
    Ok(parti)
}

/// `operations_documents._chemin_piece` : le fichier d'une piece partie, dans `sorties/<mois>/` ou
/// son `envoi/`. Un NOM, jamais un chemin : ce qui porte un separateur ne se resout pas.
pub(crate) fn chemin_piece(racine: &Path, mois: &Value, fichier: &Value) -> Option<PathBuf> {
    if !vrai(mois) || !vrai(fichier) {
        return None;
    }
    let nom = en_texte(fichier);
    if ["/", "\\", ".."].iter().any(|x| nom.contains(x)) {
        return None;
    }
    let base = racine.join("sorties").join(en_texte(mois));
    [base.join("envoi").join(&nom), base.join(&nom)].into_iter().find(|f| f.is_file())
}

/// `operations_documents._avenants_du_contrat` : les avenants du contrat parti -- pas ceux d'un
/// contrat qu'il a remplace. `tous` : ce que `journal.avenants` dit pour cette paie (la liste, ou rien).
pub(crate) fn avenants_du_contrat(tous: Option<&Value>, parti: &Parti) -> Vec<Value> {
    let depuis = if vrai(&parti.quand) { en_texte(&parti.quand) } else { String::new() };
    tous.and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .filter(|x| {
            let quand = match x.get("quand") {
                Some(v) if vrai(v) => en_texte(v),
                _ => String::new(),
            };
            quand >= depuis
        })
        .cloned()
        .collect()
}

/// `operations_documents._textes_des_avenants` : ce que ces avenants ont ecrit, {(article, bloc):
/// texte}, le dernier l'emportant. `signes_seulement` : seuls ceux qui sont revenus signes comptent.
pub(crate) fn textes_des_avenants_de(avenants: &[Value], signes_seulement: bool) -> Resultat<HashMap<(i64, i64), Value>> {
    let entier = |v: &Value| v.as_i64().or_else(|| v.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64));
    let mut textes = HashMap::new();
    for x in avenants {
        if !(x.get("signe").is_some_and(vrai) || !signes_seulement) {
            continue;
        }
        for m in x.get("modifications").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
            let numero = requis(m, "numero")?;
            let blocs = m.get("blocs").filter(|v| vrai(v)).and_then(Value::as_array);
            let apres = m.get("apres").filter(|v| vrai(v)).and_then(Value::as_array);
            if let (Some(blocs), Some(apres), Some(n)) = (blocs, apres, entier(numero)) {
                for (i, t) in blocs.iter().zip(apres) {
                    if let Some(i) = entier(i) {
                        textes.insert((n, i), t.clone());
                    }
                }
            }
        }
    }
    Ok(textes)
}

/// `_avenants_du_contrat` puis `_textes_des_avenants(signes_seulement=False)` pour ce contrat.
fn textes_des_avenants(racine: &Path, salarie: &str, debut: &str, numero: Option<&str>, parti: &Parti) -> Resultat<HashMap<(i64, i64), Value>> {
    let lignes = crate::etats::charger(racine).map_err(|e| e.message)?;
    let rep = crate::etats::avenants(&lignes, crate::etats::Maille::Contrat).map_err(|e| e.message)?;
    let mut avenants = Vec::new();
    for quadruplet in rep.as_array().map(Vec::as_slice).unwrap_or(&[]) {
        let t = quadruplet.as_array().ok_or("avenants : un quadruplet est attendu")?;
        if t.len() != 4 || t[0].as_str() != Some(salarie) || t[1].as_str() != Some(debut) || numero.is_some_and(|n| t[2].as_str() != Some(n)) {
            continue;
        }
        avenants.extend(avenants_du_contrat(Some(&t[3]), parti));
    }
    textes_des_avenants_de(&avenants, false)
}

/// `operations_socle.mois_hors_delai` : la date limite de la DSN du mois (`echeance_dsn`, reportee au jour ouvre) est-elle
/// depassee ? Un mois ferme ne se reproche plus rien : ce qui est declare est declare.
fn mois_hors_delai(ctx: &Contexte, mois: &str) -> Resultat<bool> {
    Ok(ctx.aujourdhui > crate::echeance_dsn::de_la_compagnie(ctx, mois)?.jour)
}

/// `operations_documents._ent_du_mois` : la fiche entreprise, avec la licence du mois et les gabarits de la compagnie.
pub(crate) fn ent_du_mois(ctx: &Contexte, mois: &str) -> Resultat<Value> {
    let licence = ctx.licence(&Jour::lire(&format!("{}-01", mois.get(..7).ok_or("mois illisible")?))?)?;
    let mut ent = ctx.entreprise.clone();
    let o = ent.as_object_mut().ok_or("fiche entreprise : un objet est attendu")?;
    o.insert("licence_spectacle".into(), Value::String(licence));
    o.insert("_gabarits".into(), Value::String(ctx.gabarits().to_string_lossy().into_owned()));
    Ok(ent)
}

/// Le dernier depot REEL accepte de DSN pour ce mois existe-t-il ? Alors le mois est parti : il
/// garde ses chiffres (`calcul_du_mois`).
fn dsn_partie(ctx: &Contexte, mois: &str) -> Resultat<bool> {
    let lignes = crate::etats::charger(&ctx.racine).map_err(|e| e.message)?;
    Ok(!crate::etats::dsn_du_mois(&lignes, &ctx.entreprise, mois).map_err(|e| e.message)?.is_null())
}

/// La paie de ce contrat dans la paie calculee du mois (relue sans calcul si ses entrees n'ont pas changé).
/// `rang` : la paie de ce rang parmi celles de ce salarie qui commencent ce jour-la (0 : la premiere).
fn paie_calculee(ctx: &Contexte, mois: &str, salarie: &str, debut: &str, rang: usize) -> Resultat<Option<Value>> {
    let mut o = HashMap::new();
    o.insert("racine".to_string(), ctx.racine.to_string_lossy().into_owned());
    o.insert("date".to_string(), ctx.date.iso());
    let demande = json!({ "mois": mois, "parti": dsn_partie(ctx, mois)? });
    let rep = crate::calcul_du_mois::commande(&o, &demande.to_string())?;
    let paies = rep.get("paies").and_then(Value::as_array).ok_or("la paie calculee n'a pas de `paies`")?;
    Ok(paies
        .iter()
        .filter(|p| {
            p.pointer("/salarie/_cle").and_then(Value::as_str) == Some(salarie)
                && p.pointer("/engagement/debut").and_then(Value::as_str) == Some(debut)
        })
        .nth(rang)
        .cloned())
}

/// Les valeurs que le modele de contrat lit dans la paie d'aujourd'hui de ce contrat (`contrat.marqueurs`) :
/// de quoi ecrire le contrat tel que la paie le dirait. `None` quand la paie n'a pas ce contrat, ou n'est
/// pas celle d'un spectacle (un permanent n'a pas de contrat imprime). Les montants viennent de la paie
/// calculee relue (`calcul_du_mois`), sans calcul si ses entrees n'ont pas change.
pub fn valeurs_du_contrat(ctx: &Contexte, mois: &str, salarie: &str, debut: &str) -> Resultat<Option<Valeurs>> {
    valeurs_du_rang(ctx, mois, salarie, debut, 0)
}

/// `valeurs_du_contrat` de la paie de ce `rang` parmi celles du salarie qui commencent ce jour-la.
pub fn valeurs_du_rang(ctx: &Contexte, mois: &str, salarie: &str, debut: &str, rang: usize) -> Resultat<Option<Valeurs>> {
    let Some(paie) = paie_calculee(ctx, mois, salarie, debut, rang)? else {
        return Ok(None);
    };
    let nom_spectacle = paie.pointer("/engagement/spectacle").and_then(Value::as_str);
    if nom_spectacle.is_none() || !paie.get("spectacle").map_or(false, |s| !s.is_null()) {
        return Ok(None);
    }
    let ent = ent_du_mois(ctx, mois)?;
    valeurs_de_la_paie(ctx, &ent, &paie, None).map(Some)
}

/// `contrat.marqueurs(p, ent)` pour une paie calculee (`paie`, sa forme JSON). `numero` : le numero
/// de contrat que la paie porte VRAIMENT (lu dans l'archive, fige ou propose par le compteur), quand il
/// differe de celui de la reponse du calcul.
pub(crate) fn valeurs_de_la_paie(ctx: &Contexte, ent: &Value, paie: &Value, numero: Option<&Option<String>>) -> Resultat<Valeurs> {
    let nul = Value::Null;
    let nom_spectacle = paie.pointer("/engagement/spectacle").and_then(Value::as_str).ok_or("la paie n'est pas celle d'un spectacle")?;
    let mut e = Eng::lire(requis(paie, "engagement")?)?;
    if let Some(n) = numero {
        e.numero = n.clone();
    }
    let texte = |c: &str| paie.pointer(&format!("/engagement/{}", c)).and_then(Value::as_str).map(str::to_string).filter(|s| !s.is_empty());
    let fiche = ctx.salarie(requis(paie, "engagement")?.get("salarie").and_then(Value::as_str).ok_or("engagement sans salarie")?)?;
    let emploi = EmploiEngagement {
        emploi: texte("emploi"),
        emploi_affichage: texte("emploi_affichage"),
        pcs_ese: texte("pcs_ese"),
        code_emploi_ft: texte("code_emploi_ft"),
        statut: texte("statut"),
        statut_conventionnel: texte("statut_conventionnel"),
        statut_categoriel_rc: texte("statut_categoriel_rc"),
    };
    let sal = emploi_de(&fiche, Some(&e.debut), &emploi, &|f, j| telle_qu_au(f, &Jour::lire(j)?))?;
    let sp = ctx.spectacle(nom_spectacle)?;
    let remuneration = requis(paie, "remuneration")?.as_array().ok_or("remuneration : une liste est attendue")?;
    marqueurs(ent, &sal, &sp, &e, remuneration, paie.get("brut").unwrap_or(&nul), paie.get("net_a_payer").unwrap_or(&nul))
}

/// `contrat.lire_modele(modele_de(contrat.MODELE.name, ent))` pour le contrat d'engagement.
pub(crate) fn modele_du_contrat(ent: &Value, gabarits: &Path) -> Resultat<Modele> {
    modele::lire_fichier(&crate::dpae::modele_de(MODELE, ent, gabarits).to_string_lossy()).map_err(|r| r.0)
}

fn ecart(ctx: &Contexte, mois: &str, salarie: &str, debut: &str, numero: Option<&str>, rang: usize) -> Resultat<String> {
    // UN MOIS FERME (echeance de sa DSN passee) ne se reproche plus rien : ce qui est declare est
    // declare (l'editeur, 30/09/2026 : « pour aout [...] pas de soucis, laisse ca »).
    if mois_hors_delai(ctx, mois)? {
        return Ok(String::new());
    }
    // Seulement au mois ou le contrat est parti : ailleurs, c'est la paie d'un autre mois, que le
    // contrat ne decrit pas mot pour mot.
    let lignes = crate::journal::lire(&ctx.racine, None, None, false)?;
    let Some(parti) = contrat_parti(&lignes, salarie, debut, numero).map_err(|e| e.message)? else {
        return Ok(String::new());
    };
    if parti.mois.as_str() != Some(mois) {
        return Ok(String::new());
    }
    let Some(pdf) = chemin_piece(&ctx.racine, &parti.mois, &parti.fichier) else {
        return Ok(String::new());
    };
    // Un avenant ENVOYE compte deja, signe ou non : ce qu'il attend, c'est sa signature, pas un autre avenant.
    let deja = textes_des_avenants(&ctx.racine, salarie, debut, numero, &parti)?;
    let ent = ent_du_mois(ctx, mois)?;
    let modele = modele_du_contrat(&ent, &ctx.gabarits())?;
    let texte_pdf = texte_du_pdf(&pdf)?;
    // Seulement maintenant, la paie : ses montants, la designation et le taux de ses lignes.
    let Some(vals) = valeurs_du_rang(ctx, mois, salarie, debut, rang)? else {
        return Ok(String::new());
    };
    let numero_parti = if vrai(&parti.numero) { texte_ou_invalide(&parti.numero) } else { String::new() };
    let modifies = articles_modifies(&modele, &vals, &texte_pdf, &numero_parti, &deja)?;
    Ok(modifies.iter().map(|(n, a)| format!("article {} ({})", n, a)).collect::<Vec<_>>().join(", "))
}

/// `operations_documents.ecart_au_contrat_signe` : CE QUE LE CONTRAT PARTI NE DIT PLUS (30/09/2026,
/// l'editeur : « ce qui est signe n'est pas ce qu'il y a dans l'engagement que je vois -- simple et
/// efficace »). Le PDF parti se compare a la paie, comme pour l'avenant, et la pastille le dit sans
/// qu'on ouvre rien : « article 1 (Objet et nature du contrat), article 4 (Rémunération) ». Vide si
/// rien n'a change, si rien n'est parti, ou si la comparaison est impossible (le PDF n'est plus la,
/// le modele a change) -- la pastille n'invente pas.
///
/// Un avenant ENVOYE compte deja, signe ou non : ce qu'il attend, c'est sa signature (`avenant_attendu`),
/// pas un autre avenant. Seulement au mois ou le contrat est parti : ailleurs, c'est la paie d'un
/// autre mois, que le contrat ne decrit pas mot pour mot. `ctx` est lu a sa date.
pub fn ecart_au_contrat_signe(ctx: &Contexte, mois: &str, salarie: &str, debut: &str) -> String {
    ecart(ctx, mois, salarie, debut, None, 0).unwrap_or_default()
}

/// `ecart_au_contrat_signe` d'UN contrat, quand le salarie en a deux qui commencent le meme jour : `numero` est celui sous
/// lequel le journal le connait, `rang` sa place parmi les paies du salarie a ce debut.
pub fn ecart_du_contrat(ctx: &Contexte, mois: &str, salarie: &str, debut: &str, numero: Option<&str>, rang: usize) -> String {
    ecart(ctx, mois, salarie, debut, numero, rang).unwrap_or_default()
}

#[cfg(test)]
mod tests_mois {
    use super::*;

    fn eng(mois: &str) -> Eng {
        Eng::lire(&serde_json::json!({
            "debut": "2026-09-28", "fin": "2026-11-05", "mois": mois, "dernier_mois": false, "dates_travail": [],
            "planning": ["2026-10-02"], "lieux": {}, "planning_lieux": {}, "lieu": "", "cachets": "0", "taux_cachet": null
        }))
        .unwrap()
    }

    /// Lot F1 : un contrat qui se poursuit dit « pour le mois de ... » ; le mois « 00 » n'en est aucun et se refuse
    /// comme `envoi::etiquette` (il se lisait janvier).
    #[test]
    fn le_mois_00_d_un_contrat_qui_se_poursuit_se_refuse() {
        assert_eq!(periode_remuneration(&eng("2026-00")).unwrap_err(), crate::date::mois_impossible(0));
        assert_eq!(remuneration_detaillee(&[], &eng("2026-00")).unwrap_err(), crate::date::mois_impossible(0));
        assert_eq!(periode_remuneration(&eng("2026-10")).unwrap(), " pour le mois d'octobre 2026");
    }
}
