//! Les cumuls des annexes 8 et 10 : ce qui s'accumule d'un bulletin a l'autre. Les heures et cachets declares, par
//! fenetre (mois, annee civile, 12 mois), et ce qui manque aux 507 heures. Le droit compte ce qu'on lui passe (les
//! lignes declarees, `Declare`) ; les lire dans les paies et l'historique est l'affaire de l'engagement.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Map, Value};

use crate::bareme::canon;
use crate::catalogue::{annexes, rubrique};
use crate::contexte::{aujourdhui, Contexte};
use socle::date::{d, Jour};
use socle::fichiers::repr;
use socle::num::{add, flottant, nombre, Decimal, Resultat};

/// Les constantes du decompte, lues dans les donnees livrees (le catalogue des rubriques et `annexes`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Constantes {
    /// Heures qu'un cachet fait compter (12).
    pub heures_par_cachet: i64,
    /// Cachets convertis en heures, par mois (28).
    pub cachets_par_mois: i64,
    /// Seuil d'ouverture des droits (507).
    pub seuil_heures: i64,
    /// Fenetre des droits, en jours (365).
    pub fenetre_jours: i64,
}

fn entier_de(v: &Value) -> i64 {
    match v {
        Value::Number(n) => n.as_i64().unwrap_or_else(|| n.as_f64().unwrap_or(0.0) as i64),
        Value::String(s) => s.trim().parse().unwrap_or(0),
        _ => 0,
    }
}

impl Constantes {
    pub fn lire(racine: &Path, jour: &Jour) -> Resultat<Constantes> {
        let cachet = rubrique(racine, "cachet", jour)?;
        let a = annexes()?;
        Ok(Constantes {
            heures_par_cachet: cachet.heures_forfait.unwrap_or(0.0) as i64,
            cachets_par_mois: cachet.plafond_mensuel.unwrap_or(0),
            seuil_heures: entier_de(&a["seuil_heures"]),
            fenetre_jours: entier_de(&a["fenetre_jours"]),
        })
    }
}

/// `plafond_enseignement` : 70 h, ou 120 h a partir de 50 ans a la date
/// donnee ; sans date de naissance, le plafond bas.
pub fn plafond_enseignement(racine: &Path, date_naissance: Option<Jour>, a_la_date: Jour) -> Resultat<i64> {
    let r = rubrique(racine, "enseignement", &a_la_date)?;
    let p = r.plafond_annuel_heures.filter(|p| socle::fichiers::vrai(p)).unwrap_or(Value::Object(Map::new()));
    let bas = p.get("moins_de_50_ans").map_or(0, entier_de);
    let haut = p.get("50_ans_et_plus").map_or(bas, entier_de);
    let Some(n) = date_naissance else { return Ok(bas) };
    let avant = (a_la_date.mois, a_la_date.jour) < (n.mois, n.jour);
    let age = a_la_date.an - n.an - i32::from(avant);
    Ok(if age >= 50 { haut } else { bas })
}

/// Une ligne declaree : un contrat de l'historique ou un mois de nos paies.
#[derive(Debug, Clone, PartialEq)]
pub struct Declare {
    pub salarie: String,
    pub debut: Jour,
    pub fin: Jour,
    pub numero_objet: Value,
    pub heures: i64,
    pub cachets: i64,
    pub jours: i64,
    pub brut: Decimal,
    /// Part de `heures` qui est de l'enseignement (dedans, pas en plus).
    pub heures_enseignement: i64,
}

impl Declare {
    /// Le mois de la fin, auquel se rattachent les cachets.
    pub fn mois(&self) -> String {
        self.fin.mois_iso()
    }

    pub fn heures_equivalentes(&self, c: &Constantes) -> i64 {
        self.heures + self.cachets * c.heures_par_cachet
    }

    /// Tout sauf l'enseignement.
    pub fn heures_montant(&self, c: &Constantes) -> i64 {
        self.heures_equivalentes(c) - self.heures_enseignement
    }

    pub fn canon(&self, c: &Constantes) -> Value {
        let mut m = Map::new();
        m.insert("salarie".into(), Value::String(self.salarie.clone()));
        m.insert("debut".into(), Value::String(self.debut.iso()));
        m.insert("fin".into(), Value::String(self.fin.iso()));
        m.insert("numero_objet".into(), canon(&self.numero_objet));
        m.insert("heures".into(), entier(self.heures));
        m.insert("cachets".into(), entier(self.cachets));
        m.insert("jours".into(), entier(self.jours));
        m.insert("brut".into(), Value::String(nombre(self.brut)));
        m.insert("heures_enseignement".into(), entier(self.heures_enseignement));
        m.insert("mois".into(), Value::String(self.mois()));
        m.insert("heures_equivalentes".into(), entier(self.heures_equivalentes(c)));
        m.insert("heures_montant".into(), entier(self.heures_montant(c)));
        Value::Object(m)
    }
}

fn entier(n: i64) -> Value {
    Value::String(n.to_string())
}

/// Etat d'un salarie sur une fenetre.
#[derive(Debug, Clone)]
pub struct Position {
    pub constantes: Constantes,
    pub racine: std::path::PathBuf,
    pub salarie: String,
    pub debut: Jour,
    pub fin: Jour,
    pub libelle_fenetre: String,
    pub engagements: Vec<Declare>,
    /// Pour le plafond d'enseignement (age a la fin de la fenetre).
    pub date_naissance: Option<Jour>,
}

impl Position {
    pub fn cachets(&self) -> i64 {
        self.engagements.iter().map(|e| e.cachets).sum()
    }

    pub fn heures(&self) -> i64 {
        self.engagements.iter().map(|e| e.heures).sum()
    }

    /// Le total declare, enseignement compris.
    pub fn heures_equivalentes(&self) -> i64 {
        self.engagements.iter().map(|e| e.heures_equivalentes(&self.constantes)).sum()
    }

    /// `{mois: cachets declares}`.
    pub fn cachets_par_mois(&self) -> BTreeMap<String, i64> {
        let mut out = BTreeMap::new();
        for e in &self.engagements {
            *out.entry(e.mois()).or_insert(0) += e.cachets;
        }
        out
    }

    /// Les cachets que les annexes convertissent : 28 par mois au plus.
    pub fn cachets_retenus(&self) -> i64 {
        self.cachets_par_mois().values().map(|n| (*n).min(self.constantes.cachets_par_mois)).sum()
    }

    pub fn cachets_perdus(&self) -> i64 {
        self.cachets() - self.cachets_retenus()
    }

    /// Les mois ou le plafond a mordu.
    pub fn mois_plafonnes(&self) -> Vec<String> {
        self.cachets_par_mois()
            .into_iter()
            .filter(|(_, n)| *n > self.constantes.cachets_par_mois)
            .map(|(m, _)| m)
            .collect()
    }

    pub fn heures_retenues(&self) -> i64 {
        self.heures() + self.cachets_retenus() * self.constantes.heures_par_cachet
    }

    pub fn heures_enseignement(&self) -> i64 {
        self.engagements.iter().map(|e| e.heures_enseignement).sum()
    }

    pub fn plafond_enseignement(&self) -> Resultat<i64> {
        plafond_enseignement(&self.racine, self.date_naissance, self.fin)
    }

    pub fn heures_enseignement_retenues(&self) -> Resultat<i64> {
        Ok(self.heures_enseignement().min(self.plafond_enseignement()?))
    }

    /// Les heures du MONTANT de l'ARE : tout sauf l'enseignement.
    pub fn heures_montant(&self) -> i64 {
        self.heures_retenues() - self.heures_enseignement()
    }

    /// Les heures qui comptent pour les 507 (l'enseignement y entre, plafonne).
    pub fn heures_ouverture(&self) -> Resultat<i64> {
        Ok(self.heures_montant() + self.heures_enseignement_retenues()?)
    }

    pub fn enseignement_perdu(&self) -> Resultat<i64> {
        Ok(self.heures_enseignement() - self.heures_enseignement_retenues()?)
    }

    pub fn jours(&self) -> i64 {
        self.engagements.iter().map(|e| e.jours).sum()
    }

    pub fn brut(&self) -> Decimal {
        self.engagements.iter().fold(Decimal::new(0, 2), |a, e| add(a, e.brut))
    }

    /// Heures restant a acquerir pour atteindre le seuil ; 0 si atteint.
    pub fn manque(&self) -> Resultat<i64> {
        Ok((self.constantes.seuil_heures - self.heures_ouverture()?).max(0))
    }

    pub fn atteint(&self) -> Resultat<bool> {
        Ok(self.heures_ouverture()? >= self.constantes.seuil_heures)
    }

    /// Jours restant avant la fin de la fenetre ; negatif si elle est passee.
    pub fn jours_restants(&self, aujourdhui: Jour) -> i64 {
        self.fin.moins(&aujourdhui)
    }

    pub fn cachets_manquants(&self) -> Resultat<i64> {
        let h = self.constantes.heures_par_cachet;
        Ok(-((-self.manque()?).div_euclid(h)))
    }

    /// La forme canonique de la position : ses lignes et tous ses comptes.
    pub fn canon(&self, aujourdhui: Jour) -> Resultat<Value> {
        let c = &self.constantes;
        let mut m = Map::new();
        m.insert("salarie".into(), Value::String(self.salarie.clone()));
        m.insert("debut".into(), Value::String(self.debut.iso()));
        m.insert("fin".into(), Value::String(self.fin.iso()));
        m.insert("libelle_fenetre".into(), Value::String(self.libelle_fenetre.clone()));
        m.insert("engagements".into(), Value::Array(self.engagements.iter().map(|e| e.canon(c)).collect()));
        m.insert("date_naissance".into(), self.date_naissance.map_or(Value::Null, |j| Value::String(j.iso())));
        m.insert("cachets".into(), entier(self.cachets()));
        m.insert("heures".into(), entier(self.heures()));
        m.insert("heures_equivalentes".into(), entier(self.heures_equivalentes()));
        let par_mois: Map<String, Value> = self.cachets_par_mois().into_iter().map(|(k, n)| (k, entier(n))).collect();
        m.insert("cachets_par_mois".into(), Value::Object(par_mois));
        m.insert("cachets_retenus".into(), entier(self.cachets_retenus()));
        m.insert("cachets_perdus".into(), entier(self.cachets_perdus()));
        m.insert("mois_plafonnes".into(), Value::Array(self.mois_plafonnes().into_iter().map(Value::String).collect()));
        m.insert("heures_retenues".into(), entier(self.heures_retenues()));
        m.insert("heures_enseignement".into(), entier(self.heures_enseignement()));
        m.insert("plafond_enseignement".into(), entier(self.plafond_enseignement()?));
        m.insert("heures_enseignement_retenues".into(), entier(self.heures_enseignement_retenues()?));
        m.insert("heures_montant".into(), entier(self.heures_montant()));
        m.insert("heures_ouverture".into(), entier(self.heures_ouverture()?));
        m.insert("enseignement_perdu".into(), entier(self.enseignement_perdu()?));
        m.insert("jours".into(), entier(self.jours()));
        m.insert("brut".into(), Value::String(nombre(self.brut())));
        m.insert("manque".into(), entier(self.manque()?));
        m.insert("atteint".into(), Value::Bool(self.atteint()?));
        m.insert("cachets_manquants".into(), entier(self.cachets_manquants()?));
        m.insert("jours_restants".into(), entier(self.jours_restants(aujourdhui)));
        Ok(Value::Object(m))
    }
}

/// La part d'enseignement d'une liste de lignes de paie, en heures entieres :
/// les rubriques dont le catalogue dit `compte_pour = ouverture_des_droits`.
pub fn heures_enseignement(ctx: &Contexte, lignes: &[Value]) -> Resultat<i64> {
    let mut heures = 0.0;
    for l in lignes {
        let code = l.get("rubrique").ok_or_else(|| socle::fichiers::absent("rubrique"))?;
        let code = match code {
            Value::String(s) => s.clone(),
            autre => socle::date::en_texte(autre),
        };
        let r = rubrique(&ctx.racine, &code, &ctx.lue_au)?;
        if r.compte_pour != "ouverture_des_droits" {
            continue;
        }
        let q = flottant(l.get("quantite").ok_or_else(|| socle::fichiers::absent("quantite"))?)?;
        let saisies = match l.get("heures") {
            None | Some(Value::Null) => None,
            Some(x) => Some(flottant(x)?),
        };
        // Le signe porte aussi sur les heures, comme au calcul (`calcul::lignes_brut`) : une ligne « - » en retire.
        let signe = if r.sens == "-" { -1.0 } else { 1.0 };
        heures += signe * r.heures(q, saisies);
    }
    Ok(heures.round_ties_even() as i64)
}


// ------------------------------------------------------------------ fenetres

/// (debut, fin, libelle).
pub type Fenetre = (Jour, Jour, String);

/// L'annee civile, arretee a `jusquau` quand on en donne une.
pub fn fenetre_annee_civile(annee: i32, jusquau: Option<Jour>) -> Fenetre {
    let mut fin = Jour { an: annee, mois: 12, jour: 31 };
    if let Some(j) = jusquau {
        if j < fin {
            fin = j;
        }
    }
    (Jour { an: annee, mois: 1, jour: 1 }, fin, format!("annee civile {}", annee))
}

/// Les 12 mois precedant une date.
pub fn fenetre_douze_mois(c: &Constantes, fin: Jour) -> Resultat<Fenetre> {
    Ok((fin.plus_jours(-c.fenetre_jours)?, fin, format!("12 mois au {}", fin.jj_mm_aaaa())))
}

/// Les 12 mois PRECEDANT la date anniversaire des droits : l'anniversaire est
/// la fin de la fenetre, jamais son debut.
pub fn fenetre_anniversaire(c: &Constantes, anniversaire: Jour) -> Resultat<Fenetre> {
    Ok((
        anniversaire.plus_jours(-c.fenetre_jours)?,
        anniversaire,
        format!("12 mois au {} (anniversaire des droits)", anniversaire.jj_mm_aaaa()),
    ))
}

/// Les engagements du salarie dont la FIN est dans la fenetre.
pub fn position(
    ctx: &Contexte,
    c: &Constantes,
    engagements: &[Declare],
    salarie: &str,
    fenetre: &Fenetre,
    date_naissance: Option<Jour>,
) -> Position {
    let (debut, fin, libelle) = fenetre;
    Position {
        constantes: *c,
        racine: ctx.racine.clone(),
        salarie: salarie.to_string(),
        debut: *debut,
        fin: *fin,
        libelle_fenetre: libelle.clone(),
        engagements: engagements.iter().filter(|e| e.salarie == salarie && *debut <= e.fin && e.fin <= *fin).cloned().collect(),
        date_naissance,
    }
}

/// Date de naissance du salarie, ou `None`.
pub fn naissance(ctx: &Contexte, salarie: &str) -> Resultat<Option<Jour>> {
    match ctx.salaries.get(salarie).and_then(|f| f.get("date_naissance")) {
        Some(v) if socle::fichiers::vrai(v) => d(v).map(Some),
        _ => Ok(None),
    }
}

/// Fin du dernier contrat declare : le depart de la fenetre par defaut.
pub fn derniere_fin(engagements: &[Declare], salarie: &str) -> Option<Jour> {
    engagements.iter().filter(|e| e.salarie == salarie).map(|e| e.fin).max()
}

/// Position au regard des annexes 8 et 10. `anniversaire` (notifie par France
/// Travail) prime ; sinon la fenetre est les 12 mois precedant `a_la_date` ou
/// la fin du dernier contrat declare.
pub fn position_droits(
    ctx: &Contexte,
    salarie: &str,
    engagements: &[Declare],
    anniversaire: Option<Jour>,
    a_la_date: Option<Jour>,
) -> Resultat<Position> {
    let c = Constantes::lire(&ctx.racine, &ctx.lue_au)?;
    let f = match anniversaire {
        Some(a) => fenetre_anniversaire(&c, a)?,
        None => {
            let fin = match a_la_date.or_else(|| derniere_fin(engagements, salarie)) {
                Some(j) => j,
                None => aujourdhui()?,
            };
            fenetre_douze_mois(&c, fin)?
        }
    };
    Ok(position(ctx, &c, engagements, salarie, &f, naissance(ctx, salarie)?))
}

// ------------------------------------------------------- bloc du bulletin

/// Une colonne du bloc de cumul imprime en bas du bulletin.
#[derive(Debug, Clone, PartialEq)]
pub struct Colonne {
    pub titre: String,
    pub sous_titre: String,
    pub cachets: i64,
    pub heures: i64,
    pub heures_equivalentes: i64,
    pub brut: Decimal,
    /// Heures restantes avant le seuil (colonne droits).
    pub reste: Option<i64>,
    /// Date d'anniversaire (colonne droits).
    pub echeance: Option<Jour>,
    pub enseignement: i64,
    pub heures_ouverture: Option<i64>,
}

impl Colonne {
    pub fn canon(&self) -> Value {
        let mut m = Map::new();
        m.insert("titre".into(), Value::String(self.titre.clone()));
        m.insert("sous_titre".into(), Value::String(self.sous_titre.clone()));
        m.insert("cachets".into(), entier(self.cachets));
        m.insert("heures".into(), entier(self.heures));
        m.insert("heures_equivalentes".into(), entier(self.heures_equivalentes));
        m.insert("brut".into(), Value::String(nombre(self.brut)));
        m.insert("reste".into(), self.reste.map_or(Value::Null, entier));
        m.insert("echeance".into(), self.echeance.map_or(Value::Null, |j| Value::String(j.iso())));
        m.insert("enseignement".into(), entier(self.enseignement));
        m.insert("heures_ouverture".into(), self.heures_ouverture.map_or(Value::Null, entier));
        Value::Object(m)
    }
}

/// Une tranche du mois `AAAA-MM`, lue comme un entier.
fn entier_du_mois(texte: &str) -> Resultat<i32> {
    texte.trim().parse().map_err(|_| format!("nombre entier illisible : {}", repr(texte)))
}

/// Les colonnes : le mois, l'annee civile (arretee a la fin du mois), et
/// la fenetre des droits quand `anniversaire_droits` est dans la fiche.
pub fn tableau_cumul(
    ctx: &Contexte,
    salarie: &str,
    mois: &str,
    engagements: &[Declare],
) -> Resultat<Vec<Colonne>> {
    let c = Constantes::lire(&ctx.racine, &ctx.lue_au)?;
    let tranche = |a: usize, b: usize| -> String { mois.chars().skip(a).take(b - a).collect() };
    let annee = entier_du_mois(&tranche(0, 4))?;
    let mo = entier_du_mois(&tranche(5, 7))?;
    if !(1..=9999).contains(&annee) {
        return Err(socle::date::annee_impossible(annee));
    }
    if !(1..=12).contains(&mo) {
        return Err(socle::date::mois_impossible(mo));
    }
    let mo = mo as u32;
    let debut_mois = Jour { an: annee, mois: mo, jour: 1 };
    let fin_mois = Jour { an: annee, mois: mo, jour: debut_mois.jours_du_mois() };
    let ne = naissance(ctx, salarie)?;
    let p_mois = position(ctx, &c, engagements, salarie, &(debut_mois, fin_mois, "periode".to_string()), ne);
    let p_annee = position(ctx, &c, engagements, salarie, &fenetre_annee_civile(annee, Some(fin_mois)), ne);

    let col = |titre: &str, sous: String, p: &Position, reste: Option<i64>, echeance: Option<Jour>| -> Resultat<Colonne> {
        Ok(Colonne {
            titre: titre.to_string(),
            sous_titre: sous,
            cachets: p.cachets(),
            heures: p.heures(),
            heures_equivalentes: p.heures_equivalentes(),
            brut: p.brut(),
            reste,
            echeance,
            enseignement: p.heures_enseignement(),
            heures_ouverture: Some(p.heures_ouverture()?),
        })
    };
    let mut colonnes = vec![
        col("Période", format!("{:02}/{}", mo, annee), &p_mois, None, None)?,
        col("Cumul", format!("année {}", annee), &p_annee, None, None)?,
    ];
    if let Some(anniv) = anniversaire_configure(ctx, salarie)? {
        let p_droits = position(ctx, &c, engagements, salarie, &fenetre_anniversaire(&c, anniv)?, ne);
        let reste = p_droits.manque()?;
        colonnes.push(col("Droits", format!("au {}", anniv.jj_mm_aaaa()), &p_droits, Some(reste), Some(anniv))?);
    }
    Ok(colonnes)
}

/// Date anniversaire des droits, si elle est dans `config/salaries.json`
/// (`anniversaire_droits`) ; absente tant que France Travail ne l'a pas notifiee.
pub fn anniversaire_configure(ctx: &Contexte, salarie: &str) -> Resultat<Option<Jour>> {
    match ctx.salaries.get(salarie).and_then(|f| f.get("anniversaire_droits")) {
        Some(v) if socle::fichiers::vrai(v) => d(v).map(Some),
        _ => Ok(None),
    }
}
