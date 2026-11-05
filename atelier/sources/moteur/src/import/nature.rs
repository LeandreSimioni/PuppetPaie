//! LA NATURE DES CONTRATS QUE LES BULLETINS N'ONT PAS DITE. L'import d'un bulletin de paie atteste un contrat
//! (un engagement importe, `import::bulletins::engagement`) ; aucun gabarit n'imprime sa nature. Un bulletin
//! qui porte une marque du spectacle (numero d'objet, cotisation Congés Spectacles) laisse le contrat muet, et un
//! engagement muet est un CDDU (regle 6). Un bulletin qui n'en porte aucune est celui d'un permanent : CDI ou CDD de
//! droit commun, que rien ne dit. Son contrat est marque `nature_a_dire` ; ce module :
//!
//! - `a_dire` : les salaries dont un contrat repris attend sa nature, avec les natures qu'on peut lui donner
//!   (celles d'un permanent : `contrats-base.json`, paie « mensuelle ») et, pour chacune, ce qu'il faut dire avec elle
//!   (`demande`) -- `importation-etat` les rend, la page les demande apres l'import (Configuration, « Reprendre
//!   l'historique ») ;
//! - la route `importation-nature` `{"salarie", "nature", "debut"?, "fin"?, "numero"?}` : la nature dite s'ecrit sur
//!   l'engagement de chacun de ses contrats a dire, et la fiche Entreprise dit que la compagnie emploie des
//!   permanents. Une nature qui n'est pas d'un permanent, un salarie sans contrat a dire, ce que `demande` exige et qui
//!   manque : refus, rien n'est ecrit.
//!
//! LE CONTRAT QUI CONTINUE. Un permanent paye le dernier mois repris (le plus recent de l'historique) a un contrat qui
//! court encore, sauf s'il finissait ce mois-la. Le mois suivant le continue, et la DSN le declare sous le MEME numero
//! (S21.G00.40.009, « un des identifiants du contrat » ; CCH-12) et la meme date de debut (S21.G00.40.001) que
//! l'ancien logiciel : `atelier/reference/norme-dsn/texte/bloc-S21.G00.40.txt`. Aucun bulletin ne les imprime ; un
//! numero ne se recalcule pas, il se lit (regle 4) : sur la DSN deja deposee. Ils se demandent donc avec la nature
//! (`demande` : `debut`, `numero`, le salaire mensuel et la duree du travail du contrat, `fin` s'il a un terme), et
//! s'ecrivent sur l'engagement repris lui-meme (`debut`, `fin`, `numero`, `salaire_mensuel`, `quotite_contrat`) : il
//! continue ici, sous la meme identite (`engagement::importe`, « un contrat de permanent est un engagement »).

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use engagement::importe::Importe;
use socle::date::Jour;
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::vrai;

/// La marque d'un contrat repris dont la nature reste a dire.
pub const A_DIRE: &str = engagement::importe::A_DIRE;

/// Les natures qu'un permanent peut recevoir.
fn natures_permanentes() -> R<Vec<std::sync::Arc<droit::natures::Nature>>> {
    let toutes = droit::natures::natures().map_err(|m| erreur("ErreurOperation", m))?;
    Ok(toutes.iter().filter(|n| n.mensuelle()).cloned().collect())
}

/// Ce qu'une nature demande d'un contrat qui court encore : une case de la charge d'`importation-nature`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Demande {
    Debut,
    Fin,
    Numero,
    Salaire,
    Quotite,
}

/// Ce qu'une nature demande avec elle pour un contrat qui court encore : son debut, son numero, son salaire et
/// sa duree du travail, et sa fin s'il a un terme. Rien pour un contrat fini.
fn demande(n: &droit::natures::Nature, en_cours: bool) -> Vec<Demande> {
    use Demande::*;
    match (en_cours, n.duree_indeterminee) {
        (false, _) => vec![],
        (true, true) => vec![Debut, Numero, Salaire, Quotite],
        (true, false) => vec![Debut, Fin, Numero, Salaire, Quotite],
    }
}

fn texte(e: &Value, k: &str) -> String {
    e.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default()
}

/// Les engagements importes (vide s'ils ne se lisent pas).
fn importes(racine: &Path) -> Vec<Importe> {
    engagement::importe::tous(racine).unwrap_or_default()
}

/// Le dernier mois repris : le plus recent que l'historique a verse (`AAAA-MM`).
fn dernier_mois(importes: &[Importe]) -> String {
    importes.iter().flat_map(|i| i.paies().keys().cloned()).max().unwrap_or_default()
}

/// Le premier et le dernier jour que les paies d'un engagement importe paient : ceux du bulletin dans son mois, sinon le
/// mois entier.
fn jours_payes(i: &Importe) -> (String, String) {
    let mut bornes = Vec::new();
    for (m, p) in i.paies() {
        let lu = |j: Option<Jour>| j.map(|j| j.iso()).filter(|j| j.starts_with(m.as_str()));
        let fin = socle::mois::Mois::lire(m).map(socle::mois::Mois::dernier_jour).map(|j| j.iso()).unwrap_or_default();
        bornes.push((lu(p.debut).unwrap_or_else(|| format!("{}-01", m)), lu(p.fin).unwrap_or(fin)));
    }
    (bornes.iter().map(|b| b.0.clone()).min().unwrap_or_default(), bornes.iter().map(|b| b.1.clone()).max().unwrap_or_default())
}

/// Le nom d'un salarié à l'écran, tel que sa fiche le porte aujourd'hui ; sa clé à défaut (pas de fiche, ou un fichier
/// des salariés qui ne se lit pas : ce que l'import dit n'en dépend pas).
fn nom_du_salarie(racine: &Path, cle: &str) -> String {
    let fiches = crate::routes::lire_json_ou_vide(&racine.join(droit::contexte::SALARIES)).unwrap_or(Value::Null);
    let lues = droit::fiche::lire_salaries(&fiches, droit::contexte::SALARIES).ok();
    lues.and_then(|s| s.fiches.get(cle).and_then(|f| f.au(&socle::temps::jour_de_paris()).nomme())).unwrap_or_else(|| cle.to_string())
}

/// Les contrats a dire de chaque salarie : (nombre, premier jour paye, dernier jour paye, paye le dernier mois repris).
fn a_dire_par_salarie(importes: &[Importe]) -> BTreeMap<String, (usize, String, String, bool)> {
    let dernier = dernier_mois(importes);
    let mut par: BTreeMap<String, (usize, String, String, bool)> = BTreeMap::new();
    for i in importes.iter().filter(|i| i.a_dire()) {
        let salarie = texte(&i.objet().brut, "salarie");
        let (du, au) = jours_payes(i);
        let x = par.entry(salarie).or_insert((0, du.clone(), au.clone(), false));
        x.0 += 1;
        if du < x.1 {
            x.1 = du;
        }
        if au > x.2 {
            x.2 = au;
        }
        x.3 |= i.paies().contains_key(&dernier);
    }
    par
}

/// Les salaries dont des contrats repris attendent leur nature : `[{salarie, nom, contrats, du, au, en_cours, suite,
/// natures: [{cle, libelle, demande}]}]`, dans l'ordre des cles. `en_cours` : il est paye le dernier mois repris, son
/// contrat court peut-etre encore (`suite` : le mois qui suit). Ne leve jamais : un historique illisible n'a rien a dire.
pub(crate) fn a_dire(racine: &Path) -> Vec<ADire> {
    let importes = importes(racine);
    let par = a_dire_par_salarie(&importes);
    if par.is_empty() {
        return Vec::new();
    }
    let suite = socle::mois::Mois::lire(&dernier_mois(&importes)).map(|m| m.suivant().to_string()).unwrap_or_default();
    let natures = natures_permanentes().unwrap_or_default();
    let jour = |iso: &str| socle::date::Jour::lire(iso).map(|j| j.jj_mm_aaaa()).unwrap_or_else(|_| iso.to_string());
    par.into_iter()
        .map(|(salarie, (contrats, du, au, en_cours))| {
            let natures = natures.iter().map(|x| NatureProposee { cle: x.cle.clone(), libelle: x.libelle.clone(), demande: demande(x, en_cours) }).collect();
            // Le mois de la derniere DSN qui l'a declare, tel que l'ecran le lit (« de novembre 2026 »).
            let au_dit = crate::regularisation::de_mois_en_clair(au.get(..7).unwrap_or(&au));
            let nom = nom_du_salarie(racine, &salarie);
            let periode = if du.is_empty() { String::new() } else { format!(", du {} au {}", jour(&du), jour(&au)) };
            let dit = format!(
                "{} contrat{} repris{} : ses bulletins n'ont pas de Congés Spectacles ni de numéro d'objet, c'est un contrat de permanent.",
                contrats,
                if contrats > 1 { "s" } else { "" },
                periode
            );
            let en_cours_dit = en_cours.then(|| {
                format!(
                    "Payé jusqu'en {} : son contrat continue en {}, et sa DSN le déclare avec la date de début et le numéro que votre ancien logiciel lui a donnés. Ils sont sur sa dernière DSN (bloc Contrat), pas sur le bulletin.",
                    socle::mois::Mois::en_clair(au.get(..7).unwrap_or(&au)),
                    socle::mois::Mois::en_clair(&suite)
                )
            });
            ADire {
                numero_placeholder: format!("sur la DSN {}", au_dit),
                choix_dit: format!("Nature du contrat de {}", nom),
                salarie,
                nom,
                contrats,
                du,
                au,
                au_dit,
                en_cours,
                suite: en_cours.then(|| suite.clone()),
                natures,
                dit,
                en_cours_dit,
            }
        })
        .collect()
}

/// Les contrats d'un salarie dont la nature reste a dire, et ce que la page en dit.
#[derive(Debug, serde::Serialize)]
pub(crate) struct ADire {
    pub(crate) salarie: String,
    pub(crate) nom: String,
    pub(crate) contrats: usize,
    pub(crate) du: String,
    pub(crate) au: String,
    pub(crate) au_dit: String,
    /// Paye le dernier mois repris : son contrat court peut-etre encore (`suite` : le mois qui suit).
    pub(crate) en_cours: bool,
    pub(crate) suite: Option<String>,
    pub(crate) natures: Vec<NatureProposee>,
    pub(crate) numero_placeholder: String,
    pub(crate) choix_dit: String,
    pub(crate) dit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) en_cours_dit: Option<String>,
}

/// Une nature de permanent qui peut se dire, et ce qu'elle demande d'un contrat qui court encore.
#[derive(Debug, serde::Serialize)]
pub(crate) struct NatureProposee {
    pub(crate) cle: String,
    pub(crate) libelle: String,
    pub(crate) demande: Vec<Demande>,
}

fn refus(m: impl Into<String>) -> socle::echec::Echec {
    erreur("ErreurOperation", m.into())
}

/// La charge d'`importation-nature` : le salarie, la nature, et ce que `demande` exige d'un contrat qui court encore.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeNature {
    #[serde(default)]
    pub(crate) salarie: String,
    #[serde(default)]
    pub(crate) nature: String,
    #[serde(default)]
    pub(crate) debut: Option<String>,
    #[serde(default)]
    pub(crate) fin: Option<String>,
    #[serde(default)]
    pub(crate) numero: Option<String>,
    #[serde(default, deserialize_with = "saisie")]
    pub(crate) salaire: Option<String>,
    #[serde(default, deserialize_with = "saisie")]
    pub(crate) quotite: Option<String>,
}

/// Un montant tel qu'il se saisit : un texte (« 1 850,00 ») ou un nombre.
fn saisie<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum Saisie {
        Texte(String),
        Nombre(serde_json::Number),
    }
    Ok(<Option<Saisie> as serde::Deserialize>::deserialize(d)?.map(|s| match s {
        Saisie::Texte(t) => t,
        Saisie::Nombre(n) => n.to_string(),
    }))
}

impl ChargeNature {
    /// Une case demandee, telle qu'elle est dite, sans ses blancs ; rien pour une case vide.
    fn lu(&self, champ: Demande) -> Option<String> {
        let v = match champ {
            Demande::Debut => &self.debut,
            Demande::Fin => &self.fin,
            Demande::Numero => &self.numero,
            Demande::Salaire => &self.salaire,
            Demande::Quotite => &self.quotite,
        };
        v.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
    }

    /// La case demandee, posee.
    fn poser(&mut self, champ: Demande, v: String) {
        let c = match champ {
            Demande::Debut => &mut self.debut,
            Demande::Fin => &mut self.fin,
            Demande::Numero => &mut self.numero,
            Demande::Salaire => &mut self.salaire,
            Demande::Quotite => &mut self.quotite,
        };
        *c = Some(v);
    }
}

/// Ce qu'`importation-nature` rend : les contrats ecrits, ce qui reste a dire, et ce que l'ecran en dit.
#[derive(Debug, serde::Serialize)]
pub(crate) struct NatureDite {
    pub(crate) phrase: String,
    pub(crate) ecrits: usize,
    pub(crate) a_dire: Vec<ADire>,
    /// La fiche Entreprise vient de dire que la compagnie emploie des permanents.
    pub(crate) permanents_declares: bool,
    /// Les contrats de prevoyance et de sante des permanents que les DSN reprises declarent, poses.
    pub(crate) parametrage: Vec<String>,
    pub(crate) couleur: &'static str,
}

/// `importation-nature` : la nature dite pour les contrats a dire d'un salarie, et, s'il court encore, le debut, la fin
/// et le numero du contrat tels que la DSN de l'ancien logiciel les a declares.
pub(crate) fn dire(ctx: &Contexte, charge: &ChargeNature) -> R<NatureDite> {
    let plein = |s: &str| Some(s.trim().to_string()).filter(|s| !s.is_empty());
    let salarie = plein(&charge.salarie).ok_or_else(|| refus("Le salarié manque : rien n'a été écrit."))?;
    let nature = plein(&charge.nature).ok_or_else(|| refus("La nature du contrat manque : rien n'a été écrit."))?;
    let permises = natures_permanentes()?;
    let Some(nat) = permises.iter().find(|n| n.cle == nature) else {
        let noms: Vec<&str> = permises.iter().map(|n| n.libelle.as_str()).collect();
        return Err(refus(format!("« {} » n'est pas la nature d'un contrat de permanent ({}) : rien n'a été écrit.", nature, noms.join(", "))));
    };
    let importes = importes(&ctx.racine);
    let qui = nom_du_salarie(&ctx.racine, &salarie);
    let Some((_, du, au, en_cours)) = a_dire_par_salarie(&importes).remove(&salarie) else {
        return Err(refus(format!("Aucun contrat repris de {} n'attend sa nature : rien n'a été écrit.", qui)));
    };
    let dernier = dernier_mois(&importes);
    // Le contrat qui court encore : son debut, sa fin s'il a un terme, son numero -- lus sur la DSN deja deposee ; ce
    // qui s'en ecrit sur l'engagement repris.
    let mut contrat = Map::new();
    let mut debut_dit: Option<String> = None;
    for champ in demande(nat, en_cours) {
        let v = charge.lu(champ).ok_or_else(|| {
            refus(match champ {
                Demande::Debut => format!("La date de début du contrat de {} manque (celle que la DSN déclare) : rien n'a été écrit.", qui),
                Demande::Fin => format!("La date de fin prévue du contrat de {} manque : rien n'a été écrit.", qui),
                Demande::Salaire => format!("Le salaire mensuel brut du contrat de {} manque (celui du contrat signé) : rien n'a été écrit.", qui),
                Demande::Quotite => format!(
                    "La durée du travail du contrat de {} manque (en heures par mois, celle du contrat signé) : rien n'a été écrit.",
                    qui
                ),
                Demande::Numero => format!("Le numéro du contrat de {} manque (celui que la DSN déclare) : rien n'a été écrit.", qui),
            })
        })?;
        match champ {
            Demande::Debut | Demande::Fin => {
                let j = Jour::lire(&v).map_err(|m| refus(format!("{} : rien n'a été écrit.", m)))?.iso();
                if champ == Demande::Debut && j > du {
                    return Err(refus(format!(
                        "Le contrat de {} ne peut pas commencer le {} : un bulletin le paie dès le {}. Rien n'a été écrit.",
                        qui, j, du
                    )));
                }
                if champ == Demande::Fin && j < au {
                    return Err(refus(format!("Le contrat de {} ne peut pas finir le {} : un bulletin le paie jusqu'au {}. Rien n'a été écrit.", qui, j, au)));
                }
                if champ == Demande::Debut {
                    debut_dit = Some(j.clone());
                }
                contrat.insert(if champ == Demande::Debut { "debut" } else { "fin" }.into(), json!(j));
            }
            Demande::Salaire | Demande::Quotite => {
                let s = v.replace([' ', '\u{a0}'], "").replace(',', ".");
                let (cle, quoi) = if champ == Demande::Salaire {
                    ("salaire_mensuel", "un salaire mensuel brut")
                } else {
                    ("quotite_contrat", "une durée du travail en heures par mois")
                };
                match s.parse::<socle::num::Decimal>() {
                    Ok(d) if d > socle::num::Decimal::ZERO => contrat.insert(cle.into(), json!(d.to_string().parse::<f64>().unwrap_or_default())),
                    _ => return Err(refus(format!("« {} » n'est pas {} : rien n'a été écrit.", v, quoi))),
                };
            }
            Demande::Numero => {
                let n = engagement::numeros::NumeroContrat::lire(&v).map_err(|m| refus(format!("{} Rien n'a été écrit.", m)))?;
                contrat.insert("numero".into(), n.into());
            }
        }
    }
    // La nature va sur l'engagement de chaque contrat a dire ; ce qui definit le contrat qui court encore, sur le sien.
    let ecrits = engagement::importe::completer(&ctx.racine, |e, pieces| {
        let a_dire = pieces.contrat.as_ref().is_some_and(|c| c.nature_a_dire);
        if e.get("salarie").and_then(Value::as_str) != Some(salarie.as_str()) || !a_dire || e.get("nature").is_some_and(vrai) {
            return false;
        }
        e.insert("nature".into(), json!(nature));
        if pieces.paies.contains_key(&dernier) {
            for (k, v) in &contrat {
                if k != "numero" || !e.get("numero").is_some_and(vrai) {
                    e.insert(k.clone(), v.clone());
                }
            }
        }
        true
    })
    .map_err(socle::echec::Echec::from)?;
    // Son anciennete dans l'entreprise part du debut de son contrat, s'il a commence avant le premier bulletin repris.
    if let Some(debut) = debut_dit.as_deref() {
        crate::routes::fiches::reprendre_anciennete(ctx, &salarie, &du, debut)?;
    }
    // LA COMPAGNIE EMPLOIE UN PERMANENT : c'est ce qu'on vient de dire (une nature payee au mois). La case « La
    // compagnie emploie aussi des salaries au mois » de la fiche Entreprise (`contexte::emploie_des_permanents`) se
    // coche donc ici, sans la redemander : sans elle, la paie du mois suivant refuserait ce contrat
    // (`engagement::nature_de`), et ce qu'elle ouvre (les contrats de prevoyance et de sante des permanents) se
    // demande alors dans « Pour demarrer », comme pour toute compagnie qui la coche.
    let permanents_declares = !droit::contexte::emploie_des_permanents(&ctx.entreprise);
    if permanents_declares {
        // A partir du premier jour que les bulletins repris paient.
        let depuis = Jour::lire(&du).map_err(|m| refus(format!("{} : la case des permanents n'a pas ete cochee.", m)))?;
        crate::routes::fiches::poser_entreprise(ctx, "emploie_des_permanents", json!(true), Some(depuis))?;
    }
    // Les contrats de prevoyance et de sante des permanents que l'ecran attend maintenant : ceux que les DSN reprises
    // declarent (S21.G00.15) se posent, lus sous la fiche telle qu'elle vient d'etre ecrite.
    let parametrage = match Contexte::nouveau(&ctx.racine, Some(ctx.date)) {
        Ok(frais) if permanents_declares => crate::import::parametrage::poser_organismes_repris(&frais),
        _ => Vec::new(),
    };
    // Ce que l'ecran dit du geste, et sa couleur : la nature est ecrite, c'est fait ; ce qui reste a dire pour un
    // permanent est une ligne de « Pour demarrer », qui a la sienne.
    let mut phrase = format!("Nature enregistrée sur {} contrat{} repris.", ecrits, if ecrits > 1 { "s" } else { "" });
    if permanents_declares {
        phrase.push_str(" La compagnie emploie donc des salariés au mois : « Pour démarrer » dit ce que cela demande encore.");
    }
    for p in &parametrage {
        phrase.push(' ');
        phrase.push_str(p);
    }
    Ok(NatureDite { phrase, ecrits, a_dire: a_dire(&ctx.racine), permanents_declares, parametrage, couleur: crate::routes::couleur_de_l_etat("fait") })
}

/// CE QUE LA DSN REPRISE DIT DEJA NE SE DEMANDE PAS. Pour chaque salarie dont des contrats repris attendent leur nature
/// (`a_dire`), le contrat que sa derniere DSN reprise declare la donne (S21.G00.40.007, la nature du contrat ;
/// `norme-dsn/texte/bloc-S21.G00.40.txt`), avec ce que `demande` exige d'un contrat qui court encore : le debut (.001),
/// le numero (.009), la duree du travail (.013), la fin s'il a un terme (.010) et le salaire du contrat, « la remuneration
/// brute habituelle du salarie » (S21.G00.51, type 010 ; `bloc-S21.G00.51.txt`). Tout y est : la nature se dit comme a
/// l'ecran (`dire`). Il en manque, ou la nature declaree n'est pas d'un permanent : rien ne se choisit, l'ecran le
/// demande. Rend ce que chaque nature dite a fait, en phrases.
pub(crate) fn dire_depuis_les_dsn(racine: &Path, quand: &Jour) -> Vec<String> {
    let lecture = match crate::import::dsn_aem::lire_dsn(&racine.join("historique").join("bulletins")) {
        Ok(l) => l,
        Err(e) => return vec![e.phrase()],
    };
    let Ok(permises) = natures_permanentes() else { return Vec::new() };
    let mut faits = Vec::new();
    for x in a_dire(racine) {
        let salarie = x.salarie.clone();
        let Ok(ctx) = Contexte::nouveau(racine, Some(*quand)) else { break };
        let nir = ctx.salaries.get(salarie.as_str()).map(|f| f.nir.texte().to_string()).unwrap_or_default();
        if nir.is_empty() {
            continue;
        }
        // Sa derniere paie declaree : le contrat tel que la derniere DSN le dit.
        let Some(((_, _, numero), p)) = lecture.paies.iter().filter(|((_, n, _), _)| *n == nir).last() else { continue };
        let c = p.contrat.as_ref();
        let Some(code) = c.and_then(|c| c.nature_dsn.clone()) else { continue };
        let Some(nat) = permises.iter().find(|n| n.code("S21.G00.40.007").as_deref() == Some(code.as_str())) else { continue };
        let en_cours = x.en_cours;
        let mut charge = ChargeNature { salarie: salarie.clone(), nature: nat.cle.clone(), ..ChargeNature::default() };
        let mut complet = true;
        for champ in demande(nat, en_cours) {
            let v = match champ {
                Demande::Numero => Some(numero.clone()).filter(|n| !n.is_empty()),
                Demande::Salaire => p.paie.mesures.salaire_habituel.map(|m| m.to_string()),
                Demande::Debut => c.and_then(|c| c.debut).map(|j| j.iso()),
                Demande::Fin => c.and_then(|c| c.fin).map(|j| j.iso()),
                Demande::Quotite => c.and_then(|c| c.quotite).map(|q| q.to_string()),
            };
            match v {
                Some(v) => charge.poser(champ, v),
                None => complet = false,
            }
        }
        if !complet {
            continue;
        }
        if let Ok(r) = dire(&ctx, &charge) {
            faits.push(format!("{} : {}, lu dans sa DSN reprise. {}", nom_du_salarie(racine, &salarie), nat.libelle, r.phrase));
        }
    }
    faits
}
