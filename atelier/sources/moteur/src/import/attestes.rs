//! UN CONTRAT NE S'ATTESTE QU'UNE FOIS. Une AEM deposee, une AEM en PDF, un bulletin, un certificat Congés Spectacles
//! disent chacun d'un contrat : ce salarie, ces dates, ces heures, cachets et jours, ce brut. Le registre tient ce qui
//! est deja atteste (l'historique, puis ce que l'import en cours retient) ; chaque piece lui demande si son contrat
//! l'est (`pareils`), et s'il ne l'est pas, le lui ajoute (`ajouter`). Ce qu'il retient, et ce qu'il change a
//! l'historique, est ce que l'ecriture prend (`import::ecrite`).
//!
//! LA REGLE : un contrat du meme salarie (par son NIR) dont les dates chevauchent celles de la piece l'atteste deja ;
//! la piece le confirme, n'ajoute rien, et ses ecarts se notent. Deux exceptions, de la piece la moins complete a la
//! plus complete :
//! - le contrat qu'un certificat atteste ne porte ni les heures, ni les cachets et les jours a part, et son brut est
//!   arrondi a l'euro (`import::certificat_lu`) : un bulletin ou une AEM le remplace, MOIS PAR MOIS (`reste_du_
//!   certificat`) ; les mois du certificat qu'aucun d'eux n'atteste restent attestes par lui ;
//! - le bulletin est la piece de paie : quand il dit d'autres compteurs que le contrat d'une AEM aux memes dates, son
//!   contrat remplace celui de l'AEM, et l'ecart se dit avec les deux valeurs.

use std::collections::{BTreeSet, HashMap};

use serde_json::{json, Value};

use droit::contexte::Contexte;
use socle::echec::{Echec, R};
use socle::num::Decimal;
use socle::valeur::valeur_en_texte;

/// Un contrat atteste : de qui (NIR), quand, par quoi, et ce qu'il compte.
#[derive(Clone, Debug)]
pub(crate) struct Atteste {
    pub(crate) nir: String,
    pub(crate) debut: String,
    pub(crate) fin: String,
    /// « AEM », « bulletin », « certificat » (la piece de cet import), ou « historique ».
    pub(crate) source: &'static str,
    /// Ce qui l'atteste (`atteste_par` de sa ligne) : « AEM », « bulletin », « certificat ».
    pub(crate) par: String,
    pub(crate) heures: i128,
    pub(crate) cachets: i128,
    pub(crate) jours: i128,
    pub(crate) brut: Option<Decimal>,
    /// Sa ligne, telle qu'elle s'ecrit.
    ligne: Value,
    /// Pour un contrat de l'historique : sa ligne telle que l'historique la porte (l'ecriture la retrouve par elle).
    historique: Option<Value>,
}

impl Atteste {
    /// Son nom dans le compte rendu.
    pub(crate) fn d_ou(&self) -> String {
        match (self.source, self.par.as_str()) {
            ("historique", "certificat") => "historique, d'un certificat".into(),
            ("historique", "AEM") => "historique, d'une AEM".into(),
            (s, _) => s.into(),
        }
    }

    pub(crate) fn de_certificat(&self) -> bool {
        self.par == "certificat"
    }

    /// Ce qu'il compte, en mots, pour dire un ecart avec ses deux valeurs.
    pub(crate) fn en_mots(&self) -> String {
        en_mots(self.heures, self.cachets, self.jours, self.brut)
    }
}

/// Des compteurs et un brut, en mots.
pub(crate) fn en_mots(heures: i128, cachets: i128, jours: i128, brut: Option<Decimal>) -> String {
    let brut = brut.map_or_else(|| "brut illisible".to_string(), |b| format!("{} € de brut", b));
    format!("{} heure(s), {} cachet(s), {} jour(s), {}", heures, cachets, jours, brut)
}

/// Une date ISO en `JJ/MM/AAAA`, telle quelle si elle ne se lit pas.
fn jj(iso: &str) -> String {
    socle::date::Jour::lire(iso).map(|j| j.jj_mm_aaaa()).unwrap_or_else(|_| iso.to_string())
}

/// Ce qu'une ligne de contrat (la forme qu'un engagement importe porte) dit, pour le registre.
fn lu(nir: &str, ligne: &Value, source: &'static str) -> Atteste {
    let t = |k: &str| ligne.get(k).map(valeur_en_texte).unwrap_or_default();
    let n = |k: &str| ligne.get(k).and_then(Value::as_i64).unwrap_or(0) as i128;
    Atteste {
        nir: nir.to_string(),
        debut: t("debut"),
        fin: t("fin"),
        source,
        par: t("atteste_par"),
        heures: n("heures"),
        cachets: n("cachets"),
        jours: n("jours"),
        brut: socle::num::decimal_lu(&t("brut")),
        ligne: ligne.clone(),
        historique: (source == "historique").then(|| ligne.clone()),
    }
}

/// Les dates `[d1, f1]` et `[d, f]` (ISO) se chevauchent ; une fin vide court toujours.
pub(crate) fn chevauche(d1: &str, f1: &str, d: &str, f: &str) -> bool {
    !d1.is_empty() && d1 <= f && (f1.is_empty() || d <= f1)
}

/// Les mois `AAAA-MM` de `debut` a `fin` (ISO) ; vide si une date ne se lit pas.
fn mois_de(debut: &str, fin: &str) -> Vec<String> {
    match (socle::date::Jour::lire(debut), socle::date::Jour::lire(fin)) {
        (Ok(d), Ok(f)) => socle::date::mois_entre(&d, &f),
        _ => Vec::new(),
    }
}

/// UN CERTIFICAT, MOINS LES MOIS QUE D'AUTRES PIECES ATTESTENT. Le certificat dit un contrat entier (ses dates, son
/// nombre de jours ou de cachets, son brut) ; un bulletin ou une AEM dit un mois. Les mois que `par` attestent lui
/// sont retires ; ce qui reste est ce que le certificat atteste seul : les dates des mois restants, et ses compteurs
/// et son brut moins ceux des pieces qui le coupent (une soustraction de valeurs lues, pas un partage). `Ok(None)` :
/// il ne reste rien. Ce qui ne se soustrait pas se refuse (regle 1) : des mois restants de part et d'autre d'un mois
/// coupe (les compteurs ne se partagent pas entre eux), des pieces qui comptent dans une autre unite que lui, un reste
/// nul ou negatif. La base conge du certificat ne dit que le contrat entier : le reste ne la porte pas.
fn reste_du_certificat(c: &Atteste, par: &[&Atteste]) -> Result<Option<Value>, String> {
    let siens = mois_de(&c.debut, &c.fin);
    let coupes: BTreeSet<String> = par.iter().filter(|p| chevauche(&p.debut, &p.fin, &c.debut, &c.fin)).flat_map(|p| mois_de(&p.debut, &p.fin)).collect();
    let restent: Vec<usize> = (0..siens.len()).filter(|i| !coupes.contains(&siens[*i])).collect();
    let (Some(&premier), Some(&dernier)) = (restent.first(), restent.last()) else { return Ok(None) };
    let libelle = |m: &str| socle::date::libelle_du_mois_ou_tel_quel(m, socle::date::Graphie::Accentuee);
    let mois_restants = restent.iter().map(|i| libelle(&siens[*i])).collect::<Vec<_>>().join(", ");
    if dernier - premier + 1 != restent.len() {
        return Err(format!(
            "il reste au certificat ({}) des mois de part et d'autre d'un mois attesté ailleurs : ses jours ou cachets et son brut ne se partagent pas entre eux ; déposez les bulletins de ces mois",
            mois_restants
        ));
    }
    let coupants: Vec<&&Atteste> = par.iter().filter(|p| chevauche(&p.debut, &p.fin, &c.debut, &c.fin)).collect();
    let (cachets, jours): (i128, i128) = (coupants.iter().map(|p| p.cachets).sum(), coupants.iter().map(|p| p.jours).sum());
    let brut: Option<Decimal> = coupants.iter().map(|p| p.brut).sum();
    // Le certificat compte dans une seule unite (des cachets ou des jours) : le reste se compte dans la sienne.
    let (reste_cachets, reste_jours) = (if c.cachets > 0 { c.cachets - cachets } else { 0 }, if c.jours > 0 { c.jours - jours } else { 0 });
    let reste_brut = c.brut.zip(brut).map(|(a, b)| a - b);
    let unite_lue = (c.cachets == 0 || cachets > 0) && (c.jours == 0 || jours > 0);
    let positif = reste_cachets >= 0 && reste_jours >= 0 && reste_cachets + reste_jours > 0 && reste_brut.is_some_and(|b| b > Decimal::ZERO);
    if !unite_lue || !positif {
        return Err(format!(
            "le reste du certificat ({}) ne se compte pas : le certificat dit {}, ce qui l'atteste ailleurs {} ; déposez les bulletins de ces mois",
            mois_restants,
            c.en_mots(),
            en_mots(coupants.iter().map(|p| p.heures).sum(), cachets, jours, brut)
        ));
    }
    let debut = socle::date::premier_du_mois(&siens[premier]).map(|j| j.iso()).unwrap_or_default().max(c.debut.clone());
    let fin = socle::date::dernier_du_mois(&siens[dernier]).map(|j| j.iso()).unwrap_or_default().min(c.fin.clone());
    let mut ligne = c.ligne.clone();
    let mut b = reste_brut.unwrap_or_default().round_dp(2);
    b.rescale(2);
    ligne["debut"] = json!(debut);
    ligne["fin"] = json!(fin);
    ligne["cachets"] = json!(reste_cachets as i64);
    ligne["jours"] = json!(reste_jours as i64);
    ligne["brut"] = json!(b.to_string());
    if let Some(m) = ligne.as_object_mut() {
        m.remove("base_conge");
    }
    Ok(Some(ligne))
}

/// Ce que l'import retient : (NIR, la ligne du contrat, le bloc du compte rendu qui le compte).
pub(crate) type Retenu = (String, Value, &'static str);

/// Ce qu'un contrat de l'historique devient : (sa ligne, sa ligne nouvelle ; `None` : il est retire).
pub(crate) type Change = (Value, Option<Value>);

pub(crate) struct Registre {
    tous: Vec<Atteste>,
    pub(crate) retenus: Vec<Retenu>,
    pub(crate) changes: Vec<Change>,
    /// Les contrats de l'historique qui n'attestent rien, faute d'un salarie, d'un NIR ou de dates : le compte rendu
    /// le dit.
    pub(crate) muets: Vec<String>,
}

/// Ce que l'ajout d'un contrat a fait des contrats qu'il remplace : une phrase par contrat.
pub(crate) type Remplaces = Vec<String>;

impl Registre {
    /// Le registre de l'historique : les contrats des engagements importes, rattaches a leur NIR par la fiche
    /// (`par_cle` : cle de fiche -> NIR). Un contrat sans salarie, sans debut ou sans fin, ou dont la fiche n'a pas de
    /// NIR, ne se compare a rien : il n'atteste rien, et le registre le dit (`muets`).
    pub(crate) fn de_l_historique(ctx: &Contexte, par_cle: &HashMap<String, String>) -> R<Registre> {
        let mut tous = Vec::new();
        let mut muets = Vec::new();
        for e in engagement::importe::contrats_lus(&ctx.racine).map_err(Echec::from)? {
            let t = |k: &str| e.get(k).map(valeur_en_texte).unwrap_or_default();
            let (cle, debut, fin) = (t("salarie"), t("debut"), t("fin"));
            let manque: Vec<&str> = [(cle.is_empty(), "salarié"), (debut.is_empty(), "début"), (fin.is_empty(), "fin")].iter().filter(|m| m.0).map(|m| m.1).collect();
            let quel = format!("le contrat de l'historique {}{} du {} au {}", if cle.is_empty() { "" } else { "de " }, cle, if debut.is_empty() { "?".into() } else { jj(&debut) }, if fin.is_empty() { "?".into() } else { jj(&fin) });
            if !manque.is_empty() {
                muets.push(format!("{} n'a pas de {} : il ne se compare à aucune pièce, une pièce du même contrat l'ajouterait une seconde fois", quel, manque.join(", ni de ")));
                continue;
            }
            match par_cle.get(&cle) {
                Some(nir) => tous.push(lu(nir, &e, "historique")),
                None => muets.push(format!("{} : la fiche de ce salarié n'a pas de NIR, il ne se compare à aucune pièce, une pièce du même contrat l'ajouterait une seconde fois", quel)),
            }
        }
        Ok(Registre { tous, retenus: Vec::new(), changes: Vec::new(), muets })
    }

    /// Les contrats deja attestes qui chevauchent celui que la piece `piece` (« certificat », « AEM », « bulletin »)
    /// atteste. Un certificat ne compte que pour un certificat : un bulletin ou une AEM le remplace (`ajouter`). Vide :
    /// le contrat ne l'est pas.
    pub(crate) fn pareils(&self, nir: &str, debut: &str, fin: &str, piece: &str) -> Vec<Atteste> {
        self.tous.iter().filter(|a| a.nir == nir && chevauche(&a.debut, &a.fin, debut, fin) && (piece == "certificat" || !a.de_certificat())).cloned().collect()
    }

    /// Ce qu'un certificat atteste seul, une fois retires les mois que les contrats `pareils` (ni des certificats, ni
    /// vides) attestent : `reste_du_certificat`.
    pub(crate) fn reste_du_certificat(&self, nir: &str, ligne: &Value, pareils: &[Atteste]) -> Result<Option<Value>, String> {
        reste_du_certificat(&lu(nir, ligne, "certificat"), &pareils.iter().collect::<Vec<_>>())
    }

    /// Retient le contrat `ligne` de la piece `source`, compte au bloc `bloc`. Il remplace, du meme salarie et aux dates
    /// qui chevauchent les siennes, les mois des certificats qu'il atteste, et, pour un bulletin, le contrat d'une AEM
    /// (l'appelant l'a compare : d'autres compteurs). Rend une phrase par contrat remplace.
    pub(crate) fn ajouter(&mut self, nir: &str, ligne: Value, source: &'static str, bloc: &'static str) -> Remplaces {
        let a = lu(nir, &ligne, source);
        let mut phrases = Vec::new();
        if !a.de_certificat() {
            let remplaces: Vec<usize> = (0..self.tous.len())
                .filter(|i| {
                    let x = &self.tous[*i];
                    x.nir == nir && chevauche(&x.debut, &x.fin, &a.debut, &a.fin) && (x.de_certificat() || (source == "bulletin" && x.par == "AEM"))
                })
                .collect();
            for i in remplaces.into_iter().rev() {
                let x = self.tous.remove(i);
                let nouvelle = if x.de_certificat() {
                    match reste_du_certificat(&x, &[&a]) {
                        Ok(None) => {
                            phrases.push(format!("il remplace le contrat repris d'un certificat Congés Spectacles ({}) aux mêmes dates", x.d_ou()));
                            None
                        }
                        Ok(Some(l)) => {
                            let r = lu(nir, &l, x.source);
                            phrases.push(format!(
                                "il remplace le contrat repris d'un certificat Congés Spectacles ({}) pour les mois que cette pièce atteste ; le reste du certificat, du {} au {}, reste attesté par lui : {} (ce que dit le certificat moins ce que dit cette pièce ; le brut du certificat est arrondi à l'euro)",
                                x.d_ou(),
                                jj(&r.debut),
                                jj(&r.fin),
                                r.en_mots()
                            ));
                            Some(l)
                        }
                        Err(m) => {
                            phrases.push(format!("il remplace le contrat repris d'un certificat Congés Spectacles ({}) ; {}", x.d_ou(), m));
                            None
                        }
                    }
                } else {
                    phrases.push(format!(
                        "il remplace le contrat de l'AEM ({}) aux mêmes dates, qui compte d'autres choses : l'AEM disait {}, le bulletin dit {} ; la pièce de paie l'emporte",
                        x.d_ou(),
                        x.en_mots(),
                        a.en_mots()
                    ));
                    None
                };
                self.changer(x, nouvelle);
            }
        }
        self.tous.push(a);
        self.retenus.push((nir.to_string(), ligne, bloc));
        phrases
    }

    /// `x`, retire du registre, devient `nouvelle` (`None` : il disparait), dans ce que l'import retient ou dans ce qu'il
    /// change a l'historique.
    fn changer(&mut self, x: Atteste, nouvelle: Option<Value>) {
        match &x.historique {
            Some(h) => {
                self.changes.retain(|c| &c.0 != h);
                self.changes.push((h.clone(), nouvelle.clone()));
            }
            None => {
                let i = self.retenus.iter().position(|r| r.0 == x.nir && r.1 == x.ligne);
                match (i, &nouvelle) {
                    (Some(i), Some(l)) => self.retenus[i].1 = l.clone(),
                    (Some(i), None) => {
                        self.retenus.remove(i);
                    }
                    _ => {}
                }
            }
        }
        if let Some(l) = nouvelle {
            let mut r = lu(&x.nir, &l, x.source);
            r.historique = x.historique;
            self.tous.push(r);
        }
    }

    /// Combien ce bloc retient.
    pub(crate) fn nombre(&self, bloc: &str) -> usize {
        self.retenus.iter().filter(|r| r.2 == bloc).count()
    }

    /// Les lignes que ce bloc retient.
    pub(crate) fn lignes(&self, bloc: &str) -> Vec<Value> {
        self.retenus.iter().filter(|r| r.2 == bloc).map(|r| r.1.clone()).collect()
    }
}
