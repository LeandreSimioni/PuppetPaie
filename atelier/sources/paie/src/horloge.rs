//! L'horloge de la compagnie fictive (`moteur/simulation.py`, a partir de `QUI_HORLOGE`) : `rattraper`, `dernier_rattrapage`
//! et tout ce qu'ils appellent -- lot I1.
//!
//! ECRIT, NON PROUVE, NON BRANCHE. « Que l'horloge avance automatiquement a la date limite » (l'editeur, 27/09/2026) :
//! a chaque ouverture, la COMPAGNIE FICTIVE joue ce qu'une compagnie bien tenue aurait fait AVANT aujourd'hui,
//! a sa date. Le 1er du mois, le mois s'ecrit (contrats, DPAE du mois, bulletins, certificats, DSN) ; le jour de chaque
//! embauche, a 7 h, sa DPAE part (test puis reel) et son contrat revient signe a 8 h ; a l'echeance de la DSN, la DSN
//! part (test puis reel) et les documents aux salaries ; fin janvier, l'annee passee se clot et s'exporte pour la
//! comptabilite ; les echeances annuelles qu'on coche se cochent a la fin de leur fenetre. Ce qui vient apres aujourd'hui
//! reste au visiteur. Ce qu'il a deja fait lui-meme n'est pas refait : chaque pas regarde d'abord le journal.
//!
//! ELLE NE JOUE QUE DANS UNE COMPAGNIE FICTIVE, ET LE CODE LE DIT A CHAQUE ENDROIT QUI AGIT.
//! - `rattraper` commence par `HorlogeFictive::verifier` : sans compagnie fictive active (`simulation_active` : le paquet
//!   soude, ou une compagnie dont le marqueur dit « compagnie fictive »), elle rend une liste vide et ne
//!   touche a rien -- pas meme `config/horloge.json`.
//! - Tout geste qui agit recoit la `HorlogeFictive` et la RE-VERIFIE avant d'agir (`pas`, `deposer_*_joue`) : un marqueur
//!   change en route arrete le pas, il ne le laisse pas partir vers un vrai point de depot.
//! - Les depots ne passent que par `depot_acte::deposer_dsn` et `deposer_dpae`, qui jouent la reponse `FICTIF-...` quand la
//!   simulation est active et ne s'ouvrent jamais autrement ici (la re-verification les precede) ; aucune preuve, aucun
//!   secret, aucun coffre n'est lu.
//! - Les courriels ne passent que par `BoiteMuette` : un client du trait `Courriel` qui rend un identifiant et ne garde
//!   rien -- ni `courriels-simules/` (qui montre au visiteur ce que SES gestes envoient), ni la vraie boite. Ce module
//!   n'appelle jamais `routes_envois::client_courriel`.
//!
//! Elle ne leve jamais pour un PAS : un pas en echec devient sa phrase (« Le jj/mm/aaaa : ... — non joue (...) ») et les
//! autres continuent. Seuls les refus du decor (une fiche entreprise illisible, une echeance introuvable) remontent.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::contexte::Contexte;
use crate::courriel::{Courriel, ErreurCourriel};
use crate::date::{jours_du_mois, Jour};
use crate::depot_acte::{self, Preuve};
use crate::etats::{self, erreur, py_str, R};
use crate::fichiers::{dossier_du_code, vrai};
use crate::retours::Echec;
use crate::routes_mois::Moteur;

/// Qui signe, au journal, ce que l'horloge joue.
pub const QUI_HORLOGE: &str = "horloge";
/// La memoire de l'horloge, dans `config/`.
pub const FICHIER_HORLOGE: &str = "horloge.json";
/// Les routes de ce module.
pub const ROUTES: [&str; 2] = ["horloge-rattraper", "horloge-derniere"];

const ORIGINE: &str = "compagnie fictive : joue par l'horloge (moteur/simulation.py)";
const QUOI: &str = "L'horloge de la compagnie fictive (moteur/simulation.py) : le dernier jour joue, ce qui n'est plus a regarder, et ce qu'elle a fait en dernier (le bandeau le dit).";

// ------------------------------------------------------------------ la garde

/// LA GARDE : la preuve que la racine est une COMPAGNIE FICTIVE, exigee de tout ce qui agit. Elle ne se fabrique
/// que par `verifier`, qui lit `simulation_active` ; `reverifier` la relit avant chaque geste.
pub struct HorlogeFictive<'a> {
    racine: &'a Path,
    /// LE JOUR JOUE : celui que l'horloge rattrape. Chaque contexte qu'elle ouvre porte ce jour (`aujourdhui` ET `date`),
    /// jamais l'horloge reelle de la machine : l'ancien Python jugeait la cloture d'une annee, l'export, le jour inscrit
    /// a l'echeancier, la DPAE sur `date.today()`, et rejouee dans le futur d'une copie la compagnie fictive disait « exercice
    /// 2026 clos -- non joue » (defaut du Python, corrige ici ; le contexte de l'interface, lui, garde la date reelle).
    jour: Jour,
}

impl<'a> HorlogeFictive<'a> {
    /// `simulation.active(ctx)` : `None` si les envois de cette racine ne sont pas joues.
    pub fn verifier(racine: &'a Path) -> Option<HorlogeFictive<'a>> {
        crate::routes_accueil::simulation_active(racine).then(|| HorlogeFictive { racine, jour: crate::contexte::jour_reel() })
    }

    /// Le jour que cette compagnie fictive joue.
    fn au_jour(self, jour: Jour) -> HorlogeFictive<'a> {
        HorlogeFictive { jour, ..self }
    }

    /// Un contexte AU JOUR JOUE (`date` et `aujourdhui`), a la place de `Contexte(racine)` du Python.
    fn contexte(&self) -> R<Contexte> {
        self.contexte_a(self.jour)
    }

    /// Un contexte lu a `date` (la fin du mois qu'on ecrit), dont `aujourdhui` reste le jour joue.
    fn contexte_a(&self, date: Jour) -> R<Contexte> {
        let mut ctx = Contexte::nouveau(self.racine, Some(date)).map_err(Echec::from)?;
        ctx.aujourdhui = self.jour;
        Ok(ctx)
    }

    /// Relit le marqueur : refuse tout geste si la racine a cesse d'etre une compagnie fictive.
    fn reverifier(&self) -> R<()> {
        if crate::routes_accueil::simulation_active(self.racine) {
            Ok(())
        } else {
            Err(erreur("ErreurOperation", "Ce dossier n'est pas une compagnie fictive : l'horloge ne joue rien ici."))
        }
    }
}

/// La boite muette du rattrapage : le passe joue n'a pas a remplir `courriels-simules/`, qui montre au visiteur ce que
/// SES gestes envoient. Elle rend un identifiant et ne garde rien.
struct BoiteMuette {
    compte: String,
}

impl BoiteMuette {
    fn nouvelle() -> BoiteMuette {
        BoiteMuette { compte: "fictive@puppetpaie.invalid".to_string() }
    }
}

impl Courriel for BoiteMuette {
    fn compte(&self) -> &str {
        &self.compte
    }

    fn envoyer(&mut self, _message_mime: &[u8]) -> Result<String, ErreurCourriel> {
        Ok(format!("<{}@fictive.puppetpaie>", crate::courriel::hexa_au_hasard(8)))
    }

    fn rechercher(&mut self, _requete: &str, _maximum: usize) -> Result<Vec<String>, ErreurCourriel> {
        Ok(Vec::new())
    }

    fn message_brut(&mut self, _identifiant: &str) -> Result<Vec<u8>, ErreurCourriel> {
        Ok(Vec::new())
    }

    fn fermer(&mut self) {}
}

// ------------------------------------------------------------------ petits outils

/// Les options des actes (`o`) : la racine seulement, l'horloge ne depend d'aucune option de la route.
fn options(racine: &Path) -> HashMap<String, String> {
    HashMap::from([("racine".to_string(), racine.to_string_lossy().into_owned())])
}

/// `_mois_fr` : « octobre 2026 ».
fn mois_fr(mois: &str) -> String {
    // Une ligne du recit ne se refuse pas pour son mois : un mois hors 01-12 s'ecrit tel quel, jamais janvier.
    crate::date::libelle_du_mois_ou_tel_quel(mois, crate::date::Graphie::Accentuee)
}

/// `_le` : `JJ/MM/AAAA` d'un `AAAA-MM-JJ...`.
fn le(iso: &str) -> String {
    let c: Vec<char> = iso.chars().collect();
    let t = |a: usize, b: usize| -> String { c.iter().skip(a).take(b - a).collect() };
    format!("{}/{}/{}", t(8, 10), t(5, 7), t(0, 4))
}

/// `_n_lignes` : combien de lignes le journal des depots porte (0 sans journal).
fn n_lignes(racine: &Path) -> usize {
    let p = crate::journal::chemin(racine);
    std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v.as_array().map(Vec::len)).unwrap_or(0)
}

/// `_dater` : date les lignes du journal ecrites depuis la `depuis`-ieme : un pas joue se lit a la date ou il aurait eu
/// lieu, et la ligne le dit (`origine`).
fn dater(racine: &Path, depuis: usize, quand: &str) {
    let p = crate::journal::chemin(racine);
    let Some(Value::Array(mut lignes)) = std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else {
        return;
    };
    if lignes.len() <= depuis {
        return;
    }
    for l in lignes.iter_mut().skip(depuis) {
        if let Value::Object(o) = l {
            o.insert("quand".into(), json!(quand));
            o.insert("origine".into(), json!(ORIGINE));
        }
    }
    let _ = crate::ecriture::ecrire_json(&p, &Value::Array(lignes), 2);
}

/// `_empreinte_saisie` : le CONTENU d'une saisie, pas ses octets (sha256 du JSON trie, 16 caracteres) : un
/// enregistrement qui ne change rien ne doit pas faire reecrire le mois. Vide si la saisie ne se lit pas.
fn empreinte_saisie(chemin: &Path) -> String {
    let Some(v) = std::fs::read_to_string(chemin).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else {
        return String::new();
    };
    crate::journal::empreinte(crate::ecriture::json_py_trie(&v).as_bytes())
}

/// Les refus que Python rangeait sous `ErreurOperation`.
fn est_une_erreur_d_operation(e: &Echec) -> bool {
    // Un refus du moteur sans classe est, cote Python, un `CalculRefuse` : une `ErreurOperation`.
    e.classe.is_none()
        || matches!(
        e.classe,
        Some(
            "ErreurOperation"
                | "SortieAbsente"
                | "AucunDocument"
                | "BoiteAbsente"
                | "PreuveInsuffisante"
                | "SaisieIncomplete"
                | "MoisPasPret"
                | "ExerciceClos"
                | "MoisHorsDelai"
        )
    )
}

/// `str(exc).splitlines()[0][:160]`, ou le nom de la classe quand le message est vide.
fn motif_de(e: &Echec) -> String {
    if e.message.is_empty() {
        return e.classe.unwrap_or("Exception").to_string();
    }
    e.message.split(['\n', '\r']).next().unwrap_or("").chars().take(160).collect()
}

/// `_pas` : un pas joue -- le geste, ses lignes de journal datees, et sa phrase. Une erreur devient la phrase, sans
/// arreter les autres pas. `geste` rend `true` quand il a fait quelque chose (sa phrase se dit), `false` quand il n'y
/// avait rien a faire (`return False` du Python : ni phrase, ni date).
fn pas(d: &HorlogeFictive, quoi: &str, quand: &str, mut geste: impl FnMut(&Contexte) -> R<bool>) -> Vec<String> {
    let racine = d.racine;
    let echec = |e: &Echec| vec![format!("Le {} : {} — non joué ({})", le(quand), quoi, motif_de(e))];
    let ctx = match d.contexte() {
        Ok(c) => c,
        Err(e) => return echec(&e),
    };
    let n = n_lignes(racine);
    // La re-verification precede TOUT geste : ce qui suit ne s'execute que dans une compagnie fictive.
    let dit = match d.reverifier().and_then(|_| geste(&ctx)) {
        Ok(b) => b,
        Err(e) => return echec(&e),
    };
    dater(racine, n, quand);
    if dit {
        vec![format!("Le {} : {}", le(quand), quoi)]
    } else {
        Vec::new()
    }
}

/// `journal.etat_dsn_du_mois(ctx, mois)` est-il vrai : une DSN REELLE acceptee pour ce mois.
fn dsn_partie(o: &HashMap<String, String>, ctx: &Contexte, mois: &str) -> R<bool> {
    Ok(vrai(&Moteur::nouveau(o, ctx).dsn_du_mois(mois)?))
}

/// `ops.deposer_dsn(ctx, fichier, reel=..., qui=QUI_HORLOGE)` : JOUE. La re-verification precede l'appel.
fn deposer_dsn_joue(d: &HorlogeFictive, o: &HashMap<String, String>, ctx: &Contexte, fichier: &str, reel: bool) -> R<()> {
    d.reverifier()?;
    depot_acte::deposer_dsn(o, ctx, &json!(fichier), reel, None, QUI_HORLOGE, false, "")?;
    Ok(())
}

/// `ops.deposer_dpae(ctx, fichier, reel=..., qui=QUI_HORLOGE)` : JOUE. La re-verification precede l'appel.
fn deposer_dpae_joue(d: &HorlogeFictive, o: &HashMap<String, String>, ctx: &Contexte, fichier: &str, reel: bool) -> R<()> {
    d.reverifier()?;
    depot_acte::deposer_dpae(o, ctx, &json!(fichier), reel, None, QUI_HORLOGE, false, "")?;
    Ok(())
}

/// `simulation.rattacher(ctx, delai=0)` : les retours des depots joues arrivent tout de suite.
fn rattacher(d: &HorlogeFictive, ctx: &Contexte) -> R<()> {
    d.reverifier()?;
    crate::releve_retours::rattacher_simulation(ctx, 0)?;
    Ok(())
}

// ------------------------------------------------------------------ dernier_rattrapage

/// `dernier_rattrapage` : ce que l'horloge a joue aujourd'hui, pour le bandeau « pendant votre absence ». Vide si elle
/// n'a rien joue ce jour-la.
pub fn dernier_rattrapage(racine: &Path, aujourdhui: Option<Jour>) -> R<Vec<String>> {
    let jour = match aujourdhui {
        Some(j) => j,
        None => crate::contexte::aujourdhui().map_err(Echec::from)?,
    }
    .iso();
    let Some(Value::Object(m)) = std::fs::read_to_string(racine.join("config").join(FICHIER_HORLOGE)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok())
    else {
        return Ok(Vec::new());
    };
    let Some(dernier) = m.get("dernier").and_then(Value::as_object) else {
        return Ok(Vec::new());
    };
    if dernier.get("le").and_then(Value::as_str) != Some(jour.as_str()) {
        return Ok(Vec::new());
    }
    Ok(dernier.get("fait").and_then(Value::as_array).map(|a| a.iter().map(py_str).collect()).unwrap_or_default())
}

// ------------------------------------------------------------------ rattraper

/// `rattraper(ctx, aujourdhui, retenir)` : joue ce qui est du avant `aujourdhui` et ne l'a pas ete. Rend ce qui a ete
/// fait, en phrases. Ne leve jamais pour un pas : il devient une phrase, et les autres continuent -- la compagnie fictive
/// s'ouvre toujours. Hors compagnie fictive : liste vide, rien n'est touche.
pub fn rattraper(racine: &Path, aujourdhui: Option<Jour>, retenir: bool) -> R<Vec<String>> {
    // LA GARDE, d'abord et avant tout.
    let Some(d) = HorlogeFictive::verifier(racine) else {
        return Ok(Vec::new());
    };
    let jour = match aujourdhui {
        Some(j) => j,
        None => crate::contexte::aujourdhui().map_err(Echec::from)?,
    };
    let d = d.au_jour(jour);
    // SA MEMOIRE (config/horloge.json) : le dernier jour joue, et les mois dont tout est fait. La veille repasse toutes
    // les vingt minutes : sans elle, chaque passage relisait vingt-quatre mois pour ne rien trouver.
    let memoire_f = racine.join("config").join(FICHIER_HORLOGE);
    let memoire: Map<String, Value> = match std::fs::read_to_string(&memoire_f).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        Some(Value::Object(m)) => m,
        _ => Map::new(),
    };
    // Pas de raccourci « deja passee aujourd'hui » : le visiteur qui ajoute ou retire un contrat attend de trouver, a son
    // retour, tout fait et tout vert (l'editeur, 27/09/2026). Les mois clos, eux, ne se relisent plus.
    let texte_de = |cle: &str| -> BTreeSet<String> {
        memoire.get(cle).and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
    };
    let mut clos: BTreeSet<String> = texte_de("mois_faits");
    let mut saisies: Map<String, Value> = memoire.get("saisies").and_then(Value::as_object).cloned().unwrap_or_default();
    let mut annees_faites: BTreeSet<i64> =
        memoire.get("annees_faites").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_i64).collect()).unwrap_or_default();
    let mut fait: Vec<String> = Vec::new();
    let o = options(racine);

    let saisis = crate::routes_mois::mois_saisis(racine);
    for mois in &saisis {
        let premier = crate::date::premier_du_mois(mois).map_err(Echec::from)?;
        if jour < premier {
            break;
        }
        if clos.contains(mois) {
            continue;
        }
        fait.extend(le_premier_du_mois(&d, &o, mois, &mut saisies, jour)?);
        // Le jour ou la DSN est due (`echeance_dsn`, reporte au jour ouvre) : elle part ce matin-la, avant midi.
        let echeance = {
            let ctx = d.contexte()?;
            crate::echeance_dsn::de_la_compagnie(&ctx, mois).map_err(Echec::from)?.jour
        };
        if jour >= echeance {
            fait.extend(a_l_echeance(&d, &o, mois, echeance));
            let ctx = d.contexte()?;
            if dsn_partie(&o, &ctx, mois)? {
                clos.insert(mois.clone());
            }
        }
    }
    if let Some(premier) = saisis.first() {
        let debut: i32 = premier.get(..4).and_then(|t| t.parse().ok()).unwrap_or(jour.an);
        for annee in debut..=jour.an {
            if annees_faites.contains(&i64::from(annee)) {
                continue;
            }
            fait.extend(l_annee(&d, &o, annee, jour)?);
            if Some(jour) >= Jour::nouveau(annee, 12, 31) {
                annees_faites.insert(i64::from(annee));
            }
        }
    }
    let ancien = memoire.get("dernier").filter(|v| vrai(v)).cloned().unwrap_or_else(|| json!({}));
    let dernier = if !fait.is_empty() && retenir {
        json!({"le": jour.iso(), "fait": fait})
    } else if !retenir {
        json!({})
    } else {
        ancien
    };
    let mut sortie = Map::new();
    sortie.insert("_quoi".into(), json!(QUOI));
    sortie.insert("le".into(), json!(jour.iso()));
    sortie.insert("mois_faits".into(), json!(clos.iter().collect::<Vec<_>>()));
    sortie.insert("annees_faites".into(), json!(annees_faites.iter().collect::<Vec<_>>()));
    sortie.insert("dernier".into(), dernier);
    sortie.insert("saisies".into(), Value::Object(saisies.into_iter().filter(|(k, _)| !clos.contains(k)).collect()));
    crate::ecriture::ecrire_json(&memoire_f, &Value::Object(sortie), 2).map_err(|m| erreur("OSError", m))?;
    Ok(fait)
}

// ------------------------------------------------------------------ le premier du mois

/// `_le_premier_du_mois` : le 1er, le mois s'ecrit ; le jour de chaque embauche, a sa date limite, sa DPAE part (test
/// puis reel) et son contrat revient signe.
fn le_premier_du_mois(d: &HorlogeFictive, o: &HashMap<String, String>, mois: &str, saisies: &mut Map<String, Value>, jour: Jour) -> R<Vec<String>> {
    let mut fait: Vec<String> = Vec::new();
    let quand = format!("{}-01T09:00:00", mois);

    // LE MOIS S'ECRIT, OU SE REECRIT. Une saisie qui a change depuis que le mois a ete ecrit -- un contrat ajoute ou retire
    // par le visiteur -- fait reecrire le mois, tant que sa DSN n'est pas partie : la DSN, les contrats et les DPAE qui
    // suivent sont alors les bons.
    let mut reecrit = false;
    let faits = pas(d, &format!("{} écrit — contrats, bulletins, DSN", crate::routes_envois::capitaliser(&mois_fr(mois))), &quand, |ctx| {
        let connus = crate::routes_accueil::mois_connus(&ctx.racine);
        let saisie = ctx.racine.join("paies").join(format!("{}.json", mois));
        let Some((_, fichiers)) = connus.iter().find(|(m, _)| m == mois) else { return Ok(false) };
        if !saisie.exists() {
            return Ok(false);
        }
        if !fichiers.is_empty() {
            // Le mois est deja ecrit : sa DSN partie, ou une saisie inchangee, le laissent tel quel.
            if dsn_partie(o, ctx, mois)? {
                return Ok(false);
            }
            let maintenant = empreinte_saisie(&saisie);
            match saisies.get(mois) {
                None => {
                    saisies.insert(mois.to_string(), json!(maintenant));
                    return Ok(false);
                }
                Some(connue) if *connue == json!(maintenant) => return Ok(false),
                Some(_) => reecrit = true,
            }
        }
        let fin = crate::date::dernier_du_mois(mois).map_err(Echec::from)?;
        let ctx_fin = d.contexte_a(fin)?;
        crate::produire_mois::produire_pour_l_horloge(o, &ctx_fin, mois, QUI_HORLOGE)?;
        saisies.insert(mois.to_string(), json!(empreinte_saisie(&saisie)));
        Ok(true)
    });
    fait.extend(faits.into_iter().map(|f| if reecrit { f.replace("écrit — ", "réécrit, la saisie ayant changé — ") } else { f }));

    // CHAQUE GESTE A SA DATE LIMITE (l'editeur, 27/09/2026) : « s'il arrive deux jours avant, il peut faire la DPAE ; des
    // qu'on passe la date, c'est vert et deja fait par le moteur de demo » (la compagnie fictive). La DPAE est due avant la premiere heure de
    // travail : l'horloge la fait le jour de l'embauche, a 7 h, et le contrat revient signe le meme matin. D'ici la,
    // c'est au visiteur.
    let jour_iso = jour.iso();
    let jours: BTreeSet<String> = {
        let ctx = d.contexte()?;
        let a_declarer = Moteur::nouveau(o, &ctx).embauches_a_declarer(Some(mois))?;
        a_declarer
            .as_array()
            .map(|a| a.iter().filter_map(|e| e.get("debut").and_then(Value::as_str)).filter(|debut| **debut <= *jour_iso).map(String::from).collect())
            .unwrap_or_default()
    };
    for debut in &jours {
        let faits = pas(d, &format!("DPAE de l'embauche du {} envoyée", le(debut)), &format!("{}T07:00:00", debut), |ctx| {
            let a_declarer = Moteur::nouveau(o, ctx).embauches_a_declarer(Some(mois))?;
            let attente: Vec<&Value> =
                a_declarer.as_array().map(|a| a.iter().filter(|e| e.get("debut").and_then(Value::as_str) == Some(debut.as_str())).collect()).unwrap_or_default();
            if attente.is_empty() {
                return Ok(false);
            }
            let paies: Vec<Value> = attente.iter().map(|e| json!({"salarie": e["salarie"], "debut": e["debut"]})).collect();
            let r = crate::routes_actes_h1::dpae(ctx, &json!({"mois": mois, "paies": paies}))?;
            let fichier = r["fichiers"][0]["chemin"].as_str().unwrap_or("").to_string();
            for reel in [false, true] {
                deposer_dpae_joue(d, o, ctx, &fichier, reel)?;
                // LE BILAN DU TEST AVANT LE REEL (tickets #221, #222, `depot_acte::exiger_repetition`) ; puis celui du reel.
                rattacher(d, ctx)?;
            }
            Ok(true)
        });
        fait.extend(faits);
    }

    // Les contrats revenus signes : un mois qui n'a pas pu s'ecrire n'a rien a signer.
    let signables = |ctx: &Contexte| -> R<Vec<(String, String, String)>> {
        let a = crate::routes_documents::signature_apercu_avec(o, ctx, &json!({"mois": mois}), false)?;
        Ok(a["contrats"]
            .as_array()
            .map(|c| {
                c.iter()
                    .filter(|x| !x.get("recu_le").is_some_and(vrai))
                    .map(|x| (py_str(&x["salarie"]), py_str(&x["debut"]), py_str(&crate::etats::numero_de_cle(&x["numero"]))))
                    .collect()
            })
            .unwrap_or_default())
    };
    let a_signer: BTreeSet<String> = match d.contexte().and_then(|ctx| signables(&ctx)) {
        Ok(v) => v.into_iter().map(|(_, debut, _)| debut).filter(|debut| **debut <= *jour_iso).collect(),
        Err(_) => BTreeSet::new(),
    };
    for debut in &a_signer {
        let faits = pas(d, &format!("contrats de l'embauche du {} revenus signés", le(debut)), &format!("{}T08:00:00", debut), |ctx| {
            let manquent: Vec<(String, String, String)> = signables(ctx)?.into_iter().filter(|(_, db, _)| db == debut).collect();
            if manquent.is_empty() {
                return Ok(false);
            }
            for (salarie, db, numero) in &manquent {
                crate::routes_ecritures_g4::contrat_recu_par(o, ctx, mois, salarie, db, Some(numero.as_str()), true, QUI_HORLOGE)?;
            }
            Ok(true)
        });
        fait.extend(faits);
    }
    Ok(fait)
}

// ------------------------------------------------------------------ l'echeance de la DSN

/// `_a_l_echeance` : a l'echeance, la DSN du mois part (test puis reel), puis les documents aux salaries.
fn a_l_echeance(d: &HorlogeFictive, o: &HashMap<String, String>, mois: &str, echeance: Jour) -> Vec<String> {
    let mut fait = Vec::new();
    let jour = echeance.iso();

    fait.extend(pas(d, &format!("DSN de {} déposée", mois_fr(mois)), &format!("{}T10:00:00", jour), |ctx| {
        if dsn_partie(o, ctx, mois)? {
            return Ok(false);
        }
        // TOUTES LES DSN DU MOIS QUI NE SONT PAS PARTIES, la plus ancienne d'abord (#236, trouve au banc d'essai) : la
        // mensuelle, et a cote d'elle un signalement d'amorcage (une embauche sans taux de PAS connu). L'horloge ne prenait
        // que la plus recente -- l'amorcage, ecrit apres la mensuelle -- et la mensuelle ne partait jamais.
        let f = Moteur::nouveau(o, ctx).fichiers_deposables(mois)?;
        let mut chemins: Vec<String> = f["dsn"].as_array().map(|l| l.iter().map(|x| py_str(&x["chemin"])).collect()).unwrap_or_default();
        chemins.reverse();
        let parties: BTreeSet<String> = etats::charger(&ctx.racine)?
            .iter()
            .filter(|l| l.get("quoi").and_then(Value::as_str) == Some("dsn") && l.get("mode").and_then(Value::as_str) == Some("reel"))
            .filter(|l| l.get("accepte").is_some_and(vrai))
            .filter_map(|l| l.get("fichier").and_then(Value::as_str).map(String::from))
            .collect();
        let mut fait = false;
        for chemin in chemins {
            let nom = Path::new(&chemin).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if parties.contains(&nom) {
                continue;
            }
            for reel in [false, true] {
                deposer_dsn_joue(d, o, ctx, &chemin, reel)?;
                // LE BILAN DU TEST AVANT LE REEL (tickets #221, #222 : un reel ne part que sur le bilan lu de son test,
                // `depot_acte::exiger_repetition`) ; puis celui du reel.
                rattacher(d, ctx)?;
            }
            fait = true;
        }
        Ok(fait)
    }));

    fait.extend(pas(d, &format!("documents de {} envoyés aux salariés", mois_fr(mois)), &format!("{}T11:00:00", jour), |ctx| {
        let lignes = Moteur::nouveau(o, ctx).lignes()?;
        let envoyes = etats::documents_envoyes(&lignes, mois)?;
        if envoyes.as_array().is_some_and(|a| !a.is_empty()) {
            return Ok(false);
        }
        // LA BOITE MUETTE, et jamais une autre : le passe joue n'ecrit rien dans `courriels-simules/`.
        let mut ouvrir = || -> R<Box<dyn Courriel>> { Ok(Box::new(BoiteMuette::nouvelle())) };
        match crate::routes_envois::envoyer_documents_de(o, ctx, mois, &Preuve::de_la_charge(&json!({})), QUI_HORLOGE, &mut ouvrir) {
            Ok(_) => Ok(true),
            // Un mois sans documents a remettre.
            Err(e) if est_une_erreur_d_operation(&e) => Ok(false),
            Err(e) => Err(e),
        }
    }));
    fait
}

// ------------------------------------------------------------------ l'annee

/// `_export_compta_a_jour` : l'export existe et il est plus recent que la derniere saisie de l'annee. Un export fait en
/// septembre ne dit rien d'octobre : en janvier, on le redemande.
fn export_compta_a_jour(ctx: &Contexte, annee: i32) -> bool {
    let p = ctx.racine.join("sorties").join("exercices").join(format!("Compta_{}.csv", annee));
    let Ok(export) = std::fs::metadata(&p) else { return false };
    let prefixe = format!("{}-", annee);
    let derniere = std::fs::read_dir(ctx.racine.join("paies"))
        .map(|rd| {
            rd.flatten()
                .filter(|e| {
                    let nom = e.file_name().to_string_lossy().into_owned();
                    nom.starts_with(&prefixe) && nom.ends_with(".json")
                })
                .filter_map(|e| std::fs::metadata(e.path()).ok().map(|m| crate::paris::mtime(&m)))
                .fold(0.0_f64, f64::max)
        })
        .unwrap_or(0.0);
    crate::paris::mtime(&export) >= derniere
}

/// `_l_annee` : fin janvier, l'annee precedente se clot et s'exporte ; les echeances qu'on coche se cochent a la fin de
/// leur fenetre (la revue des spectacles comprise : tous continuent).
fn l_annee(d: &HorlogeFictive, o: &HashMap<String, String>, annee: i32, jour: Jour) -> R<Vec<String>> {
    let racine = d.racine;
    let mut fait = Vec::new();
    let precedente = annee - 1;
    let fin_janvier = Jour::nouveau(annee, 1, 31).ok_or_else(|| erreur("ValueError", "date impossible"))?;
    if jour >= fin_janvier {
        fait.extend(pas(d, &format!("exercice {} clos", precedente), &format!("{}T10:00:00", fin_janvier.iso()), |ctx| {
            let a_des_paies = std::fs::read_dir(ctx.racine.join("paies"))
                .map(|rd| {
                    rd.flatten().any(|e| {
                        let nom = e.file_name().to_string_lossy().into_owned();
                        nom.starts_with(&format!("{}-", precedente)) && nom.ends_with(".json")
                    })
                })
                .unwrap_or(false);
            if Moteur::nouveau(o, ctx).exercice_clos(&precedente.to_string())? || !a_des_paies {
                return Ok(false);
            }
            crate::exercice_ecrit::cloturer(ctx, &precedente.to_string(), false, QUI_HORLOGE)?;
            Ok(true)
        }));
        fait.extend(pas(d, &format!("exercice {} exporté pour la comptabilité", precedente), &format!("{}T11:00:00", fin_janvier.iso()), |ctx| {
            if !Moteur::nouveau(o, ctx).exercice_clos(&precedente.to_string())? || export_compta_a_jour(ctx, precedente) {
                return Ok(false);
            }
            crate::exercice_ecrit::export_compta_route(ctx, &json!({"annee": precedente.to_string()}))?;
            Ok(true)
        }));
    }

    // Les echeances qu'on coche, a la fin de leur fenetre (echeancier-base), si la convention de la compagnie les porte et
    // qu'elles ne le sont pas.
    let base = crate::engagement::lire_saisie(&dossier_du_code().join("echeancier-base.json")).map_err(Echec::from)?;
    let ctx = d.contexte()?;
    for e in crate::routes_saisie::annuelles_de(&base, ctx.entreprise.get("idcc"))? {
        if e.get("fait_par").and_then(Value::as_str) != Some("coche") {
            continue;
        }
        let Some(dernier) = e.get("mois").and_then(Value::as_array).and_then(|m| m.iter().filter_map(Value::as_u64).max()) else { continue };
        let dernier = dernier as u32;
        let Some(fin) = Jour::nouveau(annee, dernier, jours_du_mois(annee, dernier)) else { continue };
        let cle = format!("{}:{}", py_str(&e["cle"]), annee);
        let deja = std::fs::read_to_string(racine.join("config").join("echeancier.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v.get("fait").and_then(Value::as_object).cloned())
            .unwrap_or_default();
        if jour < fin || deja.contains_key(&cle) {
            continue;
        }
        let titre = py_str(&e["titre"])
            .replace("{annee}", &annee.to_string())
            .replace("{annee_precedente}", &(annee - 1).to_string())
            .replace("{jour_dsn}", "");
        fait.extend(pas(d, &format!("« {} » coché", titre), &format!("{}T10:00:00", fin.iso()), |ctx| {
            crate::routes_ecrites::marquer_echeance(ctx, &cle, true, QUI_HORLOGE)?;
            Ok(true)
        }));
    }
    Ok(fait)
}

// ------------------------------------------------------------------ les routes

/// Une route de l'horloge : `horloge-rattraper` (`{"aujourdhui"?, "retenir"?}` -> les phrases jouees) et
/// `horloge-derniere` (`{"aujourdhui"?}` -> ce que le bandeau lit).
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    let jour = match charge.get("aujourdhui").filter(|v| vrai(v)) {
        Some(v) => Some(Jour::lire(&py_str(v)).map_err(Echec::from)?),
        None => None,
    };
    match nom {
        "horloge-rattraper" => {
            let retenir = charge.get("retenir").map_or(true, vrai);
            Ok(json!(rattraper(&ctx.racine, jour, retenir)?))
        }
        "horloge-derniere" => Ok(json!(dernier_rattrapage(&ctx.racine, jour)?)),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ les tests

/// Une racine d'essai (dans le dossier temporaire, jamais `prive/` ni `fictives/`) : les trois fiches vides de quoi ouvrir un
/// `Contexte`, et, si `fictive`, le marqueur d'une compagnie fictive.
#[cfg(test)]
pub(crate) fn racine_d_essai(fictive: bool) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    let r = std::env::temp_dir().join(format!("paie-horloge-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&r);
    std::fs::create_dir_all(r.join("config")).unwrap();
    for f in ["entreprise.json", "salaries.json", "spectacles.json"] {
        std::fs::write(r.join("config").join(f), "{}").unwrap();
    }
    if fictive {
        std::fs::write(r.join("COPIE-DE-DEVELOPPEMENT.json"), r#"{"copie_de": "compagnie fictive essai"}"#).unwrap();
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn jour(t: &str) -> Jour {
        Jour::lire(t).unwrap()
    }

    /// Tout ce que la racine porte : chemins relatifs et octets.
    fn photo(racine: &Path) -> Vec<(String, Vec<u8>)> {
        fn marche(d: &Path, racine: &Path, out: &mut Vec<(String, Vec<u8>)>) {
            for e in std::fs::read_dir(d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    marche(&p, racine, out);
                } else {
                    out.push((p.strip_prefix(racine).unwrap().to_string_lossy().into_owned(), std::fs::read(&p).unwrap()));
                }
            }
        }
        let mut v = Vec::new();
        marche(racine, racine, &mut v);
        v.sort();
        v
    }

    // ---------------------------------------------------------------- la garde

    #[test]
    fn la_garde_hors_compagnie_fictive_ne_joue_rien_et_ne_touche_a_rien() {
        let r = racine_d_essai(false);
        std::fs::create_dir_all(r.join("paies")).unwrap();
        std::fs::write(r.join("paies").join("2026-10.json"), "{}").unwrap();
        let avant = photo(&r);
        assert!(HorlogeFictive::verifier(&r).is_none());
        assert_eq!(rattraper(&r, Some(jour("2027-03-01")), true).unwrap(), Vec::<String>::new());
        assert_eq!(rattraper(&r, Some(jour("2027-03-01")), false).unwrap(), Vec::<String>::new());
        assert_eq!(photo(&r), avant, "pas meme config/horloge.json");
        assert!(!r.join("config").join(FICHIER_HORLOGE).exists());
        // Un marqueur de copie de developpement qui n'est PAS une compagnie fictive ne suffit pas.
        std::fs::write(r.join("COPIE-DE-DEVELOPPEMENT.json"), r#"{"copie_de": "prive"}"#).unwrap();
        let avant = photo(&r);
        assert!(HorlogeFictive::verifier(&r).is_none());
        assert_eq!(rattraper(&r, Some(jour("2027-03-01")), true).unwrap(), Vec::<String>::new());
        assert_eq!(photo(&r), avant);
        // Ni un marqueur illisible.
        std::fs::write(r.join("COPIE-DE-DEVELOPPEMENT.json"), "{ pas du json").unwrap();
        assert!(HorlogeFictive::verifier(&r).is_none());
    }

    #[test]
    fn la_garde_est_relue_avant_chaque_geste() {
        let r = racine_d_essai(true);
        let d = HorlogeFictive::verifier(&r).unwrap().au_jour(jour("2026-10-13"));
        // Le marqueur disparait en route : le geste ne part pas, le pas devient sa phrase.
        std::fs::remove_file(r.join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
        let parti = Cell::new(false);
        let phrases = pas(&d, "DPAE envoyée", "2026-10-13T07:00:00", |_| {
            parti.set(true);
            Ok(true)
        });
        assert!(!parti.get(), "le geste ne doit pas partir");
        assert_eq!(phrases.len(), 1);
        assert!(phrases[0].starts_with("Le 13/10/2026 : DPAE envoyée — non joué ("), "{}", phrases[0]);
        assert!(phrases[0].contains("n'est pas une compagnie fictive"), "{}", phrases[0]);
        assert!(d.reverifier().is_err());
        assert!(deposer_dsn_joue(&d, &options(&r), &Contexte::nouveau(&r, None).unwrap(), "x.dsn", false).is_err());
        assert!(deposer_dpae_joue(&d, &options(&r), &Contexte::nouveau(&r, None).unwrap(), "x.txt", true).is_err());
        assert!(rattacher(&d, &Contexte::nouveau(&r, None).unwrap()).is_err());
    }

    #[test]
    fn dans_une_compagnie_fictive_elle_ecrit_sa_memoire_meme_sans_rien_a_jouer() {
        let r = racine_d_essai(true);
        assert_eq!(rattraper(&r, Some(jour("2026-10-13")), true).unwrap(), Vec::<String>::new());
        let m: Value = serde_json::from_str(&std::fs::read_to_string(r.join("config").join(FICHIER_HORLOGE)).unwrap()).unwrap();
        assert_eq!(m["le"], "2026-10-13");
        assert_eq!(m["mois_faits"], json!([]));
        assert_eq!(m["annees_faites"], json!([]));
        assert_eq!(m["dernier"], json!({}));
        assert_eq!(m["saisies"], json!({}));
        assert_eq!(m["_quoi"], QUOI);
    }

    // ---------------------------------------------------------------- defaut 4 : horloge.json non objet, calcul.json

    #[test]
    fn un_horloge_json_qui_n_est_pas_un_objet_se_lit_comme_absent() {
        // L'ancien Python plantait (`'list' object has no attribute 'get'`) ; le Rust le lit comme absent et le recrit.
        for contenu in ["[1, 2]", "\"x\"", "null", "3", "true", "{ pas du json", ""] {
            let r = racine_d_essai(true);
            std::fs::write(r.join("config").join(FICHIER_HORLOGE), contenu).unwrap();
            assert_eq!(dernier_rattrapage(&r, Some(jour("2026-10-13"))).unwrap(), Vec::<String>::new(), "{:?}", contenu);
            assert_eq!(rattraper(&r, Some(jour("2026-10-13")), true).unwrap(), Vec::<String>::new(), "{:?}", contenu);
            let m: Value = serde_json::from_str(&std::fs::read_to_string(r.join("config").join(FICHIER_HORLOGE)).unwrap()).unwrap();
            assert!(m.is_object(), "{:?} doit etre recrit en objet", contenu);
            assert_eq!(m["le"], "2026-10-13");
        }
    }

    #[test]
    fn un_fichier_calcul_json_n_est_pas_un_mois() {
        // L'ancien `mois_connus` Python comptait `AAAA-MM.calcul.json` comme un mois (`fromisoformat` levait).
        let r = racine_d_essai(true);
        std::fs::create_dir_all(r.join("paies")).unwrap();
        for f in ["2026-10.json", "2026-10.calcul.json", "2026-11.calcul.json", "notes.json", "2026-1.json"] {
            std::fs::write(r.join("paies").join(f), "{}").unwrap();
        }
        assert_eq!(crate::routes_mois::mois_saisis(&r), vec!["2026-10".to_string()]);
        let connus: Vec<String> = crate::routes_accueil::mois_connus(&r).into_iter().map(|(m, _)| m).collect();
        assert_eq!(connus, vec!["2026-10".to_string()]);
        // Et l'horloge passe : le mois qui ne s'ecrit pas devient une phrase, pas une panne.
        let phrases = rattraper(&r, Some(jour("2026-10-20")), true).unwrap();
        assert!(phrases.iter().all(|p| !p.contains("calcul")), "{:?}", phrases);
        let m: Value = serde_json::from_str(&std::fs::read_to_string(r.join("config").join(FICHIER_HORLOGE)).unwrap()).unwrap();
        assert!(!m.to_string().contains("calcul"));
    }

    // ---------------------------------------------------------------- defaut 1 : le jour joue

    #[test]
    fn le_contexte_de_l_horloge_porte_le_jour_joue() {
        let r = racine_d_essai(true);
        let d = HorlogeFictive::verifier(&r).unwrap().au_jour(jour("2027-03-01"));
        let c = d.contexte().unwrap();
        assert_eq!((c.aujourdhui, c.date), (jour("2027-03-01"), jour("2027-03-01")));
        let fin = d.contexte_a(jour("2026-10-31")).unwrap();
        assert_eq!((fin.aujourdhui, fin.date), (jour("2027-03-01"), jour("2026-10-31")));
    }

    #[test]
    fn la_cloture_juge_l_annee_sur_le_jour_du_contexte_et_pas_sur_l_horloge_reelle() {
        let r = racine_d_essai(true);
        let mut ctx = Contexte::nouveau(&r, None).unwrap();
        // Rejouee au 31/01/2027 (le conteneur est en 2026) : 2026 est finie, la cloture n'est plus refusee pour cela.
        ctx.aujourdhui = jour("2027-01-31");
        let e = crate::exercice_ecrit::cloturer(&ctx, "2026", false, QUI_HORLOGE);
        assert!(e.as_ref().err().map_or(true, |e| !e.message.contains("n'est pas fini")), "{:?}", e.err().map(|e| e.message));
        assert!(r.join("sorties").join("exercices").join("Exercice_2026.zip").exists(), "2026 est close, au jour joue");
        // L'annee du contexte n'est pas finie : refus, qu'elle soit passee ou non pour la machine.
        let r = racine_d_essai(true);
        let mut ctx = Contexte::nouveau(&r, None).unwrap();
        ctx.aujourdhui = jour("2026-12-31");
        let e = crate::exercice_ecrit::cloturer(&ctx, "2026", false, QUI_HORLOGE).unwrap_err();
        assert!(e.message.contains("n'est pas fini"), "{}", e.message);
        // Hors de l'horloge le contexte porte la date reelle : le comportement de l'interface ne change pas.
        let r = racine_d_essai(true);
        let mut ctx = Contexte::nouveau(&r, None).unwrap();
        ctx.aujourdhui = crate::contexte::jour_reel();
        let an = crate::contexte::jour_reel().an.to_string();
        let e = crate::exercice_ecrit::cloturer(&ctx, &an, false, "interface").unwrap_err();
        assert!(e.message.contains("n'est pas fini"), "{}", e.message);
    }

    // ---------------------------------------------------------------- defaut 2 : le jour de l'echeancier

    #[test]
    fn l_echeance_cochee_par_l_horloge_porte_le_jour_joue() {
        let r = racine_d_essai(true);
        let d = HorlogeFictive::verifier(&r).unwrap().au_jour(jour("2027-03-01"));
        let ctx = d.contexte().unwrap();
        crate::routes_ecrites::marquer_echeance(&ctx, "at-mp:2027", true, QUI_HORLOGE).unwrap();
        let e: Value = serde_json::from_str(&std::fs::read_to_string(r.join("config").join("echeancier.json")).unwrap()).unwrap();
        assert_eq!(e["fait"]["at-mp:2027"], json!({"le": "2027-03-01", "qui": "horloge"}));
        // L'interface, elle, garde le jour du contexte (reel).
        let mut ctx = Contexte::nouveau(&r, None).unwrap();
        ctx.aujourdhui = jour("2026-10-07");
        crate::routes_ecrites::marquer_echeance(&ctx, "at-mp:2026", true, "interface").unwrap();
        let e: Value = serde_json::from_str(&std::fs::read_to_string(r.join("config").join("echeancier.json")).unwrap()).unwrap();
        assert_eq!(e["fait"]["at-mp:2026"]["le"], "2026-10-07");
    }

    // ---------------------------------------------------------------- la boite muette

    #[test]
    fn la_boite_muette_rend_un_identifiant_et_ne_garde_rien() {
        let mut b = BoiteMuette::nouvelle();
        assert_eq!(b.compte(), "fictive@puppetpaie.invalid");
        let a = b.envoyer(b"From: x\r\n\r\nbonjour").unwrap();
        let c = b.envoyer(b"From: x\r\n\r\nbonjour").unwrap();
        assert!(a.starts_with('<') && a.ends_with("@fictive.puppetpaie>"), "{}", a);
        assert_ne!(a, c, "un identifiant au hasard a chaque envoi");
        assert!(b.rechercher("tout", 50).unwrap().is_empty());
        assert!(b.message_brut(&a).unwrap().is_empty());
        b.fermer();
    }

    // ---------------------------------------------------------------- dater, n_lignes

    #[test]
    fn dater_ne_date_que_les_lignes_ecrites_depuis() {
        let r = racine_d_essai(true);
        let p = crate::journal::chemin(&r);
        // Sans journal : rien, et rien n'est cree.
        dater(&r, 0, "2026-10-13T07:00:00");
        assert!(!p.exists());
        assert_eq!(n_lignes(&r), 0);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, r#"[{"quoi": "dsn", "quand": "2026-09-30T13:53:15"}, {"quoi": "dpae", "quand": "2026-10-07T10:11:12"}, {"quoi": "contrat-recu"}]"#).unwrap();
        assert_eq!(n_lignes(&r), 3);
        dater(&r, 1, "2026-10-13T07:00:00");
        let l: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(l[0]["quand"], "2026-09-30T13:53:15");
        assert!(l[0].get("origine").is_none(), "une ligne d'avant n'est pas touchee");
        for i in [1, 2] {
            assert_eq!(l[i]["quand"], "2026-10-13T07:00:00");
            assert_eq!(l[i]["origine"], ORIGINE);
        }
        // Rien d'ecrit depuis : le fichier reste tel quel.
        let avant = std::fs::read(&p).unwrap();
        dater(&r, 3, "2030-01-01T00:00:00");
        assert_eq!(std::fs::read(&p).unwrap(), avant);
        // Un journal qui n'est pas une liste : 0 ligne.
        std::fs::write(&p, "{}").unwrap();
        assert_eq!(n_lignes(&r), 0);
    }

    // ---------------------------------------------------------------- empreinte_saisie

    #[test]
    fn l_empreinte_d_une_saisie_est_celle_de_son_contenu() {
        let r = racine_d_essai(false);
        let a = r.join("a.json");
        let b = r.join("b.json");
        // Le vecteur de l'ancien Python : sha256(json.dumps(d, sort_keys=True, ensure_ascii=False))[:16].
        std::fs::write(&a, r#"{"b": 1, "a": [1, 2], "z": "é"}"#).unwrap();
        assert_eq!(empreinte_saisie(&a), "9254832cc9ec5883");
        // Autre ordre des cles, autre mise en page : meme contenu, meme empreinte.
        std::fs::write(&b, "{\n  \"z\": \"é\",\n  \"a\": [1,\n 2],\n  \"b\": 1\n}\n").unwrap();
        assert_eq!(empreinte_saisie(&b), empreinte_saisie(&a));
        // Un contenu change : une autre empreinte.
        std::fs::write(&b, r#"{"b": 2, "a": [1, 2], "z": "é"}"#).unwrap();
        assert_ne!(empreinte_saisie(&b), empreinte_saisie(&a));
        // Illisible ou absente : vide.
        std::fs::write(&b, "{ pas du json").unwrap();
        assert_eq!(empreinte_saisie(&b), "");
        assert_eq!(empreinte_saisie(&r.join("absent.json")), "");
    }

    // ---------------------------------------------------------------- les phrases

    #[test]
    fn un_pas_dit_ce_qu_il_a_fait_et_se_tait_quand_il_n_y_a_rien_a_faire() {
        let r = racine_d_essai(true);
        let d = HorlogeFictive::verifier(&r).unwrap().au_jour(jour("2026-10-13"));
        assert_eq!(pas(&d, "DPAE de l'embauche du 13/10/2026 envoyée", "2026-10-13T07:00:00", |_| Ok(true)), vec!["Le 13/10/2026 : DPAE de l'embauche du 13/10/2026 envoyée"]);
        assert!(pas(&d, "rien", "2026-10-13T07:00:00", |_| Ok(false)).is_empty());
        let p = pas(&d, "DSN de octobre 2026 déposée", "2026-11-15T10:00:00", |_| Err(Echec { classe: Some("KeyError"), message: "'nom'\nsuite".into() }));
        assert_eq!(p, vec!["Le 15/11/2026 : DSN de octobre 2026 déposée — non joué ('nom')"]);
        // Les lignes de journal que le geste ecrit sont datees a la date du pas.
        let jp = crate::journal::chemin(&r);
        std::fs::create_dir_all(jp.parent().unwrap()).unwrap();
        std::fs::write(&jp, "[]").unwrap();
        pas(&d, "ecrit", "2026-10-13T07:00:00", |_| {
            std::fs::write(&jp, r#"[{"quoi": "dpae", "quand": "2026-10-07T15:00:00"}]"#).unwrap();
            Ok(true)
        });
        let l: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(&jp).unwrap()).unwrap();
        assert_eq!(l[0]["quand"], "2026-10-13T07:00:00");
        assert_eq!(l[0]["origine"], ORIGINE);
    }

    #[test]
    fn les_refus_d_operation_sont_ceux_de_l_ancien_python() {
        let e = |c: Option<&'static str>| Echec { classe: c, message: "m".into() };
        for oui in [None, Some("ErreurOperation"), Some("SortieAbsente"), Some("AucunDocument"), Some("BoiteAbsente"), Some("PreuveInsuffisante"), Some("SaisieIncomplete"), Some("MoisPasPret"), Some("ExerciceClos"), Some("MoisHorsDelai")] {
            assert!(est_une_erreur_d_operation(&e(oui)), "{:?}", oui);
        }
        for non in ["KeyError", "ValueError", "OSError", "TypeError"] {
            assert!(!est_une_erreur_d_operation(&e(Some(non))), "{}", non);
        }
    }

    // ---------------------------------------------------------------- la route

    #[test]
    fn la_route_rattrape_et_dit_le_dernier_rattrapage() {
        assert_eq!(ROUTES, ["horloge-rattraper", "horloge-derniere"]);
        // Pas une fictive : [] et rien d'ecrit.
        let r = racine_d_essai(false);
        let ctx = Contexte::nouveau(&r, None).unwrap();
        let o = options(&r);
        let avant = photo(&r);
        assert_eq!(route(&o, &ctx, "horloge-rattraper", &json!({"aujourdhui": "2027-03-01"})).unwrap(), json!([]));
        assert_eq!(photo(&r), avant);
        // Une fictive : la memoire dit ce que le bandeau lit, ce jour-la seulement.
        let r = racine_d_essai(true);
        let ctx = Contexte::nouveau(&r, None).unwrap();
        let o = options(&r);
        assert_eq!(route(&o, &ctx, "horloge-derniere", &json!({"aujourdhui": "2026-10-13"})).unwrap(), json!([]));
        std::fs::write(
            r.join("config").join(FICHIER_HORLOGE),
            r#"{"le": "2026-10-13", "dernier": {"le": "2026-10-13", "fait": ["Le 13/10/2026 : a", "Le 01/10/2026 : b"]}}"#,
        )
        .unwrap();
        assert_eq!(route(&o, &ctx, "horloge-derniere", &json!({"aujourdhui": "2026-10-13"})).unwrap(), json!(["Le 13/10/2026 : a", "Le 01/10/2026 : b"]));
        assert_eq!(route(&o, &ctx, "horloge-derniere", &json!({"aujourdhui": "2026-10-14"})).unwrap(), json!([]));
        // `retenir: false` : le visiteur ne lira pas le passe ; `retenir` par defaut : vrai.
        assert_eq!(route(&o, &ctx, "horloge-rattraper", &json!({"aujourdhui": "2026-10-14", "retenir": false})).unwrap(), json!([]));
        let m: Value = serde_json::from_str(&std::fs::read_to_string(r.join("config").join(FICHIER_HORLOGE)).unwrap()).unwrap();
        assert_eq!((m["le"].as_str(), &m["dernier"]), (Some("2026-10-14"), &json!({})));
        route(&o, &ctx, "horloge-rattraper", &json!({"aujourdhui": "2026-10-15"})).unwrap();
        // Un jour illisible, une route inconnue : des refus.
        assert!(route(&o, &ctx, "horloge-rattraper", &json!({"aujourdhui": "demain"})).is_err());
        assert!(route(&o, &ctx, "horloge-autre", &json!({})).is_err());
    }
}
