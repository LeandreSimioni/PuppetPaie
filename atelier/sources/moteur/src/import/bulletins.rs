//! `importation-apercu` sur la boite aux lettres `import/`.
//! RIEN N'EST ECRIT.
//!
//! 1. Le tri (`import::tri`) : chaque fichier recoit sa sorte, par le contenu.
//! 2. Les DSN et AEM trouvees, ou qu'elles soient (sous-dossier, zip), sont copiees dans un dossier de travail
//!    (temporaire, efface avant de rendre) et passent par `crate::routes::import::apercu_lu`, qui lit leurs
//!    sous-dossiers `DSN/` et `AEM_EDI/` -- celles d'une autre entreprise exceptees : devant le Siren de
//!    la compagnie (`importation::siren_de_reference`, etape 3), elles prennent le sort `incoherent` et rien n'en est lu.
//! 3. Les bulletins (`bulletin_lu`), page par page, completent cet apercu : les fiches qu'ils creent (salaries par NIR,
//!    spectacles par numero d'objet avec leur titre, l'entreprise) et leurs ecarts avec les fiches existantes (signales,
//!    jamais appliques) ; un contrat atteste par bulletin (`atteste_par: "bulletin"`), que le registre de l'import retient s'il
//!    n'est pas deja atteste (`import::attestes`) ; la paie, rapprochee des paies versees des engagements importes et des
//!    DSN par (NIR, debut, fin).
//!
//! LE BULLETIN FAIT FOI : une paie deja declaree est COMPLETEE par le bulletin (cotisations par
//! part, cout employeur, lignes, dates, titre...) et, sur le brut, le net imposable, le net a payer et le PAS, c'est
//! le montant du bulletin qui s'ecrit ; l'ecart est note dans le compte rendu. Une paie qu'aucune DSN ne declare est
//! nouvelle : `contrat` = « bulletin <numero> » (CoolPaie), sinon le nom du fichier (et la page, s'il en a plusieurs),
//! `declare_par: "bulletin"`, `mois` = le mois de la fin de periode du bulletin.
//!
//! CE QUI EST SAISI ICI N'EST PAS REPRIS : un bulletin d'un salarie qu'un engagement saisi (non importe) paie dans le
//! mois du bulletin est « deja saisi », rien d'autre (ni fiche, ni contrat, ni paie) -- sauf s'il a une paie declaree a
//! completer (une DSN de l'apercu ou une paie importee, meme salarie, meme mois) : il la complete, quel que soit
//! l'ordre de la saisie et de l'import.
//!
//! RIEN NE SE DEVINE (regle 1). Une page n'est pas reprise quand : elle n'a pas de texte, le rang d'en-tetes de son
//! tableau ne se trouve pas, elle se lit de deux facons, ce qui l'identifie ne se lit pas (le lecteur), son NIR ne se lit pas ou contredit sa cle, son
//! Siret n'est pas celui de la compagnie (« bulletin d'une autre entreprise »), plusieurs paies declarees pourraient
//! etre la sienne, ou un autre bulletin donne d'autres montants pour la meme paie. LE BULLETIN FAIT FOI : une
//! page qui se contredit elle-meme (`incohérent : ...` du lecteur : le brut n'est pas la somme des lignes, le net ne
//! se retrouve pas -- un centime sur le net avant impot chez CoolPaie) est REPRISE avec les montants qu'elle imprime,
//! et l'incoherence est un avertissement du compte rendu. Un libelle de ligne inconnu de la table n'empeche pas la
//! reprise : la ligne est gardee telle quelle, sans compteur, et le sort le dit.
//!
//! LES NOMS DE FICHIER NE PORTENT RIEN DE PERSONNEL. Ce que l'import ecrit nomme chaque fichier par sa COPIE dans
//! `historique/bulletins/` : `<16 premiers caracteres de l'empreinte SHA-256>.<pdf|dsn|txt>` (`nom_copie`) -- le
//! `fichier` d'une paie ou d'un contrat, le `bulletin.fichier`, le `contrat` d'une paie sans numero de bulletin. Le
//! meme nom dans l'apercu (lu dans `import/`) et dans l'import (relu dans la copie) : un import se rejoue, un apercu
//! d'apres l'import ne voit rien de neuf. Le compte rendu (`tri`) garde le chemin d'origine, pour l'ecran, et dit la
//! `copie` de chaque fichier repris.
//!
//! LE COMPTE RENDU (`tri`) donne a chaque fichier un sort, et un seul, avec sa raison en une phrase (sans nom ni NIR :
//! la phrase peut partir chez l'editeur) ; un PDF a aussi le sort de chacune de ses pages. Les sorts : `reponse::Sort`.

use droit::fiche::Lue as _;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::RoundingStrategy;
use serde_json::{json, Map, Value};

use crate::import::bulletin_lu::{self, BulletinLu, Compteurs, Table};
use droit::contexte::Contexte;
use declarations::champs::ParCase;
use socle::date::Jour;
use documents::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;
use documents::valeur::vrai;
use crate::import::completer as completer;
use crate::import::tri::Sorte;
use socle::num::Decimal;
use socle::texte::euros;
use crate::import::attestes::{Atteste, Registre};
use engagement::importe::{Attestation, ContratAtteste, ContratLu, Declaration, PaieLue, PaieVersee};
use crate::routes::import::{self as ri, ACreer, Apercu, CleEngagement};
use crate::import::reponse::{
    BlocBulletins, BlocPieces, DitPar, Ecart, EcartPaie, EtatContrat, EtatFiche, EtatPaie, FicheImportee, FichierVu, Importation, InfosPage, NonRepris, NonReprisVu,
    Origine, PageVu, ParSort, Sort, Source, TriVu,
};

/// Ce que l'encart dit d'un import qui laisse des documents : il n'est pas bloquant, et se rejoue.
pub const REESSAYER: &str = "Le logiciel s'améliore sans cesse : réessayez ces documents plus tard ; ce qui sera repris s'ajoutera à votre historique.";

const A_CREER: &str = "<a creer>";


/// Le compte rendu d'une page de PDF.
struct PageCr {
    page: usize,
    sort: Sort,
    raison: String,
    infos: InfosPage,
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
    /// Le mois a des engagements saisis (`mois_saisis`).
    mois_saisi: bool,
    compteurs: Result<Compteurs, String>,
    /// Ce que la page dit d'elle-meme qui ne se retrouve pas (`incohérent : ...`) : un avertissement, le bulletin fait foi.
    incoherences: Vec<String>,
}

/// Ce que la lecture d'un dossier rend : l'apercu, le tri (les fichiers sur le disque, a lire avant d'effacer le dossier
/// de travail), et le nom de la copie de chaque fichier repris (par rang dans le tri).
pub(crate) struct DossierLu {
    pub(crate) ap: Apercu,
    pub(crate) tri: crate::import::tri::Tri,
    pub(crate) copies: BTreeMap<usize, String>,
}

/// `importation-apercu` avec `{"dossier": ...}`.
pub(crate) fn apercu_dossier(ctx: &Contexte, dossier: &Path) -> R<Importation> {
    let mut r = lire_dossier(ctx, dossier, &HashMap::new(), |lu| Ok(lu.ap))?.reponse;
    r.dire_l_apercu();
    Ok(r)
}

/// Lit le dossier, et passe ce qui a ete lu a `suite` AVANT d'effacer le dossier de travail (les zip ouverts) : la
/// copie de l'import et le texte des pages refusees s'y lisent encore. `deposes` : le nom que la personne a donne a un
/// fichier du dossier, par son chemin dans le dossier, quand le dossier lu est fait de copies (`import::ecrite`) ; tout
/// ce que l'import dit d'un fichier (« lu dans… », le compte rendu) le nomme ainsi.
pub(crate) fn lire_dossier<T>(ctx: &Contexte, dossier: &Path, deposes: &HashMap<String, String>, suite: impl FnOnce(DossierLu) -> R<T>) -> R<T> {
    let travail = ri::dossier_de_travail();
    let rendu = trier_et_lire(ctx, dossier, &travail, deposes).and_then(suite);
    let _ = std::fs::remove_dir_all(&travail);
    rendu
}

/// Le nom de la copie d'un fichier dans `historique/bulletins/` : l'empreinte, rien de personnel. `None` pour ce qui ne
/// se reprend pas.
pub(crate) fn nom_copie(f: &crate::import::tri::Fichier) -> Option<String> {
    let ext = match f.sorte {
        Sorte::Dsn => "dsn",
        // `importation::lire_aem` ne lit que les `*.txt`.
        Sorte::Aem => "txt",
        Sorte::PdfTexte => "pdf",
        _ => return None,
    };
    (f.empreinte.len() >= 16).then(|| format!("{}.{}", &f.empreinte[..16], ext))
}

/// Un texte compare sans casse, sans accents, sans espaces ni ponctuation de bord (`bulletin_lu::jetons`).
pub(crate) fn compare(t: &str) -> String {
    bulletin_lu::jetons(t).concat()
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

/// LE COMPTE URSSAF imprime apres « Urssaf : » : ses chiffres, sans espaces. C'est la case que le moteur
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

fn trier_et_lire(ctx: &Contexte, dossier: &Path, travail: &Path, deposes: &HashMap<String, String>) -> R<DossierLu> {
    let mut tri = crate::import::tri::trier(dossier, travail)?;
    for f in &mut tri.fichiers {
        if let Some(nom) = deposes.get(&f.chemin) {
            f.chemin = nom.clone();
        }
    }

    // 1. Les DSN et AEM : ce que chacune dit de son employeur. Elles ne sont copiees la ou l'import existant
    // les lit qu'apres l'etape 3, et seulement celles de la compagnie.
    let pieces: Vec<(usize, bool, crate::import::dsn_aem::Employeur)> = tri
        .fichiers
        .iter()
        .enumerate()
        .filter_map(|(i, f)| match f.sorte {
            Sorte::Dsn => Some(crate::import::dsn_aem::employeur_dsn(&f.disque).map(|e| (i, false, e))),
            Sorte::Aem => Some(Ok((i, true, crate::import::dsn_aem::employeur_aem(&f.disque)))),
            _ => None,
        })
        .collect::<R<_>>()?;

    // 2. Les bulletins, page par page.
    let table = Table::livree().map_err(|m| erreur("ErreurOperation", format!("La table de lecture des bulletins ne se lit pas : {}", m)))?;
    let saisis: HashSet<String> = engagement::stock::mois_saisis(&ctx.racine).unwrap_or_default().into_iter().map(String::from).collect();
    let mut pages: BTreeMap<usize, Vec<PageCr>> = BTreeMap::new();
    let mut illisibles: BTreeMap<usize, String> = BTreeMap::new();
    let mut lus: Vec<Lu> = Vec::new();
    let mut certificats: Vec<Attestee> = Vec::new();
    let mut par_logiciel: BTreeMap<String, usize> = BTreeMap::new();
    let (mut du, mut au): (Option<Jour>, Option<Jour>) = (None, None);
    let mut n_pages = 0usize;
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
        // LES CERTIFICATS D'EMPLOI CONGES SPECTACLES du fichier : ils se lisent a part (`certificat_lu`), pas comme des
        // bulletins.
        let certificats_du_fichier = crate::import::certificat_lu::lire(&f.disque).unwrap_or_default();
        let mut pages_certificats: HashSet<usize> = certificats_du_fichier.iter().map(|c| c.page).collect();
        for pc in certificats_du_fichier {
            match pc.resultat {
                Ok(c) => {
                    certificats.push(Attestee { fichier: i, c, aem: None });
                }
                Err((raison, _)) => {
                    cr.push(PageCr { page: pc.page, sort: Sort::Incomplet, raison, infos: InfosPage::default() });
                }
            }
        }
        // L'AER CINEMA SPECTACLE (`aem_lu::lire_aer`) : un formulaire entier, chaque periode attestee comme une AEM.
        if let Some(colonnes) = std::fs::read(&f.disque).ok().and_then(|o| crate::import::aem_lu::lire_aer(&o)) {
            pages_certificats.extend(1..=f.pages.len());
            for lecture in colonnes {
                match lecture {
                    crate::import::aem_lu::Lecture::Lue(a) => {
                        let c = a.commun();
                        certificats.push(Attestee { fichier: i, c, aem: Some(a) });
                    }
                    crate::import::aem_lu::Lecture::Refusee { sort, raison, partiel } => {
                        cr.push(PageCr { page: partiel.map_or(1, |a| a.page), sort, raison, infos: InfosPage::default() });
                    }
                    crate::import::aem_lu::Lecture::Notice => {}
                }
            }
        }
        // LES AEM EN PDF du fichier (`aem_lu`), et la notice du formulaire.
        for pa in crate::import::aem_lu::lire(&f.disque).unwrap_or_default() {
            if !pages_certificats.insert(pa.page) {
                continue;
            }
            match pa.lecture {
                crate::import::aem_lu::Lecture::Lue(a) => {
                    let c = a.commun();
                    certificats.push(Attestee { fichier: i, c, aem: Some(a) });
                }
                crate::import::aem_lu::Lecture::Refusee { sort, raison, .. } => {
                    cr.push(PageCr { page: pa.page, sort, raison, infos: InfosPage::default() });
                }
                crate::import::aem_lu::Lecture::Notice => {
                    cr.push(PageCr { page: pa.page, sort: Sort::Annexe, raison: "la notice de l'AEM : rien à reprendre".into(), infos: InfosPage::default() });
                }
            }
        }
        for l in lectures {
            if pages_certificats.contains(&l.page) {
                continue;
            }
            n_pages += 1;
            let b = match l.resultat {
                Ok(b) => b,
                Err(r) => {
                    cr.push(PageCr { page: l.page, sort: r.code(), raison: r.raison(), infos: InfosPage::default() });
                    continue;
                }
            };
            *par_logiciel.entry(b.logiciel.clone().unwrap_or_else(|| "autre".into())).or_default() += 1;
            du = Some(du.map_or(b.debut, |d| d.min(b.debut)));
            au = Some(au.map_or(b.fin, |d| d.max(b.fin)));
            let mois = b.fin.mois_civil().to_string();
            let infos = InfosPage {
                logiciel: b.logiciel.clone(),
                numero: b.numero.clone(),
                debut: Some(b.debut.iso()),
                fin: Some(b.fin.iso()),
                mois: Some(mois.clone()),
                ..InfosPage::default()
            };
            let refus = |sort: Sort, raison: String, infos: InfosPage| PageCr { page: l.page, sort, raison, infos };
            let mois_saisi = saisis.contains(&mois);
            // LE BULLETIN FAIT FOI : une page qui se contredit (un centime sur le net) est reprise telle qu'imprimee.
            let incoherences: Vec<String> = b.avertissements.iter().filter(|a| a.starts_with("incohérent")).cloned().collect();
            let (nir, cle_nir) = match nir_lu(&b) {
                Ok(Some(n)) => n,
                Ok(None) => {
                    cr.push(refus(Sort::Incomplet, "le NIR ne se lit pas : la paie ne se rattache à aucune fiche".into(), infos));
                    continue;
                }
                Err(m) => {
                    cr.push(refus(Sort::Incoherent, m, infos));
                    continue;
                }
            };
            // Les compteurs se font plus bas, quand tous les bulletins du salarie sont lus (`compter`).
            lus.push(Lu { fichier: i, page: l.page, b, nir, cle_nir, mois, mois_saisi, compteurs: Ok(Compteurs::default()), incoherences });
        }
    }

    // 3. La compagnie : son Siren (`importation::siren_de_reference`, le meme pour l'export) -- la fiche, sinon les DSN
    // si elles n'en portent qu'un, sinon les bulletins et les AEM s'ils n'en portent qu'un. Une DSN, une AEM ou un
    // bulletin d'une autre entreprise n'est pas repris ; d'une DSN ou d'une AEM ecartee, RIEN n'est lu.
    let siren_fiche = Some(ctx.entreprise.siren.texte().trim().to_string()).filter(|s| !s.is_empty());
    let sirens_dsn: std::collections::BTreeSet<String> =
        pieces.iter().filter(|(_, aem, _)| !aem).flat_map(|(_, _, e)| e.sirens.iter().cloned()).collect();
    let mut sirens_autres: std::collections::BTreeSet<String> =
        pieces.iter().filter(|(_, aem, _)| *aem).flat_map(|(_, _, e)| e.sirens.iter().cloned()).collect();
    sirens_autres.extend(lus.iter().filter_map(|l| siret_lu(&l.b)).map(|s| s[..9].to_string()));
    sirens_autres.extend(certificats.iter().filter_map(|c| c.c.siret.as_ref()).map(|s| s[..9].to_string()));
    let siren = crate::import::dsn_aem::siren_de_reference(&ctx.entreprise, &sirens_dsn, &sirens_autres);
    let mut gardes_cert: Vec<Attestee> = Vec::new();
    for c in certificats {
        let raison = match (&c.c.siret, &siren) {
            (Some(s), Some(r)) if &s[..9] != r => Some(format!("{} d'une autre entreprise : son Siret n'est pas celui de la compagnie, non repris", c.nom())),
            (Some(_), None) => Some(crate::import::dsn_aem::PLUSIEURS_ENTREPRISES.to_string()),
            _ => None,
        };
        match raison {
            Some(raison) => pages.entry(c.fichier).or_default().push(PageCr { page: c.c.page, sort: Sort::Incoherent, raison, infos: InfosPage::default() }),
            None => gardes_cert.push(c),
        }
    }
    let mut certificats = gardes_cert;
    certificats.sort_by(|a, b| (a.c.fin, a.c.debut, a.fichier, a.c.page).cmp(&(b.c.fin, b.c.debut, b.fichier, b.c.page)));
    let mut gardes: Vec<Lu> = Vec::new();
    for l in lus {
        let ecart = match (siret_lu(&l.b), &siren) {
            (Some(s), Some(r)) if &s[..9] != r => {
                Some("bulletin d'une autre entreprise : son Siret n'est pas celui de la compagnie, non repris".to_string())
            }
            (Some(_), None) => Some(crate::import::dsn_aem::PLUSIEURS_ENTREPRISES.to_string()),
            _ => None,
        };
        match ecart {
            Some(raison) => {
                let infos = infos_de(&l, None);
                pages.entry(l.fichier).or_default().push(PageCr { page: l.page, sort: Sort::Incoherent, raison, infos });
            }
            None => gardes.push(l),
        }
    }
    let mut lus = gardes;
    lus.sort_by(|a, b| (a.b.fin, a.b.debut, a.fichier, a.page).cmp(&(b.b.fin, b.b.debut, b.fichier, b.page)));
    compter(&mut lus, &ctx.racine);

    // Les DSN et AEM de la compagnie, copiees la ou `apercu_lu` les lit, sous le nom de leur copie (`nom_copie`).
    // Un nom deja pris recoit un rang devant.
    let export = travail.join("export");
    let mut noms: BTreeMap<usize, String> = BTreeMap::new();
    let (mut dsn_dir, mut aem_dir): (Option<PathBuf>, Option<PathBuf>) = (None, None);
    let mut pris: HashSet<String> = HashSet::new();
    let mut ecartes: BTreeMap<usize, String> = BTreeMap::new();
    let mut avertis: BTreeMap<usize, String> = BTreeMap::new();
    for (i, aem, e) in &pieces {
        let f = &tri.fichiers[*i];
        match crate::import::dsn_aem::verdict_entreprise(*aem, e, siren.as_deref()) {
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
        std::fs::create_dir_all(&d).map_err(|e| erreur("OSError", socle::echec::du_disque(&d, &e)))?;
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
    let a_venir: HashSet<String> = lus.iter().map(|l| l.nir.clone()).chain(certificats.iter().filter_map(|c| c.c.nir.clone())).collect();
    let mut ap = ri::apercu_lu(ctx, &dossier.display().to_string(), dsn_dir.as_deref(), aem_dir.as_deref(), ctx.aujourdhui, &a_venir)?;

    let mut par_nir: HashMap<String, String> = HashMap::new();
    for (cle, s) in &ctx.salaries {
        if let Some(n) = s.nir.dite().filter(|n| !n.is_empty()) {
            par_nir.insert(n.to_string(), cle.to_string());
        }
    }
    let cle_de = |nir: &str| par_nir.get(nir).cloned().unwrap_or_else(|| A_CREER.to_string());
    // LES CONTRATS ATTESTES : l'historique, puis ce que chaque piece de cet import retient (`import::attestes`).
    let nir_de_cle: HashMap<String, String> = par_nir.iter().map(|(n, c)| (c.clone(), n.clone())).collect();
    let mut registre = Registre::de_l_historique(ctx, &nir_de_cle)?;
    let historique_paies = ri::paies_historique(ctx)?;
    let hist_par_cle: HashMap<(String, String, String), PaieLue> = historique_paies.iter().map(|x| (x.cle(), x.clone())).collect();

    // Ce qui est saisi ici : (salarie, mois) que paie un engagement saisi. Un bulletin qui en est, sans paie declaree
    // du meme salarie et du meme mois a completer (DSN de cet apercu, paie importee), est « deja saisi ».
    let mut saisis_ici: HashSet<(String, String)> = HashSet::new();
    if lus.iter().any(|l| l.mois_saisi) {
        let objets = engagement::stock::objets(&ctx.racine).map_err(|m| erreur("ErreurOperation", m))?;
        for o in objets.iter().filter(|o| !o.importe_seul()) {
            let Some(cle) = &o.brut.saisie.carte.salarie else { continue };
            for l in lus.iter().filter(|l| l.mois_saisi) {
                if o.couvre(&l.mois).map_err(|m| erreur("ErreurOperation", m))? {
                    saisis_ici.insert((cle.to_string(), l.mois.clone()));
                }
            }
        }
    }
    let declarees: HashSet<(String, String)> = ap
        .paies
        .iter()
        .chain(historique_paies.iter())
        .map(|x| (x.paie.nir.clone().unwrap_or_default(), x.mois.clone()))
        .collect();
    let mut gardes: Vec<Lu> = Vec::new();
    for l in lus {
        if saisis_ici.contains(&(cle_de(&l.nir), l.mois.clone())) && !declarees.contains(&(l.nir.clone(), l.mois.clone())) {
            let raison = format!("le mois {} de ce salarié est saisi ici, sans paie déclarée à compléter : non repris", l.mois);
            pages.entry(l.fichier).or_default().push(PageCr { page: l.page, sort: Sort::DejaSaisi, raison, infos: infos_de(&l, None) });
            continue;
        }
        gardes.push(l);
    }
    let lus = gardes;
    let nirs_dsn: HashSet<String> = ap
        .a_creer
        .iter()
        .filter(|a| a.nom == "salarie")
        .filter_map(|a| a.donnees.get("nir").map(valeur_en_texte))
        .collect();

    // Le sort des DSN et des AEM, avant que les bulletins ne completent les paies.
    let mut sorts_export: BTreeMap<usize, (Sort, String)> = BTreeMap::new();
    for (i, nom) in &noms {
        let f = &tri.fichiers[*i];
        if f.sorte == Sorte::Dsn {
            let mut n = 0usize;
            let mut nouveaux = 0usize;
            for x in ap.paies.iter().filter(|x| x.paie.fichier.as_deref() == Some(nom.as_str())) {
                n += 1;
                if !hist_par_cle.contains_key(&x.cle()) {
                    nouveaux += 1;
                }
            }
            let mois = f.mois.clone().unwrap_or_default();
            sorts_export.insert(*i, match (n, nouveaux) {
                (0, _) => (Sort::Repris, format!("DSN de {} : aucun versement à reprendre (DSN sans versement, ou remplacée par une DSN du même mois déposée après)", mois)),
                (n, 0) => (Sort::DejaRepris, format!("DSN de {} : ses {} versement(s) sont déjà dans l'historique", mois, n)),
                (n, k) => (Sort::Repris, format!("DSN de {} : {} versement(s), dont {} à reprendre", mois, n, k)),
            });
        } else {
            let (mut n, mut nouveaux, mut sans) = (0usize, 0usize, 0usize);
            for c in ap.contrats.iter().filter(|c| &c.fichier == nom) {
                n += 1;
                if !par_nir.contains_key(&c.nir) && !nirs_dsn.contains(&c.nir) && !a_venir.contains(&c.nir) {
                    sans += 1;
                    continue;
                }
                if registre.pareils(&c.nir, &c.debut.iso(), &c.fin.iso(), "AEM").is_empty() {
                    registre.ajouter(&c.nir, crate::import::dsn_aem::ligne_du_contrat(c, &cle_de(&c.nir)), "AEM", "declarations");
                    nouveaux += 1;
                }
            }
            let sans_txt = if sans > 0 { format!(" ; {} sans fiche (NIR qu'aucune DSN ni aucun bulletin ne déclare)", sans) } else { String::new() };
            // `importation::lire_aem` garde un contrat une fois : un fichier qui ne rend rien a ete lu ailleurs, sauf
            // s'il est dans ses illisibles.
            let illisible =
                ap.reponse.avertissements.iter().any(|x| x.strip_prefix("AEM illisible, ignoree : ").is_some_and(|r| r.split(" : ").next() == Some(nom.as_str())));
            sorts_export.insert(*i, match (n, nouveaux) {
                (0, _) if illisible => (Sort::NonReconnu, "AEM dont les enregistrements ne se lisent pas".to_string()),
                (0, _) => (Sort::DejaRepris, "AEM : ses contrats attestés sont déjà dans un autre fichier du dossier".to_string()),
                (n, 0) if sans == 0 => (Sort::DejaRepris, format!("AEM : ses {} contrat(s) attesté(s) sont déjà dans l'historique", n)),
                (n, k) => (Sort::Repris, format!("AEM : {} contrat(s) attesté(s), dont {} nouveau(x){}", n, k, sans_txt)),
            });
        }
    }
    // Les DSN et AEM d'une autre entreprise : rien n'en a ete lu. Une AEM dont le Siret ne se lit pas passe, et le dit.
    for (i, raison) in ecartes {
        sorts_export.insert(i, (Sort::Incoherent, raison));
    }
    let mut avertissements_aem: Vec<String> = Vec::new();
    for (i, a) in &avertis {
        if let Some((_, raison)) = sorts_export.get_mut(i) {
            *raison = format!("{} ; {}", raison, a);
        }
        avertissements_aem.push(format!("AEM reprise, mais {} : {}", a, noms.get(i).cloned().unwrap_or_default()));
    }
    ap.reponse.avertissements.extend(avertissements_aem);

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
            let fiche = &ctx.salaries[cle.as_str()];
            let ec = ecarts_salarie(&fiche.champs, &siens, &chemin_de);
            // Ce qui y est vide, de ce que les bulletins impriment (la fiche qu'ils creeraient) ; ce que la DSN complete
            // deja garde la valeur de la DSN (`completer::ajouter`).
            let champs = completer::champs_a_completer("salarie", &fiche.champs, &fiche_salarie(nir, &siens, ctx.entreprise.idcc.texte())?)?;
            let entree = fiche_existante(&mut reponse.salaries, "salarie", cle, || {
                format!("{} {}", fiche.nom.texte(), ri::titre_de(Some(&socle::ecriture::en_valeur(&fiche.prenom)).filter(|p| !p.is_null())))
            });
            entree.ajouter_ecarts(ec);
            noter_par_champ(entree, &champs, &siens, &chemin_de);
            completer::ajouter(&mut ap.a_completer, "salarie", Some(cle), &champs);
            continue;
        }
        if nirs_dsn.contains(nir) {
            // LA DSN LA CREE, LE BULLETIN LA COMPLETE : ce que la DSN ne porte pas (le n° Congés Spectacles,
            // une adresse qu'elle n'a pas) vient du bulletin ; ce que la DSN donne reste.
            let proposees = fiche_salarie(nir, &siens, ctx.entreprise.idcc.texte())?;
            completer_a_creer(&mut ap.a_creer, &mut reponse, "salarie", Some(nir), &proposees)?;
            continue;
        }
        let donnees = fiche_salarie(nir, &siens, ctx.entreprise.idcc.texte())?;
        let anomalies = crate::routes::fiches::verifier_comme_a_l_ecriture("salarie", &donnees)?;
        let libelle = format!("{} {}", donnees.get("nom").map_or(String::new(), valeur_en_texte), ri::titre_de(donnees.get("prenom")));
        ap.a_creer.push(ACreer { nom: "salarie", rang: Some(reponse.salaries.len()), donnees: donnees.clone() });
        reponse.salaries.push(FicheImportee::new("salarie", None, &libelle, EtatFiche::ACreer, Vec::new(), anomalies, &donnees));
        nouvelles_fiches += 1;
    }

    // 5b. Les salaries des certificats Congés Spectacles et des AEM en PDF : ils completent la fiche qui existe ou que les
    // DSN et les bulletins creent (ce qui y est vide), et creent celle d'un intermittent qu'aucune autre piece ne porte.
    let chemin_cert = |c: &Attestee| tri.fichiers[c.fichier].chemin.clone();
    let origine_cert = |c: &Attestee| Origine::piece(c.piece(), chemin_cert(c), c.c.page, c.mois());
    let mut par_salarie_cert: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (k, c) in certificats.iter().enumerate() {
        if let Some(n) = &c.c.nir {
            par_salarie_cert.entry(n.clone()).or_default().push(k);
        }
    }
    // Les fiches que les pieces creent, et parmi elles celles des salaries, par sorte de piece (`Attestee::source`).
    let mut fiches_certificats: BTreeMap<&str, usize> = BTreeMap::new();
    let mut salaries_certificats: BTreeMap<&str, usize> = BTreeMap::new();
    for (nir, rangs) in &par_salarie_cert {
        let siens: Vec<&Attestee> = rangs.iter().map(|&k| &certificats[k]).collect();
        let dernier = siens[siens.len() - 1];
        let proposees = fiche_de_certificats(nir, &siens, ctx.entreprise.idcc.texte())?;
        if let Some(cle) = par_nir.get(nir) {
            let fiche = &ctx.salaries[cle.as_str()];
            let champs = completer::champs_a_completer("salarie", &fiche.champs, &proposees)?;
            let entree = fiche_existante(&mut reponse.salaries, "salarie", cle, || {
                format!("{} {}", fiche.nom.texte(), ri::titre_de(Some(&socle::ecriture::en_valeur(&fiche.prenom)).filter(|p| !p.is_null())))
            });
            entree.noter(&champs, &origine_cert(dernier));
            completer::ajouter(&mut ap.a_completer, "salarie", Some(cle), &champs);
            continue;
        }
        if ap.a_creer.iter().any(|a| a.nom == "salarie" && a.donnees.get("nir").map(valeur_en_texte).as_deref() == Some(nir.as_str())) {
            completer_a_creer(&mut ap.a_creer, &mut reponse, "salarie", Some(nir), &proposees)?;
            continue;
        }
        let anomalies = crate::routes::fiches::verifier_comme_a_l_ecriture("salarie", &proposees)?;
        let libelle = format!("{} {}", proposees.get("nom").map_or(String::new(), valeur_en_texte), ri::titre_de(proposees.get("prenom")));
        ap.a_creer.push(ACreer { nom: "salarie", rang: Some(reponse.salaries.len()), donnees: proposees.clone() });
        let mut entree = FicheImportee::new("salarie", None, &libelle, EtatFiche::ACreer, Vec::new(), anomalies, &proposees);
        entree.lu_dans = Some(origine_cert(dernier));
        reponse.salaries.push(entree);
        nouvelles_fiches += 1;
        *fiches_certificats.entry(dernier.source()).or_default() += 1;
        *salaries_certificats.entry(dernier.source()).or_default() += 1;
    }

    // 6. L'entreprise : creee des bulletins si ni la fiche ni une DSN ne la donne ; sinon ses ecarts.
    if ap.licences.is_empty() {
        ap.licences = licences_lues(&lus);
    }
    if let Some(dernier) = lus.iter().rev().find(|l| siret_lu(&l.b).is_some()).or(lus.last()) {
        if siren_fiche.is_some() {
            let ec = ecarts_entreprise(&ctx.entreprise, &lus, dernier, &chemin_de(dernier));
            let entree = entreprise_existante(&mut reponse, ctx);
            entree.ajouter_ecarts(ec);
            let champs = completer::champs_a_completer("entreprise", &ctx.entreprise, &fiche_entreprise(&lus, dernier)?)?;
            entree.noter(&champs, &Origine::bulletin(Some(chemin_de(dernier)), Some(dernier.page)));
            completer::ajouter(&mut ap.a_completer, "entreprise", None, &champs);
        } else if reponse.entreprise.as_ref().is_some_and(|e| e.etat == EtatFiche::ACreer) {
            // La fiche que les DSN creent, completee de ce que le bulletin imprime et qu'elles ne portent pas (3d).
            let proposees = fiche_entreprise(&lus, dernier)?;
            completer_a_creer(&mut ap.a_creer, &mut reponse, "entreprise", None, &proposees)?;
        } else if reponse.entreprise.is_none() {
            let mut e = fiche_entreprise(&lus, dernier)?;
            ri::garder_le_nom(ctx, &mut e);
            let anomalies = crate::routes::fiches::verifier_comme_a_l_ecriture("entreprise", &e)?;
            let libelle = e.get("raison_sociale").map_or("?".to_string(), valeur_en_texte);
            ap.a_creer.push(ACreer { nom: "entreprise", rang: None, donnees: e.clone() });
            reponse.entreprise = Some(FicheImportee::new("entreprise", None, &libelle, EtatFiche::ACreer, Vec::new(), anomalies, &e));
            nouvelles_fiches += 1;
        }
    }

    // 6b. L'entreprise des certificats et des AEM : son numero d'adherent a la Caisse (« COMPTE »), que ni le bulletin ni la DSN
    // n'imprime toujours, son Siret et sa raison sociale ; comme pour un bulletin, la fiche qui existe garde ce qu'elle a.
    if let Some(dernier) = certificats.iter().rev().find(|c| c.c.siret.is_some()).or(certificats.last()) {
        let proposees = entreprise_du_certificat(dernier)?;
        if siren_fiche.is_some() {
            let mut ec = Vec::new();
            if let Some(compte) = &dernier.c.compte {
                let a = documents::conges_spectacles::forme_adherent(ctx.entreprise.congesspectacles_adherent.texte());
                if !a.is_empty() && &a != compte {
                    let (fichier, page, lu) = (chemin_cert(dernier), dernier.c.page, compte.clone());
                    let par = if dernier.aem.is_some() { DitPar::Aem { aem: lu, fichier, page } } else { DitPar::Certificat { certificat: lu, fichier, page } };
                    ec.push(Ecart::new(documents::conges_spectacles::CLE_ADHERENT, Value::String(a), par));
                }
            }
            let entree = entreprise_existante(&mut reponse, ctx);
            entree.ajouter_ecarts(ec);
            let champs = completer::champs_a_completer("entreprise", &ctx.entreprise, &proposees)?;
            entree.noter(&champs, &origine_cert(dernier));
            completer::ajouter(&mut ap.a_completer, "entreprise", None, &champs);
        } else if ap.a_creer.iter().any(|a| a.nom == "entreprise") {
            completer_a_creer(&mut ap.a_creer, &mut reponse, "entreprise", None, &proposees)?;
        } else if reponse.entreprise.is_none() {
            let mut e = proposees;
            ri::garder_le_nom(ctx, &mut e);
            let anomalies = crate::routes::fiches::verifier_comme_a_l_ecriture("entreprise", &e)?;
            let libelle = e.get("raison_sociale").map_or("?".to_string(), valeur_en_texte);
            ap.a_creer.push(ACreer { nom: "entreprise", rang: None, donnees: e.clone() });
            let mut entree = FicheImportee::new("entreprise", None, &libelle, EtatFiche::ACreer, Vec::new(), anomalies, &e);
            entree.lu_dans = Some(origine_cert(dernier));
            reponse.entreprise = Some(entree);
            nouvelles_fiches += 1;
            *fiches_certificats.entry(dernier.source()).or_default() += 1;
        }
    }

    // 7. Les spectacles, par numero d'objet, avec leur titre quand le bulletin le donne.
    let mut connus: HashMap<String, droit::contexte::IdSpectacle> = HashMap::new();
    for (cle, sp) in &ctx.spectacles {
        connus.insert(sp.numero_objet.texte().to_uppercase(), cle.clone());
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
        // LE TITRE QUE L'IMPORT PROPOSE : celui de la ligne « Spectacle » du bulletin ; a defaut, la case « Analytique »,
        // ou un logiciel (sPAIEctacle) imprime le titre du spectacle. Il se propose, il ne s'ecrit pas : la fiche garde
        // « A COMPLETER » et « Pour démarrer » demande de le valider ou de le corriger (`declarations::demarrage`). La case
        // « Analytique » qui porte le titre n'est pas un code analytique.
        let (propose, lu_dans) = match (&titre, &analytique) {
            (Some(t), _) => (Some(t.clone()), "Spectacle"),
            (None, Some(a)) => (Some(a.clone()), "Analytique"),
            (None, None) => (None, ""),
        };
        let analytique = analytique.filter(|_| titre.is_some());
        let proposition = |m: &mut Map<String, Value>| {
            if let Some(t) = &propose {
                m.insert(declarations::demarrage::TITRE_PROPOSE.into(), json!(t));
                m.insert(declarations::demarrage::TITRE_LU_DANS.into(), json!(lu_dans));
            }
        };
        if let Some(cle) = connus.get(numero) {
            let fiche = &socle::ecriture::en_valeur(&ctx.spectacles[cle]);
            let mut ec = Vec::new();
            // UN TITRE VIDE OU « A COMPLETER » (celui que l'import des DSN donne, la DSN ne portant pas le titre) recoit
            // le titre propose ; un vrai titre saisi reste, et l'ecart avec la ligne « Spectacle » est seulement signale.
            if let (Some(t), Some(p)) = (&titre, porteur) {
                let a = fiche.get("titre").map_or(String::new(), valeur_en_texte);
                if !completer::vide(fiche.get("titre")) && compare(&a) != compare(t) {
                    ec.push(Ecart::new("titre", Value::String(a), DitPar::Bulletin { bulletin: t.clone(), fichier: chemin_de(p), page: p.page }));
                }
            }
            let mut proposees = Map::new();
            if let Some(x) = &analytique {
                proposees.insert("code_analytique".into(), json!(x));
            }
            let mut champs = completer::champs_a_completer("spectacle", fiche, &proposees)?;
            // Le titre se propose a une fiche sans titre qui n'en a pas deja un a valider : comme toute case, la
            // proposition ne remplit qu'une case vide.
            if completer::vide(fiche.get("titre")) && completer::vide(fiche.get(declarations::demarrage::TITRE_PROPOSE)) {
                proposition(&mut champs);
            }
            let origine = Origine::bulletin(porteur.map(chemin_de), porteur.map(|p| p.page));
            let entree = fiche_existante(&mut reponse.spectacles, "spectacle", cle, || format!("{} ({})", fiche.get("titre").map_or("?".to_string(), valeur_en_texte), numero));
            entree.ajouter_ecarts(ec);
            entree.noter(&champs, &origine);
            completer::ajouter(&mut ap.a_completer, "spectacle", Some(cle), &champs);
            continue;
        }
        let deja_la = reponse.spectacles.iter().position(|e| e.etat == EtatFiche::ACreer && e.numero_objet().as_deref() == Some(numero.as_str()));
        if let Some(rang) = deja_la {
            // Un objet que les DSN ou les AEM creent : le bulletin lui propose son titre.
            let Some(a) = ap.a_creer.iter_mut().find(|a| a.nom == "spectacle" && a.rang == Some(rang)) else { continue };
            proposition(&mut a.donnees);
            if let Some(x) = &analytique {
                a.donnees.entry("code_analytique").or_insert_with(|| json!(x));
            }
            let anomalies = crate::routes::fiches::verifier_comme_a_l_ecriture("spectacle", &a.donnees)?;
            reponse.spectacles[rang].poser(&a.donnees, anomalies);
            continue;
        }
        let premier = rangs.iter().map(|&k| lus[k].mois.clone()).min().unwrap_or_default();
        let dernier = rangs.iter().map(|&k| lus[k].mois.clone()).max().unwrap_or_default();
        let mut donnees = Map::new();
        donnees.insert("titre".into(), json!(completer::A_COMPLETER));
        donnees.insert("numero_objet".into(), json!(numero));
        proposition(&mut donnees);
        if let Some(x) = &analytique {
            donnees.insert("code_analytique".into(), json!(x));
        }
        donnees.insert("alias".into(), json!([]));
        donnees.insert("actif".into(), json!(dernier >= seuil));
        donnees.insert("note".into(), json!(format!("bulletins de {} a {} ({} bulletin(s))", premier, dernier, rangs.len())));
        let anomalies = crate::routes::fiches::verifier_comme_a_l_ecriture("spectacle", &donnees)?;
        let libelle = match &propose {
            Some(t) => format!("numéro d'objet {}, titre proposé « {} » (bulletins de {} à {})", numero, t, premier, dernier),
            None => format!("numéro d'objet {}, sans titre (bulletins de {} à {})", numero, premier, dernier),
        };
        ap.a_creer.push(ACreer { nom: "spectacle", rang: Some(reponse.spectacles.len()), donnees: donnees.clone() });
        reponse.spectacles.push(FicheImportee::new("spectacle", None, &libelle, EtatFiche::ACreer, Vec::new(), anomalies, &donnees));
        nouvelles_fiches += 1;
    }
    // 7b. Les numeros d'objet des AEM en PDF qu'aucune fiche, aucune DSN ni aucun bulletin ne connait : un spectacle a
    // creer, sans titre (l'AEM ne l'imprime pas), comme ceux des AEM deposees.
    let mut objets_aem: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for a in certificats.iter().filter_map(|c| c.aem.as_ref()) {
        if let (Some(o), Some(m)) = (a.numero_objet.as_deref().filter(|o| !o.is_empty()), &a.mois) {
            objets_aem.entry(o.to_uppercase()).or_default().push(m.clone());
        }
    }
    for (numero, mois) in objets_aem {
        let deja_la = reponse.spectacles.iter().any(|e| e.numero_objet().as_deref() == Some(numero.as_str()));
        if connus.contains_key(&numero) || deja_la || ap.a_creer.iter().any(|a| a.nom == "spectacle" && a.donnees.get("numero_objet").map(valeur_en_texte).map(|n| n.to_uppercase()).as_deref() == Some(numero.as_str())) {
            continue;
        }
        let (premier, dernier) = (mois.iter().min().cloned().unwrap_or_default(), mois.iter().max().cloned().unwrap_or_default());
        let mut donnees = Map::new();
        donnees.insert("titre".into(), json!("A COMPLETER"));
        donnees.insert("numero_objet".into(), json!(numero));
        donnees.insert("alias".into(), json!([]));
        donnees.insert("actif".into(), json!(dernier >= seuil));
        donnees.insert("note".into(), json!(format!("AEM de {} a {} ({} contrat(s))", premier, dernier, mois.len())));
        let anomalies = crate::routes::fiches::verifier_comme_a_l_ecriture("spectacle", &donnees)?;
        let libelle = format!("numéro d'objet {}, sans titre (AEM de {} à {})", numero, premier, dernier);
        ap.a_creer.push(ACreer { nom: "spectacle", rang: Some(reponse.spectacles.len()), donnees: donnees.clone() });
        reponse.spectacles.push(FicheImportee::new("spectacle", None, &libelle, EtatFiche::ACreer, Vec::new(), anomalies, &donnees));
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
    // LA PAIE D'UN CONTRAT QUI COURT AU-DELA DU MOIS (un permanent) : la DSN la date des bornes du contrat
    // (S21.G00.40.001 et .010, un CDI sans fin), le bulletin de la periode payee ; les dates ne la rapprochent donc pas.
    // Le numero du contrat (S21.G00.40.009, que le versement rappelle en S21.G00.51.010) et le mois, si : un bulletin qui
    // imprime son numero se rattache a la paie que la DSN du mois declare sous ce numero.
    let mut par_numero: HashMap<(String, String, String), Vec<(String, String, Cible)>> = HashMap::new();
    // Sans numero imprime, la regle de l'ecriture (`import::ecrite::rapprocher`, « un contrat a cheval sur deux mois ») :
    // le bulletin complete la paie declaree du meme mois dont le contrat contient sa periode, quand il n'y en a qu'une.
    let mut par_mois: HashMap<(String, String), Vec<(String, String, String, Cible)>> = HashMap::new();
    let mut vus: HashSet<(String, String, String)> = HashSet::new();
    // (mois, NIR, contrat, debut, fin) d'une paie, en texte.
    let reperes = |x: &PaieLue| {
        let iso = |j: Option<Jour>| j.map(|j| j.iso()).unwrap_or_default();
        (x.mois.clone(), x.paie.nir.clone().unwrap_or_default(), x.paie.contrat.clone(), iso(x.paie.debut), iso(x.paie.fin))
    };
    for (k, x) in ap.paies.iter().enumerate() {
        let (mois, nir, contrat, debut, fin) = reperes(x);
        vus.insert((mois.clone(), nir.clone(), contrat.clone()));
        index.entry((nir.clone(), debut.clone(), fin.clone())).or_default().push((mois.clone(), contrat.clone(), Cible::Dsn(k)));
        if !contrat.is_empty() {
            par_numero.entry((nir.clone(), mois.clone(), contrat.clone())).or_default().push((mois.clone(), contrat.clone(), Cible::Dsn(k)));
        }
        if !debut.is_empty() {
            par_mois.entry((nir, mois)).or_default().push((debut, fin, contrat, Cible::Dsn(k)));
        }
    }
    // Une paie que l'historique tient d'un bulletin seul s'efface devant la DSN qui la declare (la DSN est lue apres le
    // bulletin : l'ordre inverse) : le bulletin complete la ligne DSN, et l'ecriture retire la ligne « bulletin »
    // (`import::ecrite::rapprocher`).
    let mut de_bulletin: HashSet<(String, String, String)> = HashSet::new();
    for x in &historique_paies {
        let (mois, nir, contrat, debut, fin) = reperes(x);
        if nir.is_empty() || !vus.insert((mois.clone(), nir.clone(), contrat.clone())) {
            continue;
        }
        let cle = x.cle();
        if x.paie.declare_par == Declaration::Bulletin {
            de_bulletin.insert(cle.clone());
        }
        index.entry((nir, debut, fin)).or_default().push((mois, contrat, Cible::Historique(cle)));
    }
    let mut prises: HashMap<String, (String, usize, Decimal, Decimal)> = HashMap::new();
    let (mut paies_nouvelles, mut paies_completees) = (0usize, 0usize);
    let mut paies_liste: Vec<PaieLue> = Vec::new();
    let mut contrats_liste: Vec<ContratLu> = Vec::new();
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
        if candidats.is_empty() {
            if let Some(n) = l.b.contrat.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
                candidats = par_numero.get(&(l.nir.clone(), l.mois.clone(), n.to_string())).cloned().unwrap_or_default();
            }
        }
        if candidats.is_empty() {
            let contient: Vec<(String, String, Cible)> = par_mois
                .get(&(l.nir.clone(), l.mois.clone()))
                .into_iter()
                .flatten()
                .filter(|(d, f, _, _)| *d <= debut && (f.is_empty() || fin <= *f))
                .map(|(_, _, contrat, cible)| (l.mois.clone(), contrat.clone(), cible.clone()))
                .collect();
            if contient.len() == 1 {
                candidats = contient;
            }
        }
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
                pages.entry(l.fichier).or_default().push(PageCr { page: l.page, sort: Sort::Incoherent, raison, infos });
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
                (Sort::DejaRepris, format!("le même bulletin est déjà lu : {} page {}", autre, page))
            } else {
                (Sort::Incoherent, format!("un autre bulletin ({} page {}) donne d'autres montants pour la même paie : aucun des deux ne l'emporte", autre, page))
            };
            pages.entry(l.fichier).or_default().push(PageCr { page: l.page, sort, raison, infos });
            continue;
        }
        prises.insert(ident, (chemin.clone(), l.page, l.b.brut, l.b.net_a_payer));

        // La paie.
        let (mut ligne, source, avant): (PaieLue, &str, Option<PaieLue>) = match &cible {
            Some((_, _, Cible::Dsn(k))) => {
                let x = ap.paies[*k].clone();
                let avant = hist_par_cle.get(&x.cle()).cloned();
                (x, "dsn", avant)
            }
            Some((_, _, Cible::Historique(c))) => {
                let x = hist_par_cle[c].clone();
                (x.clone(), "historique", Some(x))
            }
            None => {
                let contrat = match &l.b.numero {
                    Some(n) => format!("bulletin {}", n),
                    None => {
                        if nb_pages > 1 { format!("{} page {}", copie, l.page) } else { copie.clone() }
                    }
                };
                let mut paie = PaieVersee::vide(contrat, Declaration::Bulletin);
                (paie.nir, paie.debut, paie.fin, paie.fichier) = (Some(l.nir.clone()), Some(l.b.debut), Some(l.b.fin), Some(copie.clone()));
                let x = PaieLue { mois: l.mois.clone(), salarie: salarie.clone(), paie };
                let avant = hist_par_cle.get(&x.cle()).cloned();
                (x, "bulletin", avant)
            }
        };
        let ecarts = completer(&mut ligne.paie, &l.b, &copie, l.page, ctx.entreprise.idcc.texte())?;
        // Le contrat que le bulletin atteste ; celui d'un permanent ne compte que son mois : ses heures, ses cachets et
        // ses jours sont ceux de la paie de ce mois (le contrat entier n'en garde pas : `import::ecrite`).
        let atteste = l.compteurs.as_ref().map_err(|m| format!("ses compteurs ne se lisent pas ({})", m)).and_then(|c| engagement(l, c, &salarie, &copie));
        if let (Ok((cle, _)), false) = (&atteste, marque_d_intermittent(&l.b)) {
            (ligne.paie.heures, ligne.paie.cachets, ligne.paie.jours) = (Some(cle.4 as i64), Some(cle.5 as i64), Some(cle.6 as i64));
        }
        let etat_paie = if avant.as_ref().map(PaieLue::sans_date_d_import) == Some(ligne.sans_date_d_import()) {
            EtatPaie::Deja
        } else if source == "bulletin" {
            EtatPaie::Nouvelle
        } else {
            EtatPaie::Completee
        };
        match &cible {
            Some((_, _, Cible::Dsn(k))) => ap.paies[*k] = ligne.clone(),
            _ if etat_paie != EtatPaie::Deja => ap.paies.push(ligne.clone()),
            _ => {}
        }
        if etat_paie != EtatPaie::Deja {
            paies_liste.push(ligne.clone());
            if etat_paie == EtatPaie::Nouvelle {
                paies_nouvelles += 1;
            } else {
                paies_completees += 1;
            }
        }
        let mut phrases: Vec<String> = vec![match (etat_paie, source) {
            (EtatPaie::Deja, _) => "paie déjà dans l'historique".to_string(),
            (EtatPaie::Nouvelle, _) => "paie nouvelle : aucune DSN ne la déclare".to_string(),
            (_, "dsn") => format!("complète la paie déclarée en DSN ({})", ligne.mois),
            _ => format!("complète la paie de l'historique ({})", ligne.mois),
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
            infos.ecarts = ecarts.into_iter().map(|(champ, avant, gagne, apres)| EcartPaie::new(champ, source, avant, gagne, apres)).collect();
        }
        infos.paie = Some(etat_paie);

        // Le contrat atteste.
        let etat_contrat = match atteste {
            Err(m) => {
                phrases.push(format!("contrat non repris : {}", m));
                EtatContrat::Inconnu
            }
            Ok((cle, ligne_contrat)) => {
                // LE BULLETIN EST LA PIECE DE PAIE : un contrat d'AEM aux memes dates qui compte autre chose
                // s'efface devant le sien (`import::attestes`) ; un autre contrat atteste reste, et l'ecart se dit.
                let pareils = registre.pareils(&l.nir, &debut, &fin, "bulletin");
                let compte = |a: &Atteste| (a.heures, a.cachets, a.jours, a.brut);
                let le_sien = (cle.4, cle.5, cle.6, socle::num::decimal_lu(&cle.7));
                let en_mots = crate::import::attestes::en_mots(cle.4, cle.5, cle.6, le_sien.3);
                if pareils.iter().any(|a| compte(a) == le_sien) {
                    phrases.push("contrat déjà attesté".into());
                    EtatContrat::Deja
                } else if let Some(a) = pareils.iter().find(|a| a.par != Attestation::Aem) {
                    phrases.push(format!(
                        "un contrat attesté aux mêmes dates ({}) porte d'autres compteurs : il est gardé, celui du bulletin n'est pas ajouté (le contrat attesté dit {}, le bulletin {})",
                        a.d_ou(),
                        a.en_mots(),
                        en_mots
                    ));
                    infos.contrat_bulletin = Some(ligne_contrat);
                    EtatContrat::Ecart
                } else {
                    phrases.push("contrat attesté ajouté".into());
                    contrats_liste.push(ligne_contrat.clone());
                    phrases.extend(registre.ajouter(&l.nir, ligne_contrat, "bulletin", "bulletins"));
                    EtatContrat::Nouveau
                }
            }
        };
        infos.contrat = Some(etat_contrat);
        if let Ok(c) = &l.compteurs {
            infos.compteurs = c.avertissements.clone();
        }
        let inconnus: Vec<String> = l
            .b
            .avertissements
            .iter()
            .filter_map(|a| a.strip_prefix("libellé inconnu : ").map(String::from))
            .collect();
        let sort = if etat_paie == EtatPaie::Deja && etat_contrat != EtatContrat::Nouveau {
            Sort::DejaRepris
        } else if !inconnus.is_empty() {
            phrases.push(format!("ligne(s) sans rubrique du catalogue, gardée(s) telle(s) quelle(s) : {}", inconnus.join(", ")));
            Sort::LibelleInconnu
        } else {
            Sort::Repris
        };
        if sort != Sort::DejaRepris {
            pages_reprises += 1;
            if !l.incoherences.is_empty() {
                pages_incoherentes += 1;
            }
        }
        if !l.incoherences.is_empty() {
            phrases.push(format!("le bulletin fait foi, ses montants imprimés sont repris malgré : {}", l.incoherences.join(" ; ")));
            infos.avertissements = l.incoherences.clone();
        }
        pages.entry(l.fichier).or_default().push(PageCr { page: l.page, sort, raison: phrases.join(" ; "), infos });
    }

    // 8b. Les contrats des certificats Congés Spectacles et des AEM en PDF, les AEM d'abord (elles disent les heures, les
    // cachets et les jours a part, et le brut au centime). Le registre dit si le contrat est deja atteste
    // (`import::attestes`) : la piece le confirme alors, n'ajoute rien, et l'ecart de brut, de jours ou de cachets se
    // note. Une paie declaree aux memes dates, sans contrat atteste (une DSN seule), recoit le sien ; une paie declaree
    // a d'autres dates qui les chevauchent ne se rattacherait a aucun contrat : celui de la piece n'est pas ajoute.
    let declarees_dates: Vec<(String, String, String)> = ap
        .paies
        .iter()
        .chain(historique_paies.iter())
        .map(|x| {
            let (_, nir, _, debut, fin) = reperes(x);
            (nir, debut, fin)
        })
        .collect();
    let chevauche = crate::import::attestes::chevauche;
    let (mut certificats_confirmes, mut aem_confirmees) = (0usize, 0usize);
    let ordre: Vec<&Attestee> = certificats.iter().filter(|c| c.aem.is_some()).chain(certificats.iter().filter(|c| c.aem.is_none())).collect();
    for cl in ordre {
        let c = &cl.c;
        let est_aem = cl.aem.is_some();
        let (Some(nir), Some(debut), Some(fin), Some(brut)) = (&c.nir, c.debut, c.fin, c.brut) else { continue };
        let nom = cl.nom();
        let (d, f) = (debut.iso(), fin.iso());
        let du_au = format!("du {} au {}", debut.jj_mm_aaaa(), fin.jj_mm_aaaa());
        let mut infos = InfosPage { debut: Some(d.clone()), fin: Some(f.clone()), mois: cl.mois(), ..InfosPage::default() };
        if est_aem {
            infos.aem = c.numero.clone();
        } else {
            infos.certificat = c.numero.clone();
        }
        let pareils = registre.pareils(nir, &d, &f, cl.source());
        // UN CERTIFICAT DONT D'AUTRES PIECES N'ATTESTENT QUE DES MOIS (un bulletin, une AEM, chacun le sien) atteste seul
        // ses autres mois (`attestes::reste_du_certificat`) ; un certificat deja repris ne se reprend pas deux fois.
        let mut reste: Option<ContratLu> = None;
        let mut sans_reste: Option<String> = None;
        if !est_aem && !pareils.is_empty() && !pareils.iter().any(Atteste::de_certificat) {
            let statut = statut_de(ctx, &par_nir, &ap.a_creer, nir, c)?;
            let copie = nom_copie(&tri.fichiers[cl.fichier]).unwrap_or_else(|| chemin_cert(cl));
            if let Ok((ligne, _)) = contrat_du_certificat(c, statut.as_deref(), &cle_de(nir), &copie) {
                match registre.reste_du_certificat(nir, &ligne, &pareils) {
                    Ok(r) => reste = r,
                    Err(m) => sans_reste = Some(m),
                }
            }
        }
        if !pareils.is_empty() && reste.is_none() {
            if est_aem {
                aem_confirmees += 1;
            } else {
                certificats_confirmes += 1;
            }
            let mut sources: Vec<String> = pareils.iter().map(Atteste::d_ou).collect();
            sources.dedup();
            let mut phrases = vec![format!("{} {} : le contrat est déjà attesté ({}), rien n'est ajouté", nom, du_au, sources.join(", "))];
            phrases.extend(sans_reste);
            // L'ecart, quand les contrats confirmes tiennent dans les dates de la piece (un certificat coupe au 31 mars
            // n'en porte qu'une part).
            if pareils.iter().all(|a| a.debut >= d && a.fin <= f) {
                let somme: Option<Decimal> = pareils.iter().map(|a| a.brut).sum();
                let (cachets, jours): (i128, i128) = (pareils.iter().map(|a| a.cachets).sum(), pareils.iter().map(|a| a.jours).sum());
                match &cl.aem {
                    Some(a) => {
                        if somme.is_some_and(|s| s != brut) {
                            phrases.push(format!("écart noté : l'{} dit {} de brut, les contrats {}", cl.nom(), euros(brut), euros(somme.unwrap_or_default())));
                        }
                        if (i128::from(a.cachets()), i128::from(a.jours.unwrap_or(0))) != (cachets, jours) {
                            phrases.push(format!(
                                "écart noté : l'{} dit {} cachets et {} jours, les contrats {} cachets et {} jours",
                                cl.nom(),
                                a.cachets(),
                                a.jours.unwrap_or(0),
                                cachets,
                                jours
                            ));
                        }
                    }
                    None => {
                        let nombre = c.nombre.unwrap_or(0);
                        if somme.is_some_and(|s| Decimal::from(documents::conges_spectacles::arrondi_euro(s)) != brut) {
                            phrases.push(format!("écart noté : le certificat dit {} de brut, les contrats {}", euros(brut), euros(somme.unwrap_or_default())));
                        }
                        if i128::from(nombre) != cachets && i128::from(nombre) != jours {
                            phrases.push(format!("écart noté : le certificat dit {} jours ou cachets, les contrats {} cachets et {} jours", nombre, cachets, jours));
                        }
                    }
                }
            }
            infos.contrat = Some(EtatContrat::Deja);
            pages.entry(cl.fichier).or_default().push(PageCr { page: c.page, sort: Sort::DejaRepris, raison: phrases.join(" ; "), infos });
            continue;
        }
        // Le reste d'un certificat : ses dates et ses compteurs sont ceux du reste.
        let (d, f, du_au) = match &reste {
            Some(l) => {
                let (rd, rf) = (l.debut.clone().unwrap_or_default(), l.fin.clone().unwrap_or_default());
                let jj = |x: &str| Jour::lire(x).map(|j| j.jj_mm_aaaa()).unwrap_or_else(|_| x.to_string());
                let mut sources: Vec<String> = pareils.iter().map(Atteste::d_ou).collect();
                sources.dedup();
                let du_au = format!("{} (attesté ailleurs pour ses autres mois : {}) : le reste, du {} au {},", du_au, sources.join(", "), jj(&rd), jj(&rf));
                (rd, rf, du_au)
            }
            None => (d, f, du_au),
        };
        let autres: Vec<String> = declarees_dates
            .iter()
            .filter(|p| &p.0 == nir && chevauche(&p.1, &p.2, &d, &f) && !(p.1 == d && p.2 == f))
            .map(|p| format!("du {} au {}", p.1, p.2))
            .collect();
        if !autres.is_empty() {
            let raison = format!(
                "{} {} : une paie déclarée de ce salarié couvre d'autres dates ({}) ; le contrat de la pièce ne se rattacherait à aucune, il n'est pas ajouté",
                nom,
                du_au,
                autres.join(", ")
            );
            infos.contrat = Some(EtatContrat::Ecart);
            pages.entry(cl.fichier).or_default().push(PageCr { page: c.page, sort: Sort::DejaRepris, raison, infos });
            continue;
        }
        let copie = nom_copie(&tri.fichiers[cl.fichier]).unwrap_or_else(|| chemin_cert(cl));
        let est_reste = reste.is_some();
        let lu = match (&cl.aem, reste) {
            (Some(a), _) => contrat_de_l_aem(a, &cle_de(nir), &copie),
            (None, Some(l)) => {
                let n = |x: Option<i64>| x.unwrap_or(0);
                let quoi = if n(l.contrat.cachets) > 0 { format!("{} cachet(s)", n(l.contrat.cachets)) } else { format!("{} jour(s)", n(l.contrat.jours)) };
                let brut_reste = euros(l.contrat.brut.unwrap_or_default());
                Ok((l, format!("{}, {} de brut : ce que dit le certificat moins ce que disent les autres pièces ; son brut, {}, est arrondi à l'euro", quoi, brut_reste, euros(brut))))
            }
            (None, None) => {
                let statut = statut_de(ctx, &par_nir, &ap.a_creer, nir, c)?;
                contrat_du_certificat(c, statut.as_deref(), &cle_de(nir), &copie)
            }
        };
        match lu {
            Err(raison) => {
                infos.contrat = Some(EtatContrat::Inconnu);
                pages.entry(cl.fichier).or_default().push(PageCr { page: c.page, sort: Sort::Incomplet, raison: format!("{} {} : {}", nom, du_au, raison), infos });
            }
            Ok((ligne, quoi)) => {
                infos.contrat = Some(EtatContrat::Nouveau);
                let raison = if est_aem {
                    format!("{} {} : contrat attesté ajouté ({}, {} de brut)", nom, du_au, quoi, euros(ligne.contrat.brut.unwrap_or_default()))
                } else if est_reste {
                    format!("certificat Congés Spectacles {} contrat attesté ajouté ({}) ; les heures, que le certificat ne porte pas, n'y sont pas", du_au, quoi)
                } else {
                    format!("certificat Congés Spectacles {} : contrat attesté ajouté ({}, {} de brut arrondi à l'euro) ; les heures, que le certificat ne porte pas, n'y sont pas", du_au, quoi, euros(brut))
                };
                let bloc = if est_aem { "aem_pdf" } else { "certificats" };
                let raison = std::iter::once(raison).chain(registre.ajouter(nir, ligne, cl.source(), bloc)).collect::<Vec<_>>().join(" ; ");
                pages.entry(cl.fichier).or_default().push(PageCr { page: c.page, sort: Sort::Repris, raison, infos });
            }
        }
    }

    // 9. Le compte rendu, fichier par fichier.
    let mut fichiers: Vec<FichierVu> = Vec::new();
    let mut copies: BTreeMap<usize, String> = BTreeMap::new();
    let mut comptes: ParSort<usize> = ParSort::default();
    for (i, f) in tri.fichiers.iter().enumerate() {
        let tri_raison = f.raison.clone().unwrap_or_default();
        let mut ps: Vec<PageVu> = Vec::new();
        let (sort, raison): (Sort, String) = match f.sorte {
            Sorte::Dsn | Sorte::Aem => sorts_export.get(&i).cloned().unwrap_or((Sort::Repris, String::new())),
            Sorte::PdfSansTexte => (Sort::SansTexte, tri_raison),
            Sorte::Archive => (Sort::Archive, tri_raison),
            Sorte::Doublon => (Sort::Doublon, tri_raison),
            Sorte::Illisible | Sorte::Autre => (Sort::NonReconnu, tri_raison),
            Sorte::PdfTexte => match illisibles.get(&i) {
                Some(m) => (Sort::NonReconnu, m.clone()),
                None => {
                    let mut lues = pages.remove(&i).unwrap_or_default();
                    lues.sort_by_key(|p| p.page);
                    let sort = Sort::GRAVITE.iter().find(|g| lues.iter().any(|p| p.sort == **g)).copied().unwrap_or(Sort::NonReconnu);
                    let raison = match lues.as_slice() {
                        [] => "PDF sans page".to_string(),
                        [p] => p.raison.clone(),
                        _ => {
                            let par: Vec<String> = Sort::GRAVITE
                                .iter()
                                .filter_map(|g| Some(lues.iter().filter(|p| p.sort == *g).count()).filter(|n| *n > 0).map(|n| format!("{} {}", n, g.en_mots().1)))
                                .collect();
                            format!("{} pages : {}", lues.len(), par.join(", "))
                        }
                    };
                    ps = lues.into_iter().map(|p| PageVu::new(p.page, p.sort, p.raison, p.infos)).collect();
                    (sort, raison)
                }
            },
        };
        // Ce que l'import copie dans `historique/bulletins/` : une DSN ou une AEM lue, un PDF dont une page au moins est
        // reprise (ou l'a deja ete).
        let repris = match f.sorte {
            Sorte::Dsn | Sorte::Aem => matches!(sort, Sort::Repris | Sort::DejaRepris),
            Sorte::PdfTexte => ps.iter().any(|p| matches!(p.sort, Sort::Repris | Sort::LibelleInconnu | Sort::DejaRepris)),
            _ => false,
        };
        let copie = nom_copie(f).filter(|_| repris);
        if let Some(c) = &copie {
            copies.insert(i, c.clone());
        }
        *comptes.de(sort) += 1;
        fichiers.push(FichierVu {
            chemin: f.chemin.clone(),
            sorte: f.sorte.nom(),
            pages: ps,
            sort,
            raison,
            copie,
            doublon_de: f.doublon_de.clone(),
            archive: f.archive.clone(),
            mois: f.mois.clone(),
            de_dit: None,
            pages_dit: String::new(),
        });
    }
    // CE QUI N'EST PAS REPRIS, pour l'encart : chaque fichier et sa raison ; un import qui en laisse se rejoue.
    let non_repris: Vec<NonReprisVu> =
        fichiers.iter().filter(|f| f.sort.non_repris()).map(|f| NonReprisVu { chemin: f.chemin.clone(), sort: f.sort, raison: f.raison.clone() }).collect();
    reponse.tri = TriVu {
        dossier: tri.dossier.display().to_string(),
        fichiers,
        comptes,
        total: tri.fichiers.len(),
        non_repris: (!non_repris.is_empty()).then_some(NonRepris { fichiers: non_repris, phrase: REESSAYER }),
        dit: String::new(),
    };
    reponse.bulletins = BlocBulletins {
        pages: n_pages,
        lus: par_logiciel.values().sum(),
        repris: pages_reprises,
        par_logiciel,
        du: du.map(|d| d.iso()),
        au: au.map(|d| d.iso()),
        fiches_a_creer: nouvelles_fiches,
        contrats_nouveaux: contrats_liste.len(),
        contrats: contrats_liste,
        paies_nouvelles,
        paies_completees,
        paies: paies_liste,
        dit: None,
    };
    // LES CERTIFICATS ET LES AEM EN PDF, chacun son bloc : combien, de quand a quand, ce qu'ils ajoutent ; et, pour une
    // fiche qu'ils creent, ce qu'aucune de ces pieces ne porte et qui reste a saisir (la fiche dit elle-meme ses cases
    // obligatoires vides). L'AEM dit la date de naissance ; le certificat, non.
    for (bloc, source, ajoutes, confirmes) in
        [("certificats", "certificat", registre.lignes("certificats"), certificats_confirmes), ("aem_pdf", "AEM", registre.lignes("aem_pdf"), aem_confirmees)]
    {
        let siennes: Vec<&Attestee> = certificats.iter().filter(|c| c.source() == source).collect();
        if siennes.is_empty() {
            continue;
        }
        let reste: Vec<&'static str> = match (salaries_certificats.get(source).copied().unwrap_or(0) > 0, source) {
            (false, _) => Vec::new(),
            (true, "AEM") => vec!["le compte bancaire", "le taux de prélèvement à la source", "la commune de naissance"],
            (true, _) => vec![
                "le compte bancaire",
                "le taux de prélèvement à la source",
                "la date et la commune de naissance (le NIR ne dit que l'année, le mois et le département)",
            ],
        };
        let lu = BlocPieces {
            lus: siennes.len(),
            du: siennes.iter().filter_map(|c| c.c.debut).min().map(|d| d.iso()),
            au: siennes.iter().filter_map(|c| c.c.fin).max().map(|d| d.iso()),
            contrats_nouveaux: ajoutes.len(),
            contrats: ajoutes,
            confirmes,
            fiches_a_creer: fiches_certificats.get(source).copied().unwrap_or(0),
            reste_a_saisir: reste,
            dit: String::new(),
        };
        if bloc == "certificats" {
            reponse.certificats = Some(lu);
        } else {
            reponse.aem_pdf = Some(lu);
        }
    }
    if pages_incoherentes > 0 {
        reponse.avertissements.push(format!(
            "{} bulletin(s) ne retombent pas sur leurs propres totaux (brut, net) : le bulletin fait foi, ses montants imprimés sont repris tels quels ; le compte rendu dit l'écart, page par page.",
            pages_incoherentes
        ));
    }
    // UN CONTRAT QUE LA DSN DECLARE SANS SES NETS (`routes::import::SansNets`) : son bulletin, lu ici ou deja
    // repris, les donne ; sinon l'apercu le dit, contrat par contrat.
    for s in &ap.sans_nets {
        let ligne = ap.paies.iter().find(|x| x.mois == s.mois && x.paie.nir.as_deref() == Some(s.nir.as_str()) && x.paie.contrat == s.contrat);
        let dates = ligne.map(|x| (x.paie.debut, x.paie.fin));
        let complete = ligne.is_some_and(|x| x.paie.bulletin.is_some())
            || historique_paies.iter().any(|h| {
                h.mois == s.mois
                    && h.paie.nir.as_deref() == Some(s.nir.as_str())
                    && h.paie.bulletin.is_some()
                    && (h.paie.contrat == s.contrat || (h.paie.declare_par == Declaration::Bulletin && Some((h.paie.debut, h.paie.fin)) == dates))
            });
        if !complete {
            reponse.avertissements.push(s.phrase());
        }
    }
    reponse.contrats_nouveaux = registre.nombre("declarations");
    reponse.rien_a_faire = reponse.rien_a_faire
        && nouvelles_fiches == 0
        && ap.a_completer.is_empty()
        && registre.retenus.is_empty()
        && paies_nouvelles == 0
        && paies_completees == 0;
    reponse.avertissements.extend(registre.muets.iter().cloned());
    ap.contrats_attestes = registre.retenus;
    ap.contrats_changes = registre.changes;
    ap.reponse = reponse;
    Ok(DossierLu { ap, tri, copies })
}

/// Ce que la page dit d'elle-meme dans le compte rendu.
fn infos_de(l: &Lu, salarie: Option<&str>) -> InfosPage {
    InfosPage {
        logiciel: l.b.logiciel.clone(),
        numero: l.b.numero.clone(),
        salarie: salarie.map(String::from),
        debut: Some(l.b.debut.iso()),
        fin: Some(l.b.fin.iso()),
        mois: Some(l.mois.clone()),
        ..InfosPage::default()
    }
}

/// Une fiche que l'import cree des DSN (`a_creer` ; un salarie par son NIR, ou l'entreprise), completee de ce que les
/// bulletins impriment et qu'elle n'a pas ; ses anomalies se refont, et la reponse montre la fiche completee.
fn completer_a_creer(a_creer: &mut [ACreer], reponse: &mut Importation, nom: &str, nir: Option<&str>, proposees: &Map<String, Value>) -> R<()> {
    let Some(a) = a_creer.iter_mut().find(|a| a.nom == nom && nir.is_none_or(|n| a.donnees.get("nir").map(valeur_en_texte).as_deref() == Some(n))) else {
        return Ok(());
    };
    let mut champs = completer::champs_a_completer(nom, &Value::Object(a.donnees.clone()), proposees)?;
    champs.extend(lisibles_de_la_piece_sur(nom, &a.donnees, proposees)?);
    if champs.is_empty() {
        return Ok(());
    }
    a.donnees.extend(champs);
    let anomalies = crate::routes::fiches::verifier_comme_a_l_ecriture(nom, &a.donnees)?;
    let entree = match (nom, a.rang) {
        ("entreprise", _) => reponse.entreprise.as_mut(),
        (_, Some(i)) => reponse.salaries.get_mut(i),
        _ => None,
    };
    if let Some(e) = entree {
        e.poser(&a.donnees, anomalies);
    }
    Ok(())
}

/// La fiche existante `cle` de la liste, ajoutee (`libelle`) si la reponse ne la porte pas encore.
fn fiche_existante<'a>(liste: &'a mut Vec<FicheImportee>, nom: &'static str, cle: &str, libelle: impl FnOnce() -> String) -> &'a mut FicheImportee {
    let rang = match liste.iter().position(|e| e.cle.as_deref() == Some(cle)) {
        Some(i) => i,
        None => {
            liste.push(FicheImportee::new(nom, Some(cle), &libelle(), EtatFiche::Existante, Vec::new(), Vec::new(), &Map::new()));
            liste.len() - 1
        }
    };
    &mut liste[rang]
}

/// La fiche Entreprise existante de la reponse, ajoutee si elle ne la porte pas encore.
fn entreprise_existante<'a>(reponse: &'a mut Importation, ctx: &Contexte) -> &'a mut FicheImportee {
    reponse.entreprise.get_or_insert_with(|| {
        FicheImportee::new("entreprise", None, ctx.entreprise.raison_sociale.texte(), EtatFiche::Existante, Vec::new(), Vec::new(), &Map::new())
    })
}

/// Note dans l'entree d'un salarie ce que ses bulletins completent, chaque champ avec le bulletin le plus recent qui
/// l'imprime (`identite`) ; l'adresse, le statut, l'anciennete et la cle du NIR, avec le plus recent.
fn noter_par_champ(entree: &mut FicheImportee, champs: &Map<String, Value>, ls: &[&Lu], chemin: &dyn Fn(&Lu) -> String) {
    let Some(dernier) = ls.last() else { return };
    let par: HashMap<&str, &Lu> = identite(ls).into_iter().map(|(cle, _, l)| (cle, l)).collect();
    for (k, v) in champs {
        let base = k.strip_suffix("_affichage").unwrap_or(k);
        let l = par.get(base).copied().unwrap_or(dernier);
        let mut un = Map::new();
        un.insert(k.clone(), v.clone());
        entree.noter(&un, &Origine::bulletin(Some(chemin(l)), Some(l.page)));
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
fn ecarts_salarie(fiche: &dyn ParCase, ls: &[&Lu], chemin: &dyn Fn(&Lu) -> String) -> Vec<Ecart> {
    let mut out = Vec::new();
    for (cle, lu, l) in identite(ls) {
        if !matches!(cle, "nom" | "prenom" | "date_naissance" | "num_conges_spectacles") {
            continue;
        }
        let dite = fiche.case(cle);
        let a = dite.as_ref().filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
        // Une case vide ou « A COMPLETER » n'est pas un ecart : l'import la complete.
        if completer::vide(dite.as_ref()) || compare(&a) == compare(&lu) {
            continue;
        }
        out.push(Ecart::new(cle, Value::String(a), DitPar::Bulletin { bulletin: lu, fichier: chemin(l), page: l.page }));
    }
    out
}

/// Ce que le bulletin dit de l'entreprise autrement que la fiche (compare sans casse ni accents) : le Siret, le NAF, et
/// la raison sociale, l'adresse, l'IDCC et le compte Urssaf -- ce que `fiche_entreprise` en lit.
fn ecarts_entreprise(fiche: &dyn ParCase, lus: &[Lu], l: &Lu, chemin: &str) -> Vec<Ecart> {
    let mut out = Vec::new();
    let t = |k: &str| fiche.case(k).filter(|v| vrai(v)).map(|v| valeur_en_texte(&v)).unwrap_or_default();
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
        out.push(Ecart::new(cle, Value::String(a), DitPar::Bulletin { bulletin: lu, fichier: chemin.to_string(), page: l.page }));
    }
    out
}

/// CE QUE LE BULLETIN IMPRIME EN MINUSCULES EST DEJA LA FORME LISIBLE : « La Fanfare des Toits », « 3 quai de
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

/// LA FORME LISIBLE QUE LA PIECE IMPRIME, DANS UNE FICHE QUE L'IMPORT CREE D'UNE DSN : la DSN ecrit sans accent ni
/// minuscule (« CHAMPMESLE », norme DSN, caracteres autorises), la forme lisible qu'on en tire n'est qu'une
/// reconstitution (« Champmesle »). Quand le bulletin imprime le meme texte (compare sans casse ni accents) avec ce que
/// la DSN a perdu (« CHAMPMESLÉ », « Séraphine »), sa forme lisible (`lisibles_de_la_piece`, `champs::combler_affichage`)
/// remplace la reconstitution ; la forme declaree s'en derive a l'ecriture, au seul endroit qui la derive. Une forme
/// lisible que la fiche ne tient pas de la forme declaree (le metier du catalogue) reste la sienne.
fn lisibles_de_la_piece_sur(nom: &str, fiche: &Map<String, Value>, proposees: &Map<String, Value>) -> R<Map<String, Value>> {
    let mut out = Map::new();
    for c in declarations::champs::fiche(nom)? {
        let Some(source) = c.derive_de.as_deref().filter(|d| !d.is_empty()) else { continue };
        let (Some(Value::String(officiel)), Some(Value::String(lisible))) = (fiche.get(&c.cle), fiche.get(source)) else { continue };
        let (Some(Value::String(officiel_piece)), Some(Value::String(lisible_piece))) = (proposees.get(&c.cle), proposees.get(source)) else { continue };
        if lisible_piece == lisible || compare(officiel_piece) != compare(officiel) {
            continue;
        }
        let reconstitue = declarations::champs::combler_affichage(nom, &Map::from_iter([(c.cle.clone(), json!(officiel))]))?;
        if reconstitue.get(source) == Some(&json!(lisible)) {
            out.insert(source.to_string(), json!(lisible_piece));
        }
    }
    Ok(out)
}

/// La fiche d'un salarie qu'aucune fiche ni aucune DSN ne porte : ce que ses bulletins impriment, champ par champ le
/// plus recent (`ls` dans l'ordre du temps).
fn fiche_salarie(nir: &str, ls: &[&Lu], idcc: &str) -> R<Map<String, Value>> {
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
    let statut = ls.iter().rev().find_map(|l| l.b.salarie.statut.as_deref());
    let emploi = f.get("emploi").map(valeur_en_texte);
    // Une salariee recoit le metier au feminin, comme dans sa fiche (`routes::mois::emploi`) : le sexe que porte son NIR.
    let femme = declarations::ntt::sexe_de(Some(nir), None) == Some("2");
    f.extend(statut_et_metier(statut, emploi.as_deref(), femme, idcc)?);
    if let Some(premier) = ls.iter().map(|l| l.b.debut).min() {
        f.insert("date_anciennete".into(), json!(premier.iso()));
    }
    lisibles_de_la_piece("salarie", &mut f)?;
    let mut f = declarations::champs::combler_affichage("salarie", &f)?;
    f.insert("actif".into(), json!(true));
    Ok(f)
}

/// Ce que le statut et l'emploi qu'un bulletin imprime disent du salarie, en champs de sa fiche.
///
/// LE STATUT, EN TOUTES LETTRES, et rien de plus : « Artiste » ou « Technicien » font la case « Artiste ou
/// technicien » ; « Cadre » ou « Non cadre », la case « Cadre ou non cadre » (S21.G00.40.003 : 01 cadre, 04 non cadre,
/// cahier technique 2026.1 p. 183) ; « artiste cadre » dit les deux. « Non cadre » ne dit pas artiste ou technicien :
/// cette case-la reste a remplir, et l'apercu le dit.
///
/// L'EMPLOI, QUAND IL NOMME UN METIER DU CATALOGUE ET UN SEUL : « MUSICIEN » est le metier MUS010 ; la fiche
/// prend alors ce que le choix de ce metier donne (`emplois::resoudre` : le code PCS-ESE, le code France Travail, le
/// statut conventionnel), comme si on l'avait choisi dans la liste. Le statut et la qualite de cadre imprimes priment
/// sur ceux du catalogue. Un intitule qui ne nomme aucun metier, ou plusieurs, reste tel quel : le metier se choisit
/// dans la fiche apres l'import, et l'apercu le dit.
pub(crate) fn statut_et_metier(statut: Option<&str>, emploi: Option<&str>, femme: bool, idcc: &str) -> R<Map<String, Value>> {
    let mut f = Map::new();
    let intitule = emploi.map(str::trim).unwrap_or_default();
    if !intitule.is_empty() {
        f.insert("emploi".into(), json!(intitule));
    }
    let mots = statut.map(bulletin_lu::jetons).unwrap_or_default();
    if let Some(s) = mots.iter().find(|m| *m == "artiste" || *m == "technicien") {
        f.insert("statut".into(), json!(s));
    }
    if let Some(i) = mots.iter().position(|m| m == "cadre" || m == "noncadre") {
        let non = mots[i] == "noncadre" || (i > 0 && mots[i - 1] == "non");
        f.insert("statut_categoriel_rc".into(), json!(if non { "04" } else { "01" }));
    }
    if let Some((code, feminin)) = droit::emplois::par_intitule(intitule, compare).map_err(|m| erreur("ErreurOperation", m))? {
        let cadre = f.get("statut_categoriel_rc").map(valeur_en_texte).map(|s| s == "01");
        let r = droit::emplois::resoudre(&code, cadre, feminin || femme, idcc).map_err(|m| erreur("ErreurOperation", m))?;
        // TRAVAIL A FAIRE (lot de l'import) : la fiche importee se compose encore par ses cles.
        for (k, v) in crate::routes::en_json(r)?.as_object().into_iter().flatten() {
            if v.is_null() || (k == "statut" && f.contains_key("statut")) {
                continue;
            }
            f.insert(k.clone(), v.clone());
        }
    }
    Ok(f)
}

/// La suite des licences que les bulletins impriment (mois, numero), la plus ancienne d'abord, comme les DSN.
fn licences_lues(lus: &[Lu]) -> Vec<(String, String)> {
    let mut licences: Vec<(String, String)> = Vec::new();
    for x in lus {
        if let Some(n) = x.b.entreprise.licence.as_deref().filter(|n| !n.is_empty()) {
            if licences.last().is_none_or(|p| p.1 != n) {
                licences.push((x.mois.clone(), n.to_string()));
            }
        }
    }
    licences
}

/// La fiche de l'entreprise, des bulletins : le plus recent pour l'identite, la premiere licence (`poser_licence`).
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
    if let Some(c) = e.convention.as_deref() {
        let i = match idcc_lu(c) {
            Some(i) => Some(i),
            None => droit::clauses::idcc_nomme(c).map_err(|m| erreur("ErreurOperation", m))?,
        };
        if let Some(i) = i {
            f.insert("idcc".into(), json!(i));
        }
    }
    if let Some(u) = urssaf_lu(&l.b) {
        f.insert("urssaf_compte".into(), json!(u));
    }
    crate::routes::import::poser_licence(&mut f, &licences_lues(lus));
    lisibles_de_la_piece("entreprise", &mut f)?;
    let mut f = declarations::champs::combler_affichage("entreprise", &f)?;
    if f.get("raison_sociale").is_some_and(vrai) && !f.get("nom_affichage").is_some_and(vrai) {
        let t = ri::titre_de(f.get("raison_sociale"));
        f.insert("nom_affichage".into(), json!(t));
    }
    Ok(f)
}

/// Complete une paie par le bulletin. Sur le brut, le net imposable, le net a payer et le PAS, le bulletin l'emporte ;
/// ailleurs il remplit ce qui est vide et note ce qui differe. Rend les ecarts : (champ, valeur d'avant, le bulletin
/// l'emporte, valeur du bulletin).
fn completer(ligne: &mut PaieVersee, b: &BulletinLu, chemin: &str, page: usize, idcc: &str) -> R<Vec<(&'static str, String, bool, String)>> {
    use engagement::importe::{BulletinSource, CotisationImprimee, DatesTravaillees, EmploiImprime, LigneImprimee};
    let mut ecarts = Vec::new();
    let pas = b.pas.as_ref().and_then(|p| p.montant);
    for (champ, place, v) in [
        ("brut", &mut ligne.brut, Some(b.brut)),
        ("net_imposable", &mut ligne.net_imposable, b.net_imposable),
        ("net_a_payer", &mut ligne.net_a_payer, Some(b.net_a_payer)),
        ("pas", &mut ligne.pas, pas),
    ] {
        let Some(v) = v else { continue };
        if let Some(avant) = place.filter(|a| *a != v) {
            ecarts.push((champ, avant.to_string(), true, v.to_string()));
        }
        *place = Some(v);
    }
    let plein = |x: &Option<String>| x.clone().filter(|s| !s.trim().is_empty());
    let objet = plein(&b.numero_objet);
    match (&ligne.numero_objet, objet) {
        (None, o) => ligne.numero_objet = o,
        (Some(avant), Some(o)) if avant.to_uppercase() != o.to_uppercase() => ecarts.push(("numero_objet", avant.clone(), false, o)),
        _ => {}
    }
    // Le taux du prelevement que le bulletin imprime, quand la DSN ne l'a pas declare.
    if ligne.taux_pas.is_none() {
        ligne.taux_pas = b.pas.as_ref().and_then(|p| p.taux);
    }
    if ligne.verse_le.is_none() {
        ligne.verse_le = b.regle_le;
    }
    for (place, x) in [(&mut ligne.spectacle, &b.spectacle), (&mut ligne.analytique, &b.analytique)] {
        if let Some(x) = plein(x) {
            *place = Some(x);
        }
    }
    for (place, x) in [
        (&mut ligne.cotisations_salariales, b.total_salarie),
        (&mut ligne.cotisations_patronales, b.total_employeur),
        (&mut ligne.cout_employeur, b.cout_employeur),
        (&mut ligne.net_social, b.net_social),
        (&mut ligne.net_avant_impot, b.net_avant_impot),
    ] {
        if x.is_some() {
            *place = x;
        }
    }
    ligne.lignes = b
        .lignes
        .iter()
        .map(|l| LigneImprimee { quantite: l.quantite, libelle: l.libelle.clone(), taux: l.taux, montant: l.montant, soumis: l.soumis, rubrique: l.rubrique.clone(), heures: l.heures })
        .collect();
    ligne.dates = b.dates_travaillees.iter().map(|(debut, fin)| DatesTravaillees { debut: *debut, fin: *fin }).collect();
    ligne.cotisations = Some(
        b.cotisations
            .iter()
            .map(|c| CotisationImprimee {
                famille: c.famille.clone(),
                libelle: c.libelle.clone(),
                cotisation: c.cotisation.clone(),
                base: c.base,
                taux_employeur: c.taux_employeur,
                montant_employeur: c.montant_employeur,
                taux_salarie: c.taux_salarie,
                montant_salarie: c.montant_salarie,
                hors_total: c.hors_total,
            })
            .collect(),
    );
    // Le statut et l'emploi que CE bulletin imprime : un salarie artiste un mois peut etre technicien, ou non cadre au
    // regime general, un autre ; la comparaison des calculs lit celui de la paie (`engagement::ecarts`), pas la fiche.
    let imprime = statut_et_metier(b.salarie.statut.as_deref(), b.salarie.emploi.as_deref(), false, idcc).unwrap_or_default();
    if !imprime.is_empty() {
        let lu: EmploiImprime = socle::forme::lire(&Value::Object(imprime), "L'emploi qu'un bulletin imprime ne se lit pas : à signaler au développeur.").map_err(|m| erreur("ErreurOperation", m))?;
        ligne.emploi_imprime = Some(lu);
    }
    ligne.bulletin = Some(BulletinSource { fichier: chemin.to_string(), page: page as u64, logiciel: b.logiciel.clone(), numero: b.numero.clone(), contrat: b.contrat.clone() });
    Ok(ecarts)
}

/// Le contrat qu'un bulletin atteste, tel qu'un engagement importe le porte (`importe.contrat`), et sa cle `_cle_engagement`.
/// Les heures se comptent en heures entieres comme celles d'une saisie (`cumul::engagement_de_paie` : arrondi au
/// pair) ; des cachets ou des jours qui ne sont pas entiers, ou un compteur inconnu, ne font pas de contrat.
fn engagement(l: &Lu, c: &Compteurs, salarie: &str, chemin: &str) -> Result<(CleEngagement, ContratLu), String> {
    // LES JOURS TRAVAILLES sont ceux qu'atteste l'attestation employeur mensuelle de l'intermittent (annexes 8 et 10) ;
    // un permanent paye au mois n'en a pas, et son bulletin n'imprime ni dates travaillees ni cumul de jours : son
    // contrat ne compte pas de jours (0), il ne les « ignore » pas.
    let jours = if c.jours.is_none() && !marque_d_intermittent(&l.b) { Some(Decimal::ZERO) } else { c.jours };
    let (Some(cachets), Some(heures), Some(jours)) = (c.cachets, c.heures, jours) else {
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
    let plein = |x: &Option<String>| x.clone().filter(|s| !s.trim().is_empty());
    let mut a = ContratAtteste::vide(Attestation::Bulletin);
    a.numero_objet = Some(objet).filter(|o| !o.trim().is_empty());
    (a.heures, a.cachets, a.jours, a.brut) = (Some(heures), Some(cachets), Some(jours), Some(brut));
    a.heures_enseignement = Some(enseignement).filter(|h| *h > 0);
    // LA NATURE DU CONTRAT : aucun des bulletins lus ne l'imprime en mention (`bulletins-lecture-base.json`), et rien sur la
    // page ne la fixe a coup sur. Deux marques disent un engagement du spectacle : la cotisation Congés Spectacles
    // (due pour l'artiste ou le technicien qui n'est pas employe douze mois de suite, D. 7121-41 du code du travail,
    // `atelier/reference/sources/caisse-conges-affiliation/LISEZ-MOI.md`) et le numero d'objet (interdit hors des
    // natures 02 et 10, CCH-11 : regle 6). Avec l'une d'elles, le contrat est un CDDU (`engagement::importe::Importe::nature`).
    // Sans aucune, c'est le contrat d'un permanent -- CDI ou CDD de droit commun, que le bulletin ne dit pas et qu'on ne
    // devine pas : il est marque « a dire », se demande apres l'import (`import::nature`), et ne se compare ni ne se
    // reprend en attendant (`Importe::nature_rangee` dit sous quelle nature ce qui a ete verse se range).
    a.nature_a_dire = !marque_d_intermittent(&l.b);
    a.numero_bulletin = plein(&l.b.numero);
    // Le numero du contrat, quand le bulletin l'imprime : il se lit, il ne se tire pas (regle 4).
    a.numero_contrat = plein(&l.b.contrat);
    (a.fichier, a.page) = (Some(chemin.to_string()), Some(l.page as u64));
    Ok((cle, ContratLu { salarie: salarie.to_string(), debut: Some(debut), fin: Some(fin), numero: None, contrat: a }))
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

// --------------------------------------------------------------------------------------------- les certificats et les AEM en PDF

/// Un certificat d'emploi Congés Spectacles ou une AEM en PDF, lus, et le rang de son fichier dans le tri. `c` : ce que
/// la piece dit de l'employeur, du salarie et du contrat, sous la forme d'un certificat (`aem_lu::Aem::commun`) ;
/// `aem` : ce que l'AEM dit de plus (les heures, les cachets, les jours, le numero d'objet, le statut, la naissance).
struct Attestee {
    fichier: usize,
    c: crate::import::certificat_lu::Certificat,
    aem: Option<crate::import::aem_lu::Aem>,
}

impl Attestee {
    /// Son nom dans le compte rendu.
    fn nom(&self) -> &'static str {
        match &self.aem {
            Some(a) if a.aer => "AER Cinéma Spectacle",
            Some(_) => "AEM",
            None => "certificat Congés Spectacles",
        }
    }

    /// Sa `source`, dans ce que l'import dit d'ou vient une valeur (et son `atteste_par`).
    fn source(&self) -> &'static str {
        if self.aem.is_some() { "AEM" } else { "certificat" }
    }

    /// La piece qu'elle est, dans la reponse.
    fn piece(&self) -> Source {
        if self.aem.is_some() { Source::Aem } else { Source::Certificat }
    }

    /// Le mois dont elle parle : celui de l'AEM, celui de la fin du certificat.
    fn mois(&self) -> Option<String> {
        self.aem.as_ref().and_then(|a| a.mois.clone()).or_else(|| self.c.fin.map(|f| f.mois_civil().to_string()))
    }
}

/// La fiche d'un salarie que ses certificats donnent (`ls` dans l'ordre du temps : le plus recent l'emporte, champ par
/// champ) : son nom de naissance, son prenom, son NIR et sa cle, son adresse, son emploi et ce que le metier du
/// catalogue qu'il nomme donne (`statut_et_metier`), cadre ou non, et le debut de son premier contrat.
fn fiche_de_certificats(nir: &str, ls: &[&Attestee], idcc: &str) -> R<Map<String, Value>> {
    let mut f = Map::new();
    let dernier = |g: &dyn Fn(&crate::import::certificat_lu::Certificat) -> Option<String>| ls.iter().rev().find_map(|l| g(&l.c).filter(|x| !x.trim().is_empty()));
    if let Some(n) = dernier(&|c| c.nom.clone()) {
        f.insert("nom".into(), json!(n));
    }
    if let Some(p) = dernier(&|c| c.prenom.clone()) {
        f.insert("prenom".into(), json!(p));
    }
    if let Some((rue, cp, commune)) = ls.iter().rev().find_map(|l| adresse_lue(&l.c.adresse)) {
        if !rue.is_empty() {
            f.insert("adresse".into(), json!(rue));
        }
        f.insert("code_postal".into(), json!(cp));
        f.insert("commune".into(), json!(commune));
    }
    f.insert("nir".into(), json!(nir));
    let cle = ls.iter().rev().find_map(|l| l.c.cle_nir.clone()).or_else(|| declarations::champs::cle_nir(nir)).unwrap_or_default();
    f.insert("nir_cle".into(), json!(cle));
    // L'AEM dit la date de naissance, et artiste ou technicien par sa case ; le certificat, ni l'un ni l'autre.
    if let Some(n) = ls.iter().rev().find_map(|l| l.aem.as_ref().and_then(|a| a.naissance)) {
        f.insert("date_naissance".into(), json!(n.iso()));
    }
    let cadre = ls.iter().rev().find_map(|l| l.c.cadre).map(|c| if c { "cadre" } else { "non cadre" });
    let statut = ls.iter().rev().find_map(|l| l.aem.as_ref().and_then(|a| a.statut));
    let statut = match (statut, cadre) {
        (Some(s), Some(c)) => Some(format!("{} {}", s, c)),
        (s, c) => s.or(c).map(String::from),
    };
    let emploi = dernier(&|c| c.emploi.clone());
    let femme = declarations::ntt::sexe_de(Some(nir), None) == Some("2");
    f.extend(statut_et_metier(statut.as_deref(), emploi.as_deref(), femme, idcc)?);
    if let Some(premier) = ls.iter().filter_map(|l| l.c.debut).min() {
        f.insert("date_anciennete".into(), json!(premier.iso()));
    }
    lisibles_de_la_piece("salarie", &mut f)?;
    let mut f = declarations::champs::combler_affichage("salarie", &f)?;
    f.insert("actif".into(), json!(true));
    Ok(f)
}

/// La fiche de l'entreprise qu'un certificat ou une AEM donne : son Siret, sa raison sociale, son numero d'adherent a la
/// Caisse des Congés Spectacles (la case « COMPTE » du certificat, « N° d'affiliation à la caisse de congés spectacles »
/// de l'AEM : `documents::conges_spectacles::CLE_ADHERENT`) ; l'AEM dit aussi le code APE, l'IDCC, le code postal et la
/// commune.
fn entreprise_du_certificat(p: &Attestee) -> R<Map<String, Value>> {
    let c = &p.c;
    let mut f = Map::new();
    if let Some(a) = &p.aem {
        for (k, v) in [("naf", &a.ape), ("idcc", &a.idcc), ("code_postal", &a.employeur_code_postal), ("commune", &a.employeur_commune)] {
            if let Some(v) = v {
                f.insert(k.into(), json!(v));
            }
        }
    }
    if let Some(n) = c.employeur.as_deref().map(str::trim).filter(|x| !x.is_empty()) {
        f.insert("raison_sociale".into(), json!(n));
    }
    if let Some(s) = &c.siret {
        f.insert("siren".into(), json!(&s[..9]));
        f.insert("nic".into(), json!(&s[9..]));
        f.insert("siret".into(), json!(s));
    }
    if let Some(a) = &c.compte {
        f.insert(documents::conges_spectacles::CLE_ADHERENT.into(), json!(a));
    }
    lisibles_de_la_piece("entreprise", &mut f)?;
    let mut f = declarations::champs::combler_affichage("entreprise", &f)?;
    if f.get("raison_sociale").is_some_and(vrai) && !f.get("nom_affichage").is_some_and(vrai) {
        let t = ri::titre_de(f.get("raison_sociale"));
        f.insert("nom_affichage".into(), json!(t));
    }
    Ok(f)
}

/// Artiste ou technicien : la fiche qui existe, sinon celle que l'import cree (DSN, bulletin), sinon le metier du
/// catalogue que nomme l'emploi du certificat. `None` : rien ne le dit.
fn statut_de(ctx: &Contexte, par_nir: &HashMap<String, String>, a_creer: &[ACreer], nir: &str, c: &crate::import::certificat_lu::Certificat) -> R<Option<String>> {
    let lu = |v: Option<&Value>| v.filter(|x| vrai(x)).map(valeur_en_texte).filter(|s| !s.trim().is_empty());
    if let Some(s) = par_nir.get(nir).and_then(|k| lu(ctx.salaries.get(k.as_str()).map(|f| socle::ecriture::en_valeur(&f.statut)).filter(|s| !s.is_null()).as_ref())) {
        return Ok(Some(s));
    }
    if let Some(s) = a_creer.iter().filter(|a| a.nom == "salarie" && a.donnees.get("nir").map(valeur_en_texte).as_deref() == Some(nir)).find_map(|a| lu(a.donnees.get("statut"))) {
        return Ok(Some(s));
    }
    let femme = declarations::ntt::sexe_de(Some(nir), None) == Some("2");
    Ok(lu(statut_et_metier(None, c.emploi.as_deref(), femme, ctx.entreprise.idcc.texte())?.get("statut")))
}

/// Le contrat qu'une AEM en PDF atteste, tel qu'un engagement importe le porte (la forme du contrat d'une AEM deposee,
/// `import::ecrite`), et ce qu'il compte en mots : la periode du mois (`aem_lu::Aem::periode`), ses heures, ses
/// cachets isoles et groupes, ses jours, son brut et son numero d'objet, tels qu'imprimes.
fn contrat_de_l_aem(a: &crate::import::aem_lu::Aem, salarie: &str, copie: &str) -> Result<(ContratLu, String), String> {
    let (Some((debut, fin)), Some(brut)) = (a.periode(), a.brut) else {
        return Err("l'AEM ne dit pas tout ce qu'un contrat demande".into());
    };
    let (heures, cachets, jours) = (a.heures.unwrap_or(0), a.cachets(), a.jours.unwrap_or(0));
    let mut brut = brut.round_dp(2);
    brut.rescale(2);
    let pluriel = |n: i64, mot: &str| format!("{} {}{}", n, mot, if n > 1 { "s" } else { "" });
    let quoi: Vec<String> = [(heures, "heure"), (cachets, "cachet"), (jours, "jour")].iter().filter(|x| x.0 > 0).map(|x| pluriel(x.0, x.1)).collect();
    let plein = |x: &Option<String>| x.clone().filter(|s| !s.trim().is_empty());
    let mut c = ContratAtteste::vide(Attestation::Aem);
    c.numero_objet = plein(&a.numero_objet);
    (c.heures, c.cachets, c.jours, c.brut) = (Some(heures), Some(cachets), Some(jours), Some(brut));
    c.numero_attestation = plein(&a.numero);
    (c.fichier, c.page) = (Some(copie.to_string()), Some(a.page as u64));
    let ligne = ContratLu { salarie: salarie.to_string(), debut: Some(debut.iso()), fin: Some(fin.iso()), numero: None, contrat: c };
    Ok((ligne, if quoi.is_empty() { "aucune heure, aucun cachet, aucun jour".into() } else { quoi.join(", ") }))
}

/// Le contrat qu'un certificat atteste, tel qu'un engagement importe le porte, et ce qu'il compte en mots.
///
/// LE « NB JOURS OU CACHETS » : l'attestation des annexes 8 et 10 porte « le nombre d'heures de travail, ou le nombre
/// de cachets pour les artistes » (Unédic, circulaire 2016-25, `atelier/reference/sources/unedic-circulaire-2016-25-
/// annexes-8-10.txt`, l. 5802) ; le certificat n'a qu'une case pour les deux. Inference : pour un artiste, elle compte
/// des cachets ; pour un technicien, des jours. Sans statut, rien ne le dit : pas de contrat. Les heures, que le
/// certificat ne porte pas, ne s'inventent pas (0) ; le brut est celui du certificat, « arrondi à l'euro le plus proche ».
fn contrat_du_certificat(c: &crate::import::certificat_lu::Certificat, statut: Option<&str>, salarie: &str, copie: &str) -> Result<(ContratLu, String), String> {
    let (Some(debut), Some(fin), Some(nombre), Some(brut)) = (c.debut, c.fin, c.nombre, c.brut) else {
        return Err("le certificat ne dit pas tout ce qu'un contrat demande".into());
    };
    let pluriel = |n: i64, mot: &str| format!("{} {}{}", n, mot, if n > 1 { "s" } else { "" });
    let (cachets, jours, quoi) = match statut {
        Some("artiste") => (nombre, 0, pluriel(nombre, "cachet")),
        Some(s) if !s.trim().is_empty() => (0, nombre, pluriel(nombre, "jour")),
        _ => {
            return Err(format!(
                "l'emploi « {} » ne nomme aucun métier du catalogue : le nombre de jours ou cachets ne dit pas si ce sont des cachets (artiste) ou des jours (technicien), le contrat n'est pas repris",
                c.emploi.clone().unwrap_or_default()
            ))
        }
    };
    let mut brut = brut;
    brut.rescale(2);
    let mut a = ContratAtteste::vide(Attestation::Certificat);
    (a.heures, a.cachets, a.jours, a.brut) = (Some(0), Some(cachets), Some(jours), Some(brut));
    a.numero_certificat = c.numero.clone();
    a.base_conge = c.base_conge;
    a.atteste_le = c.signe_le.map(|s| s.iso());
    (a.fichier, a.page) = (Some(copie.to_string()), Some(c.page as u64));
    Ok((ContratLu { salarie: salarie.to_string(), debut: Some(debut.iso()), fin: Some(fin.iso()), numero: None, contrat: a }, quoi))
}

/// Les compteurs de chaque bulletin lu (`bulletin_lu::compteurs`), confrontes a ses cumuls avec ses freres : les autres
/// bulletins du meme salarie, du meme logiciel, dont la periode finit le meme mois (un meme bulletin lu deux fois, meme
/// numero et memes dates, n'en est pas un).
fn compter(lus: &mut [Lu], racine: &Path) {
    let comptes: Vec<Result<Compteurs, String>> = lus
        .iter()
        .map(|l| {
            let freres: Vec<&BulletinLu> = lus
                .iter()
                .filter(|s| s.nir == l.nir && s.mois == l.mois && s.b.logiciel == l.b.logiciel)
                .filter(|s| (&s.b.numero, s.b.debut, s.b.fin) != (&l.b.numero, l.b.debut, l.b.fin))
                .map(|s| &s.b)
                .collect();
            bulletin_lu::compteurs(&l.b, &freres, racine).map_err(|e| e.to_string())
        })
        .collect();
    for (l, c) in lus.iter_mut().zip(comptes) {
        l.compteurs = c;
    }
}
