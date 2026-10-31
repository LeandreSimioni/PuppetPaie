//! LES OBLIGATIONS QU'UN ARRET ET UNE FIN DE CONTRAT FONT NAITRE : ce qu'il faut faire, ou, dans quel delai,
//! a compter de quoi, jusqu'a quand, et ou on en est. UN SEUL ENDROIT les calcule ; les routes `arrets` et
//! `fins-de-contrat` (`routes::imprevus`) et le rappel de l'accueil (`routes::rappels`) ne font que les rendre, l'ecran ne
//! fait que les afficher (regle 2).
//!
//! Les regles sont des donnees : `logiciel/donnees/obligations-base.json` dit chaque obligation, son delai, son point de
//! depart et le texte qui la fonde -- tous deposes (son `_textes`) :
//!
//! - le signalement d'arret (04) et de reprise anticipee (05) : CSS R. 133-14, II
//!   (`atelier/reference/sources/code-de-la-securite-sociale/xml/R133-14.xml`) et l'arrete du 2 juillet 2013, art. 1er
//!   (`atelier/reference/sources/arrete-2013-delai-evenements-dsn/xml/1.xml`) : cinq jours ouvres quand l'employeur
//!   n'est pas subroge ; avec la DSN mensuelle du mois quand il l'est. SAUF pour les motifs que la DSN ne couvre pas
//!   (`sauf_motifs` : le deuil d'enfant, 19 ; le temps partiel therapeutique, 15 a 18, non saisissable) : la page du compte
//!   entreprise de net-entreprises (`atelier/reference/sources/accident-du-travail/net-entreprises-compte-entreprise-dat-20260331.txt`)
//!   dit que l'Assurance Maladie n'en tire rien. La norme admet pourtant le motif 19 dans un signalement : `arret-signaler`
//!   ne le refuse pas, il avertit (`signalement::avertissement_hors_dsn`, appele par `signalement::verifier`) ;
//! - l'attestation de salaire pour les indemnites journalieres (DSIJ), HORS DSN, pour le deuil d'enfant (motif 19) : la
//!   meme page, « obligatoire pour tout arret de travail, quel qu'en soit le motif », « des reception de l'avis d'arret de
//!   travail », sur net-entreprises (ecran ou EDI) ; CSS L. 331-9 (`.../code-de-la-securite-sociale/xml/L331-9.xml`) pour les
//!   indemnites du conge de deuil de C. trav. L. 3142-1-1. « Des reception » n'est pas un delai chiffre : la date butoir est
//!   le jour meme de la connaissance de l'arret, et l'obligation le dit (`inferences`) ;
//! - le signalement de fin de contrat (07) : les memes textes, cinq jours ouvres apres la fin ;
//! - la declaration d'accident du travail ou de trajet (motifs 04 et 06), HORS DSN : CSS L. 441-2 et R. 441-3
//!   (`.../code-de-la-securite-sociale/xml/R441-3.xml`) : quarante-huit heures, dimanches et jours feries non compris, a
//!   compter de la connaissance de l'accident ; les reserves motivees, dix jours francs apres (R. 441-6) ;
//! - la maladie professionnelle (motif 05) n'est PAS declaree par l'employeur : par la victime, quinze jours apres la
//!   cessation du travail (L. 461-5, R. 461-5). Le moteur le dit, il n'en fait pas une obligation de la compagnie.
//!
//! CE QUI SE SAISIT, ET SE GARDE A COTE DES IMPREVUS (`imprevus/obligations.json`) : le jour (et l'heure) ou la compagnie a
//! connu l'arret ou l'accident, la declaration d'accident faite et l'attestation de salaire faite -- le jour, ou, la
//! reference. Rien de
//! cela ne se devine : sans connaissance saisie, le moteur prend le point de depart le plus exigeant et dit lequel
//! (`depart.presume`) ; sans declaration dite, l'accident n'est pas declare. Le fichier des arrets eux-memes
//! (`imprevus/AAAA-MM.json`, les imprevus de genre arret) n'est pas touche : ce module le lit par `crate::routes::imprevus::tous_les_arrets`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use socle::date::{en_texte, Jour};
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::{dossier_du_code, json_en_cache, repr, vrai};
use declarations::signalement::sauf_motifs;

/// Les routes qui ecrivent ce que la compagnie dit d'une obligation.
pub const ROUTES: [&str; 3] = ["arret-connaissance", "accident-declare", "attestation-declare"];

/// La route qui ne fait que lire : ce qu'un arret en cours de saisie ferait naitre.
pub const ROUTES_LECTURE: [&str; 1] = ["arret-obligations"];

pub const A_FAIRE: &str = "a_faire";
pub const EN_RETARD: &str = "en_retard";
pub const FAIT: &str = "fait";
pub const A_VENIR: &str = "a_venir";
pub const INDETERMINE: &str = "indetermine";
pub const A_LA_CHARGE_DU_SALARIE: &str = "a_la_charge_du_salarie";

const JOURS: [&str; 7] = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];

// ------------------------------------------------------------------ les donnees

/// `obligations-base.json`, lu une fois tant qu'il ne change pas.
pub fn base() -> R<Arc<Value>> {
    json_en_cache(&dossier_du_code().join("obligations-base.json")).map_err(|m| erreur("OSError", m))
}

fn obligation(n: &Value, cle: &str) -> R<Value> {
    n.get("obligations")
        .and_then(|o| o.get(cle))
        .cloned()
        .ok_or_else(|| erreur("KeyError", format!("obligations-base.json : l'obligation {} manque", repr(cle))))
}

fn texte(v: &Value, k: &str) -> String {
    v.get(k).map(en_texte).unwrap_or_default()
}

fn entier(v: &Value, k: &str) -> R<i64> {
    v.get(k).and_then(Value::as_i64).ok_or_else(|| erreur("KeyError", format!("obligations-base.json : {} manque ou n'est pas un nombre", repr(k))))
}

fn motifs_de(o: &Value) -> Vec<String> {
    o.get("motifs").and_then(Value::as_array).map(|a| a.iter().map(en_texte).collect()).unwrap_or_default()
}

/// Le motif d'arret appelle-t-il l'obligation `cle` (ses `motifs`) ?
fn motif_appelle(cle: &str, motif: &str) -> R<bool> {
    Ok(motifs_de(&obligation(&*base()?, cle)?).iter().any(|m| m == motif))
}

/// Le motif d'arret appelle-t-il la declaration d'accident (`declaration_accident.motifs`) ?
pub fn motif_appelle_la_declaration(motif: &str) -> R<bool> {
    motif_appelle("declaration_accident", motif)
}

/// Un signalement d'arret (04) est-il du pour ce motif ? Non pour ceux que la DSN ne couvre pas
/// (`signalement_arret.sauf_motifs`). La liste des arrets s'en sert pour ne pas dire « a signaler » ce qui ne se signale pas.
pub fn signalement_du(motif: &str) -> R<bool> {
    Ok(!sauf_motifs(&obligation(&*base()?, "signalement_arret")?).iter().any(|m| m == motif))
}

/// Le code « oui » de la subrogation (`arrets-base.json`, S21.G00.60.004).
fn code_subrogation_oui() -> R<String> {
    let n = json_en_cache(&dossier_du_code().join("arrets-base.json")).map_err(|m| erreur("OSError", m))?;
    n["subrogation"]["codes"]
        .as_array()
        .and_then(|a| a.iter().find(|c| c["libelle"].as_str() == Some("oui")))
        .map(|c| en_texte(&c["code"]))
        .ok_or_else(|| erreur("KeyError", "arrets-base.json : pas de code « oui » pour la subrogation"))
}

// --------------------------------------------------------------- le calendrier

fn jour(v: &Value) -> Option<Jour> {
    v.as_str().filter(|s| !s.is_empty()).and_then(|s| Jour::lire(s).ok())
}

fn plus(j: Jour, n: i64) -> R<Jour> {
    j.plus_jours(n).map_err(|m| erreur("OverflowError", m))
}

/// Les feries chomes de la compagnie, annee par annee (`feries::chomes`, la fiche Entreprise).
struct Calendrier<'a> {
    entreprise: Option<&'a Value>,
    chomes: HashMap<i32, Vec<Jour>>,
    legaux: HashMap<i32, Vec<Jour>>,
}

impl<'a> Calendrier<'a> {
    fn nouveau(entreprise: Option<&'a Value>) -> Calendrier<'a> {
        Calendrier { entreprise, chomes: HashMap::new(), legaux: HashMap::new() }
    }

    fn chome(&mut self, j: Jour) -> R<bool> {
        if !self.chomes.contains_key(&j.an) {
            let f = socle::feries::chomes(j.an, self.entreprise).map_err(|m| erreur("ValueError", m))?;
            self.chomes.insert(j.an, f.into_iter().map(|(d, _)| d).collect());
        }
        Ok(self.chomes[&j.an].contains(&j))
    }

    fn ferie(&mut self, j: Jour) -> R<bool> {
        if !self.legaux.contains_key(&j.an) {
            let f = socle::feries::feries(j.an).map_err(|m| erreur("ValueError", m))?;
            self.legaux.insert(j.an, f.into_iter().map(|(d, _)| d).collect());
        }
        Ok(self.legaux[&j.an].contains(&j))
    }

    /// Le `n`-ieme jour ouvre qui suit `depart` : ni samedi, ni dimanche, ni ferie chome dans la compagnie ; le jour de
    /// depart ne compte pas (`obligations-base.json`, `jours_ouvres`).
    fn jours_ouvres_apres(&mut self, depart: Jour, n: i64) -> R<Jour> {
        let (mut j, mut reste) = (depart, n);
        while reste > 0 {
            j = plus(j, 1)?;
            if j.jour_de_la_semaine() < 5 && !self.chome(j)? {
                reste -= 1;
            }
        }
        Ok(j)
    }

    /// Un jour qui compte dans « quarante-huit heures non compris les dimanches et jours feries » (R. 441-3) : ni
    /// dimanche, ni l'une des onze fetes legales.
    fn compte_dans_les_heures(&mut self, j: Jour) -> R<bool> {
        Ok(j.jour_de_la_semaine() != 6 && !self.ferie(j)?)
    }

    /// `heures` heures a compter de `depart` a `minute` (minutes depuis minuit), dimanches et feries non compris. Rend le
    /// jour et la minute ou le delai expire (1440 : a la fin de ce jour-la). Un depart un jour qui ne compte pas court
    /// des minuit du premier jour qui compte.
    fn heures_apres(&mut self, depart: Jour, minute: u32, heures: i64) -> R<(Jour, u32)> {
        let (mut j, mut m) = (depart, minute as i64);
        while !self.compte_dans_les_heures(j)? {
            j = plus(j, 1)?;
            m = 0;
        }
        let mut reste = heures * 60;
        loop {
            let dans_ce_jour = 1440 - m;
            if reste <= dans_ce_jour {
                return Ok((j, (m + reste) as u32));
            }
            reste -= dans_ce_jour;
            j = plus(j, 1)?;
            while !self.compte_dans_les_heures(j)? {
                j = plus(j, 1)?;
            }
            m = 0;
        }
    }
}

// ---------------------------------------------------------------- les phrases

fn lisible(j: &Jour) -> String {
    format!("{} {}", JOURS[j.jour_de_la_semaine() as usize], j.jj_mm_aaaa())
}

fn heure_lisible(minute: u32) -> String {
    format!("{:02}:{:02}", minute / 60, minute % 60)
}

fn s(n: i64) -> &'static str {
    if n > 1 {
        "s"
    } else {
        ""
    }
}

/// `HH:MM` -> minutes depuis minuit.
fn lire_heure(h: &str) -> Option<u32> {
    let (a, b) = h.trim().split_once(':')?;
    let (a, b): (u32, u32) = (a.parse().ok()?, b.parse().ok()?);
    (a < 24 && b < 60 && a.to_string().len() <= 2 && b.to_string().len() <= 2).then_some(a * 60 + b)
}

// ------------------------------------------------------------ ce qui est saisi

/// Ce que la compagnie a dit des obligations de ses arrets : `imprevus/obligations.json`, a cote du magasin des imprevus.
fn chemin(ctx: &Contexte) -> PathBuf {
    engagement::imprevus::dossier(&ctx.racine).join("obligations.json")
}

/// Les fiches de suivi, par (cle du salarie, dernier jour travaille).
pub struct Suivi {
    fiches: Vec<Value>,
}

impl Suivi {
    pub fn lire(ctx: &Contexte) -> R<Suivi> {
        let p = chemin(ctx);
        if !p.exists() {
            return Ok(Suivi { fiches: Vec::new() });
        }
        let v = socle::fichiers::lire_json_echec(&p)?;
        let fiches = match v.get("arrets") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(a)) => a.clone(),
            Some(_) => return Err(erreur("TypeError", format!("{} : `arrets` doit etre une liste", p.display()))),
        };
        Ok(Suivi { fiches })
    }

    fn de(&self, salarie: &str, dernier_jour: &str) -> Value {
        self.fiches
            .iter()
            .find(|f| texte(f, "salarie") == salarie && texte(f, "dernier_jour_travaille") == dernier_jour)
            .cloned()
            .unwrap_or(Value::Null)
    }

    fn ecrire(&mut self, ctx: &Contexte, salarie: &str, dernier_jour: &str, champ: &str, valeur: Value) -> R<()> {
        let i = match self.fiches.iter().position(|f| texte(f, "salarie") == salarie && texte(f, "dernier_jour_travaille") == dernier_jour) {
            Some(i) => i,
            None => {
                self.fiches.push(json!({"salarie": salarie, "dernier_jour_travaille": dernier_jour}));
                self.fiches.len() - 1
            }
        };
        let f = self.fiches[i].as_object_mut().ok_or_else(|| erreur("TypeError", "imprevus/obligations.json : une fiche n'est pas un objet"))?;
        if valeur.is_null() {
            f.remove(champ);
        } else {
            f.insert(champ.to_string(), valeur);
        }
        // Une fiche qui ne dit plus rien s'en va.
        self.fiches.retain(|f| f.as_object().is_some_and(|o| o.keys().any(|k| k != "salarie" && k != "dernier_jour_travaille")));
        let doc = json!({
            "_quoi": "Ce que la compagnie a dit des obligations de ses arrets de travail : le jour ou elle a connu l'arret ou l'accident, la declaration d'accident du travail faite et l'attestation de salaire faite (hors DSN, le deuil d'enfant). Ecrit par les routes `arret-connaissance`, `accident-declare` et `attestation-declare` (`atelier/sources/moteur/src/obligations.rs`) ; les arrets eux-memes sont dans `AAAA-MM.json`.",
            "arrets": self.fiches,
        });
        socle::ecriture::ecrire_json(&chemin(ctx), &doc, 2).map_err(|m| erreur("OSError", m))
    }
}

// ---------------------------------------------------------- une obligation

/// Le point de depart d'un delai : le jour, l'heure s'il y en a une, ce que c'est, et s'il est saisi ou presume.
struct Depart {
    jour: Jour,
    minute: Option<u32>,
    quoi: String,
    presume: Option<String>,
}

impl Depart {
    fn en_json(&self) -> Value {
        json!({
            "jour": self.jour.iso(),
            "heure": self.minute.map(heure_lisible),
            "quoi": self.quoi,
            "saisi": self.presume.is_none(),
            "presume": self.presume,
        })
    }
}

/// Ce qui est fait : le jour, et comment.
struct Fait {
    le: Jour,
    comment: String,
    detail: Value,
}

/// L'objet commun d'une obligation : son libelle, ce qui la fait naitre, qui, ou, ses textes.
fn squelette(cle: &str, o: &Value) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("cle".into(), json!(cle));
    for k in ["libelle", "nait_de", "qui", "ou", "destinataire", "textes", "exceptions", "inferences", "a_avoir"] {
        if let Some(v) = o.get(k) {
            m.insert(k.into(), v.clone());
        }
    }
    m
}

/// Pose l'etat, la date butoir et la phrase d'une obligation dont on connait le depart et la butoir.
fn conclure(m: &mut Map<String, Value>, aujourdhui: Jour, depart: Option<&Depart>, butoir: Jour, butoir_minute: Option<u32>, delai: &str, fait: Option<Fait>) {
    let quand_butoir = match butoir_minute {
        Some(1440) => format!("le {}, jusqu'à minuit", lisible(&butoir)),
        Some(720) => format!("le {} à midi", lisible(&butoir)),
        Some(mn) => format!("le {} à {}", lisible(&butoir), heure_lisible(mn)),
        None => format!("le {}", lisible(&butoir)),
    };
    m.insert("delai".into(), json!(delai));
    if let Some(d) = depart {
        m.insert("depart".into(), d.en_json());
    }
    m.insert("butoir".into(), json!(butoir.iso()));
    m.insert("butoir_heure".into(), json!(butoir_minute.map(|mn| if mn == 1440 { "24:00".to_string() } else { heure_lisible(mn) })));
    let restants = butoir.moins(&aujourdhui);
    let (etat, phrase) = match &fait {
        Some(f) => {
            let dans_les_temps = f.le <= butoir;
            m.insert("dans_les_temps".into(), json!(dans_les_temps));
            (
                FAIT,
                format!(
                    "Fait le {} ({}){}.",
                    f.le.jj_mm_aaaa(),
                    f.comment,
                    if dans_les_temps { String::new() } else { format!(", après la date butoir du {}", butoir.jj_mm_aaaa()) }
                ),
            )
        }
        None if depart.is_some_and(|d| d.jour > aujourdhui) => (A_VENIR, format!("À venir : {}, soit au plus tard {}.", delai, quand_butoir)),
        None if restants >= 0 => (
            A_FAIRE,
            format!(
                "À faire au plus tard {}{}.",
                quand_butoir,
                match restants {
                    0 => " : c'est aujourd'hui".to_string(),
                    n => format!(" : il reste {} jour{}", n, s(n)),
                }
            ),
        ),
        None => (EN_RETARD, format!("En retard de {} jour{} : c'était à faire au plus tard {}.", -restants, s(-restants), quand_butoir)),
    };
    poser_l_etat(m, etat);
    m.insert("jours_restants".into(), if fait.is_some() { Value::Null } else { json!(restants) });
    m.insert(
        "fait".into(),
        match fait {
            Some(f) => {
                let mut d = f.detail;
                if let Some(o) = d.as_object_mut() {
                    o.insert("le".into(), json!(f.le.iso()));
                }
                d
            }
            None => Value::Null,
        },
    );
    m.insert("phrase".into(), json!(phrase));
}


/// Ce que l'ecran dit d'une obligation dans l'etat `etat` : est-elle DUE par la compagnie (en retard, a faire, un delai
/// qui ne se compte pas encore faute d'un fait), et sa pastille (`routes::pastille`).
fn poser_l_etat(m: &mut Map<String, Value>, etat: &str) {
    let (du, couleur, mots) = match etat {
        EN_RETARD => (true, EN_RETARD, "en retard"),
        A_FAIRE => (true, A_FAIRE, "à faire"),
        INDETERMINE => (true, A_FAIRE, "délai à préciser"),
        FAIT => (false, FAIT, "fait"),
        A_VENIR => (false, "information", "à venir"),
        _ => (false, "information", "à la charge du salarié"),
    };
    m.insert("etat".into(), json!(etat));
    m.insert("du".into(), json!(du));
    m.insert("pastille".into(), crate::routes::pastille(couleur, mots));
    // Le plus presse d'abord, la ou on le montre : ce qui est en retard, puis le reste (par date butoir).
    m.insert("priorite".into(), json!(if etat == EN_RETARD { 0 } else { 1 }));
}

fn indetermine(mut m: Map<String, Value>, pourquoi: String) -> Value {
    poser_l_etat(&mut m, INDETERMINE);
    m.insert("butoir".into(), Value::Null);
    m.insert("butoir_heure".into(), Value::Null);
    m.insert("jours_restants".into(), Value::Null);
    m.insert("fait".into(), Value::Null);
    m.insert("phrase".into(), json!(pourquoi));
    Value::Object(m)
}

/// Le premier envoi de cette nature trace sur l'arret (`marques_depot::marquer_envoye_depuis`, apres un depot REEL).
fn envoi(a: &Value, nature: &str) -> Option<Fait> {
    let mut premier: Option<(String, Value)> = None;
    for e in a.get("envois").and_then(Value::as_array).into_iter().flatten() {
        if e.get("nature").and_then(Value::as_str) != Some(nature) {
            continue;
        }
        let quand = texte(e, "quand");
        if quand.len() >= 10 && premier.as_ref().is_none_or(|(q, _)| quand < *q) {
            premier = Some((quand, e.clone()));
        }
    }
    let (quand, e) = premier?;
    let le = Jour::lire(&quand[..10]).ok()?;
    Some(Fait {
        le,
        comment: "déposé sur net-entreprises ; le bilan dit s'il est accepté".into(),
        detail: json!({"quand": quand, "fichier": e.get("fichier"), "ordre": e.get("ordre"), "comment": "depot"}),
    })
}

/// LE SIGNALEMENT RECTIFICATIF EST-IL DU ? Quand un avis de prolongation (`prolongations`, `crate::routes::imprevus::arret_prolonger`)
/// a change la fin de subrogation APRES le premier depot du signalement d'arret (04) : la subrogation partie etait
/// « insuffisante », et « l'employeur doit alors transmettre un nouveau signalement de nature " arret de travail " de type
/// annule et remplace, dans lequel une nouvelle date de fin de subrogation sera renseignee » (fiche net-entreprises 1983,
/// `atelier/reference/sources/net-entreprises-base-connaissances/net-entreprises-fiche1983-prolongation-arret-dates-de-subrogation-20250322.txt`,
/// l. 37). Le depart : le jour ou la derniere de ces prolongations a ete sue (`le`). Fait : un signalement d'arret de type
/// 03 (annule et remplace) depose apres sa saisie. L'ordre se lit sur les heures de saisie et de depot, toutes deux a
/// l'horloge (`saisi_le`, `quand`).
fn rectificatif_du(a: &Value) -> Option<(Depart, Option<Fait>)> {
    let envois: Vec<&Value> = a.get("envois").and_then(Value::as_array).into_iter().flatten().filter(|e| texte(e, "nature") == "04").collect();
    let premier = envois.iter().map(|e| texte(e, "quand")).filter(|q| q.len() >= 10).min()?;
    let p = a
        .get("prolongations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|p| p.get("subrogation_fin") != p.get("subrogation_fin_avant") && texte(p, "saisi_le") > premier)
        .last()?;
    let saisi = texte(p, "saisi_le");
    let depart = Depart { jour: jour(p.get("le")?)?, minute: None, quoi: "le jour où la prolongation de la subrogation a été saisie".into(), presume: None };
    let fait = envois
        .iter()
        .filter(|e| texte(e, "type") == declarations::signalement::ANNULE_ET_REMPLACE && texte(e, "quand") >= saisi)
        .map(|e| texte(e, "quand"))
        .min()
        .and_then(|quand| {
            let le = Jour::lire(quand.get(..10)?).ok()?;
            Some(Fait {
                le,
                comment: "annule et remplace déposé sur net-entreprises ; le bilan dit s'il est accepté".into(),
                detail: json!({"quand": quand, "comment": "depot"}),
            })
        });
    Some((depart, fait))
}

/// Ce que la compagnie a dit fait d'une obligation hors DSN (`champ` de sa fiche de suivi : `accident_declare`,
/// `attestation_salaire`) : le jour, et ou (le libelle court du code de `o.ou`).
fn dit_fait(fiche: &Value, champ: &str, o: &Value) -> Option<Fait> {
    let v = fiche.get(champ).filter(|v| v.is_object())?;
    let le = jour(v.get("le").unwrap_or(&Value::Null))?;
    let ou = texte(v, "depot");
    let libelle = o["ou"].as_array().and_then(|l| l.iter().find(|x| texte(x, "code") == ou)).map(|x| texte(x, "court")).unwrap_or(ou);
    Some(Fait { le, comment: format!("dit par la compagnie : {}", libelle), detail: v.clone() })
}

/// Un signalement DSN (04 ou 05) : cinq jours ouvres sans subrogation, avec la mensuelle du mois avec.
#[allow(clippy::too_many_arguments)]
fn signalement_dsn(
    ctx: &Contexte,
    cal: &mut Calendrier,
    aujourdhui: Jour,
    cle: &str,
    o: &Value,
    subrogation: &Value,
    depart: Depart,
    evenement: Jour,
    fait: Option<Fait>,
) -> R<Value> {
    let mut m = squelette(cle, o);
    if !vrai(subrogation) {
        return Ok(indetermine(
            m,
            "La subrogation de cet arrêt n'est pas dite : c'est elle qui fixe le délai (cinq jours ouvrés sans subrogation, avec la DSN du mois avec ; CSS R. 133-14, II).".into(),
        ));
    }
    if en_texte(subrogation) == code_subrogation_oui()? {
        let d = &o["delai_avec_subrogation"];
        // SUBROGE, la date butoir est l'echeance de la DSN mensuelle du mois de l'evenement (R. 133-14, II, premier
        // alinea), telle que `echeance_dsn` la donne : reportee au jour ouvre suivant un samedi, un dimanche ou un ferie
        // (R. 133-14, I), a midi (cahier technique § 1.4.2). INFERENCE : le report de la mensuelle vaut pour le signalement
        // qui part avec elle (le texte du report parle de « la declaration sociale nominative » sans distinguer).
        let echeance = declarations::echeance_dsn::de_la_compagnie(ctx, &evenement.mois_iso()).map_err(|m| erreur("ValueError", m))?;
        m.insert("subrogation".into(), json!(true));
        m.insert("echeance_dsn".into(), echeance.en_json());
        // La connaissance ne compte pas : le delai court du mois de l'evenement.
        let ev = Depart { jour: evenement, minute: None, quoi: "le jour de l'événement (sa DSN mensuelle est celle de son mois)".into(), presume: None };
        conclure(&mut m, aujourdhui, Some(&ev), echeance.jour, Some(declarations::echeance_dsn::MINUTE_LIMITE), &texte(d, "dit"), fait);
    } else {
        let d = &o["delai_sans_subrogation"];
        let butoir = cal.jours_ouvres_apres(depart.jour, entier(d, "jours_ouvres")?)?;
        m.insert("subrogation".into(), json!(false));
        conclure(&mut m, aujourdhui, Some(&depart), butoir, None, &texte(d, "dit"), fait);
    }
    Ok(Value::Object(m))
}

/// La connaissance saisie (`{"jour", "heure"}`), sinon `None`.
fn connaissance(suivi: &Value, champ: &str, quoi: &str) -> Option<Depart> {
    let c = suivi.get(champ)?;
    let j = jour(c.get("jour")?)?;
    Some(Depart { jour: j, minute: c.get("heure").and_then(Value::as_str).and_then(lire_heure), quoi: quoi.into(), presume: None })
}

// ------------------------------------------------------------- les arrets

/// Les obligations d'un arret (`a` : l'arret tel que `imprevus/AAAA-MM.json` le porte), dans l'ordre ou elles se font.
pub fn de_l_arret(ctx: &Contexte, a: &Value, suivi: &Suivi, aujourdhui: Jour) -> R<Vec<Value>> {
    let n = base()?;
    let mut cal = Calendrier::nouveau(Some(&ctx.entreprise));
    let mut out = Vec::new();
    let salarie = texte(a, "salarie");
    let dernier = texte(a, "dernier_jour_travaille");
    let fiche = suivi.de(&salarie, &dernier);
    let motif = texte(a, "motif");
    let subrogation = a.get("subrogation").cloned().unwrap_or(Value::Null);
    let debut = match jour(&json!(dernier)) {
        Some(j) => Some(plus(j, 1)?),
        None => None,
    };

    // LE SIGNALEMENT D'ARRET (04) : pas pour un motif que la DSN ne couvre pas (`sauf_motifs`).
    let o = obligation(&n, "signalement_arret")?;
    if !sauf_motifs(&o).contains(&motif) {
        match debut {
            None => out.push(indetermine(squelette("signalement_arret", &o), "Le dernier jour travaillé de cet arrêt est illisible : le délai ne se compte pas.".into())),
            Some(debut) => {
                let depart = connaissance(&fiche, "connaissance_arret", "le jour où la compagnie a connu l'arrêt").unwrap_or(Depart {
                    jour: debut,
                    minute: None,
                    quoi: "le jour où la compagnie a connu l'arrêt".into(),
                    presume: Some(texte(&o, "connaissance_presumee")),
                });
                out.push(signalement_dsn(ctx, &mut cal, aujourdhui, "signalement_arret", &o, &subrogation, depart, debut, envoi(a, "04"))?);
            }
        }
    }

    // LE SIGNALEMENT D'ARRET RECTIFICATIF (04, annule et remplace) : une fin de subrogation prolongee APRES le depot du
    // signalement d'arret, qui la portait plus courte (fiche net-entreprises 1983 ; `rectificatif_du`).
    let o = obligation(&n, "signalement_arret_rectificatif")?;
    if let Some((depart, fait)) = rectificatif_du(a) {
        let mut m = squelette("signalement_arret_rectificatif", &o);
        let d = &o["delai"];
        let butoir = cal.jours_ouvres_apres(depart.jour, entier(d, "jours_ouvres")?)?;
        conclure(&mut m, aujourdhui, Some(&depart), butoir, None, &texte(d, "dit"), fait);
        out.push(Value::Object(m));
    }

    // L'ATTESTATION DE SALAIRE HORS DSN (le deuil d'enfant) : des reception de l'avis d'arret, soit -- inference dite --
    // le jour meme ou la compagnie connait l'arret.
    let o = obligation(&n, "attestation_salaire")?;
    if motifs_de(&o).contains(&motif) {
        let mut m = squelette("attestation_salaire", &o);
        match debut {
            None => out.push(indetermine(m, "Le dernier jour travaillé de cet arrêt est illisible : le jour de l'attestation ne se compte pas.".into())),
            Some(debut) => {
                let d = &o["delai"];
                let depart = connaissance(&fiche, "connaissance_arret", "le jour où la compagnie a connu l'arrêt").unwrap_or(Depart {
                    jour: debut,
                    minute: None,
                    quoi: "le jour où la compagnie a connu l'arrêt".into(),
                    presume: Some(texte(&o, "connaissance_presumee")),
                });
                let butoir = plus(depart.jour, entier(d, "jours")?)?;
                let fait = dit_fait(&fiche, "attestation_salaire", &o);
                conclure(&mut m, aujourdhui, Some(&depart), butoir, None, &texte(d, "dit"), fait);
                out.push(Value::Object(m));
            }
        }
    }

    // LA DECLARATION D'ACCIDENT DU TRAVAIL OU DE TRAJET (hors DSN).
    let o = obligation(&n, "declaration_accident")?;
    if motifs_de(&o).contains(&motif) {
        let mut m = squelette("declaration_accident", &o);
        let d = &o["delai"];
        let depart = connaissance(&fiche, "connaissance_accident", "la connaissance de l'accident par la compagnie").or_else(|| {
            jour(a.get("date_accident").unwrap_or(&Value::Null)).map(|j| Depart {
                jour: j,
                minute: None,
                quoi: "la connaissance de l'accident par la compagnie".into(),
                presume: Some(texte(&o, "connaissance_presumee")),
            })
        });
        match depart {
            None => out.push(indetermine(m, "La date de l'accident n'est pas saisie : les quarante-huit heures ne se comptent pas.".into())),
            Some(depart) => {
                let (butoir, minute) = cal.heures_apres(depart.jour, depart.minute.unwrap_or(0), entier(d, "heures")?)?;
                let fait = dit_fait(&fiche, "accident_declare", &o);
                // LES SUITES : une fois declaree, les reserves motivees (R. 441-6) ; la feuille d'accident (L. 441-5).
                let mut suites = Vec::new();
                if let Some(r) = o["suites"].get("reserves") {
                    let mut s = json!({"cle": "reserves", "libelle": r["libelle"], "texte": r["texte"], "inference": r["inference"]});
                    if let Some(f) = &fait {
                        let jusqu = plus(f.le, entier(r, "jours_francs")?)?;
                        s["jusqu_au"] = json!(jusqu.iso());
                        s["phrase"] = json!(if jusqu >= aujourdhui {
                            format!("Des réserves motivées peuvent encore s'émettre jusqu'au {}.", lisible(&jusqu))
                        } else {
                            format!("Le délai des réserves motivées est passé (le {}).", jusqu.jj_mm_aaaa())
                        });
                    } else {
                        s["phrase"] = json!(format!("Des réserves motivées, s'il y en a : dans les {} jours francs qui suivent la déclaration.", entier(r, "jours_francs")?));
                    }
                    suites.push(s);
                }
                if let Some(f) = o["suites"].get("feuille_accident") {
                    suites.push(json!({"cle": "feuille_accident", "libelle": f["libelle"], "texte": f["texte"], "phrase": f["libelle"]}));
                }
                m.insert("suites".into(), json!(suites));
                conclure(&mut m, aujourdhui, Some(&depart), butoir, Some(minute), &texte(d, "dit"), fait);
                out.push(Value::Object(m));
            }
        }
    }

    // LA MALADIE PROFESSIONNELLE : la victime la declare, pas la compagnie.
    let o = obligation(&n, "declaration_maladie_professionnelle")?;
    if motifs_de(&o).contains(&motif) {
        let mut m = squelette("declaration_maladie_professionnelle", &o);
        let d = &o["delai"];
        match debut {
            None => out.push(indetermine(m, "Le dernier jour travaillé de cet arrêt est illisible.".into())),
            Some(debut) => {
                let butoir = plus(debut, entier(d, "jours")?)?;
                m.insert("delai".into(), d["dit"].clone());
                m.insert("depart".into(), Depart { jour: debut, minute: None, quoi: "la cessation du travail".into(), presume: None }.en_json());
                m.insert("butoir".into(), json!(butoir.iso()));
                m.insert("butoir_heure".into(), Value::Null);
                m.insert("jours_restants".into(), Value::Null);
                m.insert("fait".into(), Value::Null);
                poser_l_etat(&mut m, A_LA_CHARGE_DU_SALARIE);
                m.insert(
                    "phrase".into(),
                    json!(format!(
                        "La maladie professionnelle se déclare par le salarié lui-même à sa caisse, au plus tard le {} (CSS L. 461-5 et R. 461-5) : la compagnie n'a pas de déclaration d'accident à faire.",
                        lisible(&butoir)
                    )),
                );
                out.push(Value::Object(m));
            }
        }
    }

    // LE SIGNALEMENT DE REPRISE ANTICIPEE (05) : seulement une reprise avant la fin previsionnelle, et pas pour un motif que
    // la DSN ne couvre pas (`sauf_motifs`).
    let o = obligation(&n, "signalement_reprise")?;
    if let (Some(rep), Some(fin)) = (jour(a.get("reprise").unwrap_or(&Value::Null)), jour(a.get("fin_previsionnelle").unwrap_or(&Value::Null))) {
        if rep < fin && !sauf_motifs(&o).contains(&motif) {
            let depart = Depart { jour: rep, minute: None, quoi: "la reprise".into(), presume: None };
            out.push(signalement_dsn(ctx, &mut cal, aujourdhui, "signalement_reprise", &o, &subrogation, depart, rep, envoi(a, "05"))?);
        }
    }
    Ok(out)
}

// -------------------------------------------------------- les fins de contrat

/// Le signalement de fin de contrat (07) d'un contrat qui finit le `fin`. `cle` : la cle de la fiche du salarie (le nom
/// du fichier que `fin-contrat-signaler` ecrit) ; `journal` : les lignes du journal des depots.
pub fn de_la_fin(ctx: &Contexte, cle: Option<&str>, fin: Jour, journal: &[Value], aujourdhui: Jour) -> R<Value> {
    let n = base()?;
    let o = obligation(&n, "signalement_fin_contrat")?;
    let mut cal = Calendrier::nouveau(Some(&ctx.entreprise));
    let mut m = squelette("signalement_fin_contrat", &o);
    let d = &o["delai"];
    let butoir = cal.jours_ouvres_apres(fin, entier(d, "jours_ouvres")?)?;
    let fait = cle.and_then(|c| fctu_depose(journal, c, &fin));
    let depart = Depart { jour: fin, minute: None, quoi: "la fin du contrat".into(), presume: None };
    conclure(&mut m, aujourdhui, Some(&depart), butoir, None, &texte(d, "dit"), fait);
    Ok(Value::Object(m))
}

/// Le premier depot REEL, non refuse au bilan, du signalement de fin de ce contrat : une ligne `dsn` du journal, au jour de la fin
/// (`Apercu::evenement_lisible`), dont le fichier est celui que `fin-contrat-signaler` ecrit pour ce salarie.
fn fctu_depose(journal: &[Value], cle: &str, fin: &Jour) -> Option<Fait> {
    let nom = format!("SignalementFinContrat_{}_{}.dsn", cle.to_uppercase(), fin.aaaammjj());
    journal
        .iter()
        .filter(|l| texte(l, "quoi") == "dsn" && texte(l, "mode") == envois::journal::REEL && l.get("accepte").is_some_and(vrai))
        // Un signalement refuse au bilan n'est pas recu (cahier technique DSN, `norme-dsn/texte/chapitre-4-controles.txt` §4.2) : il reste du.
        .filter(|l| !envois::etats::refuse_par_un_retour(l).unwrap_or(false))
        .filter(|l| texte(l, "mois") == fin.iso() && texte(l, "fichier").to_uppercase() == nom.to_uppercase())
        .filter_map(|l| {
            let quand = texte(l, "quand");
            let le = Jour::lire(quand.get(..10)?).ok()?;
            Some((quand, le, l))
        })
        .min_by(|a, b| a.0.cmp(&b.0))
        .map(|(quand, le, l)| Fait {
            le,
            comment: "déposé sur net-entreprises ; le bilan dit s'il est accepté".into(),
            detail: json!({"quand": quand, "fichier": l.get("fichier"), "flux": l.get("flux"), "comment": "depot"}),
        })
}

// ------------------------------------------------------------------ les routes

/// Une route de ce module.
pub fn route(ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "arret-connaissance" => arret_connaissance(ctx, charge),
        "accident-declare" => accident_declare(ctx, charge),
        "attestation-declare" => attestation_declare(ctx, charge),
        "arret-obligations" => arret_obligations(ctx, charge),
        autre => Err(erreur("ErreurOperation", format!("route inconnue : {}", autre))),
    }
}

fn refus(m: impl Into<String>) -> socle::echec::Echec {
    erreur("ErreurOperation", m)
}

fn requis<'a>(charge: &'a Value, k: &str) -> R<&'a Value> {
    crate::routes::fiches::exiger(charge, k)
}

/// L'arret de ce salarie a ce dernier jour travaille : (la cle du salarie, l'arret).
fn l_arret(ctx: &Contexte, charge: &Value) -> R<(String, Value)> {
    let nom = en_texte(requis(charge, "salarie")?);
    let dernier = en_texte(requis(charge, "dernier_jour")?);
    let fiche = ctx.salarie(&nom).map_err(|m| erreur("KeyError", m))?;
    let cle = fiche.get("_cle").map(en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("_cle")))?;
    for a in crate::routes::imprevus::tous_les_arrets(ctx)? {
        if texte(&a, "salarie") == cle && texte(&a, "dernier_jour_travaille") == dernier {
            return Ok((cle, a));
        }
    }
    Err(refus(format!("Aucun arret de {} dont le dernier jour travaille soit le {}. Rien n'est ecrit.", cle, dernier)))
}

/// Un jour saisi : vide pour effacer, sinon une date lisible qui n'est pas dans le futur.
fn jour_saisi(ctx: &Contexte, v: &Value, quoi: &str) -> R<Option<Jour>> {
    let t = en_texte(v);
    if t.trim().is_empty() {
        return Ok(None);
    }
    let j = Jour::lire(t.trim()).map_err(|m| refus(format!("{} : {}. Rien n'est ecrit.", quoi, m)))?;
    if j > ctx.aujourdhui {
        return Err(refus(format!(
            "{} au {} : c'est apres aujourd'hui ({}). Un fait deja survenu ne se date pas dans le futur. Rien n'est ecrit.",
            quoi,
            j.jj_mm_aaaa(),
            ctx.aujourdhui.jj_mm_aaaa()
        )));
    }
    Ok(Some(j))
}

fn heure_saisie(charge: &Value) -> R<Option<String>> {
    match charge.get("heure").map(en_texte).filter(|h| !h.trim().is_empty()) {
        None => Ok(None),
        Some(h) => match lire_heure(&h) {
            Some(m) => Ok(Some(heure_lisible(m))),
            None => Err(refus(format!("Heure {} illisible : HH:MM, de 00:00 a 23:59. Rien n'est ecrit.", repr(&h)))),
        },
    }
}

fn reponse(ctx: &Contexte, a: &Value) -> R<Value> {
    let suivi = Suivi::lire(ctx)?;
    Ok(json!({"obligations": de_l_arret(ctx, a, &suivi, ctx.aujourdhui)?}))
}

/// `arret-connaissance` : `{salarie, dernier_jour, de: "arret"|"accident", jour, heure?}` -- le jour (et l'heure) ou la
/// compagnie a connu l'arret ou l'accident ; un `jour` vide efface ce qui etait dit (le moteur reprend sa presomption).
fn arret_connaissance(ctx: &Contexte, charge: &Value) -> R<Value> {
    let (cle, a) = l_arret(ctx, charge)?;
    let de = en_texte(requis(charge, "de")?);
    let champ = match de.as_str() {
        "arret" => "connaissance_arret",
        "accident" => "connaissance_accident",
        autre => return Err(refus(format!("`de` vaut \"arret\" ou \"accident\", pas {}. Rien n'est ecrit.", repr(autre)))),
    };
    if de == "accident" && !motif_appelle_la_declaration(&texte(&a, "motif"))? {
        return Err(refus(format!(
            "L'arret de {} (motif {}) n'est pas un accident du travail ou de trajet : il n'y a pas de declaration d'accident, ni de connaissance de l'accident a dire. Rien n'est ecrit.",
            cle,
            texte(&a, "motif")
        )));
    }
    let j = jour_saisi(ctx, requis(charge, "jour")?, "La connaissance")?;
    let heure = heure_saisie(charge)?;
    if let (Some(j), "accident", Some(acc)) = (j, de.as_str(), jour(a.get("date_accident").unwrap_or(&Value::Null))) {
        if j < acc {
            return Err(refus(format!(
                "Connu le {}, avant l'accident lui-meme (le {}) : ce n'est pas possible. Rien n'est ecrit.",
                j.jj_mm_aaaa(),
                acc.jj_mm_aaaa()
            )));
        }
    }
    let valeur = match j {
        None => Value::Null,
        Some(j) => json!({"jour": j.iso(), "heure": heure, "saisi_le": crate::routes::maintenant()}),
    };
    let mut suivi = Suivi::lire(ctx)?;
    suivi.ecrire(ctx, &cle, &texte(&a, "dernier_jour_travaille"), champ, valeur)?;
    reponse(ctx, &a)
}

/// `accident-declare` : `{salarie, dernier_jour, le, depot, heure?, reference?}` -- la declaration d'accident du travail
/// faite, dite par la compagnie : le jour, ou (un code de `declaration_accident.ou`), et la reference qu'elle en a (le
/// numero de l'accuse de net-entreprises, celui du recommande). Un `le` vide la retire.
fn accident_declare(ctx: &Contexte, charge: &Value) -> R<Value> {
    dire_faite(ctx, charge, &Faite {
        obligation: "declaration_accident",
        champ: "accident_declare",
        connaissance: "connaissance_accident",
        hors_motif: "n'est pas un accident du travail ou de trajet : il n'y a pas de declaration d'accident a dire faite",
        quoi: "La declaration",
        avant: "que la compagnie ait connu l'accident",
        accident: true,
    })
}

/// `attestation-declare` : `{salarie, dernier_jour, le, depot, heure?, reference?}` -- l'attestation de salaire pour les
/// indemnites journalieres faite hors DSN (le deuil d'enfant, `attestation_salaire`), dite par la compagnie : le jour, ou
/// (un code de `attestation_salaire.ou` : `net-entreprises-ecran`, `net-entreprises-edi`), et la reference de l'accuse.
/// Un `le` vide la retire. Refuse un arret dont le motif ne l'appelle pas, un jour futur, un lieu inconnu, un jour avant la
/// connaissance de l'arret saisie : rien n'est alors ecrit.
fn attestation_declare(ctx: &Contexte, charge: &Value) -> R<Value> {
    dire_faite(ctx, charge, &Faite {
        obligation: "attestation_salaire",
        champ: "attestation_salaire",
        connaissance: "connaissance_arret",
        hors_motif: "est couvert par la DSN : l'attestation de salaire passe par le signalement d'arret, il n'y a pas d'attestation hors DSN a dire faite",
        quoi: "L'attestation",
        avant: "que la compagnie ait connu l'arret",
        accident: false,
    })
}

/// Une obligation hors DSN que la compagnie dit faite : laquelle, ou elle se garde dans la fiche de suivi, et ce qui la
/// borne.
struct Faite {
    obligation: &'static str,
    champ: &'static str,
    connaissance: &'static str,
    hors_motif: &'static str,
    quoi: &'static str,
    avant: &'static str,
    /// Bornee aussi par la date de l'accident (la declaration d'accident).
    accident: bool,
}

fn dire_faite(ctx: &Contexte, charge: &Value, f: &Faite) -> R<Value> {
    let (cle, a) = l_arret(ctx, charge)?;
    let motif = texte(&a, "motif");
    if !motif_appelle(f.obligation, &motif)? {
        return Err(refus(format!("L'arret de {} (motif {}) {}. Rien n'est ecrit.", cle, motif, f.hors_motif)));
    }
    let le = jour_saisi(ctx, requis(charge, "le")?, f.quoi)?;
    let valeur = match le {
        None => Value::Null,
        Some(le) => {
            let depot = en_texte(requis(charge, "depot")?);
            let o = obligation(&*base()?, f.obligation)?;
            let codes: Vec<String> = o["ou"].as_array().map(|l| l.iter().map(|x| texte(x, "code")).collect()).unwrap_or_default();
            if !codes.contains(&depot) {
                return Err(refus(format!("Ou {} a-t-elle ete faite ? {} n'est pas connu ; les lieux sont : {}. Rien n'est ecrit.", f.quoi.to_lowercase(), repr(&depot), codes.join(", "))));
            }
            let fiche = Suivi::lire(ctx)?.de(&cle, &texte(&a, "dernier_jour_travaille"));
            if let Some(c) = connaissance(&fiche, f.connaissance, "") {
                if le < c.jour {
                    return Err(refus(format!(
                        "{} faite le {}, avant {} (le {}, saisi) : ce n'est pas possible. Rien n'est ecrit.",
                        f.quoi,
                        le.jj_mm_aaaa(),
                        f.avant,
                        c.jour.jj_mm_aaaa()
                    )));
                }
            }
            if f.accident {
                if let Some(acc) = jour(a.get("date_accident").unwrap_or(&Value::Null)) {
                    if le < acc {
                        return Err(refus(format!(
                            "Declaree le {}, avant l'accident lui-meme (le {}) : ce n'est pas possible. Rien n'est ecrit.",
                            le.jj_mm_aaaa(),
                            acc.jj_mm_aaaa()
                        )));
                    }
                }
            }
            json!({
                "le": le.iso(),
                "heure": heure_saisie(charge)?,
                "depot": depot,
                "reference": charge.get("reference").map(en_texte).unwrap_or_default(),
                "saisi_le": crate::routes::maintenant(),
            })
        }
    };
    let mut suivi = Suivi::lire(ctx)?;
    suivi.ecrire(ctx, &cle, &texte(&a, "dernier_jour_travaille"), f.champ, valeur)?;
    reponse(ctx, &a)
}

/// `arret-obligations` : `{motif, dernier_jour, fin?, subrogation?, date_accident?, reprise?, connaissance_accident?,
/// connaissance_heure?, connaissance_arret?}` -- les obligations qu'un arret AINSI SAISI ferait naitre, avant d'etre enregistre (la boite de
/// saisie les montre). Rien n'est ecrit, rien n'est verifie de l'arret lui-meme (c'est `arret-enregistrer` qui refuse) :
/// un champ vide ou illisible donne une obligation `indetermine`, qui dit pourquoi.
fn arret_obligations(ctx: &Contexte, charge: &Value) -> R<Value> {
    let champ = |k: &str| charge.get(k).map(en_texte).unwrap_or_default();
    let a = json!({
        "salarie": "", "motif": champ("motif"), "dernier_jour_travaille": champ("dernier_jour"),
        "fin_previsionnelle": champ("fin"), "subrogation": champ("subrogation"), "date_accident": champ("date_accident"),
        "reprise": champ("reprise"), "envois": [],
    });
    let mut fiche = json!({"salarie": "", "dernier_jour_travaille": champ("dernier_jour")});
    if !champ("connaissance_accident").trim().is_empty() {
        fiche["connaissance_accident"] = json!({"jour": champ("connaissance_accident"), "heure": champ("connaissance_heure")});
    }
    if !champ("connaissance_arret").trim().is_empty() {
        fiche["connaissance_arret"] = json!({"jour": champ("connaissance_arret")});
    }
    let suivi = Suivi { fiches: vec![fiche] };
    Ok(json!({"obligations": de_l_arret(ctx, &a, &suivi, ctx.aujourdhui)?}))
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn une_heure_se_lit_en_minutes() {
        assert_eq!(lire_heure("10:30"), Some(630));
        assert_eq!(lire_heure("24:00"), None);
        assert_eq!(lire_heure("9h"), None);
    }
}
