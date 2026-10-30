//! L'horloge de la compagnie fictive : `rattraper`, `dernier_rattrapage`
//! et tout ce qu'ils appellent.
//!
//! A chaque ouverture, la COMPAGNIE FICTIVE joue ce qu'une compagnie bien tenue aurait fait AVANT aujourd'hui, a sa
//! date, et seulement ce qui n'est pas fait : chaque pas regarde d'abord ce que le moteur en dit (le journal, les etapes
//! du mois, les obligations), et ne fait rien si c'est fait. Elle ne saisit jamais : elle valide, envoie, depose.
//! - Le jour de chaque embauche : ses engagements se valident a 5 h, son contrat part a la signature a 6 h, sa DPAE a 7 h
//!   (test puis reel) avec le signalement d'amorcage, et le contrat revient signe a 8 h. Un contrat parti puis rendu faux
//!   avant d'etre revenu signe repart dans sa version presente.
//! - Un contrat revenu signe que l'engagement a rendu faux part en avenant a l'instant ou l'engagement a ete revalide, et
//!   revient signe.
//! - Les signalements d'arret, de reprise anticipee et de fin de contrat que le moteur dit dus (`obligations`) partent le
//!   jour de leur date butoir, a 9 h 30 (test puis reel). Un signalement que la saisie ne permet pas d'ecrire n'est pas
//!   joue : le pas le dit, et l'horloge ne corrige pas une saisie.
//! - Le 1er du mois suivant, ce qui reste en saisie du mois se valide ; a l'echeance de la DSN, la DSN se fabrique et part
//!   (test puis reel), puis les documents aux salaries, fabriques a leur envoi, et les bulletins rectificatifs d'un mois
//!   dont les documents etaient partis.
//! - Chaque mois que la saisie touche se joue, ceux d'un contrat sans terme compris (`stock::mois_saisis`).
//! - Fin janvier, l'annee passee se clot et s'exporte pour la comptabilite. Une echeance annuelle n'est faite que par son
//!   fait (le taux AT/MP pose, la revue des spectacles) : l'horloge n'en coche aucune.
//! Ce qui vient apres aujourd'hui reste au visiteur.
//!
//! L'ORDRE EST CELUI DU CALENDRIER : l'horloge rassemble les pas dus, chacun avec son instant, les trie et les joue du plus
//! ancien au plus recent, de mois en mois sans rien sauter (une embauche du 3 avril se joue avant la DSN de mars du 15).
//!
//! FAIT : un mois dont l'echeance est passee, dont toutes les etapes sont vertes (`etapes_restantes`) et dont aucun
//! signalement n'est du. Il ne se relit plus tant que sa saisie et ses imprevus ne changent pas (`empreinte_du_mois`) et
//! que sa DSN n'est pas refusee.
//!
//! UN REFUS ATTEND SA CORRECTION : une DSN refusee par un bilan laisse son mois a faire ; a chaque ouverture, l'horloge la
//! refait datee de son echeance, et ne redepose jamais les octets qu'un organisme a refuses (`deposer_les_dsn`). Quand une
//! personne a corrige ce qui la faisait refuser, le fichier change, et il part a l'instant ou l'horloge le trouve ; ses
//! documents suivent.
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
//!   n'appelle jamais `crate::routes::documents::client_courriel`.
//!
//! Elle ne leve jamais pour un PAS : un pas en echec devient sa phrase (« Le jj/mm/aaaa : ... — non joue (...) ») et les
//! autres continuent. Seuls les refus du decor (une fiche entreprise illisible, une echeance introuvable) remontent.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use envois::courriel::{Courriel, ErreurCourriel};
use socle::date::Jour;
use crate::compagnie::depot_acte::{self, Preuve};
use envois::etats;
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::vrai;
use socle::echec::Echec;
use crate::routes::etapes::Moteur;

/// Qui signe, au journal, ce que l'horloge joue.
pub const QUI_HORLOGE: &str = "horloge";
/// La memoire de l'horloge, dans `config/`.
pub const FICHIER_HORLOGE: &str = "horloge.json";
/// Les routes de ce module.
pub const ROUTES: [&str; 2] = ["horloge-rattraper", "horloge-derniere"];

const ORIGINE: &str = "compagnie fictive : joue par l'horloge";
const QUOI: &str = "L'horloge de la compagnie fictive : le dernier jour joue, ce qui n'est plus a regarder, et ce qu'elle a fait en dernier (le bandeau le dit).";

// ------------------------------------------------------------------ la garde

/// LA GARDE : la preuve que la racine est une COMPAGNIE FICTIVE, exigee de tout ce qui agit. Elle ne se fabrique
/// que par `verifier`, qui lit `bridage::branchement` ; `reverifier` la relit avant chaque geste.
pub struct HorlogeFictive<'a> {
    racine: &'a Path,
    /// LE JOUR JOUE : celui que l'horloge rattrape. Chaque contexte qu'elle ouvre porte ce jour (`aujourdhui` ET `date`),
    /// jamais l'horloge reelle de la machine : la cloture d'une annee, l'export, le jour inscrit
    /// a l'echeancier, la DPAE se jugent au jour joue (le contexte de l'interface, lui, garde la date reelle).
    jour: Jour,
}

impl<'a> HorlogeFictive<'a> {
    /// `None` si les envois de cette racine ne sont pas joues.
    pub fn verifier(racine: &'a Path) -> Option<HorlogeFictive<'a>> {
        crate::compagnie::bridage::branchement(racine).joue().then(|| HorlogeFictive { racine, jour: droit::contexte::jour_reel() })
    }

    /// Le jour que cette compagnie fictive joue.
    fn au_jour(self, jour: Jour) -> HorlogeFictive<'a> {
        HorlogeFictive { jour, ..self }
    }

    /// Un contexte AU JOUR JOUE (`date` et `aujourdhui`).
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
        if crate::compagnie::bridage::branchement(self.racine).joue() {
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

/// Le mois en lettres : « octobre 2026 ».
fn mois_fr(mois: &str) -> String {
    // Une ligne du recit ne se refuse pas pour son mois : un mois hors 01-12 s'ecrit tel quel, jamais janvier.
    socle::date::libelle_du_mois_ou_tel_quel(mois, socle::date::Graphie::Accentuee)
}

/// `JJ/MM/AAAA` d'un `AAAA-MM-JJ...`.
fn le(iso: &str) -> String {
    let c: Vec<char> = iso.chars().collect();
    let t = |a: usize, b: usize| -> String { c.iter().skip(a).take(b - a).collect() };
    format!("{}/{}/{}", t(8, 10), t(5, 7), t(0, 4))
}

/// Combien de lignes le journal des depots porte (0 sans journal).
fn n_lignes(racine: &Path) -> usize {
    let p = envois::journal::chemin(racine);
    std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v.as_array().map(Vec::len)).unwrap_or(0)
}

/// Date les lignes du journal ecrites depuis la `depuis`-ieme : un pas joue se lit a la date ou il aurait eu
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

/// Les refus ordinaires d'une operation (un refus du moteur sans classe en est un).
fn est_une_erreur_d_operation(e: &Echec) -> bool {
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

/// La premiere ligne du message (160 caracteres au plus), ou le nom de la classe quand le message est vide.
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

/// Un pas joue -- le geste, ses lignes de journal datees, et sa phrase. Une erreur devient la phrase, sans
/// arreter les autres pas. `geste` rend `true` quand il a fait quelque chose (sa phrase se dit), `false` quand il n'y
/// avait rien a faire (ni phrase, ni date).
fn pas(d: &HorlogeFictive, quoi: &str, quand: &str, mut geste: impl FnMut(&Contexte) -> R<bool>) -> Vec<String> {
    let racine = d.racine;
    let echec = |e: &Echec| vec![format!("Le {} : {} — non joué ({})", le(quand), quoi, motif_de(e))];
    // LE CONTEXTE DU JOUR DU PAS : ce qu'il ecrit (une date de constitution, un jour inscrit) est celui ou il a lieu.
    let au_jour = match Jour::lire(quand.get(..10).unwrap_or("")) {
        Ok(j) => HorlogeFictive { racine, jour: j.min(d.jour) },
        Err(_) => HorlogeFictive { racine, jour: d.jour },
    };
    let ctx = match au_jour.contexte_a(au_jour.jour) {
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

/// JOUE. La re-verification precede l'appel.
fn deposer_dsn_joue(d: &HorlogeFictive, ctx: &Contexte, fichier: &str, reel: bool) -> R<()> {
    d.reverifier()?;
    depot_acte::deposer_dsn(ctx, &json!(fichier), reel, None, QUI_HORLOGE, false, "")?;
    Ok(())
}

/// JOUE. La re-verification precede l'appel.
fn deposer_dpae_joue(d: &HorlogeFictive, ctx: &Contexte, fichier: &str, reel: bool) -> R<()> {
    d.reverifier()?;
    depot_acte::deposer_dpae(ctx, &json!(fichier), reel, None, QUI_HORLOGE, false, "")?;
    Ok(())
}

/// LES DSN DU MOIS QUI NE SONT PAS PARTIES et que `garder` retient (sur ce que le fichier declare), deposees chacune en
/// test puis en reel -- le bilan du test lu avant le reel (`depot_acte::exiger_repetition`) --, dans
/// l'ordre de `crate::routes::etapes::ordre_de_depot`. Rend vrai si l'une est partie ; `a_faire` recoit ce que le rangement des AER
/// jouees laisse a faire (`rattacher`).
/// CE QUI EST PARTI NE REPART PAS : un fichier parti en reel et qu'aucun bilan ne refuse est fait ; les octets qu'un
/// organisme a refuses (en test ou en reel) ne repartent jamais. Seul un fichier change depuis son refus -- la correction
/// d'une personne -- part.
fn deposer_les_dsn(
    d: &HorlogeFictive,
    ctx: &Contexte,
    mois: &str,
    garder: impl Fn(&envois::depot::Apercu) -> bool,
    a_faire: &mut Vec<String>,
) -> R<bool> {
    let f = Moteur::nouveau(ctx).fichiers_deposables(mois)?;
    let chemins: Vec<String> = f["dsn"].as_array().map(|l| l.iter().map(|x| valeur_en_texte(&x["chemin"])).collect()).unwrap_or_default();
    let journal = envois::journal::lu(&ctx.racine)?;
    let lignes: Vec<&Value> = journal.lignes
        .iter()
        .filter(|l| l.get("quoi").and_then(Value::as_str) == Some("dsn") && l.get("accepte").is_some_and(vrai))
        .collect();
    let mut fait = false;
    for chemin in chemins {
        let nom = Path::new(&chemin).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let Ok(octets) = std::fs::read(&chemin) else { continue };
        if !garder(&envois::depot::lire(&octets)) {
            continue;
        }
        let empreinte = json!(envois::journal::empreinte(&octets));
        let mut deja = false;
        for l in lignes.iter().filter(|l| l.get("fichier").and_then(Value::as_str) == Some(nom.as_str())) {
            let refuse = etats::refuse_par_un_retour(l)?;
            deja |= (l.get("mode").and_then(Value::as_str) == Some("reel") && !refuse) || (refuse && l.get("empreinte") == Some(&empreinte));
        }
        if deja {
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

/// Les retours des depots joues arrivent tout de suite. Rend ce que le rangement
/// des AER jouees laisse a faire (`crate::compagnie::releve::rattacher_simulation`), comme les autres chemins de la releve.
fn rattacher(d: &HorlogeFictive, ctx: &Contexte) -> R<Vec<String>> {
    d.reverifier()?;
    Ok(crate::compagnie::releve::rattacher_simulation(ctx, 0)?.1)
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

// ------------------------------------------------------------------ le calendrier

/// Un geste du calendrier. Chacun regarde, a son instant, ce que le moteur en dit, et ne fait rien si c'est fait.
enum Geste {
    /// Les engagements du mois qui commencent au plus tard `jusqu_au` se valident.
    Valider { mois: String, jusqu_au: Jour },
    /// Les contrats de l'embauche du `debut` partent a la signature.
    Contrats { mois: String, debut: String },
    /// La DPAE de l'embauche du `debut`, et son signalement d'amorcage.
    Dpae { mois: String, debut: String },
    /// Les contrats de l'embauche du `debut` reviennent signes.
    Signes { mois: String, debut: String },
    /// L'avenant du contrat (`salarie`, `debut`) part a la signature, puis revient signe.
    Avenant { mois: String, salarie: String, debut: String, nom: String },
    /// Le signalement d'arret (04) ou de reprise (05) de l'arret de `salarie` au `dernier` jour travaille.
    Arret { mois: String, salarie: String, dernier: String, cle: &'static str, nature: &'static str },
    /// Le signalement de fin du contrat de `salarie` qui finit le `fin`.
    Fin { mois: String, salarie: String, fin: String },
    /// La DSN du mois, fabriquee a son echeance.
    Dsn { mois: String, echeance: Jour },
    /// Les documents du mois aux salaries.
    Documents { mois: String },
    /// L'exercice `annee` se clot.
    Cloture { annee: i32 },
    /// L'exercice `annee` s'exporte pour la comptabilite.
    Export { annee: i32 },
}

/// Un pas du calendrier : son instant (`AAAA-MM-JJTHH:MM:SS`, a Paris), sa phrase, son geste.
struct Pas {
    quand: String,
    quoi: String,
    geste: Geste,
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
    // SA MEMOIRE (config/horloge.json) : le dernier jour joue, et les mois dont tout est fait, chacun avec l'empreinte de
    // ce qui le fait (sa saisie et ses imprevus). La veille repasse toutes les vingt minutes : un mois fait ne se relit
    // que si ce qui le fait a change (une correction, un arret saisi apres coup).
    let memoire_f = racine.join("config").join(FICHIER_HORLOGE);
    let memoire: Map<String, Value> = match std::fs::read_to_string(&memoire_f).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        Some(Value::Object(m)) => m,
        _ => Map::new(),
    };
    let mut clos: Map<String, Value> = memoire.get("mois_faits").and_then(Value::as_object).cloned().unwrap_or_default();
    let mut annees_faites: BTreeSet<i64> =
        memoire.get("annees_faites").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_i64).collect()).unwrap_or_default();

    // CE QUI EST DU, chaque pas avec son instant : les mois qui ne sont pas faits, les annees qui ne sont pas finies.
    let saisis = engagement::stock::mois_saisis(racine).unwrap_or_default();
    let mut calendrier: Vec<Pas> = Vec::new();
    let mut echus: Vec<String> = Vec::new();
    {
        let ctx = d.contexte()?;
        for mois in &saisis {
            let premier = socle::date::premier_du_mois(mois).map_err(Echec::from)?;
            if jour < premier {
                break;
            }
            // Un mois fait dont la DSN est refusee apres coup (un bilan arrive plus tard) n'est plus fait.
            if clos.get(mois).and_then(Value::as_str) == Some(empreinte_du_mois(racine, mois).as_str()) && etat_de_la_dsn(&ctx, mois)? != EtatDsn::Refusee {
                continue;
            }
            if les_pas_du_mois(&d, &ctx, mois, &mut calendrier)? {
                echus.push(mois.clone());
            }
        }
    }
    let annees: Vec<i32> = match saisis.first() {
        Some(premier) => {
            let debut: i32 = premier.get(..4).and_then(|t| t.parse().ok()).unwrap_or(jour.an);
            (debut..=jour.an).filter(|a| !annees_faites.contains(&i64::from(*a))).collect()
        }
        None => Vec::new(),
    };
    for annee in &annees {
        les_pas_de_l_annee(*annee, jour, &mut calendrier)?;
    }

    // L'ORDRE DU CALENDRIER : du plus ancien au plus recent, de mois en mois sans rien sauter ; a instant egal, dans
    // l'ordre ou les pas se sont dits (le tri est stable).
    calendrier.sort_by(|a, b| a.quand.cmp(&b.quand));
    let mut fait: Vec<String> = Vec::new();
    for p in &calendrier {
        fait.extend(jouer(&d, p));
    }

    // FAIT : un mois echu dont toutes les etapes sont vertes (`etapes_restantes`, ce que l'ecran montre) et dont aucun
    // signalement n'est du. Un mois refuse, ou dont un signalement n'a pas pu s'ecrire, se regarde a chaque ouverture.
    for mois in &echus {
        let ctx = d.contexte()?;
        if Moteur::nouveau(&ctx).etapes_restantes(mois)?.is_empty() && signalements_dus(&ctx, mois, jour).is_empty() {
            clos.insert(mois.clone(), json!(empreinte_du_mois(racine, mois)));
        } else {
            clos.remove(mois);
        }
    }
    for annee in annees {
        if annee_finie(&d, annee - 1)? {
            annees_faites.insert(i64::from(annee));
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
    sortie.insert("mois_faits".into(), Value::Object(clos));
    sortie.insert("annees_faites".into(), json!(annees_faites.iter().collect::<Vec<_>>()));
    sortie.insert("dernier".into(), dernier);
    socle::ecriture::ecrire_json(&memoire_f, &Value::Object(sortie), 2).map_err(|m| erreur("OSError", m))?;
    Ok(fait)
}

/// LES PAS D'UN MOIS jusqu'au jour joue, chacun a son instant :
/// - le jour de chaque embauche du mois : ses engagements se valident a 5 h, ses contrats partent a la signature a 6 h
///   (imprimes, sans courriel dans la fiche),
///   sa DPAE a 7 h, ses contrats reviennent signes a 8 h ;
/// - l'avenant d'un contrat, a l'instant ou son engagement a ete valide en dernier ;
/// - les signalements dus, le jour de leur date butoir, a 9 h 30 ;
/// - le 1er du mois suivant a 9 h, ce qui reste en saisie se valide ;
/// - le jour de l'echeance de la DSN, la DSN a 10 h, puis les documents a 11 h. Un mois dont la DSN a ete refusee attend
///   sa correction : sa DSN et ses documents se regardent a l'instant ou l'horloge joue.
/// Rend vrai si l'echeance du mois est passee (le mois peut alors etre fait).
fn les_pas_du_mois(d: &HorlogeFictive, ctx: &Contexte, mois: &str, calendrier: &mut Vec<Pas>) -> R<bool> {
    let jour = d.jour;
    let jour_iso = jour.iso();
    let debuts: BTreeSet<String> = engagement::stock::objets_du_mois(d.racine, mois)
        .map_err(Echec::from)?
        .iter()
        .filter(|o| !o.est_importe())
        .map(|o| engagement::debut_brut(&Value::Object(o.brut.clone())))
        .filter(|debut| debut.starts_with(mois) && **debut <= *jour_iso)
        .collect();
    for debut in debuts {
        let Ok(le_jour) = Jour::lire(&debut) else { continue };
        let j = le(&debut);
        let m = mois.to_string();
        calendrier.push(Pas {
            quand: format!("{}T05:00:00", debut),
            quoi: format!("engagements de l'embauche du {} validés", j),
            geste: Geste::Valider { mois: m.clone(), jusqu_au: le_jour },
        });
        calendrier.push(Pas {
            quand: format!("{}T06:00:00", debut),
            quoi: format!("contrats de l'embauche du {} envoyés à la signature", j),
            geste: Geste::Contrats { mois: m.clone(), debut: debut.clone() },
        });
        calendrier.push(Pas {
            quand: format!("{}T07:00:00", debut),
            quoi: format!("DPAE de l'embauche du {} envoyée", j),
            geste: Geste::Dpae { mois: m.clone(), debut: debut.clone() },
        });
        calendrier.push(Pas {
            quand: format!("{}T08:00:00", debut),
            quoi: format!("contrats de l'embauche du {} revenus signés", j),
            geste: Geste::Signes { mois: m, debut },
        });
    }
    if let Ok(apercu) = crate::routes::contrats::signature_apercu(ctx, &json!({"mois": mois})) {
        for c in apercu["contrats"].as_array().into_iter().flatten() {
            let (salarie, debut) = (valeur_en_texte(&c["salarie"]), valeur_en_texte(&c["debut"]));
            let Some(quand) = revalide_le(ctx, mois, &salarie, &debut) else { continue };
            if quand.get(..10).unwrap_or("") > jour_iso.as_str() {
                continue;
            }
            let nom = valeur_en_texte(&c["nom"]);
            calendrier.push(Pas { quand, quoi: String::new(), geste: Geste::Avenant { mois: mois.to_string(), salarie, debut, nom } });
        }
    }
    calendrier.extend(signalements_dus(ctx, mois, jour));

    let suivant = socle::date::dernier_du_mois(mois).map_err(Echec::from)?.plus_jours(1).map_err(Echec::from)?;
    if jour < suivant {
        return Ok(false);
    }
    let fin = socle::date::dernier_du_mois(mois).map_err(Echec::from)?;
    let valide_le = match crate::interface::compagnies::cree_le(d.racine) {
        Some(c) if c > suivant => c.min(jour),
        _ => suivant,
    };
    calendrier.push(Pas {
        quand: format!("{}T09:00:00", valide_le.iso()),
        quoi: format!("engagements de {} validés", mois_fr(mois)),
        geste: Geste::Valider { mois: mois.to_string(), jusqu_au: fin },
    });
    // Le jour ou la DSN est due (`echeance_dsn`, reporte au jour ouvre) : elle part ce matin-la, avant midi.
    let echeance = declarations::echeance_dsn::de_la_compagnie(ctx, mois).map_err(Echec::from)?.jour;
    if jour < echeance {
        return Ok(false);
    }
    let (quand_dsn, quand_documents) = if etat_de_la_dsn(ctx, mois)? == EtatDsn::Refusee {
        let maintenant = maintenant_du(jour);
        (maintenant.clone(), maintenant)
    } else {
        (format!("{}T10:00:00", echeance.iso()), format!("{}T11:00:00", echeance.iso()))
    };
    calendrier.push(Pas { quand: quand_dsn, quoi: format!("DSN de {} déposée", mois_fr(mois)), geste: Geste::Dsn { mois: mois.to_string(), echeance } });
    calendrier.push(Pas {
        quand: quand_documents,
        quoi: format!("documents de {} envoyés aux salariés", mois_fr(mois)),
        geste: Geste::Documents { mois: mois.to_string() },
    });
    Ok(true)
}

/// FIN JANVIER DE `annee`, l'annee d'avant se clot a 10 h, puis s'exporte pour la comptabilite a 11 h.
fn les_pas_de_l_annee(annee: i32, jour: Jour, calendrier: &mut Vec<Pas>) -> R<()> {
    let fin_janvier = Jour::nouveau(annee, 1, 31).ok_or_else(|| erreur("ValueError", "date impossible"))?;
    if jour < fin_janvier {
        return Ok(());
    }
    let precedente = annee - 1;
    calendrier.push(Pas { quand: format!("{}T10:00:00", fin_janvier.iso()), quoi: format!("exercice {} clos", precedente), geste: Geste::Cloture { annee: precedente } });
    calendrier.push(Pas {
        quand: format!("{}T11:00:00", fin_janvier.iso()),
        quoi: format!("exercice {} exporté pour la comptabilité", precedente),
        geste: Geste::Export { annee: precedente },
    });
    Ok(())
}

/// L'instant ou l'horloge joue, le jour `jour` : maintenant, si c'est ce jour-la ; sinon ce jour a midi.
fn maintenant_du(jour: Jour) -> String {
    let m = socle::paris::maintenant();
    if m.starts_with(&jour.iso()) {
        m
    } else {
        format!("{}T12:00:00", jour.iso())
    }
}

/// UN PAS JOUE : son geste, a son instant (`pas`). Rend ses phrases.
fn jouer(d: &HorlogeFictive, p: &Pas) -> Vec<String> {
    let (quand, quoi) = (p.quand.as_str(), p.quoi.as_str());
    match &p.geste {
        Geste::Valider { mois, jusqu_au } => valider_les_engagements(d, mois, *jusqu_au, quoi, quand),
        Geste::Contrats { mois, debut } => {
            // Ceux qui ont un courriel partent a la signature ; un contrat sans courriel dans la fiche s'imprime et se
            // remet en main propre : l'impression est son depart.
            let mut imprimes: Vec<Signable> = Vec::new();
            let mut fait = pas(d, quoi, quand, |ctx| {
                let (par_courriel, sans): (Vec<Signable>, Vec<Signable>) =
                    signables(ctx, mois)?.into_iter().filter(|c| c.debut == *debut && c.a_envoyer).partition(|c| c.courriel);
                imprimes = sans;
                if par_courriel.is_empty() {
                    return Ok(false);
                }
                // LA BOITE MUETTE, et jamais une autre : le passe joue n'ecrit rien dans `courriels-simules/`.
                let mut ouvrir = || -> R<Box<dyn Courriel>> { Ok(Box::new(BoiteMuette::nouvelle())) };
                let contrats: Vec<Value> = par_courriel.iter().map(Signable::charge).collect();
                crate::routes::contrats::demander_signature(ctx, &json!({"mois": mois, "contrats": contrats}), &mut ouvrir)?;
                Ok(true)
            });
            if !imprimes.is_empty() {
                let noms = imprimes.iter().map(|c| c.nom.as_str()).collect::<Vec<_>>().join(", ");
                let quoi = format!("contrats de l'embauche du {} imprimés pour {} (pas de courriel dans sa fiche), remis en main propre", le(debut), noms);
                fait.extend(pas(d, &quoi, quand, |ctx| {
                    let contrats: Vec<Value> = imprimes.iter().map(Signable::charge).collect();
                    crate::routes::contrats::contrats_imprimer_par(ctx, &json!({"mois": mois, "contrats": contrats}), QUI_HORLOGE)?;
                    Ok(true)
                }));
            }
            fait
        }
        Geste::Dpae { mois, debut } => pas(d, quoi, quand, |ctx| {
            let a_declarer = Moteur::nouveau(ctx).embauches_a_declarer(Some(mois))?;
            let attente: Vec<&Value> =
                a_declarer.as_array().map(|a| a.iter().filter(|e| e.get("debut").and_then(Value::as_str) == Some(debut.as_str())).collect()).unwrap_or_default();
            if attente.is_empty() {
                return Ok(false);
            }
            let engagements: Vec<Value> = attente.iter().map(|e| json!({"salarie": e["salarie"], "debut": e["debut"]})).collect();
            let r = crate::routes::depots::dpae(ctx, &json!({"mois": mois, "engagements": engagements}))?;
            let fichier = r["fichiers"][0]["chemin"].as_str().unwrap_or("").to_string();
            for reel in [false, true] {
                deposer_dpae_joue(d, ctx, &fichier, reel)?;
                // LE BILAN DU TEST AVANT LE REEL (`depot_acte::exiger_repetition`) ; puis celui du reel.
                rattacher(d, ctx)?;
            }
            // LE SIGNALEMENT D'AMORCAGE (nature 08, motif 01 « embauche effective ») part AVEC LA DPAE, dans le meme
            // geste (atelier/README.md, « Demander un taux » ; `crate::routes::depots::dpae` produit les deux) : pas a
            // l'echeance de la DSN. Un signalement ne recoit pas d'AER : son rangement n'a rien a dire.
            deposer_les_dsn(d, ctx, mois, |a| a.champ("nature") == "08", &mut Vec::new())?;
            Ok(true)
        }),
        // UN CONTRAT REVIENT SIGNE S'IL EST PARTI (par courriel ou imprime) : jamais un retour sans son depart.
        Geste::Signes { mois, debut } => pas(d, quoi, quand, |ctx| {
            let manquent: Vec<Signable> = signables(ctx, mois)?.into_iter().filter(|c| c.debut == *debut && !c.a_envoyer).collect();
            if manquent.is_empty() {
                return Ok(false);
            }
            for c in &manquent {
                crate::routes::contrats::contrat_recu_par(ctx, mois, &c.salarie, &c.debut, Some(c.numero.as_str()), true, QUI_HORLOGE)?;
            }
            Ok(true)
        }),
        Geste::Avenant { mois, salarie, debut, nom } => {
            let du_contrat = |ctx: &Contexte| -> R<Option<Value>> {
                let a = crate::routes::contrats::signature_apercu(ctx, &json!({"mois": mois}))?;
                Ok(a["contrats"].as_array().into_iter().flatten().find(|x| valeur_en_texte(&x["salarie"]) == *salarie && valeur_en_texte(&x["debut"]) == *debut).cloned())
            };
            // La ligne de l'horloge dit ce que le journal dit : parti par courriel, ou imprime et remis en main propre.
            let imprime = std::cell::Cell::new(false);
            let mut fait = pas(d, &format!("avenant du contrat de {} envoyé à la signature", nom), quand, |ctx| {
                let Some(x) = du_contrat(ctx)?.filter(|x| x.get("en_avenant").is_some_and(vrai)) else {
                    return Ok(false);
                };
                let numero = valeur_en_texte(&envois::etats::numero_de_cle(&x["numero"]));
                let voulu = json!({"salarie": salarie, "debut": debut, "numero": if numero.is_empty() { Value::Null } else { json!(numero) }, "mois": mois});
                // LA BOITE MUETTE, et jamais une autre ; sans courriel dans la fiche, l'avenant s'imprime et se remet en
                // main propre : l'impression est son depart.
                let mut ouvrir = || -> R<Box<dyn Courriel>> { Ok(Box::new(BoiteMuette::nouvelle())) };
                if crate::routes::contrats::envoyer_avenants(ctx, std::slice::from_ref(&voulu), &Preuve::de_la_charge(&json!({})), &mut ouvrir).is_err() {
                    crate::routes::contrats::contrats_imprimer_par(ctx, &json!({"mois": mois, "avenants": [voulu]}), QUI_HORLOGE)?;
                    imprime.set(true);
                }
                Ok(true)
            });
            if imprime.get() {
                fait = fait.into_iter().map(|l| l.replace("envoyé à la signature", "imprimé, remis en main propre")).collect();
            }
            let signe = if imprime.get() { "signé, remis en main propre" } else { "revenu signé" };
            fait.extend(pas(d, &format!("avenant du contrat de {} {}", nom, signe), quand, |ctx| {
                let Some(x) = du_contrat(ctx)? else { return Ok(false) };
                if !(x.get("avenant_attendu").is_some_and(vrai) && x.get("en_attente").is_some_and(vrai)) {
                    return Ok(false);
                }
                let numero = valeur_en_texte(&envois::etats::numero_de_cle(&x["numero"]));
                crate::routes::contrats::contrat_recu_avec(ctx, mois, salarie, debut, Some(numero.as_str()), true, &x["avenant"]["numero"], QUI_HORLOGE, None)?;
                Ok(true)
            }));
            fait
        }
        Geste::Arret { mois, salarie, dernier, cle, nature } => pas(d, quoi, quand, |ctx| {
            // ENCORE DU au jour du pas ? Le visiteur a pu le faire.
            let a = crate::routes::imprevus::tous_les_arrets(ctx)?
                .into_iter()
                .find(|x| valeur_en_texte(&x["salarie"]) == *salarie && valeur_en_texte(&x["dernier_jour_travaille"]) == *dernier);
            let Some(a) = a else { return Ok(false) };
            let encore = crate::obligations::de_l_arret(ctx, &a, &crate::obligations::Suivi::lire(ctx)?, ctx.aujourdhui)?
                .iter()
                .any(|o| o["cle"] == json!(cle) && due(o, ctx.aujourdhui).is_some());
            if !encore {
                return Ok(false);
            }
            d.reverifier()?;
            crate::routes::imprevus::route(&HashMap::new(), ctx, "arret-signaler", &json!({"salarie": salarie, "dernier_jour": dernier, "nature": nature}))?;
            deposer_les_dsn(d, ctx, mois, |x| x.champ("nature") == *nature, &mut Vec::new())?;
            Ok(true)
        }),
        Geste::Fin { mois, salarie, fin } => pas(d, quoi, quand, |ctx| {
            let encore = crate::routes::imprevus::fins_a_signaler(ctx, mois, ctx.aujourdhui)?
                .iter()
                .any(|x| valeur_en_texte(&x["salarie"]) == *salarie && valeur_en_texte(&x["fin"]) == *fin && due(&x["obligation"], ctx.aujourdhui).is_some());
            if !encore {
                return Ok(false);
            }
            d.reverifier()?;
            crate::routes::imprevus::route(&HashMap::new(), ctx, "fin-contrat-signaler", &json!({"mois": mois, "salarie": salarie}))?;
            deposer_les_dsn(d, ctx, mois, |x| x.champ("nature") == declarations::fctu::NATURE, &mut Vec::new())?;
            Ok(true)
        }),
        Geste::Dsn { mois, echeance } => {
            let mut a_faire: Vec<String> = Vec::new();
            let mut fait = pas(d, quoi, quand, |ctx| {
                if etat_de_la_dsn(ctx, mois)? == EtatDsn::Acceptee {
                    return Ok(false);
                }
                // LA DSN SE FABRIQUE A SON DEPOT, depuis les engagements valides du mois, DATEE DE SON ECHEANCE quel que
                // soit le jour ou elle part : la DSN d'un mois refuse que personne n'a corrige se refait octet pour octet,
                // et ne repart pas (`deposer_les_dsn`).
                d.reverifier()?;
                let a_l_echeance = HorlogeFictive { racine: d.racine, jour: (*echeance).min(d.jour) };
                let ctx_e = a_l_echeance.contexte()?;
                let fabriquer = || crate::compagnie::depart::fabriquer_dsn(&ctx_e, mois, QUI_HORLOGE);
                match instant_du_pas(&format!("{}T10:00:00", echeance.iso())) {
                    Some(t) => socle::temps::pendant(t, fabriquer)?,
                    None => fabriquer()?,
                };
                // TOUTES LES DSN DU MOIS QUI NE SONT PAS PARTIES, dans l'ordre de `crate::routes::etapes::ordre_de_depot` :
                // la mensuelle, puis les signalements.
                deposer_les_dsn(d, ctx, mois, |_| true, &mut a_faire)
            });
            // Ce que le rangement de l'AER jouee laisse a faire se dit, a la date du depot, comme le dit la releve.
            fait.extend(a_faire.into_iter().map(|p| format!("Le {} : {}", le(quand), p)));
            fait
        }
        Geste::Documents { mois } => {
            // Ceux qui n'ont pas de courriel : leurs documents s'impriment et se remettent en main propre (l'impression
            // est leur depart) ; ils n'empechent pas les autres de partir.
            let mut sans: Vec<(String, String)> = Vec::new();
            let mut fait = pas(d, quoi, quand, |ctx| {
                // LES DOCUMENTS D'UN MOIS DONT LA DSN N'EST PAS ACCEPTEE ne partent pas : ils suivent sa DSN, jamais avant.
                if etat_de_la_dsn(ctx, mois)? != EtatDsn::Acceptee {
                    return Ok(false);
                }
                let bilan = Moteur::nouveau(ctx).bilan_du_mois(mois)?;
                let a_envoyer = bilan["etapes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|e| e["cle"] == json!(envois::etats::DOCUMENTS))
                    .flat_map(|e| e["groupes"].as_array().cloned().unwrap_or_default())
                    .any(|g| g["etat"] == json!("a_faire"));
                if !a_envoyer {
                    return Ok(false);
                }
                sans = crate::routes::documents::sans_courriel(ctx, mois)?;
                // LA BOITE MUETTE, et jamais une autre : le passe joue n'ecrit rien dans `courriels-simules/`.
                let mut ouvrir = || -> R<Box<dyn Courriel>> { Ok(Box::new(BoiteMuette::nouvelle())) };
                match crate::routes::documents::envoyer_documents_de(ctx, mois, &Preuve::de_la_charge(&json!({})), QUI_HORLOGE, &mut ouvrir) {
                    Ok(_) => Ok(true),
                    // Un mois sans documents a remettre par courriel.
                    Err(e) if est_une_erreur_d_operation(&e) => Ok(false),
                    Err(e) => Err(e),
                }
            });
            if !sans.is_empty() {
                let noms = sans.iter().map(|(_, n)| n.as_str()).collect::<Vec<_>>().join(", ");
                let cles: Vec<&str> = sans.iter().map(|(c, _)| c.as_str()).collect();
                let quoi = format!("documents de {} imprimés pour {} (pas de courriel dans sa fiche), remis en main propre", mois_fr(mois), noms);
                fait.extend(pas(d, &quoi, quand, |ctx| {
                    crate::routes::documents::documents_imprimer(ctx, &json!({"mois": mois, "salaries": cles}), QUI_HORLOGE)?;
                    Ok(true)
                }));
            }
            fait
        }
        Geste::Cloture { annee } => pas(d, quoi, quand, |ctx| {
            let a_des_paies = engagement::stock::mois_saisis(&ctx.racine).unwrap_or_default().iter().any(|m| m.starts_with(&format!("{}-", annee)));
            if Moteur::nouveau(ctx).exercice_clos(&annee.to_string())? || !a_des_paies {
                return Ok(false);
            }
            crate::compagnie::exercice_ecrit::cloturer(ctx, &annee.to_string(), false, QUI_HORLOGE)?;
            Ok(true)
        }),
        Geste::Export { annee } => pas(d, quoi, quand, |ctx| {
            if !Moteur::nouveau(ctx).exercice_clos(&annee.to_string())? || crate::compagnie::exercice_ecrit::export_compta_a_jour(&ctx.racine, &annee.to_string()) {
                return Ok(false);
            }
            crate::compagnie::exercice_ecrit::export_compta_route(ctx, &json!({"annee": annee.to_string()}))?;
            Ok(true)
        }),
    }
}

// ------------------------------------------------------------------ le mois et ses embauches

/// LES ENGAGEMENTS DU MOIS SE VALIDENT, comme une compagnie bien tenue les valide : ceux qui commencent au plus tard
/// `jusqu_au`, a `quand`. Rien ne part d'un engagement en saisie ; un engagement que le moteur refuse de valider (une
/// saisie incomplete) reste en saisie, et le pas le dit.
fn valider_les_engagements(d: &HorlogeFictive, mois: &str, jusqu_au: Jour, quoi: &str, quand: &str) -> Vec<String> {
    pas(d, quoi, quand, |ctx| {
        let objets = engagement::stock::objets_du_mois(&ctx.racine, mois).map_err(Echec::from)?;
        let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(Echec::from)?;
        let mut fait = false;
        for (i, o) in objets.iter().enumerate() {
            // Un engagement auquel il manque un fait attend qu'on le complete (`engagement::stock::A_COMPLETER`).
            if o.est_valide() || o.est_importe() || o.a_completer().is_some() || engagement::debut_brut(&Value::Object(o.brut.clone())) > jusqu_au.iso() {
                continue;
            }
            engagement::etats::valider(ctx, mois, i, &departs).map_err(Echec::from)?;
            let brut = Value::Object(o.brut.clone());
            if let Some(cle) = engagement::cle_de_l_engagement(ctx, &brut) {
                crate::routes::fiches::poser_anciennete_a_l_embauche(ctx, &cle, &engagement::debut_brut(&brut))?;
            }
            fait = true;
        }
        Ok(fait)
    })
}

/// Un contrat de spectacle du mois qui n'est pas revenu signe (`signature-apercu`).
struct Signable {
    salarie: String,
    debut: String,
    numero: String,
    /// « NOM Prenom ».
    nom: String,
    /// Sa fiche a un courriel : il part a la signature par courriel ; sinon il s'imprime.
    courriel: bool,
    /// Jamais parti, ou parti puis rendu faux par l'engagement avant d'etre revenu signe : il part dans sa version presente.
    a_envoyer: bool,
}

impl Signable {
    /// Ce que les routes des contrats recoivent pour lui.
    fn charge(&self) -> Value {
        json!({"salarie": self.salarie, "debut": self.debut, "numero": if self.numero.is_empty() { Value::Null } else { json!(self.numero) }})
    }
}

/// Les contrats de spectacle du mois qui ne sont pas revenus signes.
fn signables(ctx: &Contexte, mois: &str) -> R<Vec<Signable>> {
    let a = crate::routes::contrats::signature_apercu(ctx, &json!({"mois": mois}))?;
    Ok(a["contrats"]
        .as_array()
        .map(|c| {
            c.iter()
                .filter(|x| !x.get("recu_le").is_some_and(vrai))
                .map(|x| Signable {
                    salarie: valeur_en_texte(&x["salarie"]),
                    debut: valeur_en_texte(&x["debut"]),
                    nom: valeur_en_texte(&x["nom"]),
                    courriel: x.get("destinataire").is_some_and(vrai),
                    numero: valeur_en_texte(&envois::etats::numero_de_cle(&x["numero"])),
                    a_envoyer: x.get("a_envoyer").is_some_and(vrai) && !x.get("en_avenant").is_some_and(vrai),
                })
                .collect()
        })
        .unwrap_or_default())
}

// ------------------------------------------------------------------ les avenants

/// Le jour et l'heure ou l'engagement de ce contrat a ete valide en dernier (`valide.le`) : `AAAA-MM-JJTHH:MM:SS`.
fn revalide_le(ctx: &Contexte, mois: &str, salarie: &str, debut: &str) -> Option<String> {
    engagement::stock::objets_du_mois(&ctx.racine, mois).ok()?.into_iter().find_map(|o| {
        let b = Value::Object(o.brut.clone());
        let sien = engagement::cle_de_l_engagement(ctx, &b).as_deref() == Some(salarie) && engagement::debut_brut(&b) == debut;
        sien.then(|| o.brut.get("valide").and_then(|v| v.get("le")).and_then(Value::as_str).map(|t| t.chars().take(19).collect()))?
    })
}

// ------------------------------------------------------------------ les signalements

/// Une obligation que le moteur dit a faire (ou en retard) et dont la date butoir est passee ou d'aujourd'hui : sa butoir.
fn due(o: &Value, jour: Jour) -> Option<Jour> {
    let etat = o.get("etat").and_then(Value::as_str)?;
    if etat != crate::obligations::A_FAIRE && etat != crate::obligations::EN_RETARD {
        return None;
    }
    Jour::lire(o.get("butoir").and_then(Value::as_str)?).ok().filter(|b| *b <= jour)
}

/// LES SIGNALEMENTS DU MOIS que le moteur dit dus au jour `jour` (`obligations::de_l_arret`, `imprevus::fins_a_signaler`) :
/// l'arret (04) et la reprise anticipee (05) d'un arret dont le dernier jour travaille est du mois, la fin de contrat (07)
/// d'un contrat qui finit dans le mois. Chacun s'ecrit et part (test puis reel) le jour de sa date butoir, a 9 h 30 --
/// avant la DSN du mois quand c'est le jour de son echeance (un arret subroge) ; un signalement fait n'est plus du.
fn signalements_dus(ctx: &Contexte, mois: &str, jour: Jour) -> Vec<Pas> {
    let mut dus = Vec::new();
    let natures = [("signalement_arret", declarations::signalement::ARRET, "d'arrêt"), ("signalement_reprise", declarations::signalement::REPRISE, "de reprise")];
    let arrets = crate::routes::imprevus::tous_les_arrets(ctx).unwrap_or_default();
    if let Ok(suivi) = crate::obligations::Suivi::lire(ctx) {
        for a in arrets.iter().filter(|a| valeur_en_texte(&a["dernier_jour_travaille"]).starts_with(mois)) {
            let obligations = crate::obligations::de_l_arret(ctx, a, &suivi, jour).unwrap_or_default();
            for (cle, nature, dit) in natures {
                let Some(butoir) = obligations.iter().find(|o| o["cle"] == json!(cle)).and_then(|o| due(o, jour)) else { continue };
                let (salarie, dernier) = (valeur_en_texte(&a["salarie"]), valeur_en_texte(&a["dernier_jour_travaille"]));
                dus.push(Pas {
                    quand: format!("{}T09:30:00", butoir.iso()),
                    quoi: format!("signalement {} de {} (dernier jour travaillé le {}) déposé", dit, salarie, le(&dernier)),
                    geste: Geste::Arret { mois: mois.to_string(), salarie, dernier, cle, nature },
                });
            }
        }
    }
    for f in crate::routes::imprevus::fins_a_signaler(ctx, mois, jour).unwrap_or_default() {
        let Some(butoir) = due(&f["obligation"], jour) else { continue };
        let (salarie, fin) = (valeur_en_texte(&f["salarie"]), valeur_en_texte(&f["fin"]));
        dus.push(Pas {
            quand: format!("{}T09:30:00", butoir.iso()),
            quoi: format!("signalement de fin du contrat de {} (fin le {}) déposé", salarie, le(&fin)),
            geste: Geste::Fin { mois: mois.to_string(), salarie, fin },
        });
    }
    dus
}

// ------------------------------------------------------------------ ce qui fait un mois

/// L'empreinte de ce qui fait un mois : sa saisie (`stock::empreinte_de_la_saisie_du_mois`) et ses imprevus. Un mois
/// fait dont l'empreinte change se rejoue.
fn empreinte_du_mois(racine: &Path, mois: &str) -> String {
    let saisie = engagement::stock::empreinte_de_la_saisie_du_mois(racine, mois).ok().flatten().unwrap_or_default();
    let imprevus = std::fs::read(engagement::imprevus::dossier(racine).join(format!("{}.json", mois))).unwrap_or_default();
    let suivi = std::fs::read(engagement::imprevus::dossier(racine).join("obligations.json")).unwrap_or_default();
    let mut tout = saisie.into_bytes();
    tout.extend(socle::fichiers::sha256_hex(&imprevus).into_bytes());
    tout.extend(socle::fichiers::sha256_hex(&suivi).into_bytes());
    socle::fichiers::sha256_hex(&tout)[..16].to_string()
}

/// L'annee `annee` est-elle finie pour l'horloge : passee fin janvier de l'annee d'apres, et close et exportee, ou sans
/// aucun mois dans le logiciel.
fn annee_finie(d: &HorlogeFictive, annee: i32) -> R<bool> {
    if Jour::nouveau(annee + 1, 1, 31).is_none_or(|f| d.jour < f) {
        return Ok(false);
    }
    let ctx = d.contexte()?;
    let a = annee.to_string();
    if !engagement::stock::mois_saisis(&ctx.racine).unwrap_or_default().iter().any(|m| m.starts_with(&format!("{}-", a))) {
        return Ok(true);
    }
    Ok(Moteur::nouveau(&ctx).exercice_clos(&a)? && crate::compagnie::exercice_ecrit::export_compta_a_jour(&ctx.racine, &a))
}

/// Ou en est la DSN mensuelle d'un mois.
#[derive(PartialEq, Eq)]
enum EtatDsn {
    /// Rien n'est parti en reel.
    Absente,
    /// Partie en reel, et un bilan la refuse : elle attend sa correction.
    Refusee,
    /// Partie en reel, et aucun bilan ne la refuse.
    Acceptee,
}

/// L'etat de la DSN mensuelle du mois au journal (`import::mois::dsn_avant`).
fn etat_de_la_dsn(ctx: &Contexte, mois: &str) -> R<EtatDsn> {
    Ok(match crate::import::mois::dsn_avant(ctx, mois)?["etat"].as_str() {
        Some("refusee") => EtatDsn::Refusee,
        Some("passee" | "attente") => EtatDsn::Acceptee,
        _ => EtatDsn::Absente,
    })
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
}
