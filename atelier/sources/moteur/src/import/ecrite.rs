//! `importer` : ecrit ce que `importation-apercu` annonce.
//!
//! `{"dossier": ...}` : la boite aux lettres `import/` entiere, comme l'apercu (`import::bulletins`). Dans l'ordre : ce
//! qui est repris (DSN, AEM, PDF dont une page au moins est reprise) est COPIE dans `historique/bulletins/`, sous le nom
//! de sa copie (`<empreinte>.<pdf|dsn|txt>` : rien de personnel ; `import::bulletins::nom_copie`) -- rien ne s'ecrase ;
//! l'apercu est relu DANS ces copies, et c'est lui qui s'ecrit ; l'alerte CASC-SVP (`alerte_apres_import`) ; les fiches
//! a creer (entreprise, salaries, spectacles), ecrites comme celles de l'ecran (`crate::routes::fiches::ecrire_fiche`,
//! `forcer`) ; les ENGAGEMENTS IMPORTES (`engagement::importe`), chacun dans `engagements/AAAA-MM.json`, le fichier de son
//! mois d'entree (`engagement::importe::ecrire_tous`) : le contrat qu'une AEM ou un bulletin atteste, et ce qui a ete verse, mois par mois (la DSN
//! deposee, completee par le bulletin). Un import se rejoue sans dommage. La reponse est celle de l'apercu, completee de
//! ce qui a ete ecrit (`ecrit`, `fichiers`, `copie`) ; elle garde le compte rendu (`tri`) du dossier lu, pour l'ecran ;
//! un document accepte et pas lu part en incident, masque (`import::incident`). Les pieces copiees restent sous
//! `historique/` : le dossier du salarie montre les pages des bulletins, l'alerte CASC-SVP relit les DSN.
//!
//! UNE PAIE, DEUX SOURCES, UNE LIGNE (les deux chemins) : une ligne de DSN qui arrive sur une ligne que des bulletins
//! ont deja completee garde ce que le bulletin a ecrit (le bulletin fait foi) ; une ligne qu'un bulletin seul declare
//! s'efface devant la ligne de DSN des memes salarie, debut et fin, qu'elle complete (`rapprocher`) -- l'ordre DSN
//! puis bulletins, ou bulletins puis DSN, donne les memes engagements.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use engagement::importe::{Attestation, ContratLu, Declaration, PaieLue, PaieVersee};
use socle::valeur::valeur_en_texte;
use socle::echec::erreur;
use socle::fichiers::vrai;
use socle::num::Decimal;
use socle::echec::Echec;
use crate::import::reponse::{brut, Copie, EtatFiche, Importation};

type R<T> = Result<T, Echec>;

fn en_os(e: &std::io::Error, p: &Path) -> Echec {
    erreur("OSError", socle::echec::du_disque(p, e))
}

// ------------------------------------------------------------------ l'alerte CASC-SVP

/// La DSN qu'un mois importe a deposee hors de PuppetPaie : le fichier que l'import a retenu pour ce mois (la paie
/// versee qu'une DSN declare, d'un engagement importe ; celle d'un bulletin seul nomme son PDF), sa copie sous
/// `historique/bulletins/`, ou qu'est la compagnie. `None` : un mois que l'import n'a pas retenu ; un fichier retenu
/// introuvable refuse.
pub(crate) fn dsn_importee(ctx: &Contexte, mois: &str) -> Result<Option<PathBuf>, String> {
    let fichiers: Vec<String> = engagement::importe::paies_lues(&ctx.racine)?
        .into_iter()
        .filter(|x| x.mois == mois && x.paie.declare_par == Declaration::Dsn)
        .filter_map(|x| x.paie.fichier)
        .collect();
    let Some(nom) = fichiers.last() else { return Ok(None) };
    let chemin = dossier_bulletins(ctx).join(nom);
    if chemin.exists() {
        return Ok(Some(chemin));
    }
    Err(socle::echec::avec(format!("La DSN de {} a été importée, mais son fichier n'est plus dans l'historique de la compagnie.", mois), serde_json::json!({"fichier": nom})))
}

/// `lire_dsn_casc` : la DSN deposee, et si elle porte la CASC-SVP (un contrat rattache, S21.G00.70.013, a
/// l'adhesion que la DSN nomme `contrat` en bloc 15). `None` : illisible.
pub(crate) fn lire_dsn_casc(chemin: &Path, contrat: &str) -> Result<Option<(declarations::dsn_lue::DsnLue, bool)>, Echec> {
    let Some(d) = declarations::dsn_lue::lire_une_dsn(chemin)? else { return Ok(None) };
    let porte = d
        .adhesions
        .iter()
        .filter(|a| a.libelle == contrat)
        .filter_map(|a| a.reference.as_ref())
        .any(|r| d.rattachements.contains(r));
    Ok(Some((d, porte)))
}

/// L'alerte apres l'import : une compagnie de la 3090 qui arrive de CoolPaie avec des DSN de 2025
/// ou apres : la CASC-SVP y manque peut-etre. Rien si ce n'est pas la 3090, ou si tout est regularise ; toute
/// `ErreurOperation` de la regularisation (`regularisation_casc_svp(ecrire=False)`) vaut « rien ». Seuls comptent
/// le nombre de mois, la periode et le total, que cette fonction calcule comme la regularisation.
fn alerte_apres_import(ctx: &Contexte) -> R<Option<String>> {
    let base = declarations::casc::base().map_err(Echec::from)?;
    let ent = &ctx.entreprise;
    if ent.idcc.texte() != valeur_en_texte(&base["idcc"]) {
        return Ok(None);
    }
    if !matches!(droit::bareme::casc_exemptee(ent), Ok(false)) {
        return Ok(None);
    }
    let faits: HashSet<String> = declarations::casc::mois_regularises(ctx).map_err(Echec::from)?.into_iter().collect();
    // `mois_disponibles` : les mois des paies declarees, et les mois saisis.
    let mut dispo: BTreeSet<String> = BTreeSet::new();
    dispo.extend(engagement::importe::paies_lues(&ctx.racine).map_err(Echec::from)?.into_iter().map(|x| x.mois));
    dispo.extend(engagement::stock::mois_saisis(&ctx.racine).unwrap_or_default().into_iter().map(String::from));
    let depuis = valeur_en_texte(&base["depuis"]);
    let contrat = valeur_en_texte(&base["dsn"]["contrat"]);
    let mut par_mois: Vec<(String, Decimal)> = Vec::new();
    for m in dispo.iter().filter(|m| **m >= depuis && !faits.contains(*m)) {
        let chemin = match dsn_importee(ctx, m) {
            Ok(Some(c)) => c,
            Ok(None) => continue,
            Err(_) => return Ok(None),
        };
        let Some((d, porte)) = lire_dsn_casc(&chemin, &contrat)? else { return Ok(None) };
        if porte {
            continue;
        }
        let mut masse = socle::num::zero2();
        for b in d.paies().iter().filter_map(|p| p.mesures.brut) {
            masse += b;
        }
        let an: String = m.chars().take(4).collect();
        let Some(taux) = base["taux"].get(&an) else { return Ok(None) };
        let taux = socle::num::dec(taux).map_err(Echec::from)?;
        let cot = (masse * taux / Decimal::from(100)).round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointAwayFromZero);
        par_mois.push((m.clone(), cot));
    }
    if par_mois.is_empty() {
        return Ok(None);
    }
    let libelle = |m: &str| socle::mois::Mois::lire(m).map(|m| m.libelle(socle::mois::Graphie::Accentuee)).map_err(|e| erreur("ValueError", e));
    let periode = format!("{} à {}", libelle(&par_mois[0].0)?, libelle(&par_mois[par_mois.len() - 1].0)?);
    let total = par_mois.iter().fold(Decimal::ZERO, |a, (_, c)| a + *c);
    Ok(Some(format!(
        "CASC-SVP : {} DSN déposée(s) depuis janvier 2025 ({}) ne portent peut-être pas la cotisation CASC-SVP -- un logiciel qui n'était pas paramétré l'a omise. Si Audiens ne l'a pas encaissée, {} sont à régulariser : Exports > « regularisation-casc-svp » fait le dossier (virements, courriels, liste nominative).",
        par_mois.len(),
        periode,
        socle::texte::euros(total)
    )))
}

// ------------------------------------------------------------------ une paie, deux sources

/// Une paie que des bulletins ont deja completee (ou ecrite).
fn porte_un_bulletin(x: &PaieLue) -> bool {
    x.paie.bulletin.is_some()
}

fn declaree_par_bulletin(x: &PaieLue) -> bool {
    x.paie.declare_par == Declaration::Bulletin
}

/// Complete une paie par ce qu'un bulletin a ecrit d'elle (`b`), comme `import::bulletins::completer` : sur le
/// brut, le net imposable, le net a payer et le PAS, le bulletin l'emporte ; le numero d'objet et le jour du versement
/// remplissent ce qui est vide ; les colonnes du bulletin s'ajoutent (les heures, cachets et jours sont ceux du mois
/// d'un permanent).
fn completer_par(ligne: &mut PaieVersee, b: &PaieVersee) {
    fn gagne<T: Clone>(l: &mut Option<T>, b: &Option<T>) {
        if b.is_some() {
            l.clone_from(b);
        }
    }
    for (l, x) in [(&mut ligne.brut, &b.brut), (&mut ligne.net_imposable, &b.net_imposable), (&mut ligne.net_a_payer, &b.net_a_payer), (&mut ligne.pas, &b.pas)] {
        gagne(l, x);
    }
    if ligne.numero_objet.is_none() {
        ligne.numero_objet.clone_from(&b.numero_objet);
    }
    if ligne.verse_le.is_none() {
        ligne.verse_le = b.verse_le;
    }
    for (l, x) in [(&mut ligne.heures, &b.heures), (&mut ligne.cachets, &b.cachets), (&mut ligne.jours, &b.jours)] {
        gagne(l, x);
    }
    for (l, x) in [(&mut ligne.spectacle, &b.spectacle), (&mut ligne.analytique, &b.analytique)] {
        gagne(l, x);
    }
    for (l, x) in [
        (&mut ligne.cotisations_salariales, &b.cotisations_salariales),
        (&mut ligne.cotisations_patronales, &b.cotisations_patronales),
        (&mut ligne.cout_employeur, &b.cout_employeur),
        (&mut ligne.net_social, &b.net_social),
        (&mut ligne.net_avant_impot, &b.net_avant_impot),
    ] {
        gagne(l, x);
    }
    if !b.lignes.is_empty() {
        ligne.lignes.clone_from(&b.lignes);
    }
    if !b.dates.is_empty() {
        ligne.dates.clone_from(&b.dates);
    }
    gagne(&mut ligne.cotisations, &b.cotisations);
    gagne(&mut ligne.emploi_imprime, &b.emploi_imprime);
    gagne(&mut ligne.bulletin, &b.bulletin);
}

/// Une paie qu'un bulletin seul declare (`declare_par: "bulletin"`) s'efface devant la paie declaree des memes
/// salarie, debut et fin, qu'elle complete (une seule ; plusieurs : celle du meme mois, sinon rien ne se rapproche).
/// Rend le nombre de paies de bulletin laissees a cote de plusieurs paies declarees.
fn rapprocher(par_cle: &mut BTreeMap<(String, String, String), PaieLue>) -> usize {
    let de_bulletin: Vec<(String, String, String)> = par_cle.iter().filter(|(_, x)| declaree_par_bulletin(x)).map(|(k, _)| k.clone()).collect();
    let mut ambigues = 0usize;
    for kb in de_bulletin {
        let b = par_cle[&kb].clone();
        let (salarie, debut, fin) = (b.salarie.clone(), iso(b.paie.debut), iso(b.paie.fin));
        if salarie.is_empty() || salarie == "<a creer>" || debut.is_empty() || fin.is_empty() {
            continue;
        }
        let mut cibles: Vec<(String, String, String)> = par_cle
            .iter()
            .filter(|(_, x)| !declaree_par_bulletin(x) && x.salarie == salarie && iso(x.paie.debut) == debut && iso(x.paie.fin) == fin)
            .map(|(k, _)| k.clone())
            .collect();
        if cibles.len() > 1 {
            cibles.retain(|k| k.0 == kb.0);
        }
        // UN CONTRAT A CHEVAL SUR DEUX MOIS : le bulletin imprime la periode du mois, la DSN le contrat entier
        // (S21.G00.40.001 et .010, `norme-dsn/texte/bloc-S21.G00.40.txt`). Le bulletin complete la paie declaree du meme
        // mois dont le contrat contient sa periode, quand il n'y en a qu'une.
        if cibles.is_empty() {
            cibles = par_cle
                .iter()
                .filter(|(k, x)| {
                    let (d, f) = (iso(x.paie.debut), iso(x.paie.fin));
                    !declaree_par_bulletin(x) && k.0 == kb.0 && x.salarie == salarie && !d.is_empty() && d <= debut && (f.is_empty() || fin <= f)
                })
                .map(|(k, _)| k.clone())
                .collect();
        }
        match cibles.as_slice() {
            [] => {}
            [kd] => {
                if let Some(d) = par_cle.get_mut(kd) {
                    if !porte_un_bulletin(d) {
                        completer_par(&mut d.paie, &b.paie);
                    }
                }
                par_cle.remove(&kb);
            }
            _ => ambigues += 1,
        }
    }
    ambigues
}

/// Un jour en ISO, vide s'il manque.
fn iso(j: Option<socle::date::Jour>) -> String {
    j.map(|j| j.iso()).unwrap_or_default()
}

/// LES ENGAGEMENTS IMPORTES, tires des contrats attestes (`contrats`, une AEM ou un bulletin chacun) et des paies
/// versees (`paies`, une par mois et par contrat). Un engagement, c'est un salarie, un debut et une fin : le contrat
/// atteste qui les porte (plusieurs : celui du meme numero d'objet que la paie), et ses paies, une par mois. Une paie
/// qu'aucun contrat atteste ne porte fait son engagement avec celles des memes salarie, debut, fin et numero de contrat. Les mois du
/// contrat d'un permanent font un seul engagement (plus bas).
/// Son numero est celui que la DSN deposee a declare (S21.G00.40.009), sinon celui que le bulletin imprime ; jamais un
/// numero tire.
fn engagements_importes(contrats: Vec<ContratLu>, paies: Vec<PaieLue>) -> Vec<engagement::importe::Neuf> {
    use engagement::importe::{Neuf, Pieces};
    struct Groupe {
        salarie: String,
        debut: String,
        fin: String,
        contrat: Option<ContratLu>,
        paies: Vec<PaieLue>,
    }
    let objet = |c: &ContratLu| c.contrat.numero_objet.clone().unwrap_or_default();
    // Le contrat d'un permanent : sa nature, payee au mois, est dite, ou reste a dire (`import::nature`).
    let permanent = |c: &ContratLu| c.contrat.nature_a_dire || droit::natures::nature(c.contrat.nature.as_deref()).is_ok_and(|n| n.mensuelle());
    let mut groupes: Vec<Groupe> = Vec::new();
    for c in contrats {
        groupes.push(Groupe { salarie: c.salarie.clone(), debut: c.debut.clone(), fin: c.fin.clone(), contrat: Some(c), paies: Vec::new() });
    }
    for p in paies {
        let (s, d, f, o) = (p.salarie.clone(), iso(p.paie.debut), iso(p.paie.fin), p.paie.numero_objet.clone().unwrap_or_default());
        let mut candidats: Vec<usize> = (0..groupes.len()).filter(|&i| groupes[i].contrat.is_some() && groupes[i].salarie == s && groupes[i].debut == d && groupes[i].fin == f).collect();
        // Le contrat qu'un bulletin atteste porte la periode du mois ; la paie declaree, le contrat entier : la periode
        // du bulletin dans le contrat declare, et l'engagement prend les dates du contrat.
        if candidats.is_empty() && !d.is_empty() && !declaree_par_bulletin(&p) {
            candidats = (0..groupes.len())
                .filter(|&i| {
                    let g = &groupes[i];
                    g.paies.is_empty()
                        && g.contrat.as_ref().is_some_and(|c| c.contrat.atteste_par == Attestation::Bulletin)
                        && g.salarie == s
                        && d <= g.debut
                        && (f.is_empty() || g.fin <= f)
                })
                .collect();
            if let [i] = candidats.as_slice() {
                groupes[*i].debut = d.clone();
                groupes[*i].fin = f.clone();
            }
        }
        // LE MOIS D'UN CONTRAT DE PERMANENT : la paie d'un bulletin va au contrat de permanent du salarie qui couvre sa
        // periode.
        if candidats.is_empty() {
            candidats = (0..groupes.len())
                .filter(|&i| {
                    let g = &groupes[i];
                    g.salarie == s && g.contrat.as_ref().is_some_and(permanent) && g.debut <= d && (g.fin.is_empty() || (!f.is_empty() && f <= g.fin))
                })
                .collect();
        }
        let atteste = match candidats.as_slice() {
            [i] => Some(*i),
            [] => None,
            plusieurs => {
                let memes: Vec<usize> = plusieurs.iter().copied().filter(|&i| groupes[i].contrat.as_ref().is_some_and(|c| objet(c).eq_ignore_ascii_case(&o))).collect();
                (memes.len() == 1).then(|| memes[0])
            }
        };
        let libre = |g: &Groupe| !g.paies.iter().any(|x| x.mois == p.mois);
        let i = match atteste.filter(|&i| libre(&groupes[i])) {
            Some(i) => i,
            None => match (0..groupes.len()).find(|&i| {
                let g = &groupes[i];
                g.contrat.is_none() && g.salarie == s && g.debut == d && g.fin == f && g.paies.first().map(|x| &x.paie.contrat) == Some(&p.paie.contrat) && libre(g)
            }) {
                Some(i) => i,
                None => {
                    groupes.push(Groupe { salarie: s, debut: d, fin: f, contrat: None, paies: Vec::new() });
                    groupes.len() - 1
                }
            },
        };
        groupes[i].paies.push(p);
    }
    // UN CONTRAT DE PERMANENT, UN ENGAGEMENT : les bulletins d'un permanent attestent chacun le mois qu'ils paient, tous
    // du meme contrat, et la DSN le declare chaque mois sous le meme numero (S21.G00.40.009, reporte a l'identique d'une
    // DSN a la suivante : `norme-dsn/texte/chapitre-2-principes-de-constitution.txt`, § 2.1.2). Les mois du contrat d'un
    // permanent (et les paies declarees de ces mois qu'aucun contrat ne porte) font un engagement par numero ; deux
    // numeros, deux contrats, comme la DSN les a declares. Un mois sans numero lu va au contrat declare a partir de lui,
    // sinon au dernier (inference : le numero du contrat en cours ce mois-la).
    let mois_de = |g: &Groupe| -> Vec<String> { g.paies.iter().map(|p| p.mois.clone()).collect() };
    let du_permanent: HashSet<(String, String)> = groupes
        .iter()
        .filter(|g| g.contrat.as_ref().is_some_and(permanent))
        .flat_map(|g| mois_de(g).into_iter().chain(std::iter::once(g.debut.chars().take(7).collect())).map(|m| (g.salarie.clone(), m)).collect::<Vec<_>>())
        .collect();
    let (mut a_fondre, mut autres): (Vec<Groupe>, Vec<Groupe>) = groupes.into_iter().partition(|g| match &g.contrat {
        Some(c) => permanent(c),
        None => !g.paies.is_empty() && mois_de(g).into_iter().all(|m| du_permanent.contains(&(g.salarie.clone(), m))),
    });
    let premier_mois = |g: &Groupe| mois_de(g).into_iter().min().unwrap_or_else(|| g.debut.chars().take(7).collect());
    a_fondre.sort_by_key(|g| (g.salarie.clone(), premier_mois(g)));
    let declare = |g: &Groupe| g.paies.iter().filter(|p| !declaree_par_bulletin(p)).map(|p| p.paie.contrat.clone()).find(|n| !n.is_empty());
    let imprime = |g: &Groupe| g.contrat.as_ref().and_then(|c| c.contrat.numero_contrat.clone()).filter(|n| !n.is_empty());
    let numero_de = |g: &Groupe| declare(g).or_else(|| imprime(g));
    // (salarie, numero, ses groupes)
    let mut paquets: Vec<(String, Option<String>, Vec<Groupe>)> = Vec::new();
    let (numerotes, muets): (Vec<Groupe>, Vec<Groupe>) = a_fondre.into_iter().partition(|g| numero_de(g).is_some());
    for g in numerotes {
        let n = numero_de(&g);
        match paquets.iter_mut().find(|(s, x, _)| *s == g.salarie && *x == n) {
            Some(p) => p.2.push(g),
            None => paquets.push((g.salarie.clone(), n, vec![g])),
        }
    }
    for g in muets {
        let m = premier_mois(&g);
        let siens: Vec<usize> = (0..paquets.len()).filter(|&k| paquets[k].0 == g.salarie).collect();
        let a_partir = siens.iter().copied().filter(|&k| paquets[k].2.iter().map(premier_mois).min().is_some_and(|x| x >= m)).min_by_key(|&k| paquets[k].2.iter().map(premier_mois).min());
        match a_partir.or_else(|| siens.last().copied()) {
            Some(k) => paquets[k].2.push(g),
            None => paquets.push((g.salarie.clone(), None, vec![g])),
        }
    }
    for (salarie, _, mut gs) in paquets {
        gs.sort_by_key(premier_mois);
        let paies: Vec<PaieLue> = gs.iter().flat_map(|g| g.paies.iter().cloned()).collect();
        let declarees: Vec<&PaieLue> = paies.iter().filter(|p| !declaree_par_bulletin(p)).collect();
        let debut = declarees.iter().map(|p| iso(p.paie.debut)).find(|d| !d.is_empty()).unwrap_or_else(|| gs.iter().map(|g| g.debut.clone()).min().unwrap_or_default());
        // Sa fin est celle que la DSN declare ; un bulletin ne paie que son mois, il ne dit pas quand le contrat finit.
        let fin = declarees.last().map(|p| iso(p.paie.fin)).unwrap_or_default();
        // Le contrat entier : ce qu'un bulletin atteste de son seul mois reste a la paie de ce mois.
        let mut contrat = gs.iter().rev().find_map(|g| g.contrat.clone());
        if let Some(c) = contrat.as_mut().filter(|_| gs.len() > 1) {
            let a = &mut c.contrat;
            (a.fichier, a.page, a.numero_bulletin) = (None, None, None);
            (a.heures, a.cachets, a.jours, a.brut) = (None, None, None, None);
        }
        autres.push(Groupe { salarie, debut, fin, contrat, paies });
    }
    let mut out: Vec<Neuf> = autres
        .into_iter()
        .map(|g| {
            let dates = g.contrat.iter().filter_map(|c| c.contrat.importe_le.clone()).chain(g.paies.iter().filter_map(|p| p.paie.importe_le.clone()));
            let le = dates.filter(|x| !x.is_empty()).min().unwrap_or_default();
            let numero = declare(&g).or_else(|| imprime(&g));
            let paies = g.paies.iter().map(|p| (p.mois.clone(), p.paie.clone())).collect();
            let pieces = Pieces { le, contrat: g.contrat.map(|c| c.contrat), paies };
            let mut e = Map::new();
            e.insert("salarie".into(), json!(g.salarie));
            e.insert("debut".into(), json!(g.debut));
            if !g.fin.is_empty() {
                e.insert("fin".into(), json!(g.fin));
            }
            if let Some(n) = numero {
                e.insert("numero".into(), json!(n));
            }
            Neuf { champs: e, pieces }
        })
        .collect();
    out.sort_by(|a, b| {
        let k = |e: &Neuf| ["debut", "fin", "salarie"].map(|c| e.champs.get(c).and_then(Value::as_str).unwrap_or("").to_string());
        k(a).cmp(&k(b))
    });
    out
}

// ------------------------------------------------------------------ l'ecriture

/// Ce qu'une ecriture a fait.
struct Ecrit {
    fichiers: Vec<String>,
    contrats_aem: usize,
    contrats_bulletins: usize,
    contrats_certificats: usize,
    contrats_aem_pdf: usize,
    paies: usize,
    restent_a_creer: usize,
    /// Les fiches existantes completees.
    completees: usize,
    avertissements: Vec<String>,
}

impl Ecrit {
    fn rien(&self) -> bool {
        self.restent_a_creer == 0 && self.completees == 0 && self.contrats_aem == 0 && self.contrats_bulletins == 0 && self.contrats_certificats == 0 && self.contrats_aem_pdf == 0 && self.paies == 0
    }
}

/// Ecrit ce que l'apercu `imp` annonce : les fiches (et leur place dans `reponse`), puis les engagements importes
/// (`engagements_importes` : les contrats des AEM et des bulletins, les paies versees).
fn ecrire(ctx: &Contexte, imp: &crate::routes::import::Apercu, reponse: &mut Importation, note: &Value) -> R<Ecrit> {
    let aujourdhui = ctx.date;
    // Les fiches, comme celles de l'ecran (`forcer` : une fiche importee avec des anomalies est ecrite quand meme).
    let mut ecrits: BTreeSet<String> = BTreeSet::new();
    let mut restent_a_creer = 0usize;
    for a in &imp.a_creer {
        let e = crate::routes::fiches::ecrire_fiche(ctx, a.nom, None, &Value::Object(a.donnees.clone()), crate::routes::fiches::Forcer::Tout, &Value::Null, note)?;
        let emplacement = match (a.nom, a.rang) {
            ("entreprise", _) => reponse.entreprise.as_mut(),
            ("salarie", Some(i)) => reponse.salaries.get_mut(i),
            (_, Some(i)) => reponse.spectacles.get_mut(i),
            _ => None,
        };
        let Some(emplacement) = emplacement else {
            restent_a_creer += 1;
            continue;
        };
        emplacement.cle = e.cle;
        emplacement.etat = EtatFiche::Creee;
        let mut anomalies = e.anomalies;
        crate::routes::fiches::avec_libelles(a.nom, &mut anomalies)?;
        emplacement.anomalies = anomalies.iter().map(brut).collect();
        let fichier = match a.nom {
            "entreprise" => "entreprise.json",
            "salarie" => "salaries.json",
            _ => "spectacles.json",
        };
        ecrits.insert(ctx.racine.join(socle::fichiers::CONFIG).join(fichier).display().to_string());
    }
    // Les fiches qui existent : ce qui y est vide ou « A COMPLETER » (`import::completer`), rien d'autre.
    let mut completees = 0usize;
    for a in &imp.a_completer {
        let faits = crate::routes::fiches::completer_fiche(ctx, a.nom, a.cle.as_deref(), &a.champs)?;
        if faits.is_empty() {
            continue;
        }
        let emplacement = match a.nom {
            "entreprise" => reponse.entreprise.as_mut(),
            "salarie" => reponse.salaries.iter_mut().find(|e| e.cle.as_deref() == a.cle.as_deref()),
            _ => reponse.spectacles.iter_mut().find(|e| e.cle.as_deref() == a.cle.as_deref()),
        };
        if let Some(e) = emplacement {
            e.etat = EtatFiche::Completee;
            e.completer.retain(|c| faits.contains_key(&c.champ));
        }
        let fichier = match a.nom {
            "entreprise" => "entreprise.json",
            "salarie" => "salaries.json",
            _ => "spectacles.json",
        };
        ecrits.insert(ctx.racine.join(socle::fichiers::CONFIG).join(fichier).display().to_string());
        completees += 1;
    }
    crate::routes::fiches::dater_licences_suivantes(ctx, &imp.licences, note)?;
    let mut fichiers: Vec<String> = ecrits.into_iter().collect();
    let mut avertissements: Vec<String> = Vec::new();

    // Les engagements importes, avec les fiches telles qu'elles sont MAINTENANT.
    let frais = Contexte::nouveau(&ctx.racine, None).map_err(Echec::from)?;
    let mut par_nir: HashMap<String, String> = HashMap::new();
    for (cle, s) in &frais.salaries {
        if let Some(n) = s.nir.dite().filter(|n| !n.is_empty()) {
            par_nir.insert(n.to_string(), cle.to_string());
        }
    }
    // CE QUE L'IMPORT A DEJA ECRIT, relu de ses engagements importes : les contrats attestes, les paies versees.
    let importes_avant = engagement::importe::tous(&ctx.racine).map_err(Echec::from)?;
    let mut engagements: Vec<ContratLu> = engagement::importe::contrats_lus(&ctx.racine).map_err(Echec::from)?;
    let le = aujourdhui.iso();
    // LES CONTRATS QUE L'IMPORT RETIENT (`import::attestes` en a decide), rattaches par NIR a la fiche qui existe
    // maintenant, comptes par bloc du compte rendu.
    // Ce que le registre change aux contrats de l'historique : reduit, ou retire.
    for (avant, apres) in &imp.contrats_changes {
        if let Some(i) = engagements.iter().position(|e| e == avant) {
            match apres {
                Some(l) => engagements[i] = l.clone(),
                None => {
                    engagements.remove(i);
                }
            }
        }
    }
    let mut comptes: HashMap<&str, usize> = HashMap::new();
    for (nir, ligne, bloc) in &imp.contrats_attestes {
        let Some(cle) = par_nir.get(nir) else { continue };
        let mut e = ligne.clone();
        e.salarie = cle.clone();
        e.contrat.importe_le = Some(le.clone());
        engagements.push(e);
        *comptes.entry(bloc).or_default() += 1;
    }
    let compte = |bloc: &str| comptes.get(bloc).copied().unwrap_or(0);
    let (contrats_aem, contrats_bulletins, contrats_certificats, contrats_aem_pdf) =
        (compte("declarations"), compte("bulletins"), compte("certificats"), compte("aem_pdf"));

    // LES PAIES : on remplace ligne a ligne par la cle (mois, salarie, contrat). Une ligne de DSN qui arrive sur une ligne
    // completee par un bulletin garde ce que le bulletin a ecrit ; une paie de bulletin seul s'efface devant la paie
    // declaree qu'elle complete (`rapprocher`).
    let mut par_cle: BTreeMap<(String, String, String), PaieLue> = BTreeMap::new();
    for x in crate::routes::import::paies_historique(ctx)? {
        par_cle.insert(x.cle(), x);
    }
    let avant = par_cle.clone();
    for x in &imp.paies {
        let Some(cle) = x.paie.nir.as_ref().and_then(|n| par_nir.get(n)) else { continue };
        let mut x = x.clone();
        x.salarie = cle.clone();
        x.paie.importe_le = Some(le.clone());
        let k = x.cle();
        if let Some(a) = par_cle.get(&k).filter(|a| porte_un_bulletin(a) && !porte_un_bulletin(&x)) {
            completer_par(&mut x.paie, &a.paie.clone());
        }
        par_cle.insert(k, x);
    }
    let ambigues = rapprocher(&mut par_cle);
    if ambigues > 0 {
        avertissements.push(format!(
            "{} paie(s) écrite(s) d'un bulletin seul ont les dates de plusieurs paies déclarées en DSN : aucune n'est rapprochée, les engagements importés restent côte à côte, à regarder.",
            ambigues
        ));
    }
    let mut neuves = avant.keys().filter(|k| !par_cle.contains_key(*k)).count();
    for (k, v) in &par_cle {
        if avant.get(k).map(PaieLue::sans_date_d_import) != Some(v.sans_date_d_import()) {
            neuves += 1;
        }
    }
    if contrats_aem + contrats_bulletins + contrats_certificats + contrats_aem_pdf + neuves > 0 || importes_avant.is_empty() {
        let importes = engagements_importes(engagements, par_cle.into_values().collect());
        for p in engagement::importe::ecrire_tous(&ctx.racine, importes).map_err(Echec::from)? {
            fichiers.push(p.display().to_string());
        }
    }
    Ok(Ecrit { fichiers, contrats_aem, contrats_bulletins, contrats_certificats, contrats_aem_pdf, paies: neuves, restent_a_creer, completees, avertissements })
}

// ------------------------------------------------------------------ la route

/// LES MOIS IMPORTES SONT DECLARES S'ILS L'ONT ETE : les depots DSN que les retours deja releves attestent
/// entrent au journal avec l'historique, pour qu'un mois tenu avant ne se dise pas « en retard »
/// (`reprise_journal::reprendre_les_depots`). Rend le nombre de depots repris.
fn depots_attestes_au_journal(ctx: &Contexte) -> R<usize> {
    let qui = crate::routes::INTERFACE.to_string();
    crate::compagnie::incidents::noter_ceux_des_retours(&ctx.racine, envois::reprise_journal::reprendre_les_depots(ctx, &qui, &crate::routes::maintenant()))
}


/// LES ECARTS DE CALCUL PARTENT A CHAQUE IMPORT (`import::ecarts`), lus avec les fiches telles que l'import les a
/// ecrites ; rien ne s'en dit a l'ecran.
fn rapporter_les_ecarts(ctx: &Contexte) {
    if let Ok(frais) = Contexte::nouveau(&ctx.racine, Some(ctx.date)) {
        crate::import::ecarts::rapporter(&frais);
    }
}
fn avertissement_differents(differents: &[String], ou: &str) -> String {
    format!(
        "{} fichier(s) de {} portent le nom d'un fichier déjà copié avec un autre contenu : la copie est gardée telle quelle, à comparer à la main ({}).",
        differents.len(),
        ou,
        differents.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
    )
}

/// Ou l'import du dossier copie ce qu'il reprend.
fn dossier_bulletins(ctx: &Contexte) -> PathBuf {
    ctx.racine.join("historique").join("bulletins")
}

/// La copie de ce que le dossier lu reprend, dans `historique/bulletins/`, sous le nom de sa copie. RIEN NE S'ECRASE :
/// un fichier deja copie et identique est laisse ; un autre contenu sous le meme nom est laisse aussi, et SIGNALE (il
/// n'est pas relu). Rend le compte, et les noms a relire.
fn copier_repris(ctx: &Contexte, lu: &crate::import::bulletins::DossierLu) -> R<(Copie, Vec<String>)> {
    let dossier = dossier_bulletins(ctx);
    let (mut copies, mut identiques) = (0usize, 0usize);
    let (mut differents, mut noms): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for (i, nom) in &lu.copies {
        let source = &lu.tri.fichiers[*i].disque;
        let cible = dossier.join(nom);
        if cible.exists() {
            let a = std::fs::read(source).map_err(|e| en_os(&e, source))?;
            let b = std::fs::read(&cible).map_err(|e| en_os(&e, &cible))?;
            if a == b {
                identiques += 1;
                noms.push(nom.clone());
            } else {
                differents.push(nom.clone());
            }
            continue;
        }
        std::fs::create_dir_all(&dossier).map_err(|e| en_os(&e, &dossier))?;
        std::fs::copy(source, &cible).map_err(|e| en_os(&e, source))?;
        if let Ok(m) = std::fs::metadata(source).and_then(|m| m.modified()) {
            if let Ok(f) = std::fs::OpenOptions::new().write(true).open(&cible) {
                let _ = f.set_modified(m);
            }
        }
        copies += 1;
        noms.push(nom.clone());
    }
    Ok((Copie { copies, identiques, differents, dossier: dossier.display().to_string() }, noms))
}

/// `importer` : `{"dossier": ...}`, la boite aux lettres `import/` (vide ou absent : celle de la compagnie) -- la copie
/// de ce qui est repris, l'apercu relu dans la copie, l'ecriture.
pub(crate) fn importer(ctx: &Contexte, charge: &Value) -> R<Importation> {
    let aujourdhui = ctx.date;
    let dossier = crate::routes::import::dossier_import(ctx, charge)?;
    // 1. Le dossier tel qu'il est : son compte rendu (pour l'ecran), et la copie de ce qu'il reprend.
    let (tri, copie, noms, deposes) = crate::import::bulletins::lire_dossier(ctx, &dossier, &HashMap::new(), |mut lu| {
        let (copie, noms) = copier_repris(ctx, &lu)?;
        crate::import::incident::noter_les_non_lus(ctx, &lu);
        let tri = std::mem::take(&mut lu.ap.reponse.tri);
        // La copie se relit sous le nom du fichier depose.
        let deposes: HashMap<String, String> = lu.copies.iter().map(|(i, nom)| (nom.clone(), lu.tri.fichiers[*i].chemin.clone())).collect();
        Ok((tri, copie, noms, deposes))
    })?;

    // 2. D'ABORD LA COPIE, ENSUITE LA LECTURE : l'apercu relu dans les copies de ce dossier (et d'elles seules), prises
    // dans `historique/bulletins/` ; c'est lui qui s'ecrit.
    let copies = dossier_bulletins(ctx);
    let relu = PathBuf::from(format!("{}-copie", crate::routes::import::dossier_de_travail().display()));
    let relire = || -> R<crate::routes::import::Apercu> {
        std::fs::create_dir_all(&relu).map_err(|e| en_os(&e, &relu))?;
        for nom in &noms {
            std::fs::copy(copies.join(nom), relu.join(nom)).map_err(|e| en_os(&e, &copies.join(nom)))?;
        }
        let frais_avant = Contexte::nouveau(&ctx.racine, None).map_err(Echec::from)?;
        crate::import::bulletins::lire_dossier(&frais_avant, &relu, &deposes, |lu| Ok(lu.ap))
    };
    let rendu = relire();
    let _ = std::fs::remove_dir_all(&relu);
    let mut imp = rendu?;

    let mut reponse = std::mem::take(&mut imp.reponse);
    // Le compte rendu est celui du dossier lu (`tri`) : ses chemins, ses fichiers non repris (la relecture ne les voit pas).
    if !copie.differents.is_empty() {
        reponse.avertissements.push(avertissement_differents(&copie.differents, "ce dossier"));
    }
    if let Some(alerte) = alerte_apres_import(ctx)? {
        reponse.avertissements.push(alerte);
    }
    reponse.tri = tri;
    reponse.source = dossier.display().to_string();
    let note = json!(format!("Importe du dossier import/ le {}", aujourdhui.jj_mm_aaaa()));
    let e = ecrire(ctx, &imp, &mut reponse, &note)?;
    reponse.avertissements.extend(e.avertissements.iter().cloned());
    reponse.contrats_nouveaux = e.contrats_aem;
    reponse.bulletins.contrats_nouveaux = e.contrats_bulletins;
    if let Some(c) = reponse.certificats.as_mut() {
        c.contrats_nouveaux = e.contrats_certificats;
    }
    if let Some(c) = reponse.aem_pdf.as_mut() {
        c.contrats_nouveaux = e.contrats_aem_pdf;
    }
    reponse.rien_a_faire = e.rien();
    reponse.ecrit = true;
    reponse.fichiers = e.fichiers;
    reponse.copie = Some(copie);
    reponse.depots_repris = Some(depots_attestes_au_journal(ctx)?);
    // Ce que les DSN reprises disent de la configuration hors de la fiche (le taux AT/MP, les organismes), lu avec les
    // fiches telles que l'import les a ecrites.
    if let Ok(frais) = Contexte::nouveau(&ctx.racine, Some(ctx.date)) {
        let mut faits = crate::import::parametrage::parametrer(&frais);
        // La nature des contrats repris des permanents, quand leur DSN reprise la dit.
        faits.extend(crate::import::nature::dire_depuis_les_dsn(&ctx.racine, &ctx.date));
        reponse.parametrage = faits;
    }
    // Ce que l'ecran dit de l'import fait, et sa couleur : l'ecriture a eu lieu, c'est fait (ce qui reste a dire a ses
    // propres lignes : la nature des contrats repris, « Pour demarrer »).
    let ecrite = |f: &crate::import::reponse::FicheImportee| matches!(f.etat, EtatFiche::Creee | EtatFiche::Completee);
    let de = |l: &[crate::import::reponse::FicheImportee]| l.iter().filter(|f| ecrite(f)).count();
    // Les fiches ecrites, par sorte : l'entreprise, les salaries, les spectacles.
    let mut fiches: Vec<String> = Vec::new();
    if reponse.entreprise.as_ref().is_some_and(ecrite) {
        fiches.push("l'entreprise".into());
    }
    if de(&reponse.salaries) > 0 {
        fiches.push(crate::routes::compte(de(&reponse.salaries), "salarié", "salariés"));
    }
    if de(&reponse.spectacles) > 0 {
        fiches.push(crate::routes::compte(de(&reponse.spectacles), "spectacle", "spectacles"));
    }
    // UN SPECTACLE SANS TITRE (`completer::A_COMPLETER`) : les bulletins en proposent un a valider
    // (`demarrage::TITRE_PROPOSE`), ou les pieces ne portent que son numero d'objet ; le contrat imprime le titre, la phrase
    // le dit. Lu dans les fiches telles que l'import les a ecrites.
    let (mut proposes, mut sans_titre) = (0, 0);
    if let Ok(frais) = Contexte::nouveau(&ctx.racine, Some(ctx.date)) {
        for sp in frais.spectacles.values().filter(|sp| sp.get("actif") != Some(&json!(false))) {
            if sp.get("titre").map(valeur_en_texte).is_some_and(|t| t.trim().eq_ignore_ascii_case(crate::import::completer::A_COMPLETER)) {
                if sp.get(declarations::demarrage::TITRE_PROPOSE).is_some_and(vrai) {
                    proposes += 1;
                } else {
                    sans_titre += 1;
                }
            }
        }
    }
    let titres = match (proposes, sans_titre) {
        (0, 0) => String::new(),
        (p, 0) => format!(" Titre à valider pour {} spectacle(s), proposé d'après les bulletins (Pour démarrer).", p),
        (0, s) => format!(" Titre à dire pour {} spectacle(s) : les pièces ne portent que le numéro d'objet (Pour démarrer).", s),
        (p, s) => format!(" Titre à dire pour {} spectacle(s), dont {} proposé(s) d'après les bulletins (Pour démarrer).", p + s, p),
    };
    reponse.dire();
    let b = &reponse.bulletins;
    reponse.phrase = format!(
        "Repris : {}, {} ; fiches écrites : {}.{}",
        crate::routes::compte(reponse.contrats_nouveaux + b.contrats_nouveaux, "contrat", "contrats"),
        crate::routes::compte(b.paies_nouvelles + b.paies_completees, "bulletin", "bulletins"),
        if fiches.is_empty() { "aucune".to_string() } else { fiches.join(", ") },
        titres
    );
    reponse.fait();
    rapporter_les_ecarts(ctx);
    Ok(reponse)
}
