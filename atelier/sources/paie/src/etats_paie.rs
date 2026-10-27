//! Les etats de paie de l'onglet Exports (`moteur/operations_exports.py`, la part « livre de paie », et
//! `moteur/operations_etats.py`, les etats 1 a 5, 9 a 12) : la periode, la lecture des bulletins recalcules,
//! le livre de paie et ses trois ecrivains (CSV, XLSX, PDF), le livre detaille, le recapitulatif annuel, la
//! masse salariale, les virements, le prelevement a la source, la reduction generale, les assiettes, les
//! heures et cachets, le cout par spectacle.
//!
//! LES MEMES REGLES POUR TOUS (docstring de `operations_etats`) : un export LIT -- les saisies sont
//! RECALCULEES par le meme calcul que le bulletin (`engagement::charger_donnees`), le PAS par `prelevement::des_paies`,
//! aucun numero n'est consomme ; chaque ligne dit d'ou elle vient (`saisie`, `DSN déposée`, `AEM attestée`) ;
//! un montant que la source ne dit pas est VIDE, pas zero, et le total ne le compte pas.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::rc::Rc;

use rust_xlsxwriter::{Format, FormatAlign, Workbook};
use serde_json::{json, Map, Value};

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use socle::date::en_texte;
use socle::valeur::valeur_en_texte;
use socle::echec::erreur;
use socle::fichiers::vrai;
use socle::num::{eur2, zero2, Decimal};
use droit::pas::Versement;
use crate::tableaux_export::{
    col_m, col_n, col_t, col_x, ecrire as ecrire_tableau, format_possible, jj, libelle_mois, montant, nom_compagnie, qui, refus, sections_de, totaliser, Ligne,
    Section, Tableau, Val, FORMATS_TABLEAU, R, TOTAL_GENERAL,
};

pub(crate) const SAISIE: &str = "saisie";
pub(crate) const DSN: &str = "DSN déposée";
pub(crate) const AEM: &str = "AEM attestée";
/// Une paie d'avant PuppetPaie qu'aucune DSN ne declare : un bulletin de paie seul l'a ecrite (`historique/paies.json`,
/// `declare_par: "bulletin"`).
pub(crate) const BULLETIN: &str = "bulletin de paie";

/// Ce qu'un etat demande : la periode (une seule des trois), le format, ce qui ne sort que sur demande.
#[derive(Clone, Default)]
pub(crate) struct Demande {
    pub mois: Option<String>,
    pub annee: Option<String>,
    pub toutes: bool,
    pub regroupement: String,
    pub format: String,
    pub coordonnees: bool,
    pub ecrire: bool,
    pub logiciel: Option<String>,
    pub analytique: Option<bool>,
}

/// Le nom du mois `i` abrege de quatre lettres, avec son point : « Janv. ». Les appelants parcourent les douze colonnes
/// d'une annee (1 a 12) : un autre rang ne peut pas arriver, et donnerait une entete vide plutot qu'un autre mois.
pub(crate) fn mois_abrege(i: usize) -> String {
    socle::date::nom_du_mois(i as i64, socle::date::Graphie::Titre).map(|n| n.chars().take(4).collect::<String>() + ".").unwrap_or_default()
}

// ------------------------------------------------------------------ ou ca s'ecrit

/// `operations_exports.dossier_exports` : `exports/`, et le sous-dossier de l'annee quand il y en a une.
pub(crate) fn dossier_exports(ctx: &Contexte, annee: Option<&str>) -> PathBuf {
    let d = ctx.racine.join("exports");
    match annee {
        Some(a) if !a.is_empty() => d.join(a),
        _ => d,
    }
}

// ------------------------------------------------------------------ la periode

/// `operations_etats.Periode` : ce qu'on a demande, les mois qu'on a, et d'ou ils viennent.
pub(crate) struct Periode {
    pub dispo: BTreeMap<String, &'static str>,
    pub un_mois: Option<String>,
    pub annee: Option<String>,
    pub toutes: bool,
    pub mois: Vec<String>,
    pub libelle: String,
}

/// `operations_exports.mois_disponibles` : tous les mois qu'il y a matiere a exporter, et d'ou ils viennent.
pub(crate) fn mois_disponibles(ctx: &Contexte) -> R<BTreeMap<String, &'static str>> {
    Ok(crate::routes_dossiers::mois_disponibles(ctx)?.into_iter().collect())
}

/// `operations_exports._libelle_periode`.
fn libelle_periode(mois: &[String], un_mois: &Option<String>, annee: &Option<String>, toutes: bool) -> R<String> {
    if toutes {
        let annees: BTreeSet<String> = mois.iter().map(|m| m.chars().take(4).collect()).collect();
        let premiere = annees.iter().next().cloned().unwrap_or_default();
        let derniere = annees.iter().next_back().cloned().unwrap_or_default();
        return Ok(format!("toutes les années ({})", if annees.len() > 1 { format!("{} à {}", premiere, derniere) } else { premiere }));
    }
    if let Some(u) = un_mois {
        return libelle_mois(u);
    }
    Ok(annee.clone().unwrap_or_default())
}

impl Periode {
    /// `Periode(ctx, mois, annee, toutes_les_annees, obligatoire=...)`.
    pub(crate) fn nouvelle(ctx: &Contexte, mois: Option<&str>, annee: Option<&str>, toutes: bool, obligatoire: bool) -> R<Periode> {
        let mois = mois.filter(|m| !m.is_empty()).map(str::to_string);
        let annee = annee.filter(|a| !a.is_empty()).map(str::to_string);
        let donnees = usize::from(mois.is_some()) + usize::from(annee.is_some()) + usize::from(toutes);
        if donnees != 1 {
            return Err(refus("Un état se demande pour UNE période : un mois (--mois AAAA-MM), une année (--annee AAAA), ou toutes les années (--toutes-les-annees)."));
        }
        let dispo = mois_disponibles(ctx)?;
        let pris: Vec<String> = if obligatoire {
            mois_pris(&dispo, &mois, &annee, toutes)?
        } else if let Some(m) = &mois {
            vec![m.clone()]
        } else if toutes {
            dispo.keys().cloned().collect()
        } else {
            let a = annee.clone().unwrap_or_default();
            dispo.keys().filter(|m| m.starts_with(&a)).cloned().collect()
        };
        let libelle = if !pris.is_empty() {
            libelle_periode(&pris, &mois, &annee, toutes)?
        } else if let Some(m) = &mois {
            libelle_mois(m)?
        } else if let Some(a) = &annee {
            a.clone()
        } else {
            "toutes les années".to_string()
        };
        Ok(Periode { dispo, un_mois: mois, annee, toutes, mois: pris, libelle })
    }

    /// `Periode(ctx, mois, annee, toutes)` d'une demande.
    pub(crate) fn de(ctx: &Contexte, d: &Demande, obligatoire: bool) -> R<Periode> {
        Periode::nouvelle(ctx, d.mois.as_deref(), d.annee.as_deref(), d.toutes, obligatoire)
    }

    pub(crate) fn saisis(&self) -> Vec<String> {
        self.mois.iter().filter(|m| self.dispo.get(*m) == Some(&SAISIE)).cloned().collect()
    }

    pub(crate) fn declares(&self) -> Vec<String> {
        self.mois.iter().filter(|m| self.dispo.get(*m) == Some(&DSN)).cloned().collect()
    }

    pub(crate) fn contient(&self, m: &str) -> bool {
        if self.toutes {
            return true;
        }
        if let Some(u) = &self.un_mois {
            return m == u;
        }
        m.starts_with(self.annee.as_deref().unwrap_or(""))
    }

    /// `Periode.nom` : ce qui entre dans le nom du fichier.
    pub(crate) fn nom(&self) -> String {
        if self.toutes {
            return "toutes-les-annees".to_string();
        }
        self.un_mois.clone().or_else(|| self.annee.clone()).unwrap_or_default()
    }

    /// `Periode.dossier` : `exports/` (toutes les annees) ou `exports/AAAA/`.
    pub(crate) fn dossier(&self, ctx: &Contexte) -> PathBuf {
        if self.toutes {
            return dossier_exports(ctx, None);
        }
        let a: String = self.un_mois.clone().or_else(|| self.annee.clone()).unwrap_or_default().chars().take(4).collect();
        dossier_exports(ctx, Some(&a))
    }
}

/// `operations_exports._mois_pris` : les mois demandes qu'on sait exporter -- et le refus qui dit lequel manque.
fn mois_pris(dispo: &BTreeMap<String, &'static str>, mois: &Option<String>, annee: &Option<String>, toutes: bool) -> R<Vec<String>> {
    if toutes {
        if dispo.is_empty() {
            return Err(refus(
                "Aucune paie, ni saisie ni déclarée : il n'y a pas de livre de paie à faire. Les années d'avant PuppetPaie se reprennent des DSN de l'ancien logiciel (Configuration › Reprendre l'historique).",
            ));
        }
        return Ok(dispo.keys().cloned().collect());
    }
    if let Some(m) = mois {
        if !dispo.contains_key(m) {
            return Err(refus(format!("Aucune paie en {} : ni engagement (engagements/), ni déclarée par une DSN importée.", m)));
        }
        return Ok(vec![m.clone()]);
    }
    let a = annee.clone().unwrap_or_default();
    let pris: Vec<String> = dispo.keys().filter(|m| m.starts_with(&a)).cloned().collect();
    if pris.is_empty() {
        return Err(refus(format!("Aucune paie en {}, ni saisie ni déclarée.", a)));
    }
    Ok(pris)
}

/// `_ecarter_declares` : les mois saisis de la periode ; les mois declares avant PuppetPaie sont ecartes EN LE
/// DISANT, et un etat sans aucun mois saisi refuse.
pub(crate) fn ecarter_declares(p: &Periode, avert: &mut Vec<String>, ce_qui_manque: &str) -> R<Vec<String>> {
    let declares = p.declares();
    if !declares.is_empty() {
        avert.push(format!(
            "{} mois d'avant PuppetPaie ({} à {}) ne sont pas dans cet état : ils viennent des DSN déposées, qui ne déclarent pas {}. Rien n'est reconstitué.",
            declares.len(),
            declares[0],
            declares[declares.len() - 1],
            ce_qui_manque
        ));
    }
    let saisis = p.saisis();
    if saisis.is_empty() {
        return Err(refus(format!(
            "Aucun mois saisi dans PuppetPaie sur la période ({}) : cet état a besoin de {}, que seule une paie recalculée porte.",
            p.libelle, ce_qui_manque
        )));
    }
    Ok(saisis)
}

// ------------------------------------------------------------------ les lectures

/// `lire_paies_declarees(ctx)["paies"]` / `lire_historique(ctx)["engagements"]` : la liste du fichier, ou rien.
pub(crate) fn liste_historique(ctx: &Contexte, fichier: &str, cle: &str) -> R<Vec<Value>> {
    let p = ctx.racine.join("historique").join(fichier);
    if !p.exists() {
        return Ok(Vec::new());
    }
    let data = crate::routes_listes::lire_json(&p)?;
    let Value::Object(o) = data else {
        return Err(erreur("AttributeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(&data))));
    };
    match o.get(cle) {
        None => Ok(Vec::new()),
        Some(Value::Array(a)) => Ok(a.clone()),
        Some(Value::String(t)) if t.is_empty() => Ok(Vec::new()),
        Some(Value::Object(m)) if m.is_empty() => Ok(Vec::new()),
        Some(autre @ (Value::String(_) | Value::Object(_))) => Err(erreur("AttributeError", socle::fichiers::attendu("une liste", socle::valeur::nom_type(autre)))),
        Some(autre) => Err(erreur("TypeError", socle::fichiers::attendu("une liste", socle::valeur::nom_type(autre)))),
    }
}

pub(crate) fn paies_declarees(ctx: &Contexte) -> R<Vec<Value>> {
    liste_historique(ctx, "paies.json", "paies")
}

/// La saisie de `mois` reduite a ses engagements valides, et leurs paies telles qu'elles ont ete validees : ce que
/// lisent les etats et les exports.
pub(crate) fn charger_mois(ctx: &Contexte, mois: &str) -> R<(Value, Vec<Paie>)> {
    engagement::etats::validees_du_mois(ctx, mois)
        .map_err(|m| erreur("ErreurOperation", m))?
        .ok_or_else(|| erreur("ErreurOperation", format!("{} : aucun engagement ne touche ce mois.", mois)))
}

/// Les paies validees d'un mois, lues une fois par export.
pub(crate) struct Lecture<'a> {
    ctx: &'a Contexte,
    paies: HashMap<String, Rc<Vec<Paie>>>,
    pas: HashMap<String, Rc<Vec<Option<Versement>>>>,
}

impl<'a> Lecture<'a> {
    pub(crate) fn nouvelle(ctx: &'a Contexte) -> Self {
        Lecture { ctx, paies: HashMap::new(), pas: HashMap::new() }
    }

    /// Les paies du mois qui ont un bulletin (un engagement sans paie n'en a pas).
    pub(crate) fn paies(&mut self, mois: &str) -> R<Rc<Vec<Paie>>> {
        if !self.paies.contains_key(mois) {
            let (_, toutes) = charger_mois(self.ctx, mois)?;
            self.paies.insert(mois.to_string(), Rc::new(toutes.into_iter().filter(|p| !p.engagement.sans_paie()).collect()));
        }
        Ok(Rc::clone(&self.paies[mois]))
    }

    /// `pas.des_paies` : le PAS de chaque paie du mois, par position dans `paies(mois)`.
    pub(crate) fn pas(&mut self, mois: &str) -> R<Rc<Vec<Option<Versement>>>> {
        if !self.pas.contains_key(mois) {
            let paies = self.paies(mois)?;
            let v = crate::prelevement::des_paies(self.ctx, &paies).map_err(|m| erreur("ErreurOperation", m))?;
            self.pas.insert(mois.to_string(), Rc::new(v));
        }
        Ok(Rc::clone(&self.pas[mois]))
    }
}

/// Un texte de la fiche (`fiche.get(cle) or ""`).
pub(crate) fn fiche_texte(fiche: &Value, cle: &str) -> String {
    fiche.get(cle).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default()
}

/// Le titre du spectacle d'une paie (`(p.spectacle or {}).get("titre") or ""`).
pub(crate) fn titre_spectacle(p: &Paie) -> String {
    p.spectacle.get("titre").filter(|t| vrai(t)).map(en_texte).unwrap_or_default()
}

/// `str(Decimal)` d'un taux flottant : `Decimal(str(x))`.
pub(crate) fn dec_taux(x: f64) -> R<Decimal> {
    socle::num::dec_f64(x).map_err(|m| erreur("ValueError", m))
}

fn nat_libelle(p: &Paie) -> R<String> {
    Ok(p.engagement.nat().map_err(|m| erreur("ErreurOperation", m))?.libelle.clone())
}

// ------------------------------------------------------------------ le livre de paie

/// `Colonnes` qui se totalisent au livre de paie, et le nom sous lequel elles se comptent quand elles manquent.
const CUMULABLES: [&str; 7] = ["brut", "cotisations_salariales", "cotisations_patronales", "net_imposable", "net_a_payer", "pas", "cout_employeur"];

/// `LigneLivre` : un bulletin, vu du livre de paie.
#[derive(Clone)]
pub(crate) struct LigneLivre {
    pub mois: String,
    pub salarie: String,
    pub cle_salarie: String,
    pub contrat: String,
    pub nature: String,
    pub spectacle: String,
    pub brut: Option<Decimal>,
    pub cotisations_salariales: Option<Decimal>,
    pub cotisations_patronales: Option<Decimal>,
    pub net_imposable: Option<Decimal>,
    pub net_a_payer: Option<Decimal>,
    pub pas: Option<Decimal>,
    pub cout_employeur: Option<Decimal>,
    pub source: &'static str,
    pub nir: String,
    pub date_naissance: String,
    pub adresse: String,
}

impl LigneLivre {
    fn montant(&self, champ: &str) -> Option<Decimal> {
        match champ {
            "brut" => self.brut,
            "cotisations_salariales" => self.cotisations_salariales,
            "cotisations_patronales" => self.cotisations_patronales,
            "net_imposable" => self.net_imposable,
            "net_a_payer" => self.net_a_payer,
            "pas" => self.pas,
            "cout_employeur" => self.cout_employeur,
            _ => None,
        }
    }
}

/// `Totaux`.
#[derive(Clone)]
pub(crate) struct Totaux {
    pub bulletins: i64,
    pub sommes: Vec<(&'static str, Decimal)>,
    /// Combien de lignes ne declarent pas telle colonne, dans l'ordre ou on l'a vu.
    pub inconnus: Vec<(&'static str, i64)>,
}

impl Totaux {
    pub(crate) fn somme(&self, champ: &str) -> Decimal {
        self.sommes.iter().find(|(c, _)| *c == champ).map(|(_, v)| *v).unwrap_or_else(zero2)
    }

    /// `_total` : le total d'une colonne, ou `None` quand AUCUNE ligne ne la declare.
    fn total(&self, champ: &str) -> Option<Decimal> {
        let manque = self.inconnus.iter().find(|(c, _)| *c == champ).map_or(0, |(_, n)| *n);
        if manque >= self.bulletins {
            None
        } else {
            Some(self.somme(champ))
        }
    }

    fn json(&self) -> Value {
        let mut m = Map::new();
        m.insert("bulletins".into(), json!(self.bulletins));
        for c in CUMULABLES {
            m.insert(c.into(), json!(self.somme(c).to_string()));
        }
        m.insert("inconnus".into(), Value::Object(self.inconnus.iter().map(|(c, n)| ((*c).to_string(), json!(n))).collect()));
        Value::Object(m)
    }
}

/// `_totaliser` : la somme de ce qui est DECLARE, colonne par colonne, et le compte de ce qui ne l'est pas.
fn totaliser_lignes(lignes: &[&LigneLivre]) -> Totaux {
    let mut t = Totaux { bulletins: lignes.len() as i64, sommes: CUMULABLES.iter().map(|c| (*c, zero2())).collect(), inconnus: Vec::new() };
    for l in lignes {
        for champ in CUMULABLES {
            match l.montant(champ) {
                None => match t.inconnus.iter_mut().find(|(c, _)| *c == champ) {
                    Some(x) => x.1 += 1,
                    None => t.inconnus.push((champ, 1)),
                },
                Some(v) => {
                    if let Some(x) = t.sommes.iter_mut().find(|(c, _)| *c == champ) {
                        x.1 = socle::num::add(x.1, v);
                    }
                }
            }
        }
    }
    t
}

pub(crate) struct Groupe {
    pub libelle: String,
    pub lignes: Vec<LigneLivre>,
    pub totaux: Totaux,
}

/// `LivreDePaie`.
pub(crate) struct LivreDePaie {
    pub periode: String,
    pub mois: Vec<String>,
    pub regroupement: String,
    pub groupes: Vec<Groupe>,
    pub totaux: Totaux,
    pub coordonnees: bool,
    pub format: String,
    pub fichier: Option<PathBuf>,
    pub avertissements: Vec<String>,
}

impl LivreDePaie {
    pub(crate) fn lignes(&self) -> Vec<&LigneLivre> {
        self.groupes.iter().flat_map(|g| g.lignes.iter()).collect()
    }

    /// `sortie_json.brut(LivreDePaie)`.
    pub(crate) fn json(&self) -> Value {
        let ligne = |l: &LigneLivre| {
            let m = |v: Option<Decimal>| v.map_or(Value::Null, |d| json!(d.to_string()));
            json!({
                "mois": l.mois, "salarie": l.salarie, "cle_salarie": l.cle_salarie, "contrat": l.contrat, "nature": l.nature,
                "spectacle": l.spectacle, "brut": m(l.brut), "cotisations_salariales": m(l.cotisations_salariales),
                "cotisations_patronales": m(l.cotisations_patronales), "net_imposable": m(l.net_imposable),
                "net_a_payer": m(l.net_a_payer), "pas": m(l.pas), "cout_employeur": m(l.cout_employeur),
                "source": l.source, "nir": l.nir, "date_naissance": l.date_naissance, "adresse": l.adresse,
            })
        };
        json!({
            "periode": self.periode,
            "mois": self.mois,
            "regroupement": self.regroupement,
            "groupes": self.groupes.iter().map(|g| json!({"libelle": g.libelle, "lignes": g.lignes.iter().map(ligne).collect::<Vec<_>>(), "totaux": g.totaux.json()})).collect::<Vec<_>>(),
            "totaux": self.totaux.json(),
            "coordonnees": self.coordonnees,
            "format": self.format,
            "fichier": self.fichier.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()),
            "avertissements": self.avertissements,
        })
    }
}

/// `REGROUPEMENTS`.
fn regroupement_lisible(r: &str) -> &'static str {
    match r {
        "salarie" => "par salarié",
        "mois" => "par mois",
        _ => "par spectacle",
    }
}

/// `_lignes_du_mois` : les lignes du livre pour un mois SAISI, ce qu'il a fallu taire, et si ses numeros de
/// contrat sont tous ecrits. Les bulletins sont RECALCULES ; aucun numero n'est invente.
fn lignes_du_mois(ctx: &Contexte, mois: &str, coordonnees: bool) -> R<(Vec<LigneLivre>, Vec<String>, bool)> {
    let (_, toutes) = charger_mois(ctx, mois)?;
    let paies: Vec<Paie> = toutes.into_iter().filter(|p| !p.engagement.sans_paie()).collect();
    let pas = crate::prelevement::des_paies(ctx, &paies).map_err(|m| erreur("ErreurOperation", m))?;
    let (mut lignes, mut sans_pas) = (Vec::new(), Vec::new());
    for (p, r) in paies.iter().zip(pas.iter()) {
        if r.is_none() {
            sans_pas.push(qui(&p.salarie)?);
        }
        let s = &p.salarie;
        lignes.push(LigneLivre {
            mois: mois.to_string(),
            salarie: qui(s)?,
            cle_salarie: valeur_en_texte(s.get("_cle").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("_cle")))?),
            contrat: p.engagement.numero.clone().unwrap_or_default(),
            nature: nat_libelle(p)?,
            spectacle: titre_spectacle(p),
            brut: Some(eur2(p.brut)),
            cotisations_salariales: Some(eur2(p.total_sal)),
            cotisations_patronales: Some(eur2(p.total_pat)),
            net_imposable: Some(eur2(p.net_imposable)),
            net_a_payer: Some(eur2(p.net_a_payer)),
            pas: r.as_ref().map(|v| eur2(v.prelevement)),
            cout_employeur: Some(eur2(p.cout_employeur)),
            source: SAISIE,
            nir: if coordonnees { fiche_texte(s, "nir") } else { String::new() },
            date_naissance: if coordonnees { fiche_texte(s, "date_naissance") } else { String::new() },
            adresse: if coordonnees {
                let a = fiche_texte(s, "adresse_affichage");
                if a.is_empty() {
                    fiche_texte(s, "adresse")
                } else {
                    a
                }
            } else {
                String::new()
            },
        });
    }
    // Ses numeros de contrat sont ecrits quand chaque engagement est valide.
    Ok((lignes, sans_pas, paies.iter().all(|p| p.engagement.numero.as_deref().is_some_and(|n| !n.is_empty()))))
}

/// La source d'une paie de `historique/paies.json` : la DSN deposee (completee ou non par un bulletin), ou le bulletin
/// seul.
pub(crate) fn source_declaree(x: &Value) -> &'static str {
    if x.get("declare_par").and_then(Value::as_str) == Some("bulletin") {
        BULLETIN
    } else {
        DSN
    }
}

/// `_lignes_declarees` : les lignes d'un mois ANTERIEUR a PuppetPaie, telles que la DSN deposee (et les bulletins de
/// paie, quand l'import les a lus) les ont declarees.
fn lignes_declarees(ctx: &Contexte, mois: &str, coordonnees: bool, declarees: &[Value]) -> R<Vec<LigneLivre>> {
    let mut out = Vec::new();
    // LA NATURE D'UNE PAIE D'AVANT (#236) : celle du contrat repris du meme salarie qui couvre le mois -- dite apres
    // l'import pour un permanent (`importation_nature`) ; un contrat muet, ou aucun, est un CDDU (regle 6).
    let contrats = liste_historique(ctx, "engagements.json", "engagements")?;
    let nature_de = |cle: &str| -> R<String> {
        for e in &contrats {
            let Some(o) = e.as_object() else { continue };
            let t = |k: &str| o.get(k).map(valeur_en_texte).unwrap_or_default();
            let (debut, fin) = (t("debut"), t("fin"));
            if t("salarie") != cle || debut.len() < 7 || debut[..7] > *mois || (fin.len() >= 7 && fin[..7] < *mois) {
                continue;
            }
            if let Some(l) = crate::importation_nature::libelle_d_un_contrat(o)? {
                return Ok(l);
            }
        }
        Ok(droit::natures::nature(None).map_err(|m| erreur("ErreurOperation", m))?.libelle.clone())
    };
    for x in declarees {
        if x.get("mois").map_or(true, |m| valeur_en_texte(m) != mois) {
            continue;
        }
        let cle = fiche_texte(x, "salarie");
        let fiche = if cle.is_empty() { None } else { ctx.salaries.get(&cle).filter(|f| vrai(f)) };
        // Le titre de la fiche du spectacle ; a defaut, celui que le bulletin imprimait.
        let spectacle = ctx
            .spectacles
            .values()
            .find(|s| s.get("numero_objet").is_some_and(vrai) && s.get("numero_objet") == x.get("numero_objet"))
            .map(|s| fiche_texte(s, "titre"))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| fiche_texte(x, "spectacle"));
        let coord = |cle: &str| -> String {
            match (coordonnees, fiche) {
                (true, Some(f)) => fiche_texte(f, cle),
                _ => String::new(),
            }
        };
        out.push(LigneLivre {
            mois: mois.to_string(),
            salarie: match fiche {
                Some(f) => qui(f)?,
                None => cle.clone(),
            },
            cle_salarie: cle.clone(),
            contrat: fiche_texte(x, "contrat"),
            nature: nature_de(&cle)?,
            spectacle,
            brut: montant(x.get("brut"))?,
            // Les colonnes qu'un bulletin de paie ajoute a la ligne (lot E de l'import) ; vides pour une DSN seule.
            cotisations_salariales: montant(x.get("cotisations_salariales"))?,
            cotisations_patronales: montant(x.get("cotisations_patronales"))?,
            net_imposable: montant(x.get("net_imposable"))?,
            net_a_payer: montant(x.get("net_a_payer"))?,
            pas: montant(x.get("pas"))?,
            cout_employeur: montant(x.get("cout_employeur"))?,
            source: source_declaree(x),
            nir: coord("nir"),
            date_naissance: coord("date_naissance"),
            adresse: if coordonnees && fiche.is_some() {
                let f = fiche.unwrap();
                let a = fiche_texte(f, "adresse_affichage");
                if a.is_empty() {
                    fiche_texte(f, "adresse")
                } else {
                    a
                }
            } else {
                String::new()
            },
        });
    }
    Ok(out)
}

/// `_grouper` : les lignes en groupes, chacun avec son sous-total. L'ordre DANS un groupe est chronologique puis
/// alphabetique : deux editions de la meme periode donnent le meme fichier.
fn grouper(lignes: &[LigneLivre], regroupement: &str) -> R<Vec<Groupe>> {
    let cle = |l: &LigneLivre| -> String {
        match regroupement {
            "salarie" => l.salarie.clone(),
            "mois" => l.mois.clone(),
            _ => {
                if l.spectacle.is_empty() {
                    "(sans spectacle)".to_string()
                } else {
                    l.spectacle.clone()
                }
            }
        }
    };
    let dedans = |l: &LigneLivre| -> (String, String, String) {
        match regroupement {
            "salarie" => (l.mois.clone(), l.spectacle.clone(), l.contrat.clone()),
            "mois" => (l.salarie.clone(), l.spectacle.clone(), l.contrat.clone()),
            _ => (l.mois.clone(), l.salarie.clone(), l.contrat.clone()),
        }
    };
    let mut paquets: BTreeMap<String, Vec<LigneLivre>> = BTreeMap::new();
    for l in lignes {
        paquets.entry(cle(l)).or_default().push(l.clone());
    }
    let mut out = Vec::new();
    for (k, mut v) in paquets {
        v.sort_by_cached_key(|l| dedans(l));
        let refs: Vec<&LigneLivre> = v.iter().collect();
        let totaux = totaliser_lignes(&refs);
        let libelle = if regroupement == "mois" { libelle_mois(&k)? } else { k };
        out.push(Groupe { libelle, lignes: v, totaux });
    }
    Ok(out)
}

/// `operations_exports.livre_de_paie`.
pub(crate) fn livre_de_paie(ctx: &Contexte, d: &Demande) -> R<LivreDePaie> {
    let donnees = usize::from(d.mois.as_deref().is_some_and(|m| !m.is_empty())) + usize::from(d.annee.as_deref().is_some_and(|a| !a.is_empty())) + usize::from(d.toutes);
    if donnees != 1 {
        return Err(refus("Un livre de paie se demande pour UNE période : un mois (--mois AAAA-MM), une année (--annee AAAA), ou toutes les années (--toutes-les-annees)."));
    }
    if !["csv", "xlsx", "pdf"].contains(&d.format.as_str()) {
        return Err(refus(format!("Format inconnu : « {} ». Les formats possibles sont csv, pdf, xlsx.", d.format)));
    }
    if !["salarie", "mois", "spectacle"].contains(&d.regroupement.as_str()) {
        return Err(refus(format!("Regroupement inconnu : « {} ». Les regroupements possibles sont mois, salarie, spectacle.", d.regroupement)));
    }
    let mois = d.mois.clone().filter(|m| !m.is_empty());
    let annee = d.annee.clone().filter(|a| !a.is_empty());
    let dispo = mois_disponibles(ctx)?;
    let pris = mois_pris(&dispo, &mois, &annee, d.toutes)?;
    let declarees = paies_declarees(ctx)?;
    let (mut lignes, mut avert, mut sans_pas, mut sans_numeros) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for m in &pris {
        if dispo.get(m) == Some(&DSN) {
            lignes.extend(lignes_declarees(ctx, m, d.coordonnees, &declarees)?);
            continue;
        }
        let (l_m, sans, figes) = lignes_du_mois(ctx, m, d.coordonnees)?;
        lignes.extend(l_m);
        sans_pas.extend(sans);
        if !figes {
            sans_numeros.push(m.clone());
        }
    }
    if lignes.is_empty() {
        return Err(refus("Aucun bulletin sur la période : rien à porter au livre de paie."));
    }
    let sources: BTreeSet<&str> = lignes.iter().map(|l| l.source).collect();
    // Une ligne que la DSN seule declare n'a ni la repartition des cotisations ni le cout employeur ; un bulletin de paie
    // lu a l'import les lui donne (et ecrit seul les paies qu'aucune DSN ne declare).
    let incomplete = |l: &&LigneLivre| l.cotisations_salariales.is_none() || l.cotisations_patronales.is_none() || l.cout_employeur.is_none();
    if sources.contains(DSN) && !sources.contains(BULLETIN) && lignes.iter().filter(|l| l.source == DSN).all(|l| incomplete(&l)) {
        let mois_dsn: BTreeSet<&String> = lignes.iter().filter(|l| l.source == DSN).map(|l| &l.mois).collect();
        avert.push(format!(
            "{} mois ({} à {}) viennent des DSN déposées avant PuppetPaie, pas d'une saisie recalculée : la colonne Source le dit ligne par ligne. Une DSN ne déclare pas la répartition des cotisations entre part salariale et part patronale — ces deux colonnes et le coût employeur restent donc vides sur ces lignes, et les totaux ne les comptent pas.",
            mois_dsn.len(),
            mois_dsn.iter().next().unwrap(),
            mois_dsn.iter().next_back().unwrap()
        ));
    } else if sources.contains(DSN) || sources.contains(BULLETIN) {
        let mois_avant: BTreeSet<&String> = lignes.iter().filter(|l| l.source == DSN || l.source == BULLETIN).map(|l| &l.mois).collect();
        let vides = lignes.iter().filter(|l| l.source == DSN || l.source == BULLETIN).filter(|l| incomplete(l)).count();
        let mut phrase = format!(
            "{} mois ({} à {}) viennent d'avant PuppetPaie — des DSN déposées et des bulletins de paie repris à l'import —, pas d'une saisie recalculée : la colonne Source le dit ligne par ligne ; quand un bulletin et la DSN disaient autrement, c'est le bulletin qui est porté.",
            mois_avant.len(),
            mois_avant.iter().next().unwrap(),
            mois_avant.iter().next_back().unwrap()
        );
        if vides > 0 {
            phrase.push_str(&format!(
                " Une DSN ne déclare pas la répartition des cotisations entre part salariale et part patronale : sur {} ligne(s) qu'aucun bulletin ne complète, ces deux colonnes et le coût employeur restent vides, et les totaux ne les comptent pas.",
                vides
            ));
        }
        avert.push(phrase);
    }
    if !sans_numeros.is_empty() {
        avert.push(format!(
            "Numéros de contrat absents pour {} : ces mois n'ont pas été produits par PuppetPaie, rien n'y a été figé. La colonne Contrat reste vide plutôt que de porter un numéro qui n'a jamais été déclaré.",
            sans_numeros.join(", ")
        ));
    }
    if !sans_pas.is_empty() {
        let uniques: BTreeSet<&String> = sans_pas.iter().collect();
        avert.push(format!(
            "Prélèvement à la source inconnu pour {} bulletin(s) ({}) : aucun taux DGFiP au magasin et le barème de l'année ne porte pas la grille par défaut. La colonne reste vide et le total du PAS ne les compte pas.",
            sans_pas.len(),
            uniques.into_iter().cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    if d.coordonnees {
        avert.push("Ce fichier porte le NIR, la date de naissance et l'adresse des salariés : il reste dans prive/, et ne s'envoie qu'à qui en a besoin.".to_string());
    }
    let groupes = grouper(&lignes, &d.regroupement)?;
    let refs: Vec<&LigneLivre> = lignes.iter().collect();
    let mut livre = LivreDePaie {
        periode: libelle_periode(&pris, &mois, &annee, d.toutes)?,
        mois: pris,
        regroupement: d.regroupement.clone(),
        groupes,
        totaux: totaliser_lignes(&refs),
        coordonnees: d.coordonnees,
        format: d.format.clone(),
        fichier: None,
        avertissements: avert,
    };
    if d.ecrire {
        livre.fichier = Some(ecrire_livre(ctx, &livre, &mois, &annee, d.toutes)?);
    }
    Ok(livre)
}

/// `_ecrire` du livre : `exports/AAAA/livre-de-paie-<periode>-<regroupement>[-coordonnees].<ext>`.
fn ecrire_livre(ctx: &Contexte, livre: &LivreDePaie, mois: &Option<String>, annee: &Option<String>, toutes: bool) -> R<PathBuf> {
    let (dossier, periode) = if toutes {
        (dossier_exports(ctx, None), "toutes-les-annees".to_string())
    } else if let Some(m) = mois {
        (dossier_exports(ctx, Some(&m.chars().take(4).collect::<String>())), m.clone())
    } else {
        let a = annee.clone().unwrap_or_default();
        (dossier_exports(ctx, Some(&a)), a)
    };
    std::fs::create_dir_all(&dossier).map_err(|e| erreur("OSError", format!("{} : {}", dossier.display(), e)))?;
    let nom = format!("livre-de-paie-{}-{}{}.{}", periode, livre.regroupement, if livre.coordonnees { "-coordonnees" } else { "" }, livre.format);
    let chemin = dossier.join(nom);
    let octets = match livre.format.as_str() {
        "csv" => csv_livre(livre),
        "xlsx" => xlsx_livre(livre)?,
        _ => pdf_livre(ctx, livre)?,
    };
    std::fs::write(&chemin, octets).map_err(|e| erreur("OSError", format!("{} : {}", chemin.display(), e)))?;
    Ok(chemin)
}

// --------------------------------------------------- les ecrivains du livre de paie

/// Les colonnes qui portent un montant : elles s'alignent a droite au PDF et se formatent en euros au classeur.
const MONTANTS: [&str; 7] = ["Brut", "Cotisations salariales", "Cotisations patronales", "Net imposable", "Net a payer", "PAS", "Cout employeur"];

/// `colonnes(livre)` : l'en-tete, dans l'ordre.
fn colonnes_livre(livre: &LivreDePaie) -> Vec<&'static str> {
    let mut c = vec!["Mois", "Salarie"];
    if livre.coordonnees {
        c.extend(["NIR", "Date de naissance", "Adresse"]);
    }
    c.extend(["Contrat", "Nature", "Spectacle", "Brut", "Cotisations salariales", "Cotisations patronales", "Net imposable", "Net a payer", "PAS", "Cout employeur", "Source"]);
    c
}

/// Une cellule du livre : un texte, un montant (ou son absence).
#[derive(Clone)]
enum CelL {
    T(String),
    M(Option<Decimal>),
}

fn cellules(l: &LigneLivre, coordonnees: bool) -> Vec<CelL> {
    let t = |s: &str| CelL::T(s.to_string());
    let mut c = vec![t(&l.mois), t(&l.salarie)];
    if coordonnees {
        c.extend([t(&l.nir), t(&l.date_naissance), t(&l.adresse)]);
    }
    c.extend([t(&l.contrat), t(&l.nature), t(&l.spectacle)]);
    c.extend(CUMULABLES.iter().map(|ch| CelL::M(l.montant(ch))));
    c.push(t(l.source));
    c
}

fn cellules_totaux(tot: &Totaux, libelle: &str, coordonnees: bool) -> Vec<CelL> {
    let t = |s: &str| CelL::T(s.to_string());
    let mut c = vec![t(""), t(libelle)];
    if coordonnees {
        c.extend([t(""), t(""), t("")]);
    }
    c.extend([t(""), t(""), t("")]);
    c.extend(CUMULABLES.iter().map(|ch| CelL::M(tot.total(ch))));
    c.push(CelL::T(format!("{} bulletin(s)", tot.bulletins)));
    c
}

/// Les lignes du livre dans l'ordre d'ecriture : (cellules, genre) -- genre « », « total », « general ».
fn rangees_livre(livre: &LivreDePaie) -> Vec<(Vec<CelL>, &'static str)> {
    let mut out = Vec::new();
    for g in &livre.groupes {
        for l in &g.lignes {
            out.push((cellules(l, livre.coordonnees), ""));
        }
        out.push((cellules_totaux(&g.totaux, &format!("Total {}", g.libelle), livre.coordonnees), "total"));
    }
    out.push((cellules_totaux(&livre.totaux, "TOTAL GENERAL", livre.coordonnees), "general"));
    out
}

fn titre_livre(livre: &LivreDePaie) -> String {
    format!("Livre de paie -- {}, {}", livre.periode, regroupement_lisible(&livre.regroupement))
}

/// `csv_livre` : point-virgule, virgule decimale, UTF-8 avec BOM.
pub(crate) fn csv_livre(livre: &LivreDePaie) -> Vec<u8> {
    let mut s = String::from("\u{feff}");
    let ligne = |s: &mut String, c: &[String]| s.push_str(&socle::csv::ligne(c, ';'));
    let texte = |c: &CelL| -> String {
        match c {
            CelL::T(t) => t.clone(),
            CelL::M(None) => String::new(),
            CelL::M(Some(d)) => d.to_string().replace('.', ","),
        }
    };
    ligne(&mut s, &[titre_livre(livre)]);
    ligne(&mut s, &[]);
    ligne(&mut s, &colonnes_livre(livre).iter().map(|c| c.to_string()).collect::<Vec<_>>());
    for g in &livre.groupes {
        for l in &g.lignes {
            ligne(&mut s, &cellules(l, livre.coordonnees).iter().map(texte).collect::<Vec<_>>());
        }
        ligne(&mut s, &cellules_totaux(&g.totaux, &format!("Total {}", g.libelle), livre.coordonnees).iter().map(texte).collect::<Vec<_>>());
        ligne(&mut s, &[]);
    }
    ligne(&mut s, &cellules_totaux(&livre.totaux, "TOTAL GENERAL", livre.coordonnees).iter().map(texte).collect::<Vec<_>>());
    for a in &livre.avertissements {
        ligne(&mut s, &[]);
        ligne(&mut s, &[format!("! {}", a)]);
    }
    s.into_bytes()
}

/// `xlsx_livre` : une feuille, les colonnes en place, les montants en nombres (le comptable les additionne).
pub(crate) fn xlsx_livre(livre: &LivreDePaie) -> R<Vec<u8>> {
    use rust_decimal::prelude::ToPrimitive;
    let xl = |e: rust_xlsxwriter::XlsxError| erreur("OSError", format!("classeur : {}", e));
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_name("Livre de paie").map_err(xl)?;
    ws.write_string_with_format(0, 0, titre_livre(livre), &Format::new().set_bold().set_font_size(13.0)).map_err(xl)?;
    let cl = colonnes_livre(livre);
    let entete = Format::new().set_bold().set_text_wrap().set_align(FormatAlign::VerticalCenter);
    for (i, c) in cl.iter().enumerate() {
        ws.write_string_with_format(2, i as u16, *c, &entete).map_err(xl)?;
    }
    let mut ligne: u32 = 3;
    let ecrire_rang = |ws: &mut rust_xlsxwriter::Worksheet, cel: &[CelL], gras: bool, taille: Option<f64>, ligne: u32| -> R<()> {
        for (i, c) in cel.iter().enumerate() {
            let mut f = Format::new();
            if gras {
                f = f.set_bold();
            }
            if let Some(t) = taille {
                f = f.set_font_size(t);
            }
            match c {
                CelL::T(t) => {
                    if !t.is_empty() {
                        ws.write_string_with_format(ligne, i as u16, t, &f).map_err(xl)?;
                    }
                }
                CelL::M(None) => {}
                CelL::M(Some(d)) => {
                    let f = if MONTANTS.contains(&cl[i]) { f.set_num_format("# ##0.00 \u{20ac}") } else { f };
                    ws.write_number_with_format(ligne, i as u16, d.to_f64().unwrap_or(0.0), &f).map_err(xl)?;
                }
            }
        }
        Ok(())
    };
    for g in &livre.groupes {
        for l in &g.lignes {
            ecrire_rang(ws, &cellules(l, livre.coordonnees), false, None, ligne)?;
            ligne += 1;
        }
        ecrire_rang(ws, &cellules_totaux(&g.totaux, &format!("Total {}", g.libelle), livre.coordonnees), true, None, ligne)?;
        ligne += 2;
    }
    ecrire_rang(ws, &cellules_totaux(&livre.totaux, "TOTAL GENERAL", livre.coordonnees), true, Some(12.0), ligne)?;
    ligne += 1;
    for a in &livre.avertissements {
        ligne += 1;
        ws.write_string(ligne, 0, format!("! {}", a)).map_err(xl)?;
        ligne += 1;
    }
    for (i, nom) in cl.iter().enumerate() {
        ws.set_column_width(i as u16, if ["Salarie", "Adresse", "Spectacle"].contains(nom) { 34.0 } else { 16.0 }).map_err(xl)?;
    }
    ws.set_freeze_panes(3, 0).map_err(xl)?;
    wb.save_to_buffer().map_err(xl)
}

/// `pdf_livre` : A4 paysage, l'en-tete des colonnes a chaque page, un pied qui numerote.
pub(crate) fn pdf_livre(ctx: &Contexte, livre: &LivreDePaie) -> R<Vec<u8>> {
    let cel = |c: &CelL| -> String {
        match c {
            CelL::T(t) => t.clone(),
            CelL::M(None) => "—".to_string(),
            CelL::M(Some(d)) => d.to_string().replace('.', ","),
        }
    };
    let rangs: Vec<(Vec<String>, &str)> = rangees_livre(livre).iter().map(|(c, g)| (c.iter().map(cel).collect(), *g)).collect();
    let etiquette = |c: &str| -> String {
        match c {
            "Salarie" => "Salarié",
            "Net a payer" => "Net à payer",
            "Cout employeur" => "Coût employeur",
            autre => autre,
        }
        .to_string()
    };
    let entetes: Vec<(String, bool)> = colonnes_livre(livre).iter().map(|c| (etiquette(c), MONTANTS.contains(c))).collect();
    // `_pdf_nom` : `nom_affichage`, sinon `nom` (pas la raison sociale).
    let nom = ["nom_affichage", "nom"].iter().find_map(|c| ctx.entreprise.get(*c).filter(|v| vrai(v)).map(valeur_en_texte)).unwrap_or_default();
    let sous = format!("{} — {}{}", livre.periode, regroupement_lisible(&livre.regroupement), if livre.coordonnees { " — avec les coordonnées des salariés" } else { "" });
    let d = crate::tableaux_export::description_pdf(
        "Livre de paie",
        &livre.periode,
        &nom,
        &sous,
        &format!("Livre de paie — {}", livre.periode),
        &entetes,
        &rangs,
        &livre.avertissements,
        &[],
    );
    crate::tableaux_export::pdf_fabrique(&d)
}

// ------------------------------------------------------------------ _finir, _sections

/// `_finir` : le format, les totaux, puis -- si on ecrit -- le fichier `<nom>-<periode>.<ext>` dans le dossier
/// de la periode (celui d'`exports/` sans periode).
pub(crate) fn finir(ctx: &Contexte, t: &mut Tableau, p: Option<&Periode>, nom: &str, format: &str, ecrire: bool) -> R<()> {
    t.format = format_possible(format, &FORMATS_TABLEAU)?;
    totaliser(t, TOTAL_GENERAL)?;
    if ecrire {
        ecrire_fini(ctx, t, p, nom)?;
    }
    Ok(())
}

/// L'ecriture d'un tableau fini : `tb.ecrire(t, dossier, "<nom>-<periode>" ou "<nom>", ctx)`.
pub(crate) fn ecrire_fini(ctx: &Contexte, t: &mut Tableau, p: Option<&Periode>, nom: &str) -> R<()> {
    let (dossier, fichier) = match p {
        Some(p) => (p.dossier(ctx), format!("{}-{}", nom, p.nom())),
        None => (dossier_exports(ctx, None), nom.to_string()),
    };
    ecrire_tableau(t, &dossier, &fichier, ctx)?;
    Ok(())
}

/// Un montant d'une ligne de tableau : `Decimal` quand il y en a un.
fn dec_val(v: Option<Decimal>) -> Option<Val> {
    v.map(Val::Dec)
}

/// `_lignes_livre` : les lignes du livre de paie (saisies ET DSN), et ses avertissements -- les etats de synthese se
/// construisent dessus, pour tomber sur le livre.
pub(crate) fn lignes_livre(ctx: &Contexte, p: &Periode, avert: &mut Vec<String>) -> R<Vec<LigneLivre>> {
    let d = Demande {
        mois: p.un_mois.clone(),
        annee: p.annee.clone(),
        toutes: p.toutes,
        regroupement: "salarie".to_string(),
        format: "csv".to_string(),
        ..Demande::default()
    };
    let livre = livre_de_paie(ctx, &d)?;
    avert.extend(livre.avertissements.iter().filter(|a| !a.starts_with("Numéros de contrat")).cloned());
    Ok(livre.groupes.into_iter().flat_map(|g| g.lignes).collect())
}

/// `etat["mois"][:4]`.
fn annee_de(m: &str) -> String {
    m.chars().take(4).collect()
}

/// Les cles tri : un tuple de textes.
fn cle_tri(l: &Ligne, cles: &[&str]) -> Vec<String> {
    cles.iter().map(|c| l.texte(c)).collect()
}

// =========================================================== 1. le livre de paie detaille

/// `operations_etats.livre_detaille` : chaque bulletin ligne a ligne -- la remuneration, puis chaque cotisation,
/// base, taux et montants.
pub(crate) fn livre_detaille(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let mut avert = Vec::new();
    let mut lecture = Lecture::nouvelle(ctx);
    let mut sections = Vec::new();
    for m in ecarter_declares(&p, &mut avert, "le détail des lignes du bulletin")? {
        let paies = lecture.paies(&m)?;
        let mut ordre: Vec<(String, String, usize)> = Vec::new();
        for (i, paie) in paies.iter().enumerate() {
            ordre.push((qui(&paie.salarie)?, paie.engagement.numero.clone().unwrap_or_default(), i));
        }
        ordre.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
        for (nom, numero, i) in ordre {
            let paie = &paies[i];
            let titre = titre_spectacle(paie);
            let socle = |l: &mut Ligne, genre: &str| {
                l.t("mois", m.clone()).t("salarie", nom.clone()).t("contrat", numero.clone()).t("genre", genre);
            };
            let mut lignes = Vec::new();
            for r in &paie.remuneration {
                let mut l = Ligne::nouvelle();
                socle(&mut l, "Rémunération");
                l.t("code", r.code.clone()).t("libelle", r.libelle.clone()).d("base", r.quantite).d("taux_sal", r.taux).d("gain", eur2(r.montant));
                lignes.push(l);
            }
            for c in &paie.lignes {
                let mut l = Ligne::nouvelle();
                socle(&mut l, "Cotisation");
                l.t("code", c.id.clone()).t("libelle", c.libelle.clone()).d("base", eur2(c.base));
                l.od("taux_sal", c.taux_sal.map(dec_taux).transpose()?).d("part_sal", eur2(c.montant_sal));
                l.od("taux_pat", c.taux_pat.map(dec_taux).transpose()?).d("part_pat", eur2(c.montant_pat));
                lignes.push(l);
            }
            for (lib, v) in [("Net imposable", paie.net_imposable), ("Net à payer avant impôt", paie.net_a_payer)] {
                let mut l = Ligne::nouvelle();
                socle(&mut l, "Résultat");
                l.t("libelle", lib).d("net", eur2(v));
                lignes.push(l);
            }
            let sec = format!("{} — {}{}", nom, libelle_mois(&m)?, if titre.is_empty() { String::new() } else { format!(" — {}", titre) });
            sections.push(Section::nouvelle(sec, lignes));
        }
    }
    let mut t = Tableau::nouveau("livre-de-paie-detaille", "Livre de paie détaillé", p.libelle.clone());
    t.mois = p.mois.clone();
    t.sections = sections;
    t.avertissements = avert;
    t.colonnes = vec![
        col_t("mois", "Mois"),
        col_t("salarie", "Salarié"),
        col_t("contrat", "Contrat"),
        col_t("genre", "Ligne"),
        col_t("code", "Code"),
        col_t("libelle", "Libellé"),
        col_x("base", "Base ou quantité"),
        col_x("taux_sal", "Taux (sal.) ou unitaire"),
        col_m("gain", "Gain"),
        col_m("part_sal", "Part salariale"),
        col_x("taux_pat", "Taux patronal"),
        col_m("part_pat", "Part patronale"),
        col_m("net", "Net"),
    ];
    t.sources = vec!["le bulletin de paie tel que PuppetPaie le calcule et l'imprime (art. R3243-1 du Code du travail pour ses mentions)".to_string()];
    // Le total general du Net additionnerait net imposable et net a payer : il n'a pas de sens, il est retire apres coup.
    finir(ctx, &mut t, Some(&p), "livre-de-paie-detaille", &d.format, false)?;
    if let Some(tot) = t.total.as_mut() {
        tot.rien("net");
    }
    for s in t.sections.iter_mut() {
        if let Some(tot) = s.total.as_mut() {
            tot.rien("net");
        }
    }
    if d.ecrire {
        ecrire_fini(ctx, &mut t, Some(&p), "livre-de-paie-detaille")?;
    }
    Ok(t)
}

// ===================================================== 2. le recapitulatif annuel par salarie

/// `operations_etats.recapitulatif_annuel`.
pub(crate) fn recapitulatif_annuel(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    if d.mois.as_deref().is_some_and(|m| !m.is_empty()) {
        return Err(refus("Le récapitulatif annuel se demande pour une année, ou pour toutes les années — pas pour un mois."));
    }
    let p = Periode::nouvelle(ctx, None, d.annee.as_deref(), d.toutes, true)?;
    let mut avert = Vec::new();
    let lignes = lignes_livre(ctx, &p, &mut avert)?;
    let mut ordre: Vec<(String, String)> = Vec::new();
    let mut par: HashMap<(String, String), (Ligne, BTreeSet<&'static str>, i64)> = HashMap::new();
    let zero = zero2();
    for l in &lignes {
        let cle = (annee_de(&l.mois), l.salarie.clone());
        if !par.contains_key(&cle) {
            ordre.push(cle.clone());
            let mut n = Ligne::nouvelle();
            n.t("annee", cle.0.clone()).t("salarie", cle.1.clone()).n("bulletins", 0);
            par.insert(cle.clone(), (n, BTreeSet::new(), 0));
        }
        let (ligne, sources, n) = par.get_mut(&cle).expect("insere");
        *n += 1;
        ligne.n("bulletins", *n);
        let k = format!("m{}", l.mois.chars().skip(5).take(2).collect::<String>());
        let avant = ligne.decimal(&k).unwrap_or(zero);
        ligne.d(&k, socle::num::add(avant, l.brut.unwrap_or(zero)));
        for (champ, v) in [
            ("brut", l.brut),
            ("net_imposable", l.net_imposable),
            ("net_a_payer", l.net_a_payer),
            ("pas", l.pas),
            ("cotisations_patronales", l.cotisations_patronales),
            ("cout_employeur", l.cout_employeur),
        ] {
            if let Some(v) = v {
                let avant = ligne.decimal(champ).unwrap_or(zero);
                ligne.d(champ, socle::num::add(avant, v));
            }
        }
        sources.insert(l.source);
        ligne.poser("sources", None);
    }
    let mut a_trier = Vec::new();
    for cle in ordre {
        let (mut ligne, sources, _) = par.remove(&cle).expect("present");
        ligne.t("sources", sources.into_iter().collect::<Vec<_>>().join(" + "));
        a_trier.push(ligne);
    }
    let mut cols = vec![col_t("salarie", "Salarié")];
    for i in 1..=12usize {
        cols.push(col_m(&format!("m{:02}", i), &mois_abrege(i)));
    }
    cols.extend([
        col_m("brut", "Brut annuel"),
        col_m("net_imposable", "Net imposable"),
        col_m("pas", "PAS"),
        col_m("net_a_payer", "Net payé"),
        col_m("cout_employeur", "Coût employeur"),
        col_n("bulletins", "Bulletins"),
        col_t("sources", "Source"),
    ]);
    let mut t = Tableau::nouveau("recapitulatif-annuel", "Récapitulatif annuel par salarié", p.libelle.clone());
    t.mois = p.mois.clone();
    t.colonnes = cols;
    t.avertissements = avert;
    t.sections = sections_de(a_trier, |l| l.texte("annee"), |k| k.clone(), Some(&|l| cle_tri(l, &["salarie"])));
    t.sources = vec!["cumuls des bulletins de l'année ; le net imposable est celui que la DSN transmet à la DGFiP pour la déclaration de revenus".to_string()];
    finir(ctx, &mut t, Some(&p), "recapitulatif-annuel", &d.format, d.ecrire)?;
    Ok(t)
}

// ================================================== 3. la masse salariale par mois

/// `operations_etats.masse_salariale` : un mois par ligne -- ce que le comptable rapproche du compte 641.
pub(crate) fn masse_salariale(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let mut avert = Vec::new();
    let mut ordre: Vec<String> = Vec::new();
    let mut par: HashMap<String, (Ligne, BTreeSet<String>, i64)> = HashMap::new();
    let zero = zero2();
    for l in lignes_livre(ctx, &p, &mut avert)? {
        if !par.contains_key(&l.mois) {
            ordre.push(l.mois.clone());
            let mut n = Ligne::nouvelle();
            n.t("mois", l.mois.clone()).t("libelle", libelle_mois(&l.mois)?).n("bulletins", 0).t("source", l.source);
            par.insert(l.mois.clone(), (n, BTreeSet::new(), 0));
        }
        let (ligne, salaries, n) = par.get_mut(&l.mois).expect("insere");
        salaries.insert(if l.cle_salarie.is_empty() { l.salarie.clone() } else { l.cle_salarie.clone() });
        *n += 1;
        ligne.n("bulletins", *n);
        for (champ, v) in [
            ("brut", l.brut),
            ("cotisations_salariales", l.cotisations_salariales),
            ("cotisations_patronales", l.cotisations_patronales),
            ("net_imposable", l.net_imposable),
            ("net_a_payer", l.net_a_payer),
            ("pas", l.pas),
            ("cout_employeur", l.cout_employeur),
        ] {
            if let Some(v) = v {
                let avant = ligne.decimal(champ).unwrap_or(zero);
                ligne.d(champ, socle::num::add(avant, v));
            }
        }
    }
    let mut a_trier = Vec::new();
    for m in ordre {
        let (mut ligne, salaries, _) = par.remove(&m).expect("present");
        ligne.n("salaries", salaries.len() as i64);
        a_trier.push(ligne);
    }
    let mut t = Tableau::nouveau("masse-salariale", "Masse salariale par mois", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.sections = sections_de(a_trier, |l| annee_de(&l.texte("mois")), |k| k.clone(), Some(&|l| cle_tri(l, &["mois"])));
    t.colonnes = vec![
        col_t("libelle", "Mois"),
        col_n("salaries", "Salariés"),
        col_n("bulletins", "Bulletins"),
        col_m("brut", "Brut"),
        col_m("cotisations_salariales", "Cot. salariales"),
        col_m("cotisations_patronales", "Cot. patronales"),
        col_m("net_imposable", "Net imposable"),
        col_m("pas", "PAS"),
        col_m("net_a_payer", "Net à payer"),
        col_m("cout_employeur", "Coût employeur"),
        col_t("source", "Source"),
    ];
    t.sources = vec!["cumuls des bulletins du mois".to_string()];
    finir(ctx, &mut t, Some(&p), "masse-salariale", &d.format, false)?;
    // Des salaries distincts ne s'additionnent pas d'un mois a l'autre.
    for s in t.sections.iter_mut() {
        if let Some(tot) = s.total.as_mut() {
            tot.rien("salaries");
        }
    }
    if let Some(tot) = t.total.as_mut() {
        tot.rien("salaries");
    }
    if d.ecrire {
        ecrire_fini(ctx, &mut t, Some(&p), "masse-salariale")?;
    }
    Ok(t)
}

// ===================================================== 4. l'etat des virements

/// `operations_etats.etat_des_virements` : le net a verser, salarie par salarie et mois par mois.
pub(crate) fn etat_des_virements(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let mut avert = Vec::new();
    let mut verse: HashMap<(Option<String>, Option<String>), Option<String>> = HashMap::new();
    for x in paies_declarees(ctx)? {
        let texte = |k: &str| x.get(k).filter(|v| !v.is_null()).map(valeur_en_texte);
        verse.insert((texte("mois"), texte("contrat")), x.get("verse_le").filter(|v| vrai(v)).map(valeur_en_texte));
    }
    let mut lignes = Vec::new();
    for l in lignes_livre(ctx, &p, &mut avert)? {
        let v = verse.get(&(Some(l.mois.clone()), Some(l.contrat.clone()))).cloned().flatten().unwrap_or_default();
        let mut x = Ligne::nouvelle();
        x.t("mois", l.mois.clone()).t("salarie", l.salarie.clone()).t("contrat", l.contrat.clone()).t("spectacle", l.spectacle.clone());
        x.od("net_a_payer", l.net_a_payer).t("verse_le", jj(&v)).t("source", l.source);
        lignes.push(x);
    }
    let saisis = p.saisis();
    if !saisis.is_empty() {
        avert.push(format!("Mois saisis ({}) : la date du virement n'est pas dans PuppetPaie, la colonne reste vide.", saisis.join(", ")));
    }
    let mut t = Tableau::nouveau("etat-des-virements", "État des virements de salaires", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.sections = sections_de(lignes, |l| l.texte("mois"), |k| libelle_mois(k).unwrap_or_else(|_| k.clone()), Some(&|l| cle_tri(l, &["salarie", "contrat"])));
    t.colonnes = vec![
        col_t("mois", "Mois"),
        col_t("salarie", "Salarié"),
        col_t("contrat", "Contrat"),
        col_t("spectacle", "Spectacle"),
        col_m("net_a_payer", "Net à payer"),
        col_t("verse_le", "Versé le"),
        col_t("source", "Source"),
    ];
    t.sources = vec!["net à payer du bulletin ; date de versement : DSN, rubrique S21.G00.50.001".to_string()];
    finir(ctx, &mut t, Some(&p), "etat-des-virements", &d.format, d.ecrire)?;
    Ok(t)
}

// ================================================ 5. le prelevement a la source

/// `pas.TYPES` : les types de taux.
fn type_de_taux(code: &str) -> &'static str {
    match code {
        "01" => "Taux transmis par la DGFIP",
        "13" => "Bareme mensuel metropole",
        "17" => "Bareme mathematique sur base mensuelle metropole",
        "23" => "Bareme mensuel Guadeloupe, Reunion et Martinique",
        "27" => "Bareme mathematique sur base mensuelle Guadeloupe, Reunion, Martinique",
        "33" => "Bareme mensuel Guyane et Mayotte",
        "37" => "Bareme mathematique sur base mensuelle Guyane et Mayotte",
        "99" => "Indu relatif a un exercice anterieur - pas de taux de PAS",
        _ => "",
    }
}

/// `operations_etats.prelevement_a_la_source` : ce qui a ete retenu au titre de l'impot sur le revenu.
pub(crate) fn prelevement_a_la_source(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let (mut avert, mut lignes, mut sans) = (Vec::new(), Vec::new(), Vec::new());
    let mut lecture = Lecture::nouvelle(ctx);
    let coord = d.coordonnees;
    for m in p.saisis() {
        let r_pas = lecture.pas(&m)?;
        let paies = lecture.paies(&m)?;
        for (paie, r) in paies.iter().zip(r_pas.iter()) {
            if r.is_none() {
                sans.push(qui(&paie.salarie)?);
            }
            let mut l = Ligne::nouvelle();
            l.t("mois", m.clone()).t("salarie", qui(&paie.salarie)?).t("contrat", paie.engagement.numero.clone().unwrap_or_default());
            l.t("nir", if coord { fiche_texte(&paie.salarie, "nir") } else { String::new() });
            l.d("net_imposable", eur2(paie.net_imposable));
            match r {
                Some(v) => {
                    l.d("abattement", v.abattement).d("assiette", v.montant_soumis);
                    let code = valeur_en_texte(&v.type_taux);
                    l.t("type", format!("{} — {}", code, type_de_taux(&code)));
                    l.d("taux", socle::num::dec(&v.taux).map_err(|m| erreur("InvalidOperation", m))?).d("prelevement", eur2(v.prelevement));
                }
                None => {
                    l.rien("abattement").rien("assiette").t("type", "").rien("taux").rien("prelevement");
                }
            }
            l.t("source", SAISIE);
            lignes.push(l);
        }
    }
    let declares = p.declares();
    for x in paies_declarees(ctx)? {
        let m = x.get("mois").map(valeur_en_texte).unwrap_or_default();
        if !declares.contains(&m) {
            continue;
        }
        let cle = fiche_texte(&x, "salarie");
        let fiche = ctx.salaries.get(&cle).filter(|f| vrai(f));
        let mut l = Ligne::nouvelle();
        l.t("mois", m.clone());
        l.t("salarie", match fiche {
            Some(f) => qui(f)?,
            None => x.get("salarie").map(valeur_en_texte).unwrap_or_default(),
        });
        l.t("contrat", fiche_texte(&x, "contrat")).t("nir", if coord { fiche_texte(&x, "nir") } else { String::new() });
        l.od("net_imposable", montant(x.get("net_imposable"))?).od("prelevement", montant(x.get("pas"))?).t("source", DSN);
        lignes.push(l);
    }
    if !sans.is_empty() {
        let uniques: BTreeSet<&String> = sans.iter().collect();
        avert.push(format!(
            "Prélèvement inconnu pour {} : aucun taux DGFiP au magasin et le barème ne porte pas la grille par défaut. La ligne reste vide.",
            uniques.into_iter().cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    if !declares.is_empty() {
        avert.push("Mois d'avant PuppetPaie : la DSN déclare le net imposable et le montant prélevé ; le taux, l'abattement et l'assiette n'ont pas été importés, ces colonnes restent vides.".to_string());
    }
    let mut cols = vec![col_t("mois", "Mois"), col_t("salarie", "Salarié"), col_t("contrat", "Contrat")];
    if coord {
        cols.push(col_t("nir", "NIR"));
    }
    cols.extend([
        col_m("net_imposable", "Net imposable"),
        col_m("abattement", "Abattement"),
        col_m("assiette", "Assiette PAS"),
        col_t("type", "Type de taux"),
        col_x("taux", "Taux %"),
        col_m("prelevement", "Prélevé"),
        col_t("source", "Source"),
    ]);
    let mut t = Tableau::nouveau("prelevement-a-la-source", "Prélèvement à la source", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.colonnes = cols;
    t.donnees_personnelles = coord;
    t.sections = sections_de(lignes, |l| l.texte("mois"), |k| libelle_mois(k).unwrap_or_else(|_| k.clone()), Some(&|l| cle_tri(l, &["salarie", "contrat"])));
    t.sources = vec!["art. 204 A et suivants du CGI ; abattement des contrats courts : BOI-IR-PAS-20-20-30-10 ; types de taux : cahier technique DSN 2026.1, rubrique S21.G00.50.007".to_string()];
    finir(ctx, &mut t, Some(&p), "prelevement-a-la-source", &d.format, d.ecrire)?;
    Ok(t)
}

// ================================================ 9. la reduction generale

/// `Decimal(str(x or 0))` d'un montant de la reduction generale : un zero se lit `0`.
fn ou_zero(x: Decimal) -> Decimal {
    if x.is_zero() {
        Decimal::ZERO
    } else {
        x
    }
}

/// `operations_etats.reduction_generale`.
pub(crate) fn reduction_generale(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let (mut avert, mut lignes) = (Vec::new(), Vec::new());
    let mut lecture = Lecture::nouvelle(ctx);
    for m in ecarter_declares(&p, &mut avert, "la réduction générale")? {
        for paie in lecture.paies(&m)?.iter() {
            let Some(rg) = &paie.reduction_generale else { continue };
            let mut l = Ligne::nouvelle();
            l.t("mois", m.clone()).t("salarie", qui(&paie.salarie)?).t("contrat", paie.engagement.numero.clone().unwrap_or_default());
            l.d("brut", eur2(paie.brut)).d("heures", ou_zero(rg.heures)).d("smic", eur2(rg.smic_reference)).d("coefficient", ou_zero(rg.coefficient));
            l.d("montant", eur2(rg.montant)).d("urssaf", eur2(rg.urssaf)).d("chomage", eur2(rg.chomage)).d("retraite", eur2(rg.agirc_arrco));
            lignes.push(l);
        }
    }
    if lignes.is_empty() {
        avert.push("Aucun bulletin de la période ne porte la réduction générale (elle ne s'applique pas aux artistes du spectacle).".to_string());
    }
    let mut t = Tableau::nouveau("reduction-generale", "Réduction générale des cotisations", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.sections = sections_de(lignes, |l| l.texte("mois"), |k| libelle_mois(k).unwrap_or_else(|_| k.clone()), Some(&|l| cle_tri(l, &["salarie"])));
    t.colonnes = vec![
        col_t("mois", "Mois"),
        col_t("salarie", "Salarié"),
        col_t("contrat", "Contrat"),
        col_m("brut", "Brut"),
        col_n("heures", "Heures"),
        col_m("smic", "SMIC de référence"),
        col_x("coefficient", "Coefficient"),
        col_m("montant", "Réduction"),
        col_m("urssaf", "dont Urssaf"),
        col_m("chomage", "dont chômage"),
        col_m("retraite", "dont retraite"),
    ];
    t.sources = vec!["art. L241-13 et D241-7 du Code de la sécurité sociale ; le calcul est celui du bulletin (calcul.py)".to_string()];
    finir(ctx, &mut t, Some(&p), "reduction-generale", &d.format, false)?;
    // Un SMIC de reference ne s'additionne pas.
    for tot in t.sections.iter_mut().map(|s| &mut s.total).chain(std::iter::once(&mut t.total)) {
        if let Some(x) = tot.as_mut() {
            x.rien("smic");
        }
    }
    if d.ecrire {
        ecrire_fini(ctx, &mut t, Some(&p), "reduction-generale")?;
    }
    Ok(t)
}

// ===================================================== 10. les assiettes

const ASSIETTES: [(&str, &str); 10] = [
    ("brut", "Brut"),
    ("brut_cs", "Assiette Congés Spectacles"),
    ("csg", "Assiette CSG/CRDS"),
    ("brut_x115", "Brut majoré 115 %"),
    ("brut_x110", "Brut majoré 110 %"),
    ("t1", "Tranche 1"),
    ("t2", "Tranche 2"),
    ("apec", "Assiette APEC"),
    ("cet", "Assiette CET"),
    ("brut_dfs", "Brut après DFS"),
];

/// `operations_etats.assiettes`.
pub(crate) fn assiettes(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let (mut avert, mut lignes) = (Vec::new(), Vec::new());
    let mut lecture = Lecture::nouvelle(ctx);
    let mut vues: BTreeSet<String> = BTreeSet::new();
    for m in ecarter_declares(&p, &mut avert, "les assiettes de cotisation")? {
        for paie in lecture.paies(&m)?.iter() {
            let mut l = Ligne::nouvelle();
            l.t("mois", m.clone()).t("salarie", qui(&paie.salarie)?).t("contrat", paie.engagement.numero.clone().unwrap_or_default());
            l.d("plafond", eur2(paie.plafond));
            for (k, v) in &paie.bases {
                l.d(&format!("b_{}", k), eur2(*v));
                vues.insert(k.clone());
            }
            if !paie.bases.contains_key("csg") {
                l.d("b_csg", eur2(paie.base_csg));
                vues.insert("csg".to_string());
            }
            lignes.push(l);
        }
    }
    let connus: Vec<&str> = ASSIETTES.iter().map(|(k, _)| *k).collect();
    let mut cols = vec![col_t("mois", "Mois"), col_t("salarie", "Salarié"), col_t("contrat", "Contrat")];
    for (k, lib) in ASSIETTES {
        if vues.contains(k) {
            cols.push(col_m(&format!("b_{}", k), lib));
        }
    }
    for k in &vues {
        if !connus.contains(&k.as_str()) {
            cols.push(col_m(&format!("b_{}", k), k));
        }
    }
    cols.push(col_x("plafond", "Plafond appliqué"));
    let mut t = Tableau::nouveau("assiettes", "Assiettes de cotisation", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.colonnes = cols;
    t.sections = sections_de(lignes, |l| l.texte("mois"), |k| libelle_mois(k).unwrap_or_else(|_| k.clone()), Some(&|l| cle_tri(l, &["salarie", "contrat"])));
    t.sources = vec![
        "assiettes du calcul, déclarées telles quelles en DSN (S21.G00.78) ; plafond : arrêté annuel du plafond de la sécurité sociale, proratisé comme le bulletin l'applique".to_string(),
    ];
    finir(ctx, &mut t, Some(&p), "assiettes", &d.format, d.ecrire)?;
    Ok(t)
}

// =============================================== 11. les heures et les cachets

/// `int(x)` d'un nombre d'une ligne : `ValueError` quand le texte ne se lit pas.
fn entier_py(v: &Val) -> R<i64> {
    let t = v.str_py();
    if let Val::Num(n) = v {
        if let Some(f) = n.as_f64() {
            return Ok(f.trunc() as i64);
        }
    }
    crate::apercu_dsn::int_py(t.trim()).and_then(|n| i64::try_from(n).ok()).ok_or_else(|| erreur("ValueError", format!("nombre entier illisible : {}", socle::fichiers::repr(&t))))
}

/// `datetime.date(*map(int, "JJ/MM/AAAA".split("/")[::-1]))` : `None` quand le texte ne se lit pas.
fn date_jj(t: &str) -> Option<socle::date::Jour> {
    let p: Vec<&str> = t.split('/').collect();
    if p.len() != 3 {
        return None;
    }
    let n = |x: &str| x.trim().parse::<i64>().ok();
    socle::date::Jour::nouveau(n(p[2])? as i32, n(p[1])? as u32, n(p[0])? as u32)
}

/// `operations_etats.heures_et_cachets` : contrat par contrat, cachets, heures retenues pour les annexes 8 et 10,
/// jours travailles, brut. Un mois saisi se lit sur la paie ; un mois d'avant PuppetPaie, sur l'AEM attestee.
pub(crate) fn heures_et_cachets(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, false)?;
    let (mut avert, mut lignes) = (Vec::new(), Vec::new());
    let mut lecture = Lecture::nouvelle(ctx);
    let saisis: BTreeSet<String> = p.saisis().into_iter().collect();
    for m in p.saisis() {
        for paie in lecture.paies(&m)?.iter() {
            let e = &paie.engagement;
            let cachets = paie.remuneration.iter().filter(|r| r.unite == "cachet").fold(Decimal::ZERO, |a, r| a + r.quantite);
            let heures = paie.remuneration.iter().fold(Decimal::ZERO, |a, r| a + r.heures_annexes);
            let premiere = e.dates_travail.first().map(|j| j.iso()).unwrap_or_default();
            let derniere = e.dates_travail.last().map(|j| j.iso()).unwrap_or_default();
            let emploi = match e.emploi.clone().filter(|x| !x.is_empty()) {
                Some(x) => x,
                None => fiche_texte(&paie.salarie, "emploi"),
            };
            let debut = e.debut.iso();
            let fin = match e.fin {
                Some(f) => f.iso(),
                None => derniere,
            };
            let _ = premiere;
            let mut l = Ligne::nouvelle();
            l.t("mois", m.clone()).t("salarie", qui(&paie.salarie)?).t("contrat", e.numero.clone().unwrap_or_default());
            l.t("spectacle", titre_spectacle(paie)).t("emploi", emploi).t("debut", jj(&debut)).t("fin", jj(&fin));
            l.d("cachets", cachets).d("heures", heures).n("jours", e.dates_travail.len() as i64).d("brut", eur2(paie.brut)).t("source", SAISIE);
            lignes.push(l);
        }
    }
    // L'AEM attestee, pour les mois d'avant PuppetPaie.
    let mut objets: Vec<(Value, Value)> = Vec::new();
    for s in ctx.spectacles.values() {
        let Value::Object(o) = s else {
            return Err(erreur("AttributeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(s))));
        };
        objets.push((o.get("numero_objet").cloned().unwrap_or(Value::Null), o.get("titre").cloned().unwrap_or(Value::Null)));
    }
    let compte = |v: Option<&Value>| -> R<Decimal> {
        let t = match v.filter(|v| vrai(v)) {
            Some(x) => valeur_en_texte(x),
            None => "0".to_string(),
        };
        crate::apercu_dsn::decimal_py(&t).ok_or_else(|| erreur("InvalidOperation", socle::num::illisible(&t)))
    };
    for x in liste_historique(ctx, "engagements.json", "engagements")? {
        let debut = fiche_texte(&x, "debut");
        let m: String = debut.chars().take(7).collect();
        if m.is_empty() || saisis.contains(&m) || !p.contient(&m) {
            continue;
        }
        let cle = fiche_texte(&x, "salarie");
        let fiche = if cle.is_empty() { None } else { ctx.salaries.get(&cle).filter(|f| vrai(f)) };
        let objet = x.get("numero_objet").cloned().unwrap_or(Value::Null);
        let titre = objets.iter().rev().find(|(n, _)| *n == objet).map(|(_, t)| t.clone()).filter(vrai);
        let spectacle = match titre {
            Some(t) => valeur_en_texte(&t),
            None => {
                if vrai(&objet) {
                    valeur_en_texte(&objet)
                } else {
                    String::new()
                }
            }
        };
        let mut l = Ligne::nouvelle();
        l.t("mois", m).t("salarie", match fiche {
            Some(f) => qui(f)?,
            None => x.get("salarie").map(valeur_en_texte).unwrap_or_default(),
        });
        l.t("contrat", fiche_texte(&x, "numero_attestation")).t("spectacle", spectacle);
        l.t("emploi", fiche.map(|f| fiche_texte(f, "emploi")).unwrap_or_default());
        l.t("debut", jj(&x.get("debut").map(valeur_en_texte).unwrap_or_default())).t("fin", jj(&x.get("fin").map(valeur_en_texte).unwrap_or_default()));
        l.d("cachets", compte(x.get("cachets"))?).d("heures", compte(x.get("heures"))?);
        l.poser(
            "jours",
            match x.get("jours") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) => Some(Val::Txt(s.clone())),
                Some(n) => Some(Val::Num(n.clone())),
            },
        );
        l.od("brut", montant(x.get("brut"))?).t("source", AEM);
        lignes.push(l);
    }
    // UNE DONNEE IMPOSSIBLE SE DIT, elle ne se corrige pas ici.
    for l in &lignes {
        let (Some(fin), Some(debut)) = (date_jj(&l.texte("fin")), date_jj(&l.texte("debut"))) else { continue };
        let duree = fin.moins(&debut) + 1;
        if let Some(jours) = l.val("jours") {
            let vrai_jours = match jours {
                Val::Txt(s) => !s.is_empty(),
                Val::Num(n) => vrai(n),
                Val::Dec(x) => !x.is_zero(),
            };
            if vrai_jours && entier_py(jours)? > duree {
                avert.push(format!(
                    "{}, contrat du {} au {} : {} jours attestés pour {} jours de contrat — la donnée importée est à vérifier ({}).",
                    l.texte("salarie"),
                    l.texte("debut"),
                    l.texte("fin"),
                    jours.str_py(),
                    duree,
                    l.texte("source")
                ));
            }
        }
    }
    if lignes.is_empty() {
        return Err(refus(format!("Aucun contrat sur la période ({}), ni saisi ni attesté.", p.libelle)));
    }
    if lignes.iter().any(|l| l.texte("source") == AEM) {
        avert.push("Lignes « AEM attestée » : la colonne Contrat porte le numéro d'attestation France Travail, et les heures sont celles que l'AEM a déclarées.".to_string());
    }
    let mut t = Tableau::nouveau("heures-et-cachets", "Heures et cachets (annexes 8 et 10)", p.libelle.clone());
    t.mois = lignes.iter().map(|l| l.texte("mois")).collect::<BTreeSet<_>>().into_iter().collect();
    t.avertissements = avert;
    t.sections = sections_de(lignes, |l| l.texte("salarie"), |k| k.clone(), Some(&|l| cle_tri(l, &["mois", "debut", "contrat"])));
    t.colonnes = vec![
        col_t("mois", "Mois"),
        col_t("salarie", "Salarié"),
        col_t("contrat", "Contrat"),
        col_t("spectacle", "Spectacle"),
        col_t("emploi", "Emploi"),
        col_t("debut", "Début"),
        col_t("fin", "Fin"),
        col_n("cachets", "Cachets"),
        col_n("heures", "Heures (annexes)"),
        col_n("jours", "Jours"),
        col_m("brut", "Brut"),
        col_t("source", "Source"),
    ];
    t.sources = vec![
        "règlement d'assurance chômage, annexes VIII et X (décret n° 2019-797 modifié) : un cachet compte pour les heures que la rubrique déclare (heures_annexes) ; AEM : attestation employeur mensuelle déposée à France Travail".to_string(),
    ];
    finir(ctx, &mut t, Some(&p), "heures-et-cachets", &d.format, d.ecrire)?;
    Ok(t)
}

// =============================================== 12. le cout par spectacle

/// `operations_etats.cout_par_spectacle` : chaque spectacle, salarie par salarie.
pub(crate) fn cout_par_spectacle(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let mut avert = Vec::new();
    let mut ordre: Vec<(String, String)> = Vec::new();
    let mut par: HashMap<(String, String), (Ligne, BTreeSet<&'static str>, i64)> = HashMap::new();
    let zero = zero2();
    for l in lignes_livre(ctx, &p, &mut avert)? {
        let sp = if l.spectacle.is_empty() { "(sans spectacle)".to_string() } else { l.spectacle.clone() };
        let cle = (sp.clone(), l.salarie.clone());
        if !par.contains_key(&cle) {
            ordre.push(cle.clone());
            let mut n = Ligne::nouvelle();
            n.t("spectacle", sp).t("salarie", l.salarie.clone()).n("bulletins", 0);
            par.insert(cle.clone(), (n, BTreeSet::new(), 0));
        }
        let (ligne, sources, n) = par.get_mut(&cle).expect("insere");
        *n += 1;
        ligne.n("bulletins", *n);
        sources.insert(l.source);
        for (champ, v) in [("brut", l.brut), ("cotisations_patronales", l.cotisations_patronales), ("cout_employeur", l.cout_employeur), ("net_a_payer", l.net_a_payer)] {
            if let Some(v) = v {
                let avant = ligne.decimal(champ).unwrap_or(zero);
                ligne.d(champ, socle::num::add(avant, v));
            }
        }
    }
    let mut a_trier = Vec::new();
    for cle in ordre {
        let (mut ligne, sources, _) = par.remove(&cle).expect("present");
        ligne.t("source", sources.into_iter().collect::<Vec<_>>().join(" + "));
        a_trier.push(ligne);
    }
    let mut t = Tableau::nouveau("cout-par-spectacle", "Coût employeur par spectacle", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.sections = sections_de(a_trier, |l| l.texte("spectacle"), |k| k.clone(), Some(&|l| cle_tri(l, &["salarie"])));
    t.colonnes = vec![
        col_t("spectacle", "Spectacle"),
        col_t("salarie", "Salarié"),
        col_n("bulletins", "Bulletins"),
        col_m("brut", "Brut"),
        col_m("cotisations_patronales", "Charges patronales"),
        col_m("cout_employeur", "Coût employeur"),
        col_m("net_a_payer", "Net versé"),
        col_t("source", "Source"),
    ];
    t.sources = vec!["bulletins rattachés à leur spectacle par la saisie ou, avant PuppetPaie, par le numéro d'objet déclaré en DSN (S21.G00.40.050)".to_string()];
    finir(ctx, &mut t, Some(&p), "cout-par-spectacle", &d.format, d.ecrire)?;
    Ok(t)
}

#[allow(dead_code)]
fn _inutilises() {
    let _ = nom_compagnie;
    let _ = dec_val;
    let _ = json!(null);
}
