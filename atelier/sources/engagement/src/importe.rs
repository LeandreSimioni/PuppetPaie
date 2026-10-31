//! L'ENGAGEMENT IMPORTE : un engagement que l'import a tire des pieces d'un autre logiciel (les DSN deposees, les AEM,
//! les bulletins de paie). Il vit avec les autres, dans `engagements/AAAA-MM.json`, le fichier du mois ou il commence,
//! et porte `importe` :
//!
//! ```text
//! "importe": {"le", "contrat"?: {ce que l'AEM ou le bulletin atteste}, "paies": {"AAAA-MM": {ce qui a ete verse}}}
//! ```
//!
//! Il est valide et ses documents sont partis : ce qui en est parti, c'est l'import lui-meme (les pieces de l'autre
//! logiciel). IL SE LIT, IL NE SE MODIFIE PAS ET NE SE RECALCULE JAMAIS : il ne touche aucun mois de la saisie
//! (`stock::Objet::couvre`), donc aucun calcul, aucune declaration ni aucun document ne le prend ; l'ecran ne peut ni le
//! reecrire ni en poser un double (`stock::ecrire_la_carte`). Son numero de contrat est celui que la DSN deposee lui
//! a donne (S21.G00.40.009), jamais un numero tire : il compte parmi les numeros pris (`numeros`).
//!
//! Ce qui le lit : les cumuls des annexes 8 et 10 (`cumul`, par ce que le contrat atteste), le mois tenu avant
//! PuppetPaie (`contrats_lus`, `paies_lues` : une ligne par contrat atteste, une ligne par paie versee).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::stock::{self, Objet};
use socle::date::en_texte;
use socle::fichiers::vrai;
use socle::num::Resultat;

/// Dans un engagement importe : ce que l'import a lu de lui.
pub const IMPORTE: &str = "importe";

/// Un engagement importe.
#[derive(Debug, Clone)]
pub struct Importe {
    objet: Objet,
}

impl Importe {
    /// L'engagement `o` s'il est importe.
    pub fn de(o: Objet) -> Option<Importe> {
        o.est_importe().then_some(Importe { objet: o })
    }

    /// L'engagement tel qu'il est ecrit.
    pub fn objet(&self) -> &Objet {
        &self.objet
    }

    fn lu(&self) -> &Map<String, Value> {
        static VIDE: std::sync::OnceLock<Map<String, Value>> = std::sync::OnceLock::new();
        self.objet.brut.get(IMPORTE).and_then(Value::as_object).unwrap_or_else(|| VIDE.get_or_init(Map::new))
    }

    fn texte(&self, k: &str) -> String {
        self.objet.brut.get(k).filter(|x| vrai(x)).map(en_texte).unwrap_or_default()
    }

    /// Quand il a ete importe.
    pub fn le(&self) -> String {
        self.lu().get("le").map(en_texte).unwrap_or_default()
    }

    /// Ce que l'AEM ou le bulletin atteste du contrat (heures, cachets, jours, brut, numero d'objet...), s'il l'est.
    pub fn contrat(&self) -> Option<&Map<String, Value>> {
        self.lu().get("contrat").and_then(Value::as_object)
    }

    /// Ce qui a ete verse, mois par mois.
    pub fn paies(&self) -> BTreeMap<String, &Map<String, Value>> {
        self.lu().get("paies").and_then(Value::as_object).into_iter().flatten().filter_map(|(m, p)| Some((m.clone(), p.as_object()?))).collect()
    }

    /// LE CONTRAT D'USAGE QUI CONTINUE ICI : le dernier mois verse par l'ancien logiciel, quand le contrat que la DSN
    /// deposee declare finit apres lui (S21.G00.40.010, sa fin previsionnelle : `norme-dsn/texte/bloc-S21.G00.40.txt`).
    /// Les mois qui suivent se paient ici, sous le numero et les dates que cette DSN lui a donnes (regle 4). Un contrat
    /// qu'une AEM atteste est fini (l'AEM se remet a la fin du contrat) ; un contrat paye au mois a sa propre reprise,
    /// sa nature dite (`import::nature`) ; sans numero, rien ne se reprend.
    pub fn continue_apres(&self) -> Option<String> {
        let dernier = self.paies().into_keys().max()?;
        let fin = self.texte("fin");
        let contrat = self.contrat();
        let lu = |k: &str| contrat.and_then(|c| c.get(k)).filter(|x| vrai(x)).map(en_texte).unwrap_or_default();
        let paye_au_mois = droit::natures::nature(Some(&lu("nature"))).is_ok_and(|n| n.mensuelle()) || vrai(contrat.and_then(|c| c.get("nature_a_dire")).unwrap_or(&Value::Null));
        let continue_ = fin.len() >= 7 && &fin[..7] > dernier.as_str() && lu("atteste_par") != "AEM" && !paye_au_mois && !self.texte("numero").is_empty();
        continue_.then_some(dernier)
    }

    /// Les mois qu'il touche : de son debut a sa fin (au dernier mois verse, s'il continue ici : `continue_apres`), et
    /// ceux ou il a ete verse. Un contrat sans fin (un CDI) ne touche que les mois ou il a ete verse : des mois d'avant
    /// le premier bulletin repris, les pieces ne disent rien.
    pub fn mois(&self) -> Vec<String> {
        let (debut, fin) = (self.texte("debut"), self.texte("fin"));
        let mut v: Vec<String> = self.paies().into_keys().collect();
        if debut.len() >= 7 {
            let fin = match self.continue_apres() {
                Some(dernier) => format!("{}-01", dernier),
                None if fin.len() >= 7 => fin,
                None if !v.is_empty() => {
                    v.sort();
                    return v;
                }
                None => debut.clone(),
            };
            let mut m = debut[..7].to_string();
            while m.as_str() <= &fin[..7] {
                v.push(m.clone());
                match stock::mois_apres(&m) {
                    Ok(n) => m = n,
                    Err(_) => break,
                }
            }
        }
        v.sort();
        v.dedup();
        v
    }

    /// Le contrat atteste, en une ligne : `{"salarie", "debut", "fin", <ce qu'il atteste>}`.
    pub fn ligne_du_contrat(&self) -> Option<Value> {
        let c = self.contrat()?;
        let mut l = self.bornes();
        l.extend(c.iter().map(|(k, v)| (k.clone(), v.clone())));
        Some(Value::Object(l))
    }

    /// Les paies versees, une ligne par mois : `{"mois", "salarie", "debut", "fin", "contrat"?, <ce qui a ete verse>}`.
    pub fn lignes_des_paies(&self) -> Vec<Value> {
        self.paies()
            .into_iter()
            .map(|(m, p)| {
                let mut l = self.bornes();
                l.insert("mois".into(), Value::String(m));
                l.extend(p.iter().map(|(k, v)| (k.clone(), v.clone())));
                Value::Object(l)
            })
            .collect()
    }

    fn bornes(&self) -> Map<String, Value> {
        let mut l = Map::new();
        for k in ["salarie", "debut", "fin"] {
            l.insert(k.into(), Value::String(self.texte(k)));
        }
        if let Some(n) = self.objet.brut.get("numero").filter(|n| vrai(n)) {
            l.insert("contrat".into(), n.clone());
        }
        l.insert("importe_le".into(), Value::String(self.le()));
        l
    }
}

/// Tous les engagements importes, fichier par fichier.
pub fn tous(racine: &Path) -> Resultat<Vec<Importe>> {
    Ok(stock::objets(racine)?.into_iter().filter_map(Importe::de).collect())
}

/// Les engagements importes qui touchent `mois`.
pub fn du_mois(racine: &Path, mois: &str) -> Resultat<Vec<Importe>> {
    Ok(tous(racine)?.into_iter().filter(|i| i.mois().iter().any(|m| m == mois)).collect())
}

/// Les contrats attestes de tous les engagements importes, une ligne chacun (`Importe::ligne_du_contrat`).
pub fn contrats_lus(racine: &Path) -> Resultat<Vec<Value>> {
    Ok(tous(racine)?.iter().filter_map(Importe::ligne_du_contrat).collect())
}

/// Les paies versees de tous les engagements importes, une ligne par mois (`Importe::lignes_des_paies`).
pub fn paies_lues(racine: &Path) -> Resultat<Vec<Value>> {
    Ok(tous(racine)?.iter().flat_map(Importe::lignes_des_paies).collect())
}

/// LA DSN DE `mois` QUE L'ANCIEN LOGICIEL A DEPOSEE : une paie importee de ce mois qu'une DSN deposee declare
/// (`declare_par: "DSN"`), en une ligne (`Importe::lignes_des_paies` : `depose_le`, `fichier`...), qu'une saisie touche
/// aussi le mois ou non. Cette DSN n'est pas faite ici et ne se refait pas ici (regle 4) : ce que la saisie ajoute a
/// ce mois se declare dans une DSN suivante, comme un contrat ajoute apres la DSN de son mois.
pub fn dsn_deposee(racine: &Path, mois: &str) -> Resultat<Option<Value>> {
    Ok(du_mois(racine, mois)?
        .iter()
        .flat_map(Importe::lignes_des_paies)
        .find(|p| p.get("mois").map(en_texte).as_deref() == Some(mois) && p.get("declare_par").map(en_texte).as_deref() == Some("DSN")))
}

/// L'engagement importe du salarie `cle` (la cle de sa fiche) qui commence le `debut` et touche `mois`, s'il y en a un.
pub fn celui_de(racine: &Path, mois: &str, cle: &str, debut: &str) -> Resultat<Option<Importe>> {
    Ok(du_mois(racine, mois)?.into_iter().find(|i| i.texte("salarie") == cle && i.texte("debut") == debut))
}

/// Les contrats d'usage importes qui continuent en `mois` (`Importe::continue_apres`) : verses avant lui par l'ancien
/// logiciel, ils finissent en `mois` ou apres.
pub fn qui_continuent(racine: &Path, mois: &str) -> Resultat<Vec<Importe>> {
    Ok(tous(racine)?
        .into_iter()
        .filter(|i| i.continue_apres().is_some_and(|d| d.as_str() < mois) && i.texte("fin").get(..7).is_some_and(|f| f >= mois))
        .collect())
}

/// Le contrat importe que l'engagement `e` de la saisie continue : meme salarie, meme debut, meme numero, et il continue
/// apres son dernier mois verse (`continue_apres`).
pub fn continue_par(racine: &Path, e: &Map<String, Value>) -> Resultat<Option<Importe>> {
    let t = |k: &str| e.get(k).filter(|x| vrai(x)).map(en_texte).unwrap_or_default();
    let (salarie, debut, numero) = (t("salarie"), t("debut"), t("numero"));
    if salarie.is_empty() || debut.is_empty() || numero.is_empty() {
        return Ok(None);
    }
    Ok(tous(racine)?.into_iter().find(|i| {
        i.continue_apres().is_some() && i.texte("salarie") == salarie && i.texte("debut") == debut && i.texte("numero") == numero
    }))
}

/// LE CONTRAT `numero` DE `salarie` COMMENCE LE `debut` VIENT-IL DE L'ANCIEN LOGICIEL ? Un engagement importe l'atteste
/// (un permanent : `Importe::atteste`), ou il continue un contrat d'usage importe (`continue_par`). L'ecran le dit sur
/// la carte. Faux quand rien ne se lit.
pub fn vient_d_un_ancien_logiciel(racine: &Path, salarie: &str, debut: &str, numero: &str) -> bool {
    let tous = tous(racine).unwrap_or_default();
    tous.iter().any(|i| i.atteste(salarie, debut, numero) || (i.continue_apres().is_some() && i.texte("salarie") == salarie && i.texte("debut") == debut && i.texte("numero") == numero))
}

impl Importe {
    /// ATTESTE-T-IL LE CONTRAT `numero` DE `salarie` COMMENCE LE `debut` ? Le contrat d'un permanent qui court encore
    /// porte le debut et le numero que la DSN de l'ancien logiciel lui a donnes (`contrat_debut`, `numero` :
    /// `import::nature`) ; chacun de ses bulletins repris l'atteste, et l'engagement qui le continue ici est le meme
    /// contrat (S21.G00.40.001 et .009, `norme-dsn/texte/bloc-S21.G00.40.txt`), pas une copie.
    pub fn atteste(&self, salarie: &str, debut: &str, numero: &str) -> bool {
        let du_contrat = self.contrat().and_then(|c| c.get("contrat_debut")).filter(|x| vrai(x)).map(en_texte);
        !numero.is_empty()
            && self.texte("salarie") == salarie
            && self.texte("numero") == numero
            && du_contrat.unwrap_or_else(|| self.texte("debut")) == debut
    }

    /// LES JOURS DU CONTRAT ENTIER (S21.G00.40.013 d'un contrat d'usage : `Engagement::quotite`) d'un contrat qui continue
    /// ici, `propres` etant ceux que la saisie lui donne : les jours que les bulletins importes impriment, plus les siens ;
    /// a defaut d'un bulletin pour chaque mois verse, la quotite que la DSN deposee a declaree pour ce contrat (elle le
    /// dit pour le contrat entier, `norme-dsn/texte/bloc-S21.G00.40.txt`). `None` quand aucune piece ne les donne.
    pub fn jours_du_contrat_continue(&self, propres: usize) -> Option<i64> {
        let paies = self.paies();
        let jours = |p: &Map<String, Value>| -> Option<i64> {
            let dates = p.get("dates")?.as_array()?;
            let mut n = 0i64;
            for d in dates {
                let lire = |k: &str| socle::date::Jour::lire(&d.get(k).map(en_texte).unwrap_or_default()).ok();
                let (a, b) = (lire("debut")?, lire("fin")?);
                n += b.moins(&a) + 1;
            }
            (n > 0).then_some(n)
        };
        let lus: Option<Vec<i64>> = paies.values().map(|p| jours(p)).collect();
        if let Some(l) = lus.filter(|l| !l.is_empty()) {
            return Some(l.iter().sum::<i64>() + propres as i64);
        }
        let dernier = paies.into_values().last()?;
        dernier.get("quotite").map(en_texte).and_then(|q| q.trim().replace(',', ".").parse::<f64>().ok()).filter(|q| *q > 0.0 && q.fract() == 0.0).map(|q| q as i64)
    }
}

/// Le refus d'un geste sur un engagement importe : il se lit, il ne se modifie pas.
pub fn refus(qui: &str, debut: &str) -> String {
    let jour = socle::date::Jour::lire(debut).map(|j| j.jj_mm_aaaa()).unwrap_or_else(|_| debut.to_string());
    format!(
        "L'engagement de {} du {} est importé : il se lit tel que l'ancien logiciel l'a déclaré, il ne se modifie pas, ne se \
         recalcule pas et ne se supprime pas. Rien n'est écrit.",
        qui, jour
    )
}

/// ECRIT LES ENGAGEMENTS IMPORTES, tous : chacun dans le fichier du mois ou il commence (son `debut`, sinon le premier
/// mois ou il a ete verse), apres les engagements de la saisie. Ceux qui y etaient s'en vont ; rien d'autre ne change.
/// Rend les fichiers ecrits.
pub fn ecrire_tous(racine: &Path, importes: Vec<Map<String, Value>>) -> Resultat<Vec<PathBuf>> {
    let mut par_mois: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for e in importes {
        let debut = e.get("debut").map(en_texte).unwrap_or_default();
        let premier = e.get(IMPORTE).and_then(|i| i.get("paies")).and_then(Value::as_object).and_then(|p| p.keys().next().cloned());
        let mois = if debut.len() >= 7 { debut[..7].to_string() } else { premier.ok_or("un engagement importe sans debut ni paie")? };
        par_mois.entry(mois).or_default().push(Value::Object(e));
    }
    let mut touches: Vec<String> = tous(racine)?.iter().map(|i| i.objet.fichier.clone()).collect();
    touches.extend(par_mois.keys().cloned());
    touches.sort();
    touches.dedup();
    let mut ecrits = Vec::new();
    for f in touches {
        let existe = stock::lire_fichier(racine, &f)?;
        let neufs = par_mois.remove(&f).unwrap_or_default();
        let mut contenu = existe.clone().unwrap_or_default();
        contenu.insert("mois".into(), Value::String(f.clone()));
        let mut es: Vec<Value> =
            contenu.get("engagements").and_then(Value::as_array).into_iter().flatten().filter(|e| !e.get(IMPORTE).is_some_and(Value::is_object)).cloned().collect();
        es.extend(neufs);
        contenu.insert("engagements".into(), Value::Array(es));
        if existe.as_ref() == Some(&contenu) {
            continue;
        }
        ecrits.push(stock::poser(racine, &f, &Value::Object(contenu))?);
    }
    Ok(ecrits)
}

/// CE QUE L'IMPORT COMPLETE DE LUI-MEME : ce que les pieces ne disaient pas et que la personne dit apres l'import (la
/// nature d'un contrat de permanent, son numero). `completer` recoit chaque engagement importe tel qu'il est ecrit et
/// dit s'il l'a change ; ceux-la se reecrivent. Rend combien ont change.
pub fn completer(racine: &Path, mut completer: impl FnMut(&mut Map<String, Value>) -> bool) -> Resultat<usize> {
    let mut tous: Vec<Map<String, Value>> = tous(racine)?.into_iter().map(|i| i.objet.brut).collect();
    let mut changes = 0usize;
    for e in tous.iter_mut() {
        if completer(e) {
            changes += 1;
        }
    }
    if changes > 0 {
        ecrire_tous(racine, tous)?;
    }
    Ok(changes)
}
