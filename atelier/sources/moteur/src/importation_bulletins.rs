//! `importation-apercu` sur la boite aux lettres `import/` (lot D du plan d'import, `prive/journal/PLAN-IMPORT-BULLETINS.md`).
//! RIEN N'EST ECRIT.
//!
//! 1. Le tri (`importation_tri`) : chaque fichier recoit sa sorte, par le contenu.
//! 2. Les DSN et AEM trouvees, ou qu'elles soient (sous-dossier, zip), sont copiees dans un `export/` du dossier de
//!    travail (temporaire, efface avant de rendre) et passent par `crate::routes::import::apercu_lu` : le meme code que
//!    l'import d'un export de CoolPaie, inchange -- celles d'une autre entreprise exceptees (lot G1) : devant le Siren de
//!    la compagnie (`importation::siren_de_reference`, etape 3), elles prennent le sort `incoherent` et rien n'en est lu.
//! 3. Les bulletins (`bulletin_lu`), page par page, completent cet apercu : les fiches qu'ils creent (salaries par NIR,
//!    spectacles par numero d'objet avec leur titre, l'entreprise) et leurs ecarts avec les fiches existantes (signales,
//!    jamais appliques) ; un contrat atteste par bulletin (`atteste_par: "bulletin"`, meme cle `_cle_engagement` que
//!    les AEM) ; la paie, rapprochee des paies versees des engagements importes et des DSN par (NIR, debut, fin).
//!
//! LE BULLETIN FAIT FOI (decide, plan §4) : une paie deja declaree est COMPLETEE par le bulletin (cotisations par
//! part, cout employeur, lignes, dates, titre...) et, sur le brut, le net imposable, le net a payer et le PAS, c'est
//! le montant du bulletin qui s'ecrit ; l'ecart est note dans le compte rendu. Une paie qu'aucune DSN ne declare est
//! nouvelle : `contrat` = « bulletin <numero> » (CoolPaie), sinon le nom du fichier (et la page, s'il en a plusieurs),
//! `declare_par: "bulletin"`, `mois` = le mois de la fin de periode du bulletin.
//!
//! UN MOIS SAISI N'EST PAS REPRIS : un bulletin dont le mois a son fichier dans `paies/` est « deja saisi », rien
//! d'autre (ni fiche, ni contrat, ni paie).
//!
//! RIEN NE SE DEVINE (regle 1). Une page n'est pas reprise quand : elle n'a pas de texte, le rang d'en-tetes de son
//! tableau ne se trouve pas, elle se lit de deux facons, ce qui l'identifie ne se lit pas (le lecteur), son NIR ne se lit pas ou contredit sa cle, son
//! Siret n'est pas celui de la compagnie (« bulletin d'une autre entreprise »), plusieurs paies declarees pourraient
//! etre la sienne, ou un autre bulletin donne d'autres montants pour la meme paie. LE BULLETIN FAIT FOI (lot E) : une
//! page qui se contredit elle-meme (`incohérent : ...` du lecteur : le brut n'est pas la somme des lignes, le net ne
//! se retrouve pas -- un centime sur le net avant impot chez CoolPaie) est REPRISE avec les montants qu'elle imprime,
//! et l'incoherence est un avertissement du compte rendu. Un libelle de ligne inconnu de la table n'empeche pas la
//! reprise (decide, plan §4) : la ligne est gardee telle quelle, sans compteur, et le sort le dit.
//!
//! LES NOMS DE FICHIER NE PORTENT RIEN DE PERSONNEL. Ce que l'import ecrit nomme chaque fichier par sa COPIE dans
//! `historique/bulletins/` : `<16 premiers caracteres de l'empreinte SHA-256>.<pdf|dsn|txt>` (`nom_copie`) -- le
//! `fichier` d'une paie ou d'un contrat, le `bulletin.fichier`, le `contrat` d'une paie sans numero de bulletin. Le
//! meme nom dans l'apercu (lu dans `import/`) et dans l'import (relu dans la copie) : un import se rejoue, un apercu
//! d'apres l'import ne voit rien de neuf. Le compte rendu (`tri`) garde le chemin d'origine, pour l'ecran, et dit la
//! `copie` de chaque fichier repris.
//!
//! LE COMPTE RENDU (`tri`) donne a chaque fichier un sort, et un seul, avec sa raison en une phrase (sans nom ni NIR :
//! la phrase peut partir chez l'editeur) ; un PDF a aussi le sort de chacune de ses pages. Les sorts : voir `SORTS`.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::RoundingStrategy;
use serde_json::{json, Map, Value};

use crate::bulletin_lu::{self, BulletinLu, Compteurs, Table};
use droit::contexte::Contexte;
use socle::date::Jour;
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::vrai;
use crate::importation_completer as completer;
use crate::importation_tri::Sorte;
use socle::num::Decimal;
use crate::routes::import::{self as ri, ACreer, Apercu, CleEngagement};

/// Les sorts du compte rendu, dans l'ordre ou il les compte.
///
/// | sort | ce qu'il veut dire |
/// |---|---|
/// | `repris` | ce que le fichier porte entre dans l'apercu |
/// | `deja_repris` | tout ce qu'il porte est deja dans l'historique (ou dans un autre fichier du dossier) |
/// | `libelle_inconnu` | repris, mais une ligne au moins a un libelle que la table ne connait pas : gardee telle quelle |
/// | `deja_saisi` | le mois a des engagements dans `engagements/` : non repris |
/// | `incoherent` | une DSN ou une AEM d'une autre entreprise (rien n'en est lu) ; la page contredit le reste (bulletin d'une autre entreprise, NIR qui contredit sa cle, plusieurs paies possibles, deux bulletins qui different) : non repris -- une page qui ne retombe pas sur ses propres totaux est reprise (le bulletin fait foi) |
/// | `incomplet` | ce qui identifie le bulletin ne se lit pas : non repris |
/// | `sans_texte` | aucun mot (lettres en traces, scan) : non repris |
/// | `sans_entetes` | le rang d'en-tetes de son tableau (« Libellé », « Base », les parts) ne se trouve pas : non repris |
/// | `ambigu` | la page se lit de deux facons (deux en-tetes, une colonne sans sa part, deux adresses de salarie) : non repris |
/// | `annexe` | l'annexe d'un bulletin, qui en detaille les cotisations : rien a reprendre |
/// | `non_reconnu` | ni DSN, ni AEM, ni PDF lisible, ni archive |
/// | `doublon` | la copie exacte d'un autre fichier du dossier |
/// | `archive` | un zip, ouvert : son contenu a son propre sort |
pub const SORTS: [&str; 13] = [
    "repris",
    "deja_repris",
    "libelle_inconnu",
    "deja_saisi",
    "incoherent",
    "incomplet",
    "sans_texte",
    "sans_entetes",
    "ambigu",
    "annexe",
    "non_reconnu",
    "doublon",
    "archive",
];

/// Les sorts d'une page, du plus grave au moins grave : un PDF de plusieurs pages prend le plus grave.
const GRAVITE: [&str; 10] =
    ["incoherent", "incomplet", "ambigu", "sans_entetes", "sans_texte", "libelle_inconnu", "repris", "deja_saisi", "deja_repris", "annexe"];

/// Les sorts qui laissent quelque chose a voir (l'editeur, la table de lecture) : le fichier n'est pas repris.
pub(crate) fn non_repris(sort: &str) -> bool {
    matches!(sort, "incoherent" | "incomplet" | "ambigu" | "sans_entetes" | "sans_texte" | "non_reconnu")
}

const A_CREER: &str = "<a creer>";

/// Le compte rendu d'une page de PDF.
struct PageCr {
    page: usize,
    sort: &'static str,
    raison: String,
    infos: Map<String, Value>,
}

impl PageCr {
    fn json(&self) -> Value {
        let mut m = Map::new();
        m.insert("page".into(), json!(self.page));
        m.insert("sort".into(), json!(self.sort));
        m.insert("raison".into(), json!(self.raison));
        for (k, v) in &self.infos {
            m.insert(k.clone(), v.clone());
        }
        Value::Object(m)
    }
}

/// Un bulletin lu qui peut etre repris.
struct Lu {
    /// Le rang du fichier dans le tri.
    fichier: usize,
    page: usize,
    b: BulletinLu,
    /// Les 13 caracteres du NIR (sans la cle), comme dans les DSN, les AEM et les fiches.
    nir: String,
    /// La cle du NIR, quand le bulletin l'imprime.
    cle_nir: Option<String>,
    /// Le mois de la fin de periode.
    mois: String,
    compteurs: Result<Compteurs, String>,
    /// Ce que la page dit d'elle-meme qui ne se retrouve pas (`incohérent : ...`) : un avertissement, le bulletin fait foi.
    incoherences: Vec<String>,
}

/// Ce que la lecture d'un dossier rend : l'apercu, le tri (les fichiers sur le disque, a lire avant d'effacer le dossier
/// de travail), le nom de la copie de chaque fichier repris (par rang dans le tri), et ce que les bulletins ont dit de
/// leurs salaries et de leurs entreprises, pages refusees comprises (`Lecture::partiel`) : de quoi caviarder.
pub(crate) struct DossierLu {
    pub(crate) ap: Apercu,
    pub(crate) tri: crate::importation_tri::Tri,
    pub(crate) copies: BTreeMap<usize, String>,
    pub(crate) salaries_lus: Vec<bulletin_lu::Salarie>,
    pub(crate) entreprises_lues: Vec<bulletin_lu::Entreprise>,
}

/// `importation-apercu` avec `{"dossier": ...}`.
pub(crate) fn apercu_dossier(ctx: &Contexte, dossier: &Path) -> R<Apercu> {
    lire_dossier(ctx, dossier, |lu| Ok(lu.ap))
}

/// Lit le dossier, et passe ce qui a ete lu a `suite` AVANT d'effacer le dossier de travail (les zip ouverts) : la
/// copie de l'import et le texte des pages refusees s'y lisent encore.
pub(crate) fn lire_dossier<T>(ctx: &Contexte, dossier: &Path, suite: impl FnOnce(DossierLu) -> R<T>) -> R<T> {
    let travail = ri::dossier_de_travail();
    let rendu = trier_et_lire(ctx, dossier, &travail).and_then(suite);
    let _ = std::fs::remove_dir_all(&travail);
    rendu
}

/// Le nom de la copie d'un fichier dans `historique/bulletins/` : l'empreinte, rien de personnel. `None` pour ce qui ne
/// se reprend pas.
pub(crate) fn nom_copie(f: &crate::importation_tri::Fichier) -> Option<String> {
    let ext = match f.sorte {
        Sorte::Dsn => "dsn",
        // `importation::lire_aem` ne lit que les `*.txt`.
        Sorte::Aem => "txt",
        Sorte::PdfTexte => "pdf",
        _ => return None,
    };
    (f.empreinte.len() >= 16).then(|| format!("{}.{}", &f.empreinte[..16], ext))
}

fn mois_de(j: &Jour) -> String {
    format!("{:04}-{:02}", j.an, j.mois)
}

/// Un texte compare sans casse, sans accents, sans espaces ni ponctuation de bord (`bulletin_lu::jetons`).
fn compare(t: &str) -> String {
    bulletin_lu::jetons(t).concat()
}

fn dec(x: &Option<Decimal>) -> Value {
    x.map_or(Value::Null, |v| Value::String(v.to_string()))
}

/// Le NIR imprime : 13 caracteres, ou 15 avec la cle (alors verifiee).
fn nir_lu(b: &BulletinLu) -> Result<Option<(String, Option<String>)>, String> {
    let Some(brut) = &b.salarie.nir else { return Ok(None) };
    let n: String = brut.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();
    match n.chars().count() {
        13 => Ok(Some((n, None))),
        15 => {
            let (nir, cle): (String, String) = (n.chars().take(13).collect(), n.chars().skip(13).collect());
            if declarations::champs::cle_nir(&nir).as_deref() != Some(cle.as_str()) {
                return Err("la clé du NIR imprimé ne correspond pas à son numéro".into());
            }
            Ok(Some((nir, Some(cle))))
        }
        _ => Err("le NIR imprimé n'a ni 13 ni 15 caractères".into()),
    }
}

/// Les chiffres d'un Siret imprime.
fn siret_lu(b: &BulletinLu) -> Option<String> {
    let s: String = b.entreprise.siret.as_deref()?.chars().filter(char::is_ascii_digit).collect();
    (s.len() == 14).then_some(s)
}

/// Une adresse imprimee : les rangs avant le code postal, puis « 99999 Commune ». Sans rang de code postal, rien
/// (on ne devine pas ce qui est la rue et ce qui est la ville).
fn adresse_lue(rangs: &[String]) -> Option<(String, String, String)> {
    let i = rangs.iter().position(|r| {
        let (cp, reste) = r.split_at(r.char_indices().nth(5).map_or(r.len(), |(i, _)| i));
        cp.len() == 5 && cp.chars().all(|c| c.is_ascii_digit()) && reste.starts_with(' ') && !reste.trim().is_empty()
    })?;
    let rue = rangs[..i].join(", ");
    let (cp, commune) = rangs[i].split_at(5);
    Some((rue, cp.to_string(), commune.trim().to_string()))
}

/// LE COMPTE URSSAF imprime apres « Urssaf : » (lot I2, 3e) : ses chiffres, sans espaces. C'est la case que le moteur
/// imprime de `urssaf_compte` sous la meme etiquette (`bulletin.rs`) ; sur les bulletins CoolPaie du poste, la valeur
/// imprimee a cette place est celle du `urssaf_compte` de la fiche (releve du 14/10/2026 sur trois bulletins de 2022 a
/// 2026, valeurs non recopiees) -- une inference pour un autre logiciel. Ce qui n'est pas que des chiffres ne se reprend pas.
fn urssaf_lu(b: &BulletinLu) -> Option<String> {
    let t: String = b.entreprise.urssaf.as_deref()?.chars().filter(|c| !c.is_whitespace()).collect();
    (!t.is_empty() && t.chars().all(|c| c.is_ascii_digit())).then_some(t)
}

/// L'IDCC d'une convention imprimee (« IDCC n° 3090 - ... ») ; rien si le texte ne le dit pas.
fn idcc_lu(texte: &str) -> Option<String> {
    let haut = texte.to_uppercase();
    let i = haut.find("IDCC")?;
    let apres: String = haut[i + 4..].chars().skip_while(|c| !c.is_ascii_digit()).take_while(char::is_ascii_digit).collect();
    let saut = haut[i + 4..].chars().take_while(|c| !c.is_ascii_digit()).count();
    (apres.len() == 4 && saut <= 6).then_some(apres)
}

fn trier_et_lire(ctx: &Contexte, dossier: &Path, travail: &Path) -> R<DossierLu> {
    let tri = crate::importation_tri::trier(dossier, travail)?;

    // 1. Les DSN et AEM : ce que chacune dit de son employeur (lot G1). Elles ne sont copiees la ou l'import existant
    // les lit qu'apres l'etape 3, et seulement celles de la compagnie.
    let pieces: Vec<(usize, bool, crate::importation::Employeur)> = tri
        .fichiers
        .iter()
        .enumerate()
        .filter_map(|(i, f)| match f.sorte {
            Sorte::Dsn => Some((i, false, crate::importation::employeur_dsn(&f.disque))),
            Sorte::Aem => Some((i, true, crate::importation::employeur_aem(&f.disque))),
            _ => None,
        })
        .collect();

    // 2. Les bulletins, page par page.
    let table = Table::livree().map_err(|m| erreur("ErreurOperation", format!("La table de lecture des bulletins ne se lit pas : {}", m)))?;
    let saisis: HashSet<String> = crate::routes::etapes::mois_saisis(&ctx.racine).into_iter().collect();
    let mut pages: BTreeMap<usize, Vec<PageCr>> = BTreeMap::new();
    let mut illisibles: BTreeMap<usize, String> = BTreeMap::new();
    let mut lus: Vec<Lu> = Vec::new();
    let mut par_logiciel: BTreeMap<String, usize> = BTreeMap::new();
    let (mut du, mut au): (Option<Jour>, Option<Jour>) = (None, None);
    let mut n_pages = 0usize;
    let (mut salaries_lus, mut entreprises_lues): (Vec<bulletin_lu::Salarie>, Vec<bulletin_lu::Entreprise>) = (Vec::new(), Vec::new());
    for (i, f) in tri.fichiers.iter().enumerate() {
        if f.sorte != Sorte::PdfTexte {
            continue;
        }
        let lectures = match bulletin_lu::lire_avec(&f.disque, &table) {
            Ok(l) => l,
            Err(m) => {
                illisibles.insert(i, m.replace(&*f.disque.to_string_lossy(), &f.chemin));
                continue;
            }
        };
        let cr = pages.entry(i).or_default();
        for l in lectures {
            n_pages += 1;
            if let Some(s) = l.partiel {
                salaries_lus.push(s);
            }
            let b = match l.resultat {
                Ok(b) => {
                    salaries_lus.push(b.salarie.clone());
                    entreprises_lues.push(b.entreprise.clone());
                    b
                }
                Err(r) => {
                    cr.push(PageCr { page: l.page, sort: r.code(), raison: r.message(), infos: Map::new() });
                    continue;
                }
            };
            *par_logiciel.entry(b.logiciel.clone().unwrap_or_else(|| "autre".into())).or_default() += 1;
            du = Some(du.map_or(b.debut, |d| d.min(b.debut)));
            au = Some(au.map_or(b.fin, |d| d.max(b.fin)));
            let mut infos = Map::new();
            infos.insert("logiciel".into(), json!(b.logiciel));
            infos.insert("numero".into(), json!(b.numero));
            infos.insert("debut".into(), json!(b.debut.iso()));
            infos.insert("fin".into(), json!(b.fin.iso()));
            let mois = mois_de(&b.fin);
            infos.insert("mois".into(), json!(mois));
            let refus = |sort: &'static str, raison: String, infos: Map<String, Value>| PageCr { page: l.page, sort, raison, infos };
            if saisis.contains(&mois) {
                cr.push(refus("deja_saisi", format!("le mois {} a des engagements dans engagements/ : non repris", mois), infos));
                continue;
            }
            // LE BULLETIN FAIT FOI : une page qui se contredit (un centime sur le net) est reprise telle qu'imprimee.
            let incoherences: Vec<String> = b.avertissements.iter().filter(|a| a.starts_with("incohérent")).cloned().collect();
            let (nir, cle_nir) = match nir_lu(&b) {
                Ok(Some(n)) => n,
                Ok(None) => {
                    cr.push(refus("incomplet", "le NIR ne se lit pas : la paie ne se rattache à aucune fiche".into(), infos));
                    continue;
                }
                Err(m) => {
                    cr.push(refus("incoherent", m, infos));
                    continue;
                }
            };
            let compteurs = bulletin_lu::compteurs(&b, &ctx.racine);
            lus.push(Lu { fichier: i, page: l.page, b, nir, cle_nir, mois, compteurs, incoherences });
        }
    }

    // 3. La compagnie : son Siren (`importation::siren_de_reference`, le meme pour l'export) -- la fiche, sinon les DSN
    // si elles n'en portent qu'un, sinon les bulletins et les AEM s'ils n'en portent qu'un. Une DSN, une AEM ou un
    // bulletin d'une autre entreprise n'est pas repris ; d'une DSN ou d'une AEM ecartee, RIEN n'est lu.
    let siren_fiche = ctx.entreprise.get("siren").filter(|v| vrai(v)).map(valeur_en_texte).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let sirens_dsn: std::collections::BTreeSet<String> =
        pieces.iter().filter(|(_, aem, _)| !aem).flat_map(|(_, _, e)| e.sirens.iter().cloned()).collect();
    let mut sirens_autres: std::collections::BTreeSet<String> =
        pieces.iter().filter(|(_, aem, _)| *aem).flat_map(|(_, _, e)| e.sirens.iter().cloned()).collect();
    sirens_autres.extend(lus.iter().filter_map(|l| siret_lu(&l.b)).map(|s| s[..9].to_string()));
    let siren = crate::importation::siren_de_reference(&ctx.entreprise, &sirens_dsn, &sirens_autres);
    let mut gardes: Vec<Lu> = Vec::new();
    for l in lus {
        let ecart = match (siret_lu(&l.b), &siren) {
            (Some(s), Some(r)) if &s[..9] != r => {
                Some("bulletin d'une autre entreprise : son Siret n'est pas celui de la compagnie, non repris".to_string())
            }
            (Some(_), None) => Some(crate::importation::PLUSIEURS_ENTREPRISES.to_string()),
            _ => None,
        };
        match ecart {
            Some(raison) => {
                let infos = infos_de(&l, None);
                pages.entry(l.fichier).or_default().push(PageCr { page: l.page, sort: "incoherent", raison, infos });
            }
            None => gardes.push(l),
        }
    }
    let mut lus = gardes;
    lus.sort_by(|a, b| (a.b.fin, a.b.debut, a.fichier, a.page).cmp(&(b.b.fin, b.b.debut, b.fichier, b.page)));

    // Les DSN et AEM de la compagnie, copiees la ou l'import existant les lit, sous le nom de leur copie (`nom_copie`).
    // Un nom deja pris recoit un rang devant.
    let export = travail.join("export");
    let mut noms: BTreeMap<usize, String> = BTreeMap::new();
    let (mut dsn_dir, mut aem_dir): (Option<PathBuf>, Option<PathBuf>) = (None, None);
    let mut pris: HashSet<String> = HashSet::new();
    let mut ecartes: BTreeMap<usize, String> = BTreeMap::new();
    let mut avertis: BTreeMap<usize, String> = BTreeMap::new();
    for (i, aem, e) in &pieces {
        let f = &tri.fichiers[*i];
        match crate::importation::verdict_entreprise(*aem, e, siren.as_deref()) {
            Err(raison) => {
                ecartes.insert(*i, raison);
                continue;
            }
            Ok(Some(a)) => {
                avertis.insert(*i, a);
            }
            Ok(None) => {}
        }
        let sous = if *aem { "AEM_EDI" } else { "DSN" };
        let d = export.join(sous);
        std::fs::create_dir_all(&d).map_err(|e| erreur("OSError", format!("{} : {}", d.display(), e)))?;
        let base = nom_copie(f).unwrap_or_else(|| if *aem { "fichier.txt".into() } else { "fichier.dsn".into() });
        let mut nom = base.clone();
        let mut k = 2;
        while !pris.insert(format!("{}/{}", sous, nom)) {
            nom = format!("{}-{}", k, base);
            k += 1;
        }
        std::fs::copy(&f.disque, d.join(&nom)).map_err(|e| erreur("OSError", format!("{} : {}", f.chemin, e)))?;
        noms.insert(*i, nom);
        if *aem {
            aem_dir = Some(d);
        } else {
            dsn_dir = Some(d);
        }
    }

    // 4. Les DSN et AEM par l'import existant ; les NIR des bulletins y comptent comme des fiches a venir.
    let a_venir: HashSet<String> = lus.iter().map(|l| l.nir.clone()).collect();
    let mut ap = ri::apercu_lu(ctx, &dossier.display().to_string(), dsn_dir.as_deref(), aem_dir.as_deref(), ctx.aujourdhui, &a_venir)?;

    let mut par_nir: HashMap<String, String> = HashMap::new();
    for (cle, s) in &ctx.salaries {
        if let Some(n) = s.get("nir").filter(|v| vrai(v)) {
            par_nir.insert(valeur_en_texte(n), cle.clone());
        }
    }
    let cle_de = |nir: &str| par_nir.get(nir).cloned().unwrap_or_else(|| A_CREER.to_string());
    let deja = ri::cles_historique(ctx)?;
    let historique_paies = ri::paies_historique(ctx)?;
    let mut hist_par_cle: HashMap<(String, String, String), Value> = HashMap::new();
    for x in &historique_paies {
        hist_par_cle.insert(ri::cle_paie(x)?, x.clone());
    }
    let nirs_dsn: HashSet<String> = ap
        .a_creer
        .iter()
        .filter(|a| a.nom == "salarie")
        .filter_map(|a| a.donnees.get("nir").map(valeur_en_texte))
        .collect();

    // Le sort des DSN et des AEM, avant que les bulletins ne completent les paies.
    let mut sorts_export: BTreeMap<usize, (&'static str, String)> = BTreeMap::new();
    for (i, nom) in &noms {
        let f = &tri.fichiers[*i];
        if f.sorte == Sorte::Dsn {
            let mut n = 0usize;
            let mut nouveaux = 0usize;
            for x in ap.paies.iter().filter(|x| x.get("fichier").map(valeur_en_texte).as_deref() == Some(nom.as_str())) {
                n += 1;
                if !hist_par_cle.contains_key(&ri::cle_paie(x)?) {
                    nouveaux += 1;
                }
            }
            let mois = f.mois.clone().unwrap_or_default();
            sorts_export.insert(*i, match (n, nouveaux) {
                (0, _) => ("repris", format!("DSN de {} : aucun versement à reprendre (DSN sans versement, ou remplacée par une DSN du même mois déposée après)", mois)),
                (n, 0) => ("deja_repris", format!("DSN de {} : ses {} versement(s) sont déjà dans l'historique", mois, n)),
                (n, k) => ("repris", format!("DSN de {} : {} versement(s), dont {} à reprendre", mois, n, k)),
            });
        } else {
            let (mut n, mut nouveaux, mut sans) = (0usize, 0usize, 0usize);
            for c in ap.contrats.iter().filter(|c| &c.fichier == nom) {
                n += 1;
                if !par_nir.contains_key(&c.nir) && !nirs_dsn.contains(&c.nir) && !a_venir.contains(&c.nir) {
                    sans += 1;
                    continue;
                }
                let k: CleEngagement = (cle_de(&c.nir), c.debut.iso(), c.fin.iso(), c.numero_objet.clone(), c.heures, c.cachets, c.jours, c.brut.clone());
                if !deja.contains(&k) {
                    nouveaux += 1;
                }
            }
            let sans_txt = if sans > 0 { format!(" ; {} sans fiche (NIR qu'aucune DSN ni aucun bulletin ne déclare)", sans) } else { String::new() };
            // `importation::lire_aem` garde un contrat une fois : un fichier qui ne rend rien a ete lu ailleurs, sauf
            // s'il est dans ses illisibles.
            let illisible = ap.reponse.get("avertissements").and_then(Value::as_array).is_some_and(|a| {
                a.iter().filter_map(Value::as_str).any(|x| x.strip_prefix("AEM illisible, ignoree : ").is_some_and(|r| r.split(" : ").next() == Some(nom.as_str())))
            });
            sorts_export.insert(*i, match (n, nouveaux) {
                (0, _) if illisible => ("non_reconnu", "AEM dont les enregistrements ne se lisent pas".to_string()),
                (0, _) => ("deja_repris", "AEM : ses contrats attestés sont déjà dans un autre fichier du dossier".to_string()),
                (n, 0) if sans == 0 => ("deja_repris", format!("AEM : ses {} contrat(s) attesté(s) sont déjà dans l'historique", n)),
                (n, k) => ("repris", format!("AEM : {} contrat(s) attesté(s), dont {} nouveau(x){}", n, k, sans_txt)),
            });
        }
    }
    // Les DSN et AEM d'une autre entreprise : rien n'en a ete lu. Une AEM dont le Siret ne se lit pas passe, et le dit.
    for (i, raison) in ecartes {
        sorts_export.insert(i, ("incoherent", raison));
    }
    let mut avertissements_aem: Vec<String> = Vec::new();
    for (i, a) in &avertis {
        if let Some((_, raison)) = sorts_export.get_mut(i) {
            *raison = format!("{} ; {}", raison, a);
        }
        avertissements_aem.push(format!("AEM reprise, mais {} : {}", a, noms.get(i).cloned().unwrap_or_default()));
    }
    if let Some(Value::Array(a)) = ap.reponse.get_mut("avertissements") {
        a.extend(avertissements_aem.into_iter().map(Value::String));
    }

    let mut reponse = std::mem::take(&mut ap.reponse);
    let seuil = format!("{:04}-{:02}", ctx.aujourdhui.an - 1, ctx.aujourdhui.mois);
    let mut nouvelles_fiches = 0usize;
    let chemin_de = |l: &Lu| tri.fichiers[l.fichier].chemin.clone();

    // 5. Les salaries, par NIR : champ par champ, le bulletin le plus recent qui l'imprime.
    let mut par_salarie: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (k, l) in lus.iter().enumerate() {
        par_salarie.entry(l.nir.clone()).or_default().push(k);
    }
    for (nir, rangs) in &par_salarie {
        let siens: Vec<&Lu> = rangs.iter().map(|&k| &lus[k]).collect();
        if let Some(cle) = par_nir.get(nir) {
            let fiche = &ctx.salaries[cle];
            let ec = ecarts_salarie(fiche, &siens, &chemin_de);
            // Ce qui y est vide, de ce que les bulletins impriment (la fiche qu'ils creeraient) ; ce que la DSN complete
            // deja garde la valeur de la DSN (`completer::ajouter`).
            let champs = completer::champs_a_completer("salarie", fiche, &fiche_salarie(nir, &siens)?)?;
            let liste = reponse.get_mut("salaries").and_then(Value::as_array_mut).expect("une liste");
            let rang = match liste.iter().position(|e| e.get("cle").and_then(Value::as_str) == Some(cle.as_str())) {
                Some(i) => {
                    fusionner_ecarts(&mut liste[i], ec);
                    i
                }
                None => {
                    let libelle = format!("{} {}", fiche.get("nom").map_or(String::new(), valeur_en_texte), ri::titre_de(fiche.get("prenom")));
                    liste.push(ri::fiche_importee("salarie", Some(cle), &libelle, "existante", ec, Vec::new(), json!({})));
                    liste.len() - 1
                }
            };
            noter_par_champ(&mut liste[rang], &champs, &siens, &chemin_de);
            completer::ajouter(&mut ap.a_completer, "salarie", Some(cle), &champs);
            continue;
        }
        if nirs_dsn.contains(nir) {
            // LA DSN LA CREE, LE BULLETIN LA COMPLETE (lot I2, 3d) : ce que la DSN ne porte pas (le n° Congés Spectacles,
            // une adresse qu'elle n'a pas) vient du bulletin ; ce que la DSN donne reste.
            let proposees = fiche_salarie(nir, &siens)?;
            completer_a_creer(&mut ap.a_creer, &mut reponse, "salarie", Some(nir), &proposees)?;
            continue;
        }
        let donnees = fiche_salarie(nir, &siens)?;
        let anomalies = crate::routes::fiches::verifier_comme_a_l_ecriture("salarie", &donnees)?;
        let libelle = format!("{} {}", donnees.get("nom").map_or(String::new(), valeur_en_texte), ri::titre_de(donnees.get("prenom")));
        let liste = reponse.get_mut("salaries").and_then(Value::as_array_mut).expect("une liste");
        ap.a_creer.push(ACreer { nom: "salarie", rang: Some(liste.len()), donnees: donnees.clone() });
        liste.push(ri::fiche_importee("salarie", None, &libelle, "a_creer", json!({}), anomalies, Value::Object(donnees)));
        nouvelles_fiches += 1;
    }

    // 6. L'entreprise : creee des bulletins si ni la fiche ni une DSN ne la donne ; sinon ses ecarts.
    if let Some(dernier) = lus.iter().rev().find(|l| siret_lu(&l.b).is_some()).or(lus.last()) {
        if siren_fiche.is_some() {
            let ec = ecarts_entreprise(&ctx.entreprise, &lus, dernier, &chemin_de(dernier));
            if reponse.get("entreprise").is_some_and(Value::is_object) {
                fusionner_ecarts(&mut reponse["entreprise"], ec);
            } else {
                let libelle = ctx.entreprise.get("raison_sociale").map_or(String::new(), valeur_en_texte);
                reponse.insert("entreprise".into(), ri::fiche_importee("entreprise", None, &libelle, "existante", ec, Vec::new(), json!({})));
            }
            let champs = completer::champs_a_completer("entreprise", &ctx.entreprise, &fiche_entreprise(&lus, dernier)?)?;
            completer::noter(&mut reponse["entreprise"], &champs, json!({"source": "bulletin", "fichier": chemin_de(dernier), "page": dernier.page}));
            completer::ajouter(&mut ap.a_completer, "entreprise", None, &champs);
        } else if reponse.get("entreprise").and_then(|e| e.get("etat")).and_then(Value::as_str) == Some("a_creer") {
            // La fiche que les DSN creent, completee de ce que le bulletin imprime et qu'elles ne portent pas (3d).
            let proposees = fiche_entreprise(&lus, dernier)?;
            completer_a_creer(&mut ap.a_creer, &mut reponse, "entreprise", None, &proposees)?;
        } else if reponse.get("entreprise").is_none_or(Value::is_null) {
            let mut e = fiche_entreprise(&lus, dernier)?;
            ri::garder_le_nom(ctx, &mut e);
            let anomalies = crate::routes::fiches::verifier_comme_a_l_ecriture("entreprise", &e)?;
            let libelle = e.get("raison_sociale").map_or("?".to_string(), valeur_en_texte);
            ap.a_creer.push(ACreer { nom: "entreprise", rang: None, donnees: e.clone() });
            reponse.insert("entreprise".into(), ri::fiche_importee("entreprise", None, &libelle, "a_creer", json!({}), anomalies, Value::Object(e)));
            nouvelles_fiches += 1;
        }
    }

    // 7. Les spectacles, par numero d'objet, avec leur titre quand le bulletin le donne.
    let mut connus: HashMap<String, String> = HashMap::new();
    for (cle, sp) in &ctx.spectacles {
        connus.insert(sp.get("numero_objet").map_or(String::new(), valeur_en_texte).to_uppercase(), cle.clone());
    }
    let mut par_objet: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (k, l) in lus.iter().enumerate() {
        if let Some(o) = l.b.numero_objet.as_deref().filter(|o| !o.is_empty()) {
            par_objet.entry(o.to_uppercase()).or_default().push(k);
        }
    }
    let mut objets: Vec<(String, Vec<usize>)> = par_objet.into_iter().collect();
    objets.sort_by(|a, b| (lus[a.1[0]].b.fin, &a.0).cmp(&(lus[b.1[0]].b.fin, &b.0)));
    for (numero, rangs) in &objets {
        let titre = rangs.iter().rev().find_map(|&k| lus[k].b.spectacle.clone().filter(|t| !t.trim().is_empty()));
        let porteur = rangs.iter().rev().find(|&&k| lus[k].b.spectacle.as_ref().is_some_and(|t| !t.trim().is_empty())).map(|&k| &lus[k]);
        let analytique = rangs.iter().rev().find_map(|&k| lus[k].b.analytique.clone().filter(|t| !t.trim().is_empty()));
        let liste = reponse.get_mut("spectacles").and_then(Value::as_array_mut).expect("une liste");
        if let Some(cle) = connus.get(numero) {
            let fiche = &ctx.spectacles[cle];
            let mut ec = Map::new();
            // UN TITRE VIDE OU « A COMPLETER » (celui que l'import des DSN donne, la DSN ne portant pas le titre) prend
            // celui du bulletin, comme une fiche que l'import cree (Leandre, 14/10/2026) ; un vrai titre saisi reste, et
            // l'ecart est seulement signale.
            if let (Some(t), Some(p)) = (&titre, porteur) {
                let a = fiche.get("titre").map_or(String::new(), valeur_en_texte);
                if !completer::vide(fiche.get("titre")) && compare(&a) != compare(t) {
                    ec.insert("titre".into(), json!({"fiche": a, "bulletin": t, "fichier": chemin_de(p), "page": p.page}));
                }
            }
            let mut proposees = Map::new();
            if let Some(t) = &titre {
                proposees.insert("titre".into(), json!(t));
            }
            if let Some(x) = &analytique {
                proposees.insert("code_analytique".into(), json!(x));
            }
            let champs = completer::champs_a_completer("spectacle", fiche, &proposees)?;
            let origine = porteur.map_or_else(|| json!({"source": "bulletin"}), |p| json!({"source": "bulletin", "fichier": chemin_de(p), "page": p.page}));
            let rang = match liste.iter().position(|e| e.get("cle").and_then(Value::as_str) == Some(cle.as_str())) {
                Some(i) => {
                    fusionner_ecarts(&mut liste[i], Value::Object(ec));
                    i
                }
                None => {
                    let t = fiche.get("titre").map_or("?".to_string(), valeur_en_texte);
                    liste.push(ri::fiche_importee("spectacle", Some(cle), &format!("{} ({})", t, numero), "existante", Value::Object(ec), Vec::new(), json!({})));
                    liste.len() - 1
                }
            };
            completer::noter(&mut liste[rang], &champs, origine);
            completer::ajouter(&mut ap.a_completer, "spectacle", Some(cle), &champs);
            continue;
        }
        let deja_la = liste.iter().position(|e| {
            e.get("etat").and_then(Value::as_str) == Some("a_creer")
                && e.get("donnees").and_then(|d| d.get("numero_objet")).map(valeur_en_texte).map(|n| n.to_uppercase()).as_deref() == Some(numero.as_str())
        });
        if let Some(rang) = deja_la {
            // Un objet que les DSN ou les AEM creent : le bulletin lui donne son titre.
            let Some(a) = ap.a_creer.iter_mut().find(|a| a.nom == "spectacle" && a.rang == Some(rang)) else { continue };
            if let Some(t) = &titre {
                a.donnees.insert("titre".into(), json!(t));
            }
            if let Some(x) = &analytique {
                a.donnees.entry("code_analytique").or_insert_with(|| json!(x));
            }
            let anomalies = crate::routes::fiches::verifier_comme_a_l_ecriture("spectacle", &a.donnees)?;
            liste[rang]["donnees"] = ri::brut(&Value::Object(a.donnees.clone()));
            liste[rang]["anomalies"] = ri::brut(&Value::Array(anomalies));
            continue;
        }
        let premier = rangs.iter().map(|&k| lus[k].mois.clone()).min().unwrap_or_default();
        let dernier = rangs.iter().map(|&k| lus[k].mois.clone()).max().unwrap_or_default();
        let mut donnees = Map::new();
        donnees.insert("titre".into(), json!(titre.clone().unwrap_or_else(|| "A COMPLETER".into())));
        donnees.insert("numero_objet".into(), json!(numero));
        if let Some(x) = &analytique {
            donnees.insert("code_analytique".into(), json!(x));
        }
        donnees.insert("alias".into(), json!([]));
        donnees.insert("actif".into(), json!(dernier >= seuil));
        donnees.insert("_vu_dans".into(), json!(format!("bulletins de {} a {} ({} bulletin(s))", premier, dernier, rangs.len())));
        let anomalies = crate::routes::fiches::verifier_comme_a_l_ecriture("spectacle", &donnees)?;
        let libelle = match &titre {
            Some(t) => format!("{} ({})", t, numero),
            None => format!("numéro d'objet {}, sans titre (bulletins de {} à {})", numero, premier, dernier),
        };
        ap.a_creer.push(ACreer { nom: "spectacle", rang: Some(liste.len()), donnees: donnees.clone() });
        liste.push(ri::fiche_importee("spectacle", None, &libelle, "a_creer", json!({}), anomalies, Value::Object(donnees)));
        nouvelles_fiches += 1;
    }

    // 8. Les paies et les contrats, bulletin par bulletin, dans l'ordre du temps.
    // Les paies deja connues, par (NIR, debut, fin) : celles des DSN de cet apercu, puis celles de l'historique.
    #[derive(Clone, PartialEq)]
    enum Cible {
        Dsn(usize),
        Historique((String, String, String)),
    }
    let mut index: HashMap<(String, String, String), Vec<(String, String, Cible)>> = HashMap::new();
    let mut vus: HashSet<(String, String, String)> = HashSet::new();
    for (k, x) in ap.paies.iter().enumerate() {
        let t = |c: &str| x.get(c).map(valeur_en_texte).unwrap_or_default();
        vus.insert((t("mois"), t("nir"), t("contrat")));
        index.entry((t("nir"), t("debut"), t("fin"))).or_default().push((t("mois"), t("contrat"), Cible::Dsn(k)));
    }
    // Une paie que l'historique tient d'un bulletin seul s'efface devant la DSN qui la declare (la DSN est lue apres le
    // bulletin : l'ordre inverse) : le bulletin complete la ligne DSN, et l'ecriture retire la ligne « bulletin »
    // (`importation_ecrite::rapprocher`).
    let mut de_bulletin: HashSet<(String, String, String)> = HashSet::new();
    for x in &historique_paies {
        let t = |c: &str| x.get(c).map(valeur_en_texte).unwrap_or_default();
        if t("nir").is_empty() || !vus.insert((t("mois"), t("nir"), t("contrat"))) {
            continue;
        }
        let cle = ri::cle_paie(x)?;
        if t("declare_par") == "bulletin" {
            de_bulletin.insert(cle.clone());
        }
        index.entry((t("nir"), t("debut"), t("fin"))).or_default().push((t("mois"), t("contrat"), Cible::Historique(cle)));
    }
    let aem_cles: HashSet<CleEngagement> = ap
        .contrats
        .iter()
        .map(|c| (cle_de(&c.nir), c.debut.iso(), c.fin.iso(), c.numero_objet.clone(), c.heures, c.cachets, c.jours, c.brut.clone()))
        .collect();
    let mut dates_attestees: HashSet<(String, String, String)> =
        deja.iter().chain(aem_cles.iter()).map(|k| (k.0.clone(), k.1.clone(), k.2.clone())).collect();
    let mut faits: HashSet<CleEngagement> = HashSet::new();
    let mut sans_objet: HashSet<CleEngagement> = deja.iter().chain(aem_cles.iter()).map(sans_l_objet).collect();
    let mut prises: HashMap<String, (String, usize, Decimal, Decimal)> = HashMap::new();
    let (mut paies_nouvelles, mut paies_completees) = (0usize, 0usize);
    let mut paies_liste: Vec<Value> = Vec::new();
    let mut contrats_liste: Vec<Value> = Vec::new();
    let mut pages_reprises = 0usize;
    let mut pages_incoherentes = 0usize;
    for l in &lus {
        let chemin = chemin_de(l);
        // Ce qui s'ecrit nomme la copie, pas le fichier d'origine (rien de personnel, le meme a l'apercu et a l'import).
        let copie = nom_copie(&tri.fichiers[l.fichier]).unwrap_or_else(|| chemin.clone());
        let nb_pages = tri.fichiers[l.fichier].pages.len();
        let salarie = cle_de(&l.nir);
        let mut infos = infos_de(l, Some(&salarie));
        let (debut, fin) = (l.b.debut.iso(), l.b.fin.iso());
        let mut candidats: Vec<(String, String, Cible)> = index.get(&(l.nir.clone(), debut.clone(), fin.clone())).cloned().unwrap_or_default();
        if candidats.iter().any(|c| matches!(c.2, Cible::Dsn(_))) {
            candidats.retain(|c| !matches!(&c.2, Cible::Historique(k) if de_bulletin.contains(k)));
        }
        if candidats.len() > 1 {
            let memes: Vec<(String, String, Cible)> = candidats.iter().filter(|c| c.0 == l.mois).cloned().collect();
            if memes.len() == 1 {
                candidats = memes;
            } else {
                let mois: Vec<String> = candidats.iter().map(|c| c.0.clone()).collect();
                let raison = format!("plusieurs paies déclarées pour les mêmes dates ({}) : le bulletin ne se rattache à aucune", mois.join(", "));
                pages.entry(l.fichier).or_default().push(PageCr { page: l.page, sort: "incoherent", raison, infos });
                continue;
            }
        }
        let cible = candidats.into_iter().next();
        let ident = match &cible {
            Some((mois, contrat, _)) => format!("paie {} {} {}", mois, l.nir, contrat),
            None => format!("bulletin {} {} {}", l.nir, debut, fin),
        };
        if let Some((autre, page, brut, net)) = prises.get(&ident) {
            let (sort, raison) = if *brut == l.b.brut && *net == l.b.net_a_payer {
                ("deja_repris", format!("le même bulletin est déjà lu : {} page {}", autre, page))
            } else {
                ("incoherent", format!("un autre bulletin ({} page {}) donne d'autres montants pour la même paie : aucun des deux ne l'emporte", autre, page))
            };
            pages.entry(l.fichier).or_default().push(PageCr { page: l.page, sort, raison, infos });
            continue;
        }
        prises.insert(ident, (chemin.clone(), l.page, l.b.brut, l.b.net_a_payer));

        // La paie.
        let (ligne, source, avant): (Map<String, Value>, &str, Option<Value>) = match &cible {
            Some((_, _, Cible::Dsn(k))) => {
                let x = ap.paies[*k].clone();
                let avant = ri::cle_paie(&x).ok().and_then(|c| hist_par_cle.get(&c).cloned());
                (x.as_object().cloned().unwrap_or_default(), "dsn", avant)
            }
            Some((_, _, Cible::Historique(c))) => {
                let x = hist_par_cle[c].clone();
                (x.as_object().cloned().unwrap_or_default(), "historique", Some(x))
            }
            None => {
                let contrat = match &l.b.numero {
                    Some(n) => format!("bulletin {}", n),
                    None => {
                        if nb_pages > 1 { format!("{} page {}", copie, l.page) } else { copie.clone() }
                    }
                };
                let mut m = Map::new();
                for (k, v) in [
                    ("mois", json!(l.mois)),
                    ("salarie", json!(salarie)),
                    ("nir", json!(l.nir)),
                    ("contrat", json!(contrat)),
                    ("numero_objet", json!("")),
                    ("debut", json!(debut)),
                    ("fin", json!(fin)),
                    ("brut", json!("")),
                    ("net_imposable", json!("")),
                    ("net_a_payer", json!("")),
                    ("pas", json!("")),
                    ("verse_le", json!("")),
                    ("declare_par", json!("bulletin")),
                    ("depose_le", json!("")),
                    ("fichier", json!(copie)),
                ] {
                    m.insert(k.into(), v);
                }
                let avant = hist_par_cle.get(&ri::cle_paie(&Value::Object(m.clone()))?).cloned();
                (m, "bulletin", avant)
            }
        };
        let mut ligne = ligne;
        let ecarts = completer(&mut ligne, &l.b, &copie, l.page);
        let ligne = Value::Object(ligne);
        let etat_paie = if avant.as_ref() == Some(&ligne) {
            "deja"
        } else if source == "bulletin" {
            "nouvelle"
        } else {
            "completee"
        };
        match &cible {
            Some((_, _, Cible::Dsn(k))) => ap.paies[*k] = ligne.clone(),
            _ if etat_paie != "deja" => ap.paies.push(ligne.clone()),
            _ => {}
        }
        if etat_paie != "deja" {
            paies_liste.push(ligne.clone());
            if etat_paie == "nouvelle" {
                paies_nouvelles += 1;
            } else {
                paies_completees += 1;
            }
        }
        let mut phrases: Vec<String> = vec![match (etat_paie, source) {
            ("deja", _) => "paie déjà dans l'historique".to_string(),
            ("nouvelle", _) => "paie nouvelle : aucune DSN ne la déclare".to_string(),
            (_, "dsn") => format!("complète la paie déclarée en DSN ({})", valeur_en_texte(&ligne["mois"])),
            _ => format!("complète la paie de l'historique ({})", valeur_en_texte(&ligne["mois"])),
        }];
        if !ecarts.is_empty() {
            let gagnants: Vec<&str> = ecarts.iter().filter(|e| e.2).map(|e| e.0).collect();
            let notes: Vec<&str> = ecarts.iter().filter(|e| !e.2).map(|e| e.0).collect();
            if !gagnants.is_empty() {
                phrases.push(format!("le bulletin l'emporte sur : {}", gagnants.iter().map(|c| libelle_champ(c)).collect::<Vec<_>>().join(", ")));
            }
            if !notes.is_empty() {
                phrases.push(format!("écart noté, non appliqué : {}", notes.iter().map(|c| libelle_champ(c)).collect::<Vec<_>>().join(", ")));
            }
            infos.insert(
                "ecarts".into(),
                Value::Array(
                    ecarts
                        .iter()
                        .map(|(champ, avant, gagne, apres)| {
                            json!({"champ": champ, "source": source, "valeur": avant, "bulletin": apres, "le_bulletin_l_emporte": gagne})
                        })
                        .collect(),
                ),
            );
        }
        infos.insert("paie".into(), json!(etat_paie));

        // Le contrat atteste.
        let etat_contrat = match &l.compteurs {
            Err(m) => {
                phrases.push(format!("contrat non repris : ses compteurs ne se lisent pas ({})", m));
                "inconnu"
            }
            Ok(c) => match engagement(l, c, &salarie, &copie) {
                Err(m) => {
                    phrases.push(format!("contrat non repris : {}", m));
                    "inconnu"
                }
                Ok((cle, ligne_contrat)) => {
                    let dates = (cle.0.clone(), cle.1.clone(), cle.2.clone());
                    // Un bulletin qui n'imprime pas de numero d'objet ne contredit pas le contrat atteste qui en porte un.
                    let meme = deja.contains(&cle) || aem_cles.contains(&cle) || faits.contains(&cle);
                    if meme || (cle.3.is_empty() && sans_objet.contains(&sans_l_objet(&cle))) {
                        phrases.push("contrat déjà attesté".into());
                        "deja"
                    } else if dates_attestees.contains(&dates) {
                        phrases.push("un contrat attesté aux mêmes dates (AEM ou historique) porte d'autres compteurs ou un autre numéro d'objet : il est gardé, celui du bulletin n'est pas ajouté".into());
                        infos.insert("contrat_bulletin".into(), ligne_contrat);
                        "ecart"
                    } else {
                        phrases.push("contrat attesté ajouté".into());
                        sans_objet.insert(sans_l_objet(&cle));
                        faits.insert(cle);
                        dates_attestees.insert(dates);
                        contrats_liste.push(ligne_contrat.clone());
                        ap.engagements_bulletins.push((l.nir.clone(), ligne_contrat));
                        "nouveau"
                    }
                }
            },
        };
        infos.insert("contrat".into(), json!(etat_contrat));
        if let Ok(c) = &l.compteurs {
            if !c.avertissements.is_empty() {
                infos.insert("compteurs".into(), json!(c.avertissements));
            }
        }
        let inconnus: Vec<String> = l
            .b
            .avertissements
            .iter()
            .filter_map(|a| a.strip_prefix("libellé inconnu : ").map(String::from))
            .collect();
        let sort = if etat_paie == "deja" && etat_contrat != "nouveau" {
            "deja_repris"
        } else if !inconnus.is_empty() {
            phrases.push(format!("ligne(s) sans rubrique du catalogue, gardée(s) telle(s) quelle(s) : {}", inconnus.join(", ")));
            "libelle_inconnu"
        } else {
            "repris"
        };
        if sort != "deja_repris" {
            pages_reprises += 1;
            if !l.incoherences.is_empty() {
                pages_incoherentes += 1;
            }
        }
        if !l.incoherences.is_empty() {
            phrases.push(format!("le bulletin fait foi, ses montants imprimés sont repris malgré : {}", l.incoherences.join(" ; ")));
            infos.insert("avertissements".into(), json!(l.incoherences));
        }
        pages.entry(l.fichier).or_default().push(PageCr { page: l.page, sort, raison: phrases.join(" ; "), infos });
    }

    // 9. Le compte rendu, fichier par fichier.
    let mut fichiers: Vec<Value> = Vec::new();
    let mut copies: BTreeMap<usize, String> = BTreeMap::new();
    let mut comptes: BTreeMap<&str, usize> = SORTS.iter().map(|s| (*s, 0)).collect();
    let mut comptes_pages: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, f) in tri.fichiers.iter().enumerate() {
        let mut m = Map::new();
        m.insert("chemin".into(), json!(f.chemin));
        m.insert("sorte".into(), json!(f.sorte.nom()));
        let tri_raison = f.raison.clone().unwrap_or_default();
        let (sort, raison): (&str, String) = match f.sorte {
            Sorte::Dsn | Sorte::Aem => sorts_export.get(&i).cloned().unwrap_or(("repris", String::new())),
            Sorte::PdfSansTexte => ("sans_texte", tri_raison),
            Sorte::Archive => ("archive", tri_raison),
            Sorte::Doublon => ("doublon", tri_raison),
            Sorte::Illisible | Sorte::Autre => ("non_reconnu", tri_raison),
            Sorte::PdfTexte => match illisibles.get(&i) {
                Some(m) => ("non_reconnu", m.clone()),
                None => {
                    let mut ps = pages.remove(&i).unwrap_or_default();
                    ps.sort_by_key(|p| p.page);
                    for p in &ps {
                        *comptes_pages.entry(p.sort).or_default() += 1;
                    }
                    let sort = GRAVITE.iter().find(|g| ps.iter().any(|p| p.sort == **g)).copied().unwrap_or("non_reconnu");
                    let raison = match ps.as_slice() {
                        [] => "PDF sans page".to_string(),
                        [p] => p.raison.clone(),
                        _ => {
                            let mut par: Vec<(&str, usize)> = Vec::new();
                            for g in GRAVITE {
                                let n = ps.iter().filter(|p| p.sort == g).count();
                                if n > 0 {
                                    par.push((g, n));
                                }
                            }
                            format!(
                                "{} pages : {}",
                                ps.len(),
                                par.iter().map(|(g, n)| format!("{} {}", n, g.replace('_', " "))).collect::<Vec<_>>().join(", ")
                            )
                        }
                    };
                    m.insert("pages".into(), Value::Array(ps.iter().map(PageCr::json).collect()));
                    (sort, raison)
                }
            },
        };
        m.insert("sort".into(), json!(sort));
        m.insert("raison".into(), json!(raison));
        // Ce que l'import copie dans `historique/bulletins/` : une DSN ou une AEM lue, un PDF dont une page au moins est
        // reprise (ou l'a deja ete).
        let repris = match f.sorte {
            Sorte::Dsn | Sorte::Aem => matches!(sort, "repris" | "deja_repris"),
            Sorte::PdfTexte => m.get("pages").and_then(Value::as_array).is_some_and(|ps| {
                ps.iter().any(|p| matches!(p.get("sort").and_then(Value::as_str), Some("repris" | "libelle_inconnu" | "deja_repris")))
            }),
            _ => false,
        };
        if let Some(c) = nom_copie(f).filter(|_| repris) {
            m.insert("copie".into(), json!(c));
            copies.insert(i, c);
        }
        if let Some(d) = &f.doublon_de {
            m.insert("doublon_de".into(), json!(d));
        }
        if let Some(a) = &f.archive {
            m.insert("archive".into(), json!(a));
        }
        if let Some(x) = &f.mois {
            m.insert("mois".into(), json!(x));
        }
        *comptes.entry(sort).or_default() += 1;
        fichiers.push(Value::Object(m));
    }
    let non_repris_n = fichiers.iter().filter(|f| f.get("sort").and_then(Value::as_str).is_some_and(non_repris)).count();
    let ordre = |c: &BTreeMap<&str, usize>| -> Map<String, Value> {
        SORTS.iter().filter_map(|s| c.get(s).map(|n| (s.to_string(), json!(n)))).collect()
    };
    reponse.insert(
        "tri".into(),
        json!({
            "dossier": tri.dossier.display().to_string(),
            "fichiers": fichiers,
            "comptes": ordre(&comptes),
            "comptes_pages": ordre(&comptes_pages),
            "total": tri.fichiers.len(),
        }),
    );
    let lus_n: usize = par_logiciel.values().sum();
    reponse.insert(
        "bulletins".into(),
        json!({
            "pages": n_pages,
            "lus": lus_n,
            "repris": pages_reprises,
            "par_logiciel": par_logiciel,
            "du": du.map(|d| d.iso()),
            "au": au.map(|d| d.iso()),
            "fiches_a_creer": nouvelles_fiches,
            "contrats_nouveaux": contrats_liste.len(),
            "contrats": contrats_liste,
            "paies_nouvelles": paies_nouvelles,
            "paies_completees": paies_completees,
            "paies": paies_liste,
        }),
    );
    if non_repris_n > 0 {
        if let Some(Value::Array(a)) = reponse.get_mut("avertissements") {
            a.push(json!(format!(
                "{} fichier(s) du dossier ne sont pas repris : le compte rendu (`tri`) dit pourquoi, fichier par fichier.",
                non_repris_n
            )));
        }
    }
    if pages_incoherentes > 0 {
        if let Some(Value::Array(a)) = reponse.get_mut("avertissements") {
            a.push(json!(format!(
                "{} bulletin(s) ne retombent pas sur leurs propres totaux (brut, net) : le bulletin fait foi, ses montants imprimés sont repris tels quels ; le compte rendu dit l'écart, page par page.",
                pages_incoherentes
            )));
        }
    }
    let rien = reponse.get("rien_a_faire").is_some_and(|v| v == &json!(true));
    reponse.insert(
        "rien_a_faire".into(),
        json!(
            rien && nouvelles_fiches == 0
                && ap.a_completer.is_empty()
                && ap.engagements_bulletins.is_empty()
                && paies_nouvelles == 0
                && paies_completees == 0
        ),
    );
    ap.reponse = reponse;
    Ok(DossierLu { ap, tri, copies, salaries_lus, entreprises_lues })
}

/// Ce que la page dit d'elle-meme dans le compte rendu.
fn infos_de(l: &Lu, salarie: Option<&str>) -> Map<String, Value> {
    let mut infos = Map::new();
    infos.insert("logiciel".into(), json!(l.b.logiciel));
    infos.insert("numero".into(), json!(l.b.numero));
    if let Some(s) = salarie {
        infos.insert("salarie".into(), json!(s));
    }
    infos.insert("debut".into(), json!(l.b.debut.iso()));
    infos.insert("fin".into(), json!(l.b.fin.iso()));
    infos.insert("mois".into(), json!(l.mois));
    infos
}

/// Une fiche que l'import cree des DSN (`a_creer` ; un salarie par son NIR, ou l'entreprise), completee de ce que les
/// bulletins impriment et qu'elle n'a pas ; ses anomalies se refont, et la reponse montre la fiche completee.
fn completer_a_creer(a_creer: &mut [ACreer], reponse: &mut Map<String, Value>, nom: &str, nir: Option<&str>, proposees: &Map<String, Value>) -> R<()> {
    let Some(a) = a_creer.iter_mut().find(|a| a.nom == nom && nir.is_none_or(|n| a.donnees.get("nir").map(valeur_en_texte).as_deref() == Some(n))) else {
        return Ok(());
    };
    let champs = completer::champs_a_completer(nom, &Value::Object(a.donnees.clone()), proposees)?;
    if champs.is_empty() {
        return Ok(());
    }
    a.donnees.extend(champs);
    let anomalies = crate::routes::fiches::verifier_comme_a_l_ecriture(nom, &a.donnees)?;
    let entree = match (nom, a.rang) {
        ("entreprise", _) => reponse.get_mut("entreprise"),
        (_, Some(i)) => reponse.get_mut("salaries").and_then(|l| l.get_mut(i)),
        _ => None,
    };
    if let Some(e) = entree {
        e["donnees"] = ri::brut(&Value::Object(a.donnees.clone()));
        e["anomalies"] = ri::brut(&Value::Array(anomalies));
    }
    Ok(())
}

/// Note dans l'entree d'un salarie ce que ses bulletins completent, chaque champ avec le bulletin le plus recent qui
/// l'imprime (`identite`) ; l'adresse, le statut, l'anciennete et la cle du NIR, avec le plus recent.
fn noter_par_champ(entree: &mut Value, champs: &Map<String, Value>, ls: &[&Lu], chemin: &dyn Fn(&Lu) -> String) {
    let Some(dernier) = ls.last() else { return };
    let par: HashMap<&str, &Lu> = identite(ls).into_iter().map(|(cle, _, l)| (cle, l)).collect();
    for (k, v) in champs {
        let base = k.strip_suffix("_affichage").unwrap_or(k);
        let l = par.get(base).copied().unwrap_or(dernier);
        let mut un = Map::new();
        un.insert(k.clone(), v.clone());
        completer::noter(entree, &un, json!({"source": "bulletin", "fichier": chemin(l), "page": l.page}));
    }
}

/// Les ecarts d'une entree de la reponse, completes de ceux du bulletin (une cle deja notee par la DSN reste la sienne).
fn fusionner_ecarts(entree: &mut Value, ecarts: Value) {
    let Value::Object(nouveaux) = ecarts else { return };
    if nouveaux.is_empty() {
        return;
    }
    if !entree.get("ecarts").is_some_and(Value::is_object) {
        entree["ecarts"] = json!({});
    }
    let Some(Value::Object(e)) = entree.get_mut("ecarts") else { return };
    for (k, v) in nouveaux {
        e.entry(k).or_insert(v);
    }
}

/// Ce que les bulletins d'un salarie impriment de lui, champ par champ : le plus recent qui l'imprime (`ls` dans
/// l'ordre du temps). (cle de la fiche, valeur, bulletin d'ou elle vient.)
fn identite<'a>(ls: &[&'a Lu]) -> Vec<(&'static str, String, &'a Lu)> {
    let mut out: Vec<(&'static str, String, &'a Lu)> = Vec::new();
    for l in ls {
        let s = &l.b.salarie;
        for (cle, v) in [
            ("nom", s.nom.clone()),
            ("prenom", s.prenom.clone()),
            ("date_naissance", s.naissance.map(|d| d.iso())),
            ("num_conges_spectacles", s.conges_spectacles.clone()),
            ("emploi", s.emploi.clone()),
            ("code_salarie", s.code.clone()),
        ] {
            let Some(v) = v.map(|x| x.trim().to_string()).filter(|x| !x.is_empty()) else { continue };
            out.retain(|x| x.0 != cle);
            out.push((cle, v, l));
        }
    }
    out
}

/// Les champs d'identite que les bulletins disent autrement que la fiche (compares sans casse ni accents). L'emploi
/// change d'un contrat a l'autre, le code salarie d'un logiciel a l'autre : ils ne sont pas compares.
fn ecarts_salarie(fiche: &Value, ls: &[&Lu], chemin: &dyn Fn(&Lu) -> String) -> Value {
    let mut out = Map::new();
    for (cle, lu, l) in identite(ls) {
        if !matches!(cle, "nom" | "prenom" | "date_naissance" | "num_conges_spectacles") {
            continue;
        }
        let a = fiche.get(cle).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
        // Une case vide ou « A COMPLETER » n'est pas un ecart : l'import la complete.
        if completer::vide(fiche.get(cle)) || compare(&a) == compare(&lu) {
            continue;
        }
        out.insert(cle.into(), json!({"fiche": a, "bulletin": lu, "fichier": chemin(l), "page": l.page}));
    }
    Value::Object(out)
}

/// Ce que le bulletin dit de l'entreprise autrement que la fiche (compare sans casse ni accents) : le Siret, le NAF, et
/// (lot I2, 3g) la raison sociale, l'adresse, l'IDCC et le compte Urssaf -- ce que `fiche_entreprise` en lit.
fn ecarts_entreprise(fiche: &Value, lus: &[Lu], l: &Lu, chemin: &str) -> Value {
    let mut out = Map::new();
    let t = |k: &str| fiche.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    let siret_fiche = if t("siret").is_empty() { format!("{}{}", t("siren"), t("nic")) } else { t("siret") };
    let lue = fiche_entreprise(lus, l).unwrap_or_default();
    let de = |k: &str| lue.get(k).filter(|v| vrai(v)).map(valeur_en_texte);
    for (cle, a, lu) in [
        ("siret", siret_fiche, siret_lu(&l.b)),
        ("naf", t("naf"), l.b.entreprise.naf.clone()),
        ("raison_sociale", t("raison_sociale"), de("raison_sociale")),
        ("adresse", t("adresse"), de("adresse")),
        ("code_postal", t("code_postal"), de("code_postal")),
        ("commune", t("commune"), de("commune")),
        ("idcc", t("idcc"), de("idcc")),
        ("urssaf_compte", t("urssaf_compte"), de("urssaf_compte")),
    ] {
        let Some(lu) = lu else { continue };
        if completer::vide(Some(&json!(a))) || compare(&a) == compare(&lu) {
            continue;
        }
        out.insert(cle.into(), json!({"fiche": a, "bulletin": lu, "fichier": chemin, "page": l.page}));
    }
    Value::Object(out)
}

/// CE QUE LE BULLETIN IMPRIME EN MINUSCULES EST DEJA LA FORME LISIBLE (#236) : « La Fanfare des Toits », « 3 quai de
/// la Fosse » vont tels quels dans la case qu'on lit (`<cle>_affichage`), la forme declaree s'en derive a l'ecriture.
/// Un texte tout en capitales (CoolPaie imprime ainsi noms et adresses) n'en dit pas la casse : sa forme lisible se
/// fait comme ailleurs (`champs::combler_affichage`). Sans cela, « La Fanfare des Toits » devenait « La Fanfare Des
/// Toits ».
fn lisibles_de_la_piece(nom: &str, f: &mut Map<String, Value>) -> R<()> {
    for c in declarations::champs::fiche(nom)? {
        let Some(source) = c.derive_de.as_deref().filter(|d| !d.is_empty()) else { continue };
        if f.get(source).is_some_and(vrai) {
            continue;
        }
        let Some(Value::String(v)) = f.get(&c.cle).cloned() else { continue };
        if v.chars().any(char::is_lowercase) {
            f.insert(source.to_string(), json!(v));
        }
    }
    Ok(())
}

/// La fiche d'un salarie qu'aucune fiche ni aucune DSN ne porte : ce que ses bulletins impriment, champ par champ le
/// plus recent (`ls` dans l'ordre du temps).
fn fiche_salarie(nir: &str, ls: &[&Lu]) -> R<Map<String, Value>> {
    let mut f = Map::new();
    for (cle, v, _) in identite(ls) {
        f.insert(cle.into(), json!(v));
    }
    if let Some((rue, cp, commune)) = ls.iter().rev().find_map(|l| adresse_lue(&l.b.salarie.adresse)) {
        if !rue.is_empty() {
            f.insert("adresse".into(), json!(rue));
        }
        f.insert("code_postal".into(), json!(cp));
        f.insert("commune".into(), json!(commune));
    }
    f.insert("nir".into(), json!(nir));
    let cle = ls.iter().rev().find_map(|l| l.cle_nir.clone()).or_else(|| declarations::champs::cle_nir(nir)).unwrap_or_default();
    f.insert("nir_cle".into(), json!(cle));
    // CE QUE LE STATUT IMPRIME DIT, EN TOUTES LETTRES (#236), et rien de plus : « Artiste » ou « Technicien » font
    // la case « Artiste ou technicien » ; « Cadre » ou « Non cadre », la case « Cadre ou non cadre » (S21.G00.40.003 :
    // 01 cadre, 04 non cadre, cahier technique 2026.1 p. 183). « Non cadre » ne dit pas artiste ou technicien : cette
    // case-la reste a remplir, et l'apercu le dit.
    match ls.iter().rev().find_map(|l| l.b.salarie.statut.as_deref()).map(compare).as_deref() {
        Some(s @ ("artiste" | "technicien")) => {
            f.insert("statut".into(), json!(s));
        }
        Some("cadre") => {
            f.insert("statut_categoriel_rc".into(), json!("01"));
        }
        Some("noncadre") => {
            f.insert("statut_categoriel_rc".into(), json!("04"));
        }
        _ => {}
    }
    // L'EMPLOI IMPRIME, QUAND IL NOMME UN METIER DU CATALOGUE ET UN SEUL (#236) : « MUSICIEN » est le metier MUS010 ;
    // la fiche prend alors ce que le choix de ce metier donne dans la fiche (`emplois::resoudre` : le code PCS-ESE, le
    // code France Travail, le statut conventionnel), comme si on l'avait choisi dans la liste. Le statut et la qualite
    // de cadre imprimes priment sur ceux du catalogue. Un intitule qui ne nomme aucun metier, ou plusieurs, reste tel
    // quel : le metier se choisit dans la fiche apres l'import, et l'apercu le dit.
    let intitule = f.get("emploi").map(valeur_en_texte).unwrap_or_default();
    if let Some((code, feminin)) = droit::emplois::par_intitule(&intitule, compare).map_err(|m| erreur("ErreurOperation", m))? {
        let cadre = f.get("statut_categoriel_rc").map(valeur_en_texte).map(|s| s == "01");
        let r = droit::emplois::resoudre(&code, cadre, feminin).map_err(|m| erreur("ErreurOperation", m))?;
        for (k, v) in r.as_object().into_iter().flatten() {
            if v.is_null() || (k == "statut" && f.contains_key("statut")) {
                continue;
            }
            f.insert(k.clone(), v.clone());
        }
    }
    if let Some(premier) = ls.iter().map(|l| l.b.debut).min() {
        f.insert("date_anciennete".into(), json!(premier.iso()));
    }
    lisibles_de_la_piece("salarie", &mut f)?;
    let mut f = declarations::champs::combler_affichage("salarie", &f)?;
    f.insert("actif".into(), json!(true));
    Ok(f)
}

/// La fiche de l'entreprise, des bulletins : le plus recent pour l'identite, la suite des licences comme les DSN.
fn fiche_entreprise(lus: &[Lu], l: &Lu) -> R<Map<String, Value>> {
    let e = &l.b.entreprise;
    let mut f = Map::new();
    if let Some(n) = e.nom.as_deref().map(str::trim).filter(|x| !x.is_empty()) {
        f.insert("raison_sociale".into(), json!(n));
    }
    if let Some(s) = siret_lu(&l.b) {
        f.insert("siren".into(), json!(&s[..9]));
        f.insert("nic".into(), json!(&s[9..]));
        f.insert("siret".into(), json!(s));
    }
    if let Some(n) = e.naf.as_deref().filter(|x| !x.is_empty()) {
        f.insert("naf".into(), json!(n));
    }
    if let Some((rue, cp, commune)) = adresse_lue(&e.adresse) {
        if !rue.is_empty() {
            f.insert("adresse".into(), json!(rue));
        }
        f.insert("code_postal".into(), json!(cp));
        f.insert("commune".into(), json!(commune));
    }
    if let Some(i) = e.convention.as_deref().and_then(idcc_lu) {
        f.insert("idcc".into(), json!(i));
    }
    if let Some(u) = urssaf_lu(&l.b) {
        f.insert("urssaf_compte".into(), json!(u));
    }
    let mut licences: Vec<(String, String)> = Vec::new();
    for x in lus {
        if let Some(n) = x.b.entreprise.licence.as_deref().filter(|n| !n.is_empty()) {
            if licences.last().is_none_or(|p| p.1 != n) {
                licences.push((x.mois.clone(), n.to_string()));
            }
        }
    }
    if !licences.is_empty() {
        let l: Vec<Value> = licences.iter().map(|(m, n)| json!({"depuis": format!("{}-01", m), "numero": n})).collect();
        f.insert("licence_spectacle".into(), Value::Array(l));
    }
    lisibles_de_la_piece("entreprise", &mut f)?;
    let mut f = declarations::champs::combler_affichage("entreprise", &f)?;
    if f.get("raison_sociale").is_some_and(vrai) && !f.get("nom_affichage").is_some_and(vrai) {
        let t = ri::titre_de(f.get("raison_sociale"));
        f.insert("nom_affichage".into(), json!(t));
    }
    Ok(f)
}

/// Complete une ligne de paie par le bulletin. Sur le brut, le net imposable, le net a payer et le PAS, le bulletin
/// l'emporte ; ailleurs il remplit ce qui est vide et note ce qui differe. Rend les ecarts : (champ, valeur d'avant,
/// le bulletin l'emporte, valeur du bulletin).
fn completer(ligne: &mut Map<String, Value>, b: &BulletinLu, chemin: &str, page: usize) -> Vec<(&'static str, String, bool, String)> {
    let mut ecarts = Vec::new();
    let pas = b.pas.as_ref().and_then(|p| p.montant);
    for (champ, v) in [("brut", Some(b.brut)), ("net_imposable", b.net_imposable), ("net_a_payer", Some(b.net_a_payer)), ("pas", pas)] {
        let Some(v) = v else { continue };
        let avant = ligne.get(champ).map(valeur_en_texte).unwrap_or_default();
        let vide = avant.trim().is_empty();
        let differe = !vide && socle::num::decimal_py(avant.trim()).is_none_or(|a| a != v);
        if differe {
            ecarts.push((champ, avant, true, v.to_string()));
        }
        if vide || differe {
            ligne.insert(champ.into(), json!(v.to_string()));
        }
    }
    let objet = b.numero_objet.clone().unwrap_or_default();
    let avant = ligne.get("numero_objet").map(valeur_en_texte).unwrap_or_default();
    if avant.trim().is_empty() {
        if !objet.is_empty() {
            ligne.insert("numero_objet".into(), json!(objet));
        }
    } else if !objet.is_empty() && avant.to_uppercase() != objet.to_uppercase() {
        ecarts.push(("numero_objet", avant, false, objet));
    }
    if ligne.get("verse_le").map(valeur_en_texte).unwrap_or_default().trim().is_empty() {
        if let Some(r) = b.regle_le {
            ligne.insert("verse_le".into(), json!(r.iso()));
        }
    }
    let v = b.to_json();
    let mut poser = |k: &str, x: Value| {
        if !x.is_null() {
            ligne.insert(k.into(), x);
        }
    };
    poser("spectacle", json!(b.spectacle));
    poser("analytique", json!(b.analytique));
    poser("cotisations_salariales", dec(&b.total_salarie));
    poser("cotisations_patronales", dec(&b.total_employeur));
    poser("cout_employeur", dec(&b.cout_employeur));
    poser("net_social", dec(&b.net_social));
    poser("net_avant_impot", dec(&b.net_avant_impot));
    poser("lignes", v["lignes"].clone());
    poser("dates", v["dates_travaillees"].clone());
    poser("cotisations", v["cotisations"].clone());
    poser("bulletin", json!({"fichier": chemin, "page": page, "logiciel": b.logiciel, "numero": b.numero, "contrat": b.contrat}));
    ecarts
}

/// Le contrat qu'un bulletin atteste, tel qu'un engagement importe le porte (`importe.contrat`), et sa cle `_cle_engagement`.
/// Les heures se comptent en heures entieres comme celles d'une saisie (`cumul::engagement_de_paie` : arrondi au
/// pair) ; des cachets ou des jours qui ne sont pas entiers, ou un compteur inconnu, ne font pas de contrat.
fn engagement(l: &Lu, c: &Compteurs, salarie: &str, chemin: &str) -> Result<(CleEngagement, Value), String> {
    let (Some(cachets), Some(heures), Some(jours)) = (c.cachets, c.heures, c.jours) else {
        let manque: Vec<&str> =
            [("cachets", c.cachets), ("heures", c.heures), ("jours", c.jours)].iter().filter(|x| x.1.is_none()).map(|x| x.0).collect();
        return Err(format!("{} inconnu(s), on ne les devine pas", manque.join(", ")));
    };
    let entier = |x: Decimal, quoi: &str| -> Result<i64, String> {
        if !x.fract().is_zero() {
            return Err(format!("{} non entiers ({})", quoi, x.normalize()));
        }
        x.to_i64().ok_or_else(|| format!("{} hors limites", quoi))
    };
    let (cachets, jours) = (entier(cachets, "cachets")?, entier(jours, "jours")?);
    let heures = heures.round_dp_with_strategy(0, RoundingStrategy::MidpointNearestEven).to_i64().ok_or("heures hors limites")?;
    let enseignement = c
        .heures_enseignement
        .map(|h| h.round_dp_with_strategy(0, RoundingStrategy::MidpointNearestEven).to_i64().unwrap_or(0))
        .unwrap_or(0);
    let mut brut = l.b.brut;
    brut.rescale(2);
    let objet = l.b.numero_objet.clone().unwrap_or_default();
    let (debut, fin) = (l.b.debut.iso(), l.b.fin.iso());
    let cle: CleEngagement =
        (salarie.to_string(), debut.clone(), fin.clone(), objet.clone(), heures as i128, cachets as i128, jours as i128, brut.to_string());
    let mut m = Map::new();
    m.insert("salarie".into(), json!(salarie));
    m.insert("debut".into(), json!(debut));
    m.insert("fin".into(), json!(fin));
    m.insert("numero_objet".into(), json!(objet));
    m.insert("heures".into(), json!(heures));
    m.insert("cachets".into(), json!(cachets));
    m.insert("jours".into(), json!(jours));
    m.insert("brut".into(), json!(brut.to_string()));
    if enseignement > 0 {
        m.insert("heures_enseignement".into(), json!(enseignement));
    }
    m.insert("atteste_par".into(), json!("bulletin"));
    // LA NATURE DU CONTRAT : aucun des bulletins lus ne l'imprime en mention (`bulletins-lecture-base.json`), et rien sur la
    // page ne la fixe a coup sur. Deux marques disent un engagement du spectacle : la cotisation Congés Spectacles
    // (due pour l'artiste ou le technicien qui n'est pas employe douze mois de suite, D. 7121-41 du code du travail,
    // `atelier/reference/sources/caisse-conges-affiliation/LISEZ-MOI.md`) et le numero d'objet (interdit hors des
    // natures 02 et 10, CCH-11 : regle 6). Avec l'une d'elles, le contrat reste muet : un engagement muet est un
    // CDDU (regle 6), comme partout ailleurs. Sans aucune, c'est le contrat d'un permanent -- CDI ou CDD de droit
    // commun, que le bulletin ne dit pas et qu'on ne devine pas : il est marque « a dire », se demande apres l'import
    // (`importation_nature`), et ne se lit pas en CDDU en attendant.
    if !marque_d_intermittent(&l.b) {
        m.insert(crate::importation_nature::A_DIRE.into(), json!(true));
    }
    if let Some(n) = &l.b.numero {
        m.insert("numero_bulletin".into(), json!(n));
    }
    // Le numero du contrat, quand le bulletin l'imprime : il se lit, il ne se tire pas (regle 4).
    if let Some(n) = &l.b.contrat {
        m.insert("numero_contrat".into(), json!(n));
    }
    m.insert("fichier".into(), json!(chemin));
    m.insert("page".into(), json!(l.page));
    Ok((cle, Value::Object(m)))
}

/// Le bulletin porte-t-il une marque d'engagement du spectacle : un numero d'objet, ou une cotisation Congés
/// Spectacles (sous sa famille, ou une ligne qui la nomme : CoolPaie la range sous les autres contributions) ?
fn marque_d_intermittent(b: &BulletinLu) -> bool {
    b.numero_objet.as_deref().is_some_and(|o| !o.trim().is_empty())
        || b.cotisations.iter().any(|c| {
            (c.famille.as_deref() == Some("conges_spectacles") || compare(&c.libelle).contains("congesspectacles"))
                && c.montant_employeur.is_some_and(|m| !m.is_zero())
        })
}

/// Le nom d'un champ de paie dans une phrase du compte rendu.
fn libelle_champ(champ: &str) -> &str {
    match champ {
        "net_imposable" => "net imposable",
        "net_a_payer" => "net à payer",
        "pas" => "prélèvement à la source",
        "numero_objet" => "numéro d'objet",
        autre => autre,
    }
}

/// La cle d'un contrat, son numero d'objet efface.
fn sans_l_objet(k: &CleEngagement) -> CleEngagement {
    let mut k = k.clone();
    k.3 = String::new();
    k
}
