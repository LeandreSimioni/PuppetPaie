//! L'horloge de la compagnie fictive (`moteur/simulation.py`, a partir de `QUI_HORLOGE`) : `rattraper`, `dernier_rattrapage`
//! et tout ce qu'ils appellent -- lot I1.
//!
//! A chaque ouverture, la COMPAGNIE FICTIVE joue ce qu'une compagnie bien tenue aurait fait AVANT aujourd'hui, a sa
//! date : elle saisit, valide, envoie. Le jour de chaque embauche, ses engagements se valident a 5 h, son contrat part a
//! la signature a 6 h, sa DPAE a 7 h (test puis reel), et le contrat revient signe a 8 h ; a l'echeance de la DSN, la DSN
//! se fabrique et part (test puis reel), puis les documents aux salaries, fabriques a leur envoi ; le 1er du mois suivant,
//! ce qui reste en saisie du mois se valide ; fin janvier, l'annee passee se clot et s'exporte pour la
//! comptabilite ; les echeances annuelles qu'on coche se cochent a la fin de leur fenetre. Ce qui vient apres aujourd'hui
//! reste au visiteur. Ce qu'il a deja fait lui-meme n'est pas refait : chaque pas regarde d'abord le journal.
//!
//! ELLE NE JOUE QUE DANS UNE COMPAGNIE FICTIVE, ET LE CODE LE DIT A CHAQUE ENDROIT QUI AGIT.
//! - `rattraper` commence par `HorlogeFictive::verifier` : sans compagnie fictive active (`bridage::branchement` : le paquet
//!   soude, ou une compagnie dont le marqueur dit « compagnie fictive »), elle rend une liste vide et ne
//!   touche a rien -- pas meme `config/horloge.json`.
//! - Tout geste qui agit recoit la `HorlogeFictive` et la RE-VERIFIE avant d'agir (`pas`, `deposer_*_joue`) : un marqueur
//!   change en route arrete le pas, il ne le laisse pas partir vers un vrai point de depot.
//! - Les depots ne passent que par `depot_acte::deposer_dsn` et `deposer_dpae`, qui jouent la reponse `FICTIF-...` quand la
//!   simulation est active et ne s'ouvrent jamais autrement ici (la re-verification les precede) ; aucune preuve, aucun
//!   secret, aucun coffre n'est lu.
//! - Les courriels ne passent que par `BoiteMuette` : un client du trait `Courriel` qui rend un identifiant et ne garde
//!   rien -- ni `courriels-simules/` (qui montre au visiteur ce que SES gestes envoient), ni la vraie boite. Ce module
//!   n'appelle jamais `crate::routes::envois::client_courriel`.
//!
//! Elle ne leve jamais pour un PAS : un pas en echec devient sa phrase (« Le jj/mm/aaaa : ... — non joue (...) ») et les
//! autres continuent. Seuls les refus du decor (une fiche entreprise illisible, une echeance introuvable) remontent.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use envois::courriel::{Courriel, ErreurCourriel};
use socle::date::{jours_du_mois, Jour};
use crate::depot_acte::{self, Preuve};
use envois::etats;
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::{dossier_du_code, vrai};
use socle::echec::Echec;
use crate::routes::mois::Moteur;

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
/// que par `verifier`, qui lit `bridage::branchement` ; `reverifier` la relit avant chaque geste.
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
        crate::bridage::branchement(racine).joue().then(|| HorlogeFictive { racine, jour: droit::contexte::jour_reel() })
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
        if crate::bridage::branchement(self.racine).joue() {
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
        Ok(format!("<{}@fictive.puppetpaie>", envois::courriel::hexa_au_hasard(8)))
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


/// `_mois_fr` : « octobre 2026 ».
fn mois_fr(mois: &str) -> String {
    // Une ligne du recit ne se refuse pas pour son mois : un mois hors 01-12 s'ecrit tel quel, jamais janvier.
    socle::date::libelle_du_mois_ou_tel_quel(mois, socle::date::Graphie::Accentuee)
}

/// `_le` : `JJ/MM/AAAA` d'un `AAAA-MM-JJ...`.
fn le(iso: &str) -> String {
    let c: Vec<char> = iso.chars().collect();
    let t = |a: usize, b: usize| -> String { c.iter().skip(a).take(b - a).collect() };
    format!("{}/{}/{}", t(8, 10), t(5, 7), t(0, 4))
}

/// `_n_lignes` : combien de lignes le journal des depots porte (0 sans journal).
fn n_lignes(racine: &Path) -> usize {
    let p = envois::journal::chemin(racine);
    std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v.as_array().map(Vec::len)).unwrap_or(0)
}

/// `_dater` : date les lignes du journal ecrites depuis la `depuis`-ieme : un pas joue se lit a la date ou il aurait eu
/// lieu, et la ligne le dit (`origine`).
fn dater(racine: &Path, depuis: usize, quand: &str) {
    let p = envois::journal::chemin(racine);
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
    let _ = socle::ecriture::ecrire_json(&p, &Value::Array(lignes), 2);
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
                | "ExerciceClos"
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

/// L'instant d'un pas, `AAAA-MM-JJTHH:MM:SS` a l'heure de Paris, en secondes UTC.
fn instant_du_pas(quand: &str) -> Option<f64> {
    let (jour, heure) = quand.split_once('T')?;
    let jour = Jour::lire(jour).ok()?;
    let mut hms = heure.split(':').map(|x| x.parse::<i64>().ok());
    let (h, m, s) = (hms.next()??, hms.next().flatten().unwrap_or(0), hms.next().flatten().unwrap_or(0));
    Some(socle::temps::instant_de(jour, h) + (m * 60 + s) as f64)
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
    // Le geste a lieu a son heure : ce qu'il ecrit (un nom de fichier, une ligne de journal) en porte la date.
    // La re-verification precede TOUT geste : ce qui suit ne s'execute que dans une compagnie fictive.
    let mut jouer = || d.reverifier().and_then(|_| geste(&ctx));
    let fait = match instant_du_pas(quand) {
        Some(t) => socle::temps::pendant(t, jouer),
        None => jouer(),
    };
    let dit = match fait {
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
fn dsn_partie(ctx: &Contexte, mois: &str) -> R<bool> {
    Ok(vrai(&Moteur::nouveau(ctx).dsn_du_mois(mois)?))
}

/// `ops.deposer_dsn(ctx, fichier, reel=..., qui=QUI_HORLOGE)` : JOUE. La re-verification precede l'appel.
fn deposer_dsn_joue(d: &HorlogeFictive, ctx: &Contexte, fichier: &str, reel: bool) -> R<()> {
    d.reverifier()?;
    depot_acte::deposer_dsn(ctx, &json!(fichier), reel, None, QUI_HORLOGE, false, "")?;
    Ok(())
}

/// `ops.deposer_dpae(ctx, fichier, reel=..., qui=QUI_HORLOGE)` : JOUE. La re-verification precede l'appel.
fn deposer_dpae_joue(d: &HorlogeFictive, ctx: &Contexte, fichier: &str, reel: bool) -> R<()> {
    d.reverifier()?;
    depot_acte::deposer_dpae(ctx, &json!(fichier), reel, None, QUI_HORLOGE, false, "")?;
    Ok(())
}

/// LES DSN DU MOIS QUI NE SONT PAS PARTIES et que `garder` retient (sur ce que le fichier declare), deposees chacune en
/// test puis en reel -- le bilan du test lu avant le reel (tickets #221, #222 : `depot_acte::exiger_repetition`) --, dans
/// l'ordre de `crate::routes::mois::ordre_de_depot`. Rend vrai si l'une est partie ; `a_faire` recoit ce que le rangement des AER
/// jouees laisse a faire (`rattacher`).
fn deposer_les_dsn(
    d: &HorlogeFictive,
    ctx: &Contexte,
    mois: &str,
    garder: impl Fn(&envois::depot::Apercu) -> bool,
    a_faire: &mut Vec<String>,
) -> R<bool> {
    let f = Moteur::nouveau(ctx).fichiers_deposables(mois)?;
    let chemins: Vec<String> = f["dsn"].as_array().map(|l| l.iter().map(|x| valeur_en_texte(&x["chemin"])).collect()).unwrap_or_default();
    let parties: BTreeSet<String> = etats::charger(&ctx.racine)?
        .iter()
        .filter(|l| l.get("quoi").and_then(Value::as_str) == Some("dsn") && l.get("mode").and_then(Value::as_str) == Some("reel"))
        .filter(|l| l.get("accepte").is_some_and(vrai))
        .filter_map(|l| l.get("fichier").and_then(Value::as_str).map(String::from))
        .collect();
    let mut fait = false;
    for chemin in chemins {
        let nom = Path::new(&chemin).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if parties.contains(&nom) || !std::fs::read(&chemin).is_ok_and(|o| garder(&envois::depot::lire(&o))) {
            continue;
        }
        for reel in [false, true] {
            deposer_dsn_joue(d, ctx, &chemin, reel)?;
            a_faire.extend(rattacher(d, ctx)?);
        }
        fait = true;
    }
    Ok(fait)
}

/// `simulation.rattacher(ctx, delai=0)` : les retours des depots joues arrivent tout de suite. Rend ce que le rangement
/// des AER jouees laisse a faire (`crate::releve::rattacher_simulation`), comme les autres chemins de la releve.
fn rattacher(d: &HorlogeFictive, ctx: &Contexte) -> R<Vec<String>> {
    d.reverifier()?;
    Ok(crate::releve::rattacher_simulation(ctx, 0)?.1)
}

// ------------------------------------------------------------------ dernier_rattrapage

/// `dernier_rattrapage` : ce que l'horloge a joue aujourd'hui, pour le bandeau « pendant votre absence ». Vide si elle
/// n'a rien joue ce jour-la.
pub fn dernier_rattrapage(racine: &Path, aujourdhui: Option<Jour>) -> R<Vec<String>> {
    let jour = match aujourdhui {
        Some(j) => j,
        None => droit::contexte::aujourdhui().map_err(Echec::from)?,
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
    Ok(dernier.get("fait").and_then(Value::as_array).map(|a| a.iter().map(valeur_en_texte).collect()).unwrap_or_default())
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
        None => droit::contexte::aujourdhui().map_err(Echec::from)?,
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
    let mut annees_faites: BTreeSet<i64> =
        memoire.get("annees_faites").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_i64).collect()).unwrap_or_default();
    let mut fait: Vec<String> = Vec::new();

    let saisis = crate::routes::mois::mois_saisis(racine);
    for mois in &saisis {
        let premier = socle::date::premier_du_mois(mois).map_err(Echec::from)?;
        if jour < premier {
            break;
        }
        if clos.contains(mois) {
            continue;
        }
        // PENDANT LE MOIS, ses embauches : la DPAE et le contrat, chacun a sa date.
        fait.extend(les_embauches_du_mois(&d, mois, jour)?);
        // LE 1ER DU MOIS SUIVANT, ce qui reste en saisie se valide : un mois en cours n.a ni DSN ni documents.
        let suivant = socle::date::dernier_du_mois(mois).map_err(Echec::from)?.plus_jours(1).map_err(Echec::from)?;
        if jour < suivant {
            continue;
        }
        let fin = socle::date::dernier_du_mois(mois).map_err(Echec::from)?;
        let valide_le = match crate::interface::compagnies::cree_le(d.racine) {
            Some(c) if c > suivant => c.min(jour),
            _ => suivant,
        };
        fait.extend(valider_les_engagements(&d, mois, fin, &format!("engagements de {} validés", mois_fr(mois)), &format!("{}T09:00:00", valide_le.iso())));
        // Le jour ou la DSN est due (`echeance_dsn`, reporte au jour ouvre) : elle part ce matin-la, avant midi.
        let echeance = {
            let ctx = d.contexte()?;
            declarations::echeance_dsn::de_la_compagnie(&ctx, mois).map_err(Echec::from)?.jour
        };
        if jour >= echeance {
            fait.extend(a_l_echeance(&d, mois, echeance));
            let ctx = d.contexte()?;
            if dsn_partie(&ctx, mois)? {
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
            fait.extend(l_annee(&d, annee, jour)?);
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
    socle::ecriture::ecrire_json(&memoire_f, &Value::Object(sortie), 2).map_err(|m| erreur("OSError", m))?;
    Ok(fait)
}

// ------------------------------------------------------------------ le mois et ses embauches

/// LES ENGAGEMENTS DU MOIS SE VALIDENT, comme une compagnie bien tenue les valide : ceux qui commencent au plus tard
/// `jusqu_au`, a `quand`. Rien ne part d'un engagement en saisie ; un engagement que le moteur refuse de valider (une
/// saisie incomplete) reste en saisie, et le pas le dit.
fn valider_les_engagements(d: &HorlogeFictive, mois: &str, jusqu_au: Jour, quoi: &str, quand: &str) -> Vec<String> {
    pas(d, quoi, quand, |ctx| {
        let objets = engagement::stock::objets_du_mois(&ctx.racine, mois).map_err(Echec::from)?;
        let mut fait = false;
        for (i, o) in objets.iter().enumerate() {
            if o.est_valide() || engagement::debut_brut(&Value::Object(o.brut.clone())) > jusqu_au.iso() {
                continue;
            }
            engagement::etats::valider(ctx, mois, i).map_err(Echec::from)?;
            fait = true;
        }
        Ok(fait)
    })
}

/// LES EMBAUCHES DU MOIS, chacune a sa date limite (l'editeur, 27/09/2026) : « s'il arrive deux jours avant, il peut faire
/// la DPAE ; des qu'on passe la date, c'est vert et deja fait par le moteur de demo » (la compagnie fictive). Le jour de
/// l'embauche, son contrat part a la signature a 6 h s'il n'est pas parti (une compagnie bien tenue l'envoie avant la
/// premiere heure de travail), sa DPAE part a 7 h (test puis reel), le contrat revient signe a 8 h. D'ici la, c'est au
/// visiteur. Le mois n'a pas a etre ecrit : la DPAE et le contrat se font depuis la saisie, comme dans la page.
fn les_embauches_du_mois(d: &HorlogeFictive, mois: &str, jour: Jour) -> R<Vec<String>> {
    let mut fait: Vec<String> = Vec::new();
    let jour_iso = jour.iso();

    // LE JOUR DE CHAQUE EMBAUCHE, A 5 H, ses engagements en saisie se valident : rien ne part d'un engagement en saisie.
    let a_valider: BTreeSet<String> = engagement::stock::objets_du_mois(d.racine, mois)
        .map_err(Echec::from)?
        .iter()
        .filter(|o| !o.est_valide())
        .map(|o| engagement::debut_brut(&Value::Object(o.brut.clone())))
        .filter(|debut| debut.starts_with(mois) && **debut <= *jour_iso)
        .collect();
    for debut in &a_valider {
        let Ok(le_jour) = Jour::lire(debut) else { continue };
        fait.extend(valider_les_engagements(d, mois, le_jour, &format!("engagements de l'embauche du {} validés", le(debut)), &format!("{}T05:00:00", debut)));
    }

    // Les contrats de spectacle du mois qui ne sont pas revenus signes : (salarie, debut, numero, demande).
    let signables = |ctx: &Contexte| -> R<Vec<(String, String, String, bool)>> {
        let a = crate::routes::documents::signature_apercu(ctx, &json!({"mois": mois}))?;
        Ok(a["contrats"]
            .as_array()
            .map(|c| {
                c.iter()
                    .filter(|x| !x.get("recu_le").is_some_and(vrai))
                    .map(|x| {
                        (valeur_en_texte(&x["salarie"]), valeur_en_texte(&x["debut"]), valeur_en_texte(&envois::etats::numero_de_cle(&x["numero"])), x.get("demande_le").is_some_and(vrai))
                    })
                    .collect()
            })
            .unwrap_or_default())
    };
    let a_signer: BTreeSet<String> = match d.contexte().and_then(|ctx| signables(&ctx)) {
        Ok(v) => v.into_iter().map(|(_, debut, _, _)| debut).filter(|debut| **debut <= *jour_iso).collect(),
        Err(_) => BTreeSet::new(),
    };
    for debut in &a_signer {
        let faits = pas(d, &format!("contrats de l'embauche du {} envoyés à la signature", le(debut)), &format!("{}T06:00:00", debut), |ctx| {
            let a_demander: Vec<Value> = signables(ctx)?
                .into_iter()
                .filter(|(_, db, _, demande)| db == debut && !demande)
                .map(|(salarie, db, numero, _)| json!({"salarie": salarie, "debut": db, "numero": if numero.is_empty() { Value::Null } else { json!(numero) }}))
                .collect();
            if a_demander.is_empty() {
                return Ok(false);
            }
            // LA BOITE MUETTE, et jamais une autre : le passe joue n'ecrit rien dans `courriels-simules/`.
            let mut ouvrir = || -> R<Box<dyn Courriel>> { Ok(Box::new(BoiteMuette::nouvelle())) };
            crate::routes::envois::demander_signature(ctx, &json!({"mois": mois, "contrats": a_demander}), &mut ouvrir)?;
            Ok(true)
        });
        fait.extend(faits);
    }

    let jours: BTreeSet<String> = {
        let ctx = d.contexte()?;
        let a_declarer = Moteur::nouveau(&ctx).embauches_a_declarer(Some(mois))?;
        a_declarer
            .as_array()
            .map(|a| a.iter().filter_map(|e| e.get("debut").and_then(Value::as_str)).filter(|debut| **debut <= *jour_iso).map(String::from).collect())
            .unwrap_or_default()
    };
    for debut in &jours {
        let faits = pas(d, &format!("DPAE de l'embauche du {} envoyée", le(debut)), &format!("{}T07:00:00", debut), |ctx| {
            let a_declarer = Moteur::nouveau(ctx).embauches_a_declarer(Some(mois))?;
            let attente: Vec<&Value> =
                a_declarer.as_array().map(|a| a.iter().filter(|e| e.get("debut").and_then(Value::as_str) == Some(debut.as_str())).collect()).unwrap_or_default();
            if attente.is_empty() {
                return Ok(false);
            }
            let paies: Vec<Value> = attente.iter().map(|e| json!({"salarie": e["salarie"], "debut": e["debut"]})).collect();
            let r = crate::routes::actes_h1::dpae(ctx, &json!({"mois": mois, "paies": paies}))?;
            let fichier = r["fichiers"][0]["chemin"].as_str().unwrap_or("").to_string();
            for reel in [false, true] {
                deposer_dpae_joue(d, ctx, &fichier, reel)?;
                // LE BILAN DU TEST AVANT LE REEL (tickets #221, #222, `depot_acte::exiger_repetition`) ; puis celui du reel.
                rattacher(d, ctx)?;
            }
            // LE SIGNALEMENT D'AMORCAGE (nature 08, motif 01 « embauche effective ») part AVEC LA DPAE, dans le meme
            // geste (atelier/README.md, « Demander un taux » ; `crate::routes::actes_h1::dpae` produit les deux) : pas a
            // l'echeance de la DSN.
            // Un signalement ne recoit pas d'AER : son rangement n'a rien a dire.
            deposer_les_dsn(d, ctx, mois, |a| a.champ("nature") == "08", &mut Vec::new())?;
            Ok(true)
        });
        fait.extend(faits);
    }

    // Les contrats revenus signes. Celui qui n'a pas pu partir par courriel s'imprime et se remet en main propre :
    // l'impression est son depart.
    for debut in &a_signer {
        let faits = pas(d, &format!("contrats de l'embauche du {} revenus signés", le(debut)), &format!("{}T08:00:00", debut), |ctx| {
            let a_imprimer: Vec<Value> = signables(ctx)?
                .into_iter()
                .filter(|(_, db, _, demande)| db == debut && !demande)
                .map(|(salarie, db, numero, _)| json!({"salarie": salarie, "debut": db, "numero": if numero.is_empty() { Value::Null } else { json!(numero) }}))
                .collect();
            if !a_imprimer.is_empty() {
                crate::routes::envois::contrats_imprimer_par(ctx, &json!({"mois": mois, "contrats": a_imprimer}), QUI_HORLOGE)?;
            }
            let manquent: Vec<(String, String, String, bool)> = signables(ctx)?.into_iter().filter(|(_, db, _, _)| db == debut).collect();
            if manquent.is_empty() {
                return Ok(false);
            }
            for (salarie, db, numero, _) in &manquent {
                crate::routes::ecritures_g4::contrat_recu_par(ctx, mois, salarie, db, Some(numero.as_str()), true, QUI_HORLOGE)?;
            }
            Ok(true)
        });
        fait.extend(faits);
    }
    Ok(fait)
}

// ------------------------------------------------------------------ l'echeance de la DSN

/// `_a_l_echeance` : a l'echeance, la DSN du mois part (test puis reel), puis les documents aux salaries.
fn a_l_echeance(d: &HorlogeFictive, mois: &str, echeance: Jour) -> Vec<String> {
    let mut fait = Vec::new();
    let jour = echeance.iso();

    let mut a_faire: Vec<String> = Vec::new();
    fait.extend(pas(d, &format!("DSN de {} déposée", mois_fr(mois)), &format!("{}T10:00:00", jour), |ctx| {
        if dsn_partie(ctx, mois)? {
            return Ok(false);
        }
        // LA DSN SE FABRIQUE A SON DEPOT, depuis les engagements valides du mois.
        d.reverifier()?;
        crate::depart::fabriquer_dsn(ctx, mois, QUI_HORLOGE)?;
        // TOUTES LES DSN DU MOIS QUI NE SONT PAS PARTIES (#236, trouve au banc d'essai), dans l'ordre de
        // `crate::routes::mois::ordre_de_depot` : la mensuelle, puis les signalements. L'horloge ne prenait que la plus recente --
        // un signalement d'amorcage, ecrit apres la mensuelle -- et la mensuelle ne partait jamais.
        deposer_les_dsn(d, ctx, mois, |_| true, &mut a_faire)
    }));
    // Ce que le rangement de l'AER jouee laisse a faire se dit, a la date du depot, comme le dit la releve.
    fait.extend(a_faire.into_iter().map(|p| format!("Le {} : {}", le(&format!("{}T10:00:00", jour)), p)));

    fait.extend(pas(d, &format!("documents de {} envoyés aux salariés", mois_fr(mois)), &format!("{}T11:00:00", jour), |ctx| {
        let lignes = Moteur::nouveau(ctx).lignes()?;
        let engagements = engagement::etats::du_mois(ctx, mois).map_err(Echec::from)?;
        if !crate::routes::mois::documents_partis(ctx, mois, &engagements, &lignes)?.is_empty() {
            return Ok(false);
        }
        // LES DOCUMENTS D'UN MOIS QUI N'EST PAS PARTI ne partent pas (#236, trouve sur la demo Les Faisceaux : un mois que
        // l'horloge n'avait pas pu ecrire voyait « ses documents envoyes ») : ils suivent sa DSN, jamais avant.
        if !dsn_partie(ctx, mois)? {
            return Ok(false);
        }
        // LA BOITE MUETTE, et jamais une autre : le passe joue n'ecrit rien dans `courriels-simules/`.
        let mut ouvrir = || -> R<Box<dyn Courriel>> { Ok(Box::new(BoiteMuette::nouvelle())) };
        match crate::routes::envois::envoyer_documents_de(ctx, mois, &Preuve::de_la_charge(&json!({})), QUI_HORLOGE, &mut ouvrir) {
            Ok(_) => Ok(true),
            // Un mois sans documents a remettre.
            Err(e) if est_une_erreur_d_operation(&e) => Ok(false),
            Err(e) => Err(e),
        }
    }));
    fait
}

// ------------------------------------------------------------------ l'annee

/// `_l_annee` : fin janvier, l'annee precedente se clot et s'exporte ; les echeances qu'on coche se cochent a la fin de
/// leur fenetre (la revue des spectacles comprise : tous continuent).
fn l_annee(d: &HorlogeFictive, annee: i32, jour: Jour) -> R<Vec<String>> {
    let racine = d.racine;
    let mut fait = Vec::new();
    let precedente = annee - 1;
    let fin_janvier = Jour::nouveau(annee, 1, 31).ok_or_else(|| erreur("ValueError", "date impossible"))?;
    if jour >= fin_janvier {
        fait.extend(pas(d, &format!("exercice {} clos", precedente), &format!("{}T10:00:00", fin_janvier.iso()), |ctx| {
            // Une annee qui a des engagements se clot ; les mois saisis sont ceux de `crate::routes::mois::mois_saisis`.
            let a_des_paies = crate::routes::mois::mois_saisis(&ctx.racine).iter().any(|m| m.starts_with(&format!("{}-", precedente)));
            if Moteur::nouveau(ctx).exercice_clos(&precedente.to_string())? || !a_des_paies {
                return Ok(false);
            }
            crate::exercice_ecrit::cloturer(ctx, &precedente.to_string(), false, QUI_HORLOGE)?;
            Ok(true)
        }));
        fait.extend(pas(d, &format!("exercice {} exporté pour la comptabilité", precedente), &format!("{}T11:00:00", fin_janvier.iso()), |ctx| {
            if !Moteur::nouveau(ctx).exercice_clos(&precedente.to_string())? || crate::exercice_ecrit::export_compta_a_jour(&ctx.racine, &precedente.to_string()) {
                return Ok(false);
            }
            crate::exercice_ecrit::export_compta_route(ctx, &json!({"annee": precedente.to_string()}))?;
            Ok(true)
        }));
    }

    // Les echeances qu'on coche, a la fin de leur fenetre (echeancier-base), si la convention de la compagnie les porte et
    // qu'elles ne le sont pas.
    let base = socle::fichiers::lire_json(&dossier_du_code().join("echeancier-base.json")).map_err(Echec::from)?;
    let ctx = d.contexte()?;
    for e in crate::routes::saisie::annuelles_de(&base, ctx.entreprise.get("idcc"))? {
        if e.get("fait_par").and_then(Value::as_str) != Some("coche") {
            continue;
        }
        let Some(dernier) = e.get("mois").and_then(Value::as_array).and_then(|m| m.iter().filter_map(Value::as_u64).max()) else { continue };
        let dernier = dernier as u32;
        let Some(fin) = Jour::nouveau(annee, dernier, jours_du_mois(annee, dernier)) else { continue };
        let cle = format!("{}:{}", valeur_en_texte(&e["cle"]), annee);
        let deja = std::fs::read_to_string(racine.join("config").join("echeancier.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v.get("fait").and_then(Value::as_object).cloned())
            .unwrap_or_default();
        if jour < fin || deja.contains_key(&cle) {
            continue;
        }
        // UNE ECHEANCE D'AVANT LA COMPAGNIE ne la regarde pas (#236, trouve au banc : une compagnie ajoutee en novembre
        // cochait « Les spectacles de 2026 » au 31/03/2026) : le jour ou elle a ete ajoutee (`compagnies::cree_le`).
        if crate::interface::compagnies::cree_le(racine).is_some_and(|c| fin < c) {
            continue;
        }
        let titre = valeur_en_texte(&e["titre"])
            .replace("{annee}", &annee.to_string())
            .replace("{annee_precedente}", &(annee - 1).to_string())
            .replace("{jour_dsn}", "");
        fait.extend(pas(d, &format!("« {} » coché", titre), &format!("{}T10:00:00", fin.iso()), |ctx| {
            crate::routes::ecrites::marquer_echeance(ctx, &cle, true, QUI_HORLOGE)?;
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
        Some(v) => Some(Jour::lire(&valeur_en_texte(v)).map_err(Echec::from)?),
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
        std::fs::create_dir_all(r.join("engagements")).unwrap();
        std::fs::write(r.join("engagements").join("2026-10.json"), "{}").unwrap();
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
        assert!(deposer_dsn_joue(&d, &Contexte::nouveau(&r, None).unwrap(), "x.dsn", false).is_err());
        assert!(deposer_dpae_joue(&d, &Contexte::nouveau(&r, None).unwrap(), "x.txt", true).is_err());
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
        assert_eq!(m["_quoi"], QUOI);
    }

    // ---------------------------------------------------------------- defaut 4 : horloge.json non objet

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
        ctx.aujourdhui = droit::contexte::jour_reel();
        let an = droit::contexte::jour_reel().an.to_string();
        let e = crate::exercice_ecrit::cloturer(&ctx, &an, false, "interface").unwrap_err();
        assert!(e.message.contains("n'est pas fini"), "{}", e.message);
    }

    // ---------------------------------------------------------------- defaut 2 : le jour de l'echeancier

    #[test]
    fn l_echeance_cochee_par_l_horloge_porte_le_jour_joue() {
        let r = racine_d_essai(true);
        let d = HorlogeFictive::verifier(&r).unwrap().au_jour(jour("2027-03-01"));
        let ctx = d.contexte().unwrap();
        crate::routes::ecrites::marquer_echeance(&ctx, "at-mp:2027", true, QUI_HORLOGE).unwrap();
        let e: Value = serde_json::from_str(&std::fs::read_to_string(r.join("config").join("echeancier.json")).unwrap()).unwrap();
        assert_eq!(e["fait"]["at-mp:2027"], json!({"le": "2027-03-01", "qui": "horloge"}));
        // L'interface, elle, garde le jour du contexte (reel).
        let mut ctx = Contexte::nouveau(&r, None).unwrap();
        ctx.aujourdhui = jour("2026-10-07");
        crate::routes::ecrites::marquer_echeance(&ctx, "at-mp:2026", true, "interface").unwrap();
        let e: Value = serde_json::from_str(&std::fs::read_to_string(r.join("config").join("echeancier.json")).unwrap()).unwrap();
        assert_eq!(e["fait"]["at-mp:2026"]["le"], "2026-10-07");
    }

    /// #236 : une echeance dont la fenetre s'est fermee AVANT que la compagnie soit ajoutee (`config/creation.json`) ne se
    /// coche pas ; ajoutee avant, elle se coche. Sans date d'ajout (une fictive livree), rien ne change.
    #[test]
    fn une_echeance_d_avant_l_ajout_de_la_compagnie_ne_se_coche_pas() {
        let cochees = |cree: Option<&str>| {
            let r = racine_d_essai(true);
            std::fs::write(r.join("config/entreprise.json"), r#"{"idcc": [{"depuis": "2026-01-01", "valeur": "3090"}]}"#).unwrap();
            if let Some(c) = cree {
                std::fs::write(r.join("config").join(crate::interface::compagnies::FICHIER_CREATION), format!(r#"{{"le": "{}"}}"#, c)).unwrap();
            }
            let d = HorlogeFictive::verifier(&r).unwrap().au_jour(jour("2027-01-05"));
            l_annee(&d, 2026, jour("2027-01-05")).unwrap().into_iter().filter(|p| p.contains("coché")).count()
        };
        let toutes = cochees(None);
        assert!(toutes > 0, "une fictive livree coche les echeances de son annee");
        assert_eq!(cochees(Some("2026-01-01")), toutes);
        assert!(cochees(Some("2026-11-02")) < toutes, "ajoutee en novembre, elle ne coche pas ce qui fermait avant");
        assert_eq!(cochees(Some("2027-01-01")), 0, "ajoutee apres l'annee, rien de 2026 ne la regarde");
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
        let p = envois::journal::chemin(&r);
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
        let jp = envois::journal::chemin(&r);
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
        for oui in [None, Some("ErreurOperation"), Some("SortieAbsente"), Some("AucunDocument"), Some("BoiteAbsente"), Some("PreuveInsuffisante"), Some("SaisieIncomplete"), Some("ExerciceClos")] {
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
        let o = HashMap::new();
        let avant = photo(&r);
        assert_eq!(route(&o, &ctx, "horloge-rattraper", &json!({"aujourdhui": "2027-03-01"})).unwrap(), json!([]));
        assert_eq!(photo(&r), avant);
        // Une fictive : la memoire dit ce que le bandeau lit, ce jour-la seulement.
        let r = racine_d_essai(true);
        let ctx = Contexte::nouveau(&r, None).unwrap();
        let o = HashMap::new();
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
