//! L'AVENANT D'UN CONTRAT PARTI : ce que le contrat dirait aujourd'hui, compare a ce qu'il disait sur la version de
//! l'engagement sur laquelle il est parti (`engagement::etats::version_au_depart`). L'ancien texte n'est pas relu dans le
//! PDF signe : il s'ecrit du modele avec les valeurs de la version d'avant, telles qu'elles etaient ecrites (regle 4), et
//! chaque paragraphe des articles du modele qui differe d'une version a l'autre est une modification (`modifications`).
//!
//! Ici, les valeurs que le modele de contrat lit dans une paie (`valeurs_de_la_paie`, `contrat.marqueurs`), le contrat
//! parti que le journal connait (`contrat_parti`) et ses avenants. Le texte de l'avenant vit dans `avenant-cddu.modele`,
//! une donnee, comme celui du contrat.


use std::collections::BTreeMap;
use std::path::Path;

use fabrique_pdf::modele::{self, Modele, Valeurs};
use serde_json::Value;

use crate::bulletin::{convention_du_contrat, fmt_eur};
use droit::contexte::Contexte;
use socle::date::{d, en_texte, Jour};
use droit::emplois::{emploi_de, EmploiEngagement};
use socle::fichiers::vrai;
use socle::nfkc::nfkc;
use socle::num::{dec, Resultat};

/// `contrat.MODELE.name`.
const MODELE: &str = "contrat-cddu.modele";

/// Ce qui remplace une valeur qui n'est pas un texte : elle ne se refuse que si le modele s'en
/// sert. Une phrase qui la porte est un refus.
const INVALIDE: &str = "\u{0}\u{0}VALEUR-SANS-TEXTE\u{0}\u{0}";


// ------------------------------------------------------ le texte qu'on compare

use socle::texte::blanc as est_blanc;

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
/// (`Engagement::canon`, dans la paie calculee).
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
    /// `None` : un contrat sans cachet dont ni l'engagement ni le mois n'ecrivent de taux.
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
        let nom = socle::date::nom_du_mois(mois_de(&e.mois)? as i64, socle::date::Graphie::Accentuee)?;
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
        if dec(requis(l, "montant")?)? > socle::num::Decimal::ZERO {
            morceaux.push(ligne_citee(l)?);
        }
    }
    Ok(if morceaux.is_empty() { String::new() } else { format!(", auxquels s'ajoutent {} hors brut", et(&morceaux)) })
}

/// Le numero du mois, de 1 a 12 : « 00 » se refuse (`date::numero_du_mois`).
fn mois_de(mois: &str) -> Resultat<usize> {
    socle::date::numero_du_mois(mois).map(|n| n as usize)
}

fn an_de(mois: &str) -> Resultat<i32> {
    mois.get(..4).and_then(|m| m.parse().ok()).ok_or_else(|| format!("mois illisible : {}", mois))
}

/// `contrat._periode_remuneration` : « pour le mois d'octobre 2026 » quand le contrat se poursuit.
fn periode_remuneration(e: &Eng) -> Resultat<String> {
    if e.dernier_mois {
        return Ok(String::new());
    }
    let nom = socle::date::nom_du_mois(mois_de(&e.mois)? as i64, socle::date::Graphie::Accentuee)?;
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
    let (idcc, convention) = convention_du_contrat(ent, sp, sal, &Jour::lire(&e.debut)?)?;
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
    // Un numero que le moteur ne porte pas (contrat pas encore numerote) n'est pas devine ni pris au compteur.
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

/// Un article du contrat qui a change depuis la version sur laquelle il est parti : son numero, son titre, et pour chaque
/// paragraphe change l'ancien texte, le nouveau et son rang dans l'article.
pub struct Modification {
    pub numero: usize,
    pub article: String,
    pub avant: Vec<Value>,
    pub apres: Vec<String>,
    pub blocs: Vec<usize>,
}

/// Les articles du modele `m` dont un paragraphe s'ecrit autrement avec les valeurs d'aujourd'hui (`apres`) qu'avec celles
/// de la version sur laquelle le contrat est parti (`avant`), dans l'ordre du modele.
pub fn modifications(m: &Modele, avant: &Valeurs, apres: &Valeurs) -> Resultat<Vec<Modification>> {
    let mut out = Vec::new();
    for (n, (titre, corps)) in m.articles.iter().enumerate() {
        let n = n + 1;
        let (mut anciens, mut neufs, mut blocs): (Vec<Value>, Vec<String>, Vec<usize>) = (Vec::new(), Vec::new(), Vec::new());
        for (i, bloc) in corps.iter().enumerate() {
            let (ancien, neuf) = (normaliser(&texte_brut(bloc, avant)?), normaliser(&texte_brut(bloc, apres)?));
            if ancien != neuf {
                anciens.push(Value::String(ancien));
                neufs.push(neuf);
                blocs.push(i);
            }
        }
        if !anciens.is_empty() {
            out.push(Modification { numero: n, article: texte_brut(titre, apres)?, avant: anciens, apres: neufs, blocs });
        }
    }
    Ok(out)
}

/// La fiche entreprise, avec la licence du mois et les gabarits de la compagnie.
pub fn ent_du_mois(ctx: &Contexte, mois: &str) -> Resultat<Value> {
    let licence = ctx.licence(&Jour::lire(&format!("{}-01", mois.get(..7).ok_or("mois illisible")?))?)?;
    let mut ent = ctx.entreprise.clone();
    let o = ent.as_object_mut().ok_or("fiche entreprise : un objet est attendu")?;
    o.insert("licence_spectacle".into(), Value::String(licence));
    o.insert("_gabarits".into(), Value::String(ctx.gabarits().to_string_lossy().into_owned()));
    Ok(ent)
}

/// La paie validee de ce contrat (`engagement::etats::validees_du_mois`), dans sa forme ecrite. `rang` : la paie de ce
/// rang parmi celles de ce salarie qui commencent ce jour-la (0 : la premiere).
fn paie_calculee(ctx: &Contexte, mois: &str, salarie: &str, debut: &str, rang: usize) -> Resultat<Option<Value>> {
    let Some((_, paies)) = engagement::etats::validees_du_mois(ctx, mois)? else { return Ok(None) };
    let Some(p) = paies
        .iter()
        .filter(|p| p.salarie.get("_cle").and_then(Value::as_str) == Some(salarie) && p.engagement.debut.iso() == debut)
        .nth(rang)
    else {
        return Ok(None);
    };
    engagement::relire::ecrire(ctx, p).map(Some)
}

/// Les valeurs que le modele de contrat lit dans la paie d'aujourd'hui de ce contrat (`contrat.marqueurs`) :
/// de quoi ecrire le contrat tel que la paie le dirait. `None` quand la paie n'a pas ce contrat, ou n'est
/// pas celle d'un spectacle (un permanent n'a pas de contrat imprime). Les montants viennent de la paie
/// validee de l'engagement, sans calcul.
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
/// de contrat que la paie porte VRAIMENT (lu dans l'archive, ou celui de l'engagement), quand il
/// differe de celui de la reponse du calcul.
pub fn valeurs_de_la_paie(ctx: &Contexte, ent: &Value, paie: &Value, numero: Option<&Option<String>>) -> Resultat<Valeurs> {
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
    let cle = fiche.get("_cle").and_then(Value::as_str).unwrap_or_default();
    let sal = emploi_de(&ctx.salarie_au(cle, &Jour::lire(&e.debut)?)?, &emploi)?;
    let sp = ctx.spectacle(nom_spectacle)?;
    let remuneration = requis(paie, "remuneration")?.as_array().ok_or("remuneration : une liste est attendue")?;
    marqueurs(ent, &sal, &sp, &e, remuneration, paie.get("brut").unwrap_or(&nul), paie.get("net_a_payer").unwrap_or(&nul))
}

/// `contrat.lire_modele(modele_de(contrat.MODELE.name, ent))` pour le contrat d'engagement.
pub fn modele_du_contrat(ent: &Value, gabarits: &Path) -> Resultat<Modele> {
    modele::lire_fichier(&socle::fichiers::modele_de(MODELE, ent, gabarits).to_string_lossy()).map_err(|r| r.0)
}
