//! LES TROIS ETATS D'UN ENGAGEMENT, pour un mois :
//!
//! - EN SAISIE (`EnSaisie`) : il se calcule a mesure pour l'ecran ; rien ne peut partir de lui ;
//! - VALIDE (`Valide`) : « Valider l'engagement » a ecrit, dans l'engagement, `valide` : la date, ses paies calculees,
//!   une par mois qu'il couvre, et pour chacune l'empreinte de ce dont son calcul depend (`empreinte`). Tout ce qui part
//!   part d'elles (`paies_du_mois`, `Valide::paie`) ;
//! - DOCUMENTS PARTIS (`Partis`) : les documents du mois (bulletin, certificat Conges Spectacles, AER) sont partis au
//!   salarie ; ce mois ne se recalcule plus de lui-meme. Le journal des envois le dit (`journal/depots.json`, la ligne
//!   `documents` du mois qui nomme le salarie).
//!
//! Trois types, et seul un engagement valide (ou parti, qui l'est) donne une paie a qui fabrique un document : un
//! engagement en saisie n'a pas de `paie`.
//!
//! LE RECALCUL DE LUI-MEME (`rafraichir`) : a chaque lecture d'un mois (`du_mois`, par ou passe tout ce qui lit une
//! paie validee : l'ecran, les exports, les declarations, les documents, les envois), la paie de chaque engagement
//! valide dont les documents ne sont pas partis se compare a son empreinte. Si elle ne tient plus, la paie se recalcule
//! et se reecrit ; si un chiffre a change, elle dit pourquoi (`raison`) : « taux de PAS reçu » (seul le prelevement
//! change), « règle du JJ/MM/AAAA » (une regle datee s'applique), « calcul corrigé » (le code qui calcule a change),
//! « un autre engagement du mois a changé » (un engagement que son calcul lit), « saisie du mois modifiée » (le taux du
//! cachet, la date de paiement), « configuration de la compagnie modifiée » (fiches, taux, contrats). Un changement qui
//! ne change aucun chiffre ne laisse aucune trace.
//!
//! MODIFIER (`modifier`) repasse l'engagement en saisie ; les paies des mois dont les documents sont partis se gardent
//! (`figees`), et la validation suivante les reprend telles quelles tant que la saisie de leur mois n'a pas change. Si
//! les documents d'un de ses mois sont partis, le moteur pose d'abord la question (`question_avant_de_modifier`).

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::calcul::Paie;
use crate::stock::{self, Objet};
use crate::{empreinte, relire};
use droit::contexte::Contexte;
use socle::date::{en_texte, Jour};
use socle::fichiers::vrai;
use socle::num::Resultat;

/// La raison d'un recalcul : le taux de prelevement a la source a change.
pub const TAUX_DE_PAS: &str = "taux de PAS reçu";
/// La raison d'un recalcul : le code qui calcule a change (le moteur, ou les baremes et catalogues qu'il livre).
pub const CALCUL_CORRIGE: &str = "calcul corrigé";
/// La raison d'un recalcul : la saisie d'un autre engagement que le calcul lit a change (plafond, cumuls, conges).
pub const AUTRE_ENGAGEMENT: &str = "un autre engagement du mois a changé";
/// La raison d'un recalcul : ce que la saisie du mois pose pour tous ses engagements (taux du cachet, date de paiement) a
/// change.
pub const SAISIE_DU_MOIS: &str = "saisie du mois modifiée";
/// La raison d'un recalcul : la configuration de la compagnie (fiches, taux, contrats d'organismes) a change.
pub const CONFIGURATION: &str = "configuration de la compagnie modifiée";

/// La question que le moteur pose avant de modifier un engagement dont des documents sont partis.
pub const QUESTION_DOCUMENTS_PARTIS: &str = "Les bulletins de salaire sont déjà partis. Êtes-vous sûr de vouloir changer \
l'engagement maintenant ? Cela entraînera l'édition d'un nouveau bulletin de salaire.";

// ------------------------------------------------------------------ le journal des envois

/// Ce que le journal des envois (`journal/depots.json`) dit des documents partis : pour chaque (mois, salarie), quand.
pub struct Journal {
    documents: BTreeMap<(String, String), String>,
}

impl Journal {
    /// Le journal tel qu'il est ; vide s'il n'existe pas. Une ligne `documents` refusee (`accepte` faux) ne compte pas.
    pub fn lire(racine: &Path) -> Resultat<Journal> {
        let p = racine.join("journal").join("depots.json");
        let lignes: Vec<Value> = match std::fs::read(&p) {
            Ok(o) => match serde_json::from_slice::<Value>(&o).map_err(|e| format!("{} : {}", p.display(), e))? {
                Value::Array(a) => a,
                _ => Vec::new(),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(socle::fichiers::panne_fichier(&e, &p)),
        };
        let mut documents = BTreeMap::new();
        for l in lignes {
            if l.get("quoi").and_then(Value::as_str) != Some("documents") || l.get("accepte") == Some(&Value::Bool(false)) {
                continue;
            }
            let (Some(mois), quand) = (l.get("mois").and_then(Value::as_str), l.get("quand").map(en_texte).unwrap_or_default()) else { continue };
            for s in l.pointer("/detail/salaries").and_then(Value::as_array).into_iter().flatten() {
                if let Some(cle) = s.get("cle").and_then(Value::as_str) {
                    documents.entry((mois.to_string(), cle.to_string())).or_insert_with(|| quand.clone());
                }
            }
        }
        Ok(Journal { documents })
    }

    /// Quand les documents de `mois` sont partis a ce salarie (la cle de sa fiche) ; `None` s'ils ne le sont pas.
    pub fn documents_partis(&self, mois: &str, cle: &str) -> Option<&str> {
        self.documents.get(&(mois.to_string(), cle.to_string())).map(String::as_str)
    }
}

// ------------------------------------------------------------------ les trois etats

/// Un engagement en saisie.
#[derive(Debug, Clone)]
pub struct EnSaisie {
    pub objet: Objet,
}

/// Un engagement valide : ses paies sont ecrites. Seul ce module en fait un.
#[derive(Debug, Clone)]
pub struct Valide {
    objet: Objet,
    le: String,
}

/// Un engagement valide dont les documents du mois `mois` sont partis le `le`.
#[derive(Debug, Clone)]
pub struct Partis {
    valide: Valide,
    pub mois: String,
    pub le: String,
}

/// Ce qu'est un engagement pour un mois.
#[derive(Debug, Clone)]
pub enum Etat {
    EnSaisie(EnSaisie),
    Valide(Valide),
    Partis(Partis),
}

impl Valide {
    fn de(objet: Objet) -> Option<Valide> {
        let le = objet.brut.get("valide")?.get("le").map(en_texte)?;
        Some(Valide { objet, le })
    }

    /// L'engagement tel qu'il est ecrit.
    pub fn objet(&self) -> &Objet {
        &self.objet
    }

    /// Quand il a ete valide.
    pub fn le(&self) -> &str {
        &self.le
    }

    /// La paie ecrite de `mois` (`{"paie", "empreinte", "recalcul"?}`), si elle l'est.
    pub fn paie_ecrite(&self, mois: &str) -> Option<&Value> {
        self.objet.brut.get("valide")?.get("paies")?.get(mois)
    }

    /// Pourquoi la paie de `mois` s'est recalculee d'elle-meme, et quand : `{"le", "raison"}`.
    pub fn recalcul(&self, mois: &str) -> Option<&Value> {
        self.paie_ecrite(mois)?.get("recalcul").filter(|r| r.is_object())
    }

    /// LA PAIE DE `mois`, TELLE QU'ELLE A ETE VALIDEE : ce que lit ce qui fabrique un document. `data` : la saisie du mois
    /// (`stock::vue_du_mois`), `i` : le rang de l'engagement dans cette saisie.
    pub fn paie(&self, ctx: &Contexte, data: &Value, i: usize) -> Resultat<Paie> {
        let mois = data.get("mois").and_then(Value::as_str).ok_or_else(|| socle::fichiers::absent("mois"))?;
        let ecrite = self.paie_ecrite(mois).and_then(|p| p.get("paie")).ok_or_else(|| {
            format!("{} : l'engagement valide le {} n'a pas de paie ecrite pour {}. Le revalider.", qui_de(&self.objet), self.le, mois)
        })?;
        let une = relire::saisie_d_un_engagement(data, i);
        let mut p = relire::paies(ctx, &une, &[ecrite])?.pop().ok_or("paie ecrite illisible")?;
        p.pas = self.paie_ecrite(mois).and_then(|x| x.get("pas")).filter(|x| x.is_object()).map(droit::pas::Versement::lire).transpose()?;
        Ok(p)
    }
}

impl Partis {
    /// L'engagement valide dont les documents sont partis.
    pub fn valide(&self) -> &Valide {
        &self.valide
    }
}

impl Etat {
    /// L'engagement tel qu'il est ecrit.
    pub fn objet(&self) -> &Objet {
        match self {
            Etat::EnSaisie(e) => &e.objet,
            Etat::Valide(v) => &v.objet,
            Etat::Partis(p) => &p.valide.objet,
        }
    }

    /// L'engagement valide, qu'il soit parti ou non ; `None` en saisie.
    pub fn valide(&self) -> Option<&Valide> {
        match self {
            Etat::EnSaisie(_) => None,
            Etat::Valide(v) => Some(v),
            Etat::Partis(p) => Some(&p.valide),
        }
    }
}

/// La cle de la fiche du salarie d'un engagement (`None` s'il ne nomme aucune fiche).
fn cle_du_salarie(ctx: &Contexte, o: &Objet) -> Option<String> {
    crate::cle_de_l_engagement(ctx, &Value::Object(o.brut.clone()))
}

fn qui_de(o: &Objet) -> String {
    format!("{} (contrat du {})", o.brut.get("salarie").map(en_texte).unwrap_or_default(), crate::debut_brut(&Value::Object(o.brut.clone())))
}

/// Ce qu'est l'engagement `o` pour le mois `mois`, tel qu'il est ecrit.
fn etat(ctx: &Contexte, o: Objet, mois: &str, journal: &Journal) -> Etat {
    let cle = cle_du_salarie(ctx, &o);
    match Valide::de(o.clone()) {
        None => Etat::EnSaisie(EnSaisie { objet: o }),
        Some(v) => match cle.as_deref().and_then(|c| journal.documents_partis(mois, c)) {
            Some(le) => Etat::Partis(Partis { valide: v, mois: mois.to_string(), le: le.to_string() }),
            None => Etat::Valide(v),
        },
    }
}

/// L'etat de chaque engagement que `mois` montre, tel qu'il est ecrit, sans le recalcul.
fn ecrits(ctx: &Contexte, mois: &str) -> Resultat<Vec<Etat>> {
    let journal = Journal::lire(&ctx.racine)?;
    Ok(stock::objets_du_mois(&ctx.racine, mois)?.into_iter().map(|o| etat(ctx, o, mois, &journal)).collect())
}

/// LA LECTURE DE L'ENGAGEMENT VALIDE : l'etat de chaque engagement que `mois` montre, dans l'ordre de l'ecran, apres le
/// recalcul de lui-meme (`rafraichir`). C'est le seul chemin vers une paie validee : l'ecran, les exports, les
/// declarations, les documents et les envois lisent tous par ici, et lisent donc une paie juste. Un recalcul qui ne se
/// fait pas (une saisie du mois qui ne se calcule pas encore) laisse la paie telle qu'elle a ete validee.
pub fn du_mois(ctx: &Contexte, mois: &str) -> Resultat<Vec<Etat>> {
    let _ = rafraichir(ctx, mois);
    ecrits(ctx, mois)
}

/// Pour chaque engagement que `mois` montre, dans l'ordre de l'ecran : est-il valide ? Ne lit aucune paie, donc ne
/// recalcule rien : ce que comptent l'accueil et le bilan d'un mois.
pub fn valides_du_mois(ctx: &Contexte, mois: &str) -> Resultat<Vec<bool>> {
    Ok(stock::objets_du_mois(&ctx.racine, mois)?.iter().map(|o| o.brut.get("valide").is_some_and(|v| v.get("le").is_some())).collect())
}

/// La paie ecrite de `mois` de l'engagement du salarie `cle` (la cle de sa fiche) qui commence le `debut` (AAAA-MM-JJ),
/// lue par `du_mois` ; `None` s'il n'est pas valide ou n'a pas de paie ecrite pour ce mois.
pub fn paie_ecrite_de(ctx: &Contexte, mois: &str, cle: &str, debut: &str) -> Resultat<Option<Value>> {
    Ok(du_mois(ctx, mois)?.into_iter().find_map(|e| {
        let b = Value::Object(e.objet().brut.clone());
        let sien = crate::cle_de_l_engagement(ctx, &b).as_deref() == Some(cle) && crate::debut_brut(&b) == debut;
        if sien { e.valide().and_then(|v| v.paie_ecrite(mois)).cloned() } else { None }
    }))
}

/// RIEN NE PART SANS VALIDATION : les paies validees de `mois`, une par engagement qu'il montre, dans l'ordre de
/// l'ecran, telles qu'elles ont ete ecrites. Refus si un engagement du mois est encore en saisie : il se valide d'abord.
pub fn paies_du_mois(ctx: &Contexte, mois: &str) -> Resultat<Vec<Paie>> {
    let etats = du_mois(ctx, mois)?;
    let Some(data) = stock::vue_du_mois(&ctx.racine, mois)? else { return Ok(Vec::new()) };
    let en_saisie: Vec<String> = etats.iter().filter(|e| e.valide().is_none()).map(|e| qui_de(e.objet())).collect();
    if !en_saisie.is_empty() {
        return Err(format!(
            "{} : {} engagement(s) ne sont pas encore valides ({}). Rien ne part d'un engagement en saisie : le valider \
             d'abord (« Valider l'engagement »).",
            crate::libelle_mois(mois),
            en_saisie.len(),
            en_saisie.join(", ")
        ));
    }
    etats.iter().enumerate().map(|(i, e)| e.valide().ok_or("engagement en saisie")?.paie(ctx, &data, i)).collect()
}

/// CE QUI PEUT PARTIR DE `mois` : sa saisie reduite aux engagements valides (dans l'ordre de l'ecran), et la paie ecrite
/// de chacun, au meme rang. Un engagement en saisie n'y est pas : aucun document ne part de lui. `None` : aucune saisie.
pub fn validees_du_mois(ctx: &Contexte, mois: &str) -> Resultat<Option<(Value, Vec<Paie>)>> {
    let etats = du_mois(ctx, mois)?;
    let Some(data) = stock::vue_du_mois(&ctx.racine, mois)? else { return Ok(None) };
    let tous = data.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut gardes = Vec::new();
    let mut paies = Vec::new();
    for (i, e) in etats.iter().enumerate() {
        let Some(v) = e.valide() else { continue };
        paies.push(v.paie(ctx, &data, i)?);
        gardes.push(tous.get(i).cloned().unwrap_or(Value::Null));
    }
    let mut reduite = data.clone();
    reduite["engagements"] = Value::Array(gardes);
    Ok(Some((reduite, paies)))
}

// ------------------------------------------------------------------ calculer et ecrire

/// Le rang de l'engagement `o` parmi ceux que `mois` montre.
fn rang_dans(racine: &Path, o: &Objet, mois: &str) -> Resultat<usize> {
    stock::objets_du_mois(racine, mois)?
        .iter()
        .position(|x| x.fichier == o.fichier && x.rang == o.rang)
        .ok_or_else(|| format!("{} ne touche pas {}", qui_de(o), mois))
}

/// Le prelevement a la source de la paie `p`, au taux que la DGFiP a transmis et qui vaut a la fin de sa periode
/// (`droit::pas::taux_dgfip`), sinon a la grille de l'article 204 H ; `None` pour un mois sans paie ou un bareme sans grille.
fn prelevement(ctx: &Contexte, p: &Paie) -> Resultat<Option<Value>> {
    let e = &p.engagement;
    if e.sans_paie() {
        return Ok(None);
    }
    let cle = en_texte(p.salarie.get("_cle").ok_or_else(|| socle::fichiers::absent("_cle"))?);
    let fin = e.fin_periode()?;
    let taux = droit::pas::taux_dgfip(ctx, &cle, Some(fin))?;
    let bareme = ctx.bareme(&Jour { an: fin.an, mois: fin.mois, jour: 1 })?;
    let t = |j: Jour| (j.an, j.mois, j.jour);
    let court = droit::natures::contrat_court(&*e.nat()?, t(e.debut), e.fin.map(t), t(fin));
    match droit::pas::du_versement(taux.as_ref(), &bareme, p.net_imposable, Some(fin), court, droit::pas::ZONE_DEFAUT) {
        Ok(v) => Ok(Some(v.canon())),
        Err(droit::pas::ErreurPas::SansGrille(_)) => Ok(None),
        Err(autre) => Err(autre.message().to_string()),
    }
}

/// La paie de l'engagement `o` pour `mois`, calculee maintenant : `{"paie", "pas", "empreinte"}`.
pub fn calculer(ctx: &Contexte, o: &Objet, mois: &str) -> Resultat<Value> {
    let i = rang_dans(&ctx.racine, o, mois)?;
    calculer_les_rangs(ctx, mois, &[i])?.pop().ok_or_else(|| "le calcul du mois n'a pas cette paie".to_string())
}

/// Les paies des engagements de rangs `rangs` dans l'ecran de `mois`, d'un seul calcul du mois : chacune
/// `{"paie", "pas", "empreinte"}`, dans l'ordre de `rangs`. Le calcul d'un mois lit tous ses engagements (plafonds,
/// cumuls, conges) : il se fait une fois, et chaque paie en garde l'empreinte.
fn calculer_les_rangs(ctx: &Contexte, mois: &str, rangs: &[usize]) -> Resultat<Vec<Value>> {
    let data = stock::vue_du_mois(&ctx.racine, mois)?.ok_or_else(|| format!("{} : aucune saisie", mois))?;
    let (calculees, entrees) = empreinte::suivre(ctx, &data, || {
        let s = crate::charger_donnees(ctx, &data)?;
        rangs
            .iter()
            .map(|i| {
                let p = s.paies.get(*i).ok_or("le calcul du mois n'a pas cette paie")?;
                Ok((relire::ecrire(ctx, p)?, prelevement(ctx, p)?, p.engagement.fin_periode()?))
            })
            .collect::<Resultat<Vec<_>>>()
    })?;
    calculees
        .into_iter()
        .map(|(paie, pas, fin)| {
            let regles = empreinte::regles(ctx, &fin)?;
            let mut v = json!({ "paie": paie, "empreinte": empreinte::en_json(&entrees, &regles) });
            if let Some(pas) = pas {
                v["pas"] = pas;
            }
            Ok(v)
        })
        .collect()
}

/// Les chiffres d'une paie ecrite : tout sauf ce qu'elle dit en mots.
fn chiffres(v: &Value) -> Value {
    let mut x = json!({ "paie": v.get("paie").cloned().unwrap_or(Value::Null), "pas": v.get("pas").cloned().unwrap_or(Value::Null) });
    fn sans_avis(v: &mut Value) {
        if let Some(m) = v.as_object_mut() {
            m.remove("avertissements");
            for (_, x) in m.iter_mut() {
                sans_avis(x);
            }
        } else if let Some(a) = v.as_array_mut() {
            a.iter_mut().for_each(sans_avis);
        }
    }
    sans_avis(&mut x["paie"]);
    x
}

/// Pourquoi la paie a change de `avant` a `apres` : son prelevement seul, une regle datee nouvelle, le code qui calcule
/// (le moteur ou ce qu'il livre), la saisie d'un autre engagement, ou la configuration de la compagnie.
fn raison(avant: &Value, apres: &Value) -> String {
    let (a, b) = (chiffres(avant), chiffres(apres));
    if a["paie"] == b["paie"] && a["pas"] != b["pas"] {
        return TAUX_DE_PAS.to_string();
    }
    let vieilles = empreinte::regles_de(avant.get("empreinte").unwrap_or(&Value::Null));
    let nouvelles: Vec<String> = empreinte::regles_de(apres.get("empreinte").unwrap_or(&Value::Null)).into_iter().filter(|r| !vieilles.contains(r)).collect();
    if let Some(r) = nouvelles.iter().max() {
        let jour = Jour::lire(r).map(|j| j.jj_mm_aaaa()).unwrap_or_else(|_| r.clone());
        return format!("règle du {}", jour);
    }
    let entrees = |v: &Value| v.get("empreinte").and_then(empreinte::entrees_de).unwrap_or_default();
    let (ea, eb) = (entrees(avant), entrees(apres));
    let changees: Vec<&String> = ea.keys().chain(eb.keys()).filter(|k| ea.get(*k) != eb.get(*k)).collect();
    if changees.iter().any(|k| *k == "moteur" || k.split_once(' ').is_some_and(|(_, c)| c.starts_with("code:"))) {
        return CALCUL_CORRIGE.to_string();
    }
    // La saisie d'un mois : `<le mois>:<ses engagements>` (`stock::empreinte_de_saisie`).
    let saisies: Vec<&String> = changees.iter().copied().filter(|k| k.starts_with("saisie ")).collect();
    let part = |e: &BTreeMap<String, String>, k: &str, i: usize| e.get(k).and_then(|v| v.split(':').nth(i).map(String::from));
    if saisies.iter().any(|k| part(&ea, k, 1) != part(&eb, k, 1)) {
        return AUTRE_ENGAGEMENT.to_string();
    }
    if !saisies.is_empty() {
        return SAISIE_DU_MOIS.to_string();
    }
    CONFIGURATION.to_string()
}

/// Le moment d'un geste, a l'heure du moteur.
fn maintenant() -> String {
    socle::paris::iso_secondes(socle::temps::secondes())
}

/// « VALIDER L'ENGAGEMENT » : l'engagement de rang `rang` dans l'ecran de `mois` ecrit ses paies, une par mois qu'il
/// couvre (jusqu'au mois de l'ecran pour un contrat sans terme), chacune avec son empreinte. La paie d'un mois dont les
/// documents sont partis, gardee quand on l'a modifie (`figees`), se reprend telle quelle si la saisie de son mois n'a
/// pas change. Refus : un mois qui ne se calcule pas ; rien n'est ecrit.
pub fn valider(ctx: &Contexte, mois: &str, rang: usize) -> Resultat<Valide> {
    let mut o = stock::objet_du_mois(&ctx.racine, mois, rang)?.ok_or_else(|| format!("{} : pas d'engagement de rang {}", mois, rang + 1))?;
    let figees = o.brut.get("figees").and_then(Value::as_object).cloned().unwrap_or_default();
    let mut paies = Map::new();
    for m in o.mois_couverts(mois)? {
        let garde = figees.get(&m).filter(|f| {
            let saisie = stock::vue_du_mois(&ctx.racine, &m).ok().flatten().map(|v| stock::empreinte_de_saisie(&v));
            let ecrite = f.get("empreinte").and_then(empreinte::entrees_de).and_then(|e| e.get(&stock::cle_de_saisie(&m)).cloned());
            saisie.is_some() && saisie == ecrite
        });
        let p = match garde {
            Some(f) => f.clone(),
            None => calculer(ctx, &o, &m)?,
        };
        paies.insert(m, p);
    }
    o.brut.remove("figees");
    o.brut.insert("valide".into(), json!({ "le": maintenant(), "paies": paies }));
    stock::reecrire(&ctx.racine, &o)?;
    Valide::de(o).ok_or_else(|| "validation illisible".to_string())
}

/// Les mois de l'engagement `o` dont les documents sont partis, tous ses mois compris.
fn mois_partis(ctx: &Contexte, o: &Objet, journal: &Journal) -> Vec<String> {
    let Some(cle) = cle_du_salarie(ctx, o) else { return Vec::new() };
    let paies = o.brut.get("valide").and_then(|x| x.get("paies")).and_then(Value::as_object).cloned().unwrap_or_default();
    paies.keys().filter(|m| journal.documents_partis(m, &cle).is_some()).cloned().collect()
}

/// LA QUESTION AVANT DE MODIFIER : si les documents d'un des mois de l'engagement sont partis, la phrase que le moteur
/// pose (`QUESTION_DOCUMENTS_PARTIS`) ; sinon `None`.
pub fn question_avant_de_modifier(ctx: &Contexte, mois: &str, rang: usize) -> Resultat<Option<String>> {
    let Some(o) = stock::objet_du_mois(&ctx.racine, mois, rang)? else { return Ok(None) };
    let journal = Journal::lire(&ctx.racine)?;
    Ok((!mois_partis(ctx, &o, &journal).is_empty()).then(|| QUESTION_DOCUMENTS_PARTIS.to_string()))
}

/// « MODIFIER… » : l'engagement repasse en saisie. Les paies des mois dont les documents sont partis se gardent
/// (`figees`) pour la validation suivante.
pub fn modifier(ctx: &Contexte, mois: &str, rang: usize) -> Resultat<EnSaisie> {
    let mut o = stock::objet_du_mois(&ctx.racine, mois, rang)?.ok_or_else(|| format!("{} : pas d'engagement de rang {}", mois, rang + 1))?;
    let journal = Journal::lire(&ctx.racine)?;
    let cle = cle_du_salarie(ctx, &o);
    if let Some(Value::Object(v)) = o.brut.remove("valide") {
        let mut figees = o.brut.get("figees").and_then(Value::as_object).cloned().unwrap_or_default();
        for (m, p) in v.get("paies").and_then(Value::as_object).into_iter().flatten() {
            if cle.as_deref().is_some_and(|c| journal.documents_partis(m, c).is_some()) {
                figees.insert(m.clone(), p.clone());
            }
        }
        if !figees.is_empty() {
            o.brut.insert("figees".into(), Value::Object(figees));
        }
        stock::reecrire(&ctx.racine, &o)?;
    }
    Ok(EnSaisie { objet: o })
}

/// LE RECALCUL DE LUI-MEME, a la lecture de `mois` (`du_mois`) : chaque engagement valide du mois dont les documents ne sont pas
/// partis et dont la paie de ce mois n'est plus juste (son empreinte ne tient plus, ou elle manque : un contrat sans
/// terme qui entre dans un nouveau mois) se recalcule et se reecrit. Un chiffre qui change note pourquoi, et quand ; rien
/// d'autre ne change.
fn rafraichir(ctx: &Contexte, mois: &str) -> Resultat<()> {
    // Ce qui ne tient plus : le rang dans l'ecran, l'engagement, sa paie ecrite du mois.
    let mut a_refaire: Vec<(usize, Objet, Option<Value>)> = Vec::new();
    let mut relues = empreinte::Relues::default();
    for (i, e) in ecrits(ctx, mois)?.into_iter().enumerate() {
        // Des documents partis ne bougent pas ; une paie qui manque (un contrat sans terme qui entre dans le mois) s'ecrit.
        let v = match e {
            Etat::Valide(v) => v,
            Etat::Partis(p) if p.valide().paie_ecrite(mois).is_none() => p.valide().clone(),
            _ => continue,
        };
        let ancienne = v.paie_ecrite(mois).cloned();
        let tient = ancienne
            .as_ref()
            .and_then(|a| a.get("empreinte"))
            .and_then(empreinte::entrees_de)
            .is_some_and(|en| empreinte::tient(ctx, &en, &mut relues));
        if !tient {
            a_refaire.push((i, v.objet, ancienne));
        }
    }
    if a_refaire.is_empty() {
        return Ok(());
    }
    let rangs: Vec<usize> = a_refaire.iter().map(|(i, _, _)| *i).collect();
    let nouvelles = calculer_les_rangs(ctx, mois, &rangs)?;
    for ((_, mut o, ancienne), mut nouvelle) in a_refaire.into_iter().zip(nouvelles) {
        let raison = match &ancienne {
            Some(a) if chiffres(a) != chiffres(&nouvelle) => Some(raison(a, &nouvelle)),
            Some(a) => {
                if let Some(r) = a.get("recalcul").filter(|r| r.is_object()) {
                    nouvelle["recalcul"] = r.clone();
                }
                None
            }
            None => None,
        };
        if let Some(r) = &raison {
            nouvelle["recalcul"] = json!({ "le": maintenant(), "raison": r });
        }
        if let Some(Value::Object(paies)) = o.brut.get_mut("valide").and_then(|x| x.get_mut("paies")) {
            paies.insert(mois.to_string(), nouvelle);
        }
        stock::reecrire(&ctx.racine, &o)?;
    }
    Ok(())
}

/// La phrase d'un recalcul, pour l'ecran : « Recalculé le JJ/MM/AAAA : taux de PAS reçu. »
pub fn phrase_du_recalcul(r: &Value) -> Option<String> {
    let raison = r.get("raison").and_then(Value::as_str)?;
    let le = r.get("le").map(en_texte).unwrap_or_default();
    let jour = Jour::lire(le.get(..10).unwrap_or("")).map(|j| j.jj_mm_aaaa()).unwrap_or(le);
    Some(format!("Recalculé le {} : {}.", jour, raison))
}

/// La paie de l'engagement de rang `i` dans la saisie `data` est-elle validee ? (Une saisie de l'ecran, pas encore
/// ecrite, n'a pas de rang sur le disque : on la compare a ce qui est ecrit.)
pub fn est_validee(v: &Value) -> bool {
    v.get("valide").is_some_and(vrai)
}
