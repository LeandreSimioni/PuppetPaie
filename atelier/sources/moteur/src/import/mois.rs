//! UN MOIS IMPORTE SE LIT PAR SES ENGAGEMENTS IMPORTES (`engagement::importe`) : un mois tenu avant ce logiciel n'a
//! que des engagements que l'import a ecrits, valides et partis, qui se lisent et ne se modifient pas. Ce module le rend
//! tel que la page Paie le montre : une CARTE PAR ENGAGEMENT, la ligne fermee « importé », avec ses cases (salarie,
//! spectacle, debut et fin, jours travailles, cachets et heures, montants, etats), remplies de ce que les pieces
//! importees portent, et rapprochees de ce que le moteur sait par ailleurs -- la fiche du salarie (son nom), celle du
//! spectacle (son titre), le journal des depots et les certificats DPAE (`retours/dpae/`). Aucune carte n'a de geste.
//!
//! RIEN NE SE DEVINE (regle 1) : une case qu'aucune piece ne porte reste vide, et la carte DIT pourquoi (`manque` : quel
//! champ, et ce qui l'aurait donne). Rien ne s'ecrit ici : c'est une lecture.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use engagement::importe::{Attestation, ContratAtteste, ContratLu, DatesTravaillees, Declaration, Importe, PaieLue, PaieVersee};
use envois::etats;
use socle::valeur::valeur_en_texte;
use socle::echec::{Echec, R};
use socle::fichiers::vrai;
use crate::import::completer::A_COMPLETER;

fn texte(v: &Value, k: &str) -> String {
    v.get(k).filter(|x| vrai(x)).map(valeur_en_texte).map(|t| t.trim().to_string()).unwrap_or_default()
}

/// Les engagements importes qui touchent `mois`.
fn importes_du_mois(racine: &Path, mois: &str) -> R<Vec<Importe>> {
    engagement::importe::du_mois(racine, mois).map_err(Echec::from)
}

/// LES BULLETINS REPRIS DE `mois` : la copie que l'import garde de chaque fichier lu (`historique/bulletins/`, le nom que
/// porte la paie importee), un par fichier, avec le salarie qu'il paie (`[{chemin, salarie}]`, chemin relatif a la
/// compagnie). Un fichier qui manque ne se liste pas. Ne leve jamais.
pub(crate) fn bulletins_repris(racine: &Path, mois: &str) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for p in engagement::importe::paies_lues(racine).unwrap_or_default() {
        if p.mois != mois {
            continue;
        }
        let fichier = p.paie.bulletin.as_ref().map(|b| b.fichier.trim().to_string()).unwrap_or_default();
        if fichier.is_empty() || fichier.contains(['/', '\\']) || fichier.contains("..") {
            continue;
        }
        let chemin = format!("historique/bulletins/{}", fichier);
        if !racine.join(&chemin).is_file() || out.iter().any(|x| x["chemin"] == json!(chemin)) {
            continue;
        }
        out.push(json!({"chemin": chemin, "salarie": p.salarie}));
    }
    out
}

/// Le mois est-il tenu avant ce logiciel : aucun engagement de la saisie ne le touche, et un engagement importe le touche.
pub(crate) fn importe(racine: &Path, mois: &str) -> R<bool> {
    if engagement::stock::a_une_saisie(racine, mois).map_err(Echec::from)? {
        return Ok(false);
    }
    Ok(!importes_du_mois(racine, mois)?.is_empty())
}

/// Les mois que les engagements importes touchent, qu'aucun engagement de la saisie ne touche.
pub(crate) fn mois_importes(racine: &Path) -> R<Vec<String>> {
    let mois: BTreeSet<String> = engagement::importe::tous(racine).map_err(Echec::from)?.iter().flat_map(Importe::mois).collect();
    Ok(mois.into_iter().filter(|m| !engagement::stock::a_une_saisie(racine, m).unwrap_or(false)).collect())
}

/// LA DSN QUE L'IMPORT A LUE POUR `mois` : le fichier de l'ancien logiciel (`engagement::importe::dsn_deposee`), ou
/// `None`, que la saisie touche aussi le mois ou non. Rend `{"importee": true, "etablie_le", "fichier"}` -- la date est
/// celle que le fichier porte (S20.G00.05.007, sa constitution), pas celle d'un depot : aucun bilan de la plate-forme
/// n'est lu pour lui.
pub(crate) fn dsn_importee(racine: &Path, mois: &str) -> R<Option<Value>> {
    Ok(engagement::importe::dsn_deposee(racine, mois).map_err(Echec::from)?.map(|x| {
        let dit = |v: Option<String>| v.map(|t| t.trim().to_string()).unwrap_or_default();
        json!({"importee": true, "passee": false, "refusee": false, "etablie_le": dit(x.paie.depose_le), "fichier": dit(x.paie.fichier)})
    }))
}

/// LA DSN D'UN MOIS DONT L'ANCIEN LOGICIEL A DEPOSE LA DSN, que la page Paie et la boite d'ouverture disent avec les
/// memes mots : `None` si l'ancien logiciel n'a pas depose la DSN de `mois`. Sinon ce que la saisie ajoute a ce mois et
/// qui n'est pas encore declare (`engagements`, en saisie ou valides) se declare dans la DSN de `porteur`, la premiere
/// DSN d'apres qui n'est pas partie, comme un contrat ajoute apres la DSN de son mois
/// (`produire_regularisations::regularisations_pour` : cahier technique DSN 2026.1, § 2.1.2.1,
/// `atelier/reference/norme-dsn/texte/chapitre-2-principes-de-constitution.txt`). La DSN de l'ancien logiciel ne se
/// refait pas ici (regle 4).
pub(crate) fn dsn_de_l_ancien_logiciel(ctx: &Contexte, mois: &str) -> R<Option<DsnDeLAncienLogiciel>> {
    if engagement::importe::dsn_deposee(&ctx.racine, mois).map_err(Echec::from)?.is_none() {
        return Ok(None);
    }
    let porteur = crate::regularisation::dsn_qui_porte(ctx, mois)?;
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let mut n = 0;
    for e in engagement::etats::du_mois(ctx, mois, &departs)? {
        let a_declarer = match e.valide() {
            Some(v) => engagement::etats::correction_due(ctx, v, &departs, mois, &porteur).map_err(Echec::from)?.is_some(),
            None => true,
        };
        n += usize::from(a_declarer);
    }
    let lib = socle::mois::Mois::en_clair(&porteur);
    let phrase = match n {
        0 => "DSN déposée par l'ancien logiciel.".to_string(),
        1 => format!("DSN déposée par l'ancien logiciel ; cet engagement se déclare dans la DSN de {}.", lib),
        _ => format!("DSN déposée par l'ancien logiciel ; ces {} engagements se déclarent dans la DSN de {}.", n, lib),
    };
    Ok(Some(DsnDeLAncienLogiciel { porteur, engagements: n, phrase }))
}

/// La DSN d'un mois deposee par l'ancien logiciel (`dsn_de_l_ancien_logiciel`) : le mois de la DSN qui porte ce que la
/// saisie y ajoute, combien d'engagements s'y declarent, et la phrase qui le dit.
pub(crate) struct DsnDeLAncienLogiciel {
    pub(crate) porteur: String,
    pub(crate) engagements: usize,
    pub(crate) phrase: String,
}

/// LA DSN MENSUELLE D'UN MOIS DE L'ANCIEN LOGICIEL NE SE FABRIQUE PAS ICI (regle 4 ; un mois importe ne recoit pas
/// d'engagement neuf) : celle que l'import a lue (`dsn_de_l_ancien_logiciel`), ou, sans elle, celle d'un mois tenu avant
/// ce logiciel (`importe` : `compagnie::etat_dsn` le dit « aucune trace de sa DSN ») -- l'ancien logiciel a calcule ces
/// paies, sa DSN est la sienne, et elle se garde en l'important (la route `importer`). Le refus que `fabriquer_dsn`
/// oppose avant d'ecrire quoi que ce soit, le seul.
pub(crate) fn refuser_la_dsn_de_l_ancien_logiciel(ctx: &Contexte, mois: &str) -> R<()> {
    let phrase = match dsn_de_l_ancien_logiciel(ctx, mois)? {
        Some(d) => format!("{} Elle ne se refait pas ici.", d.phrase),
        None if importe(&ctx.racine, mois)? => "mois tenu avant PuppetPaie, sa DSN est celle de l'ancien logiciel et ne se fait pas ici. Pour la garder, mettez son fichier dans le dossier d'import, puis « Importer » (Configuration › Reprendre l'historique).".to_string(),
        None => return Ok(()),
    };
    Err(socle::echec::erreur("DsnDeLAncienLogiciel", format!("{} : {} Rien n'est écrit.", socle::mois::Mois::en_clair(mois), phrase)))
}

// ------------------------------------------------------------------ les cartes

/// Le nom d'un salarié à l'écran, ou `None` sans fiche.
fn nom_du_salarie(ctx: &Contexte, cle: &str) -> Option<String> {
    ctx.salaries.get(cle).and_then(|s| s.nomme())
}

/// La fiche du spectacle de ce numero d'objet : `(cle, titre)` ; le titre est vide s'il est « A COMPLETER ».
fn spectacle_de(ctx: &Contexte, numero: &str) -> Option<(String, String)> {
    if numero.is_empty() {
        return None;
    }
    ctx.spectacles.iter().filter(|(k, _)| !k.starts_with('_')).find(|(_, s)| texte(s, "numero_objet").eq_ignore_ascii_case(numero)).map(|(k, s)| {
        let t = texte(s, "titre");
        (k.clone(), if t.eq_ignore_ascii_case(A_COMPLETER) { String::new() } else { t })
    })
}

/// LA PERIODE D'UN CONTRAT IMPORTE DANS `mois`, en clair, quand le bulletin ne dit pas ses jours : « 20–27 » pour un
/// contrat qui commence et finit dans le mois (le 1er quand il a commence avant), « 20/11 – 03/12 » pour un contrat a
/// cheval sur un autre mois (« 01/09/2022 – 31/12 » quand il a commence une autre annee), « … » pour une borne que
/// l'import n'a pas lue.
fn periode_en_clair(debut: &str, fin: &str, mois: &str) -> String {
    let (d, f) = (socle::date::Jour::lire(debut).ok(), socle::date::Jour::lire(fin).ok());
    let dans = |j: &socle::date::Jour| j.iso().starts_with(mois);
    let dehors = d.as_ref().is_some_and(|j| !dans(j)) || f.as_ref().is_some_and(|j| !dans(j));
    let borne = |j: Option<&socle::date::Jour>, defaut: &str| match j {
        Some(j) if dehors && !j.iso().starts_with(mois.get(..4).unwrap_or(mois)) => j.jj_mm_aaaa(),
        Some(j) if dehors => j.jj_mm(),
        Some(j) => j.jour.to_string(),
        None => defaut.to_string(),
    };
    let (a, b) = (borne(d.as_ref(), if dehors { "…" } else { "1" }), borne(f.as_ref(), "…"));
    match (a == b, dehors) {
        (true, _) => a,
        (false, true) => format!("{} – {}", a, b),
        (false, false) => format!("{}–{}", a, b),
    }
}

/// Les jours des dates travaillees d'un bulletin, dans `mois`.
fn jours_du_mois(dates: &[DatesTravaillees], mois: &str) -> Vec<String> {
    let mut out = Vec::new();
    for d in dates {
        let (mut j, b) = (d.debut, d.fin);
        while j <= b {
            let iso = j.iso();
            if iso.starts_with(mois) && !out.contains(&iso) {
                out.push(iso);
            }
            match j.plus_jours(1) {
                Ok(n) => j = n,
                Err(_) => break,
            }
        }
    }
    out.sort();
    out
}

fn manque(champ: &str, libelle: &str, phrase: String) -> Value {
    json!({"champ": champ, "libelle": libelle, "phrase": phrase, "dit": "non lu"})
}


/// L'etat de la DPAE d'une embauche tenue avant : le journal (reel accepte, par le numero du contrat), puis les certificats
/// de `retours/dpae/` (par le salarie et le jour d'embauche qu'ils attestent).
fn dpae_de(journal: &std::collections::HashMap<etats::Paire, String>, certifiees: &[Value], salarie: &str, numero: &str, debut: &str) -> Value {
    if let Some(q) = journal.get(&(salarie.to_string(), numero.to_string())) {
        return json!({"couleur": "vert", "courte": "DPAE faite", "phrase": format!("DPAE déposée le {}", etats::quand_lisible(q))});
    }
    let certifiee = certifiees.iter().find(|c| {
        c.get("embauches").and_then(Value::as_array).is_some_and(|l| l.iter().any(|e| texte(e, "salarie") == salarie && texte(e, "debut") == debut))
    });
    if let Some(c) = certifiee {
        return json!({"couleur": "vert", "courte": "DPAE faite", "phrase": format!("DPAE certifiée par l'Urssaf ({})", etats::quand_lisible(&texte(c, "quand")))});
    }
    json!({"couleur": "gris", "courte": "avant PuppetPaie", "phrase": "DPAE faite avant PuppetPaie : aucun certificat de l'Urssaf n'est relevé pour cette embauche"})
}

/// Une carte : une paie importee (et le contrat atteste qui va avec), ou un contrat atteste seul.
fn carte(ctx: &Contexte, mois: &str, paie: Option<&PaieLue>, contrat: Option<&ContratLu>, dpae: Value) -> Value {
    let p = paie.map(|x| &x.paie);
    let c = contrat.map(|x| &x.contrat);
    let plein = |t: String| Some(t.trim().to_string()).filter(|t| !t.is_empty());
    let salarie = paie.and_then(|x| plein(x.salarie.clone())).or_else(|| contrat.and_then(|x| plein(x.salarie.clone()))).unwrap_or_default();
    let jour = |j: Option<socle::date::Jour>| j.map(|j| j.iso());
    let debut = p.and_then(|p| jour(p.debut)).or_else(|| contrat.and_then(|x| plein(x.debut.clone()))).unwrap_or_default();
    let fin = p.and_then(|p| jour(p.fin)).or_else(|| contrat.and_then(|x| plein(x.fin.clone()))).unwrap_or_default();
    let numero = p.and_then(|p| p.numero_objet.clone()).or_else(|| c.and_then(|c| c.numero_objet.clone())).and_then(plein).unwrap_or_default();
    let spectacle = spectacle_de(ctx, &numero);
    let titre_bulletin = p.and_then(|p| p.spectacle.clone()).and_then(plein).unwrap_or_default();
    let titre = match &spectacle {
        Some((_, t)) if !t.is_empty() => t.clone(),
        _ => titre_bulletin.clone(),
    };
    let nom = nom_du_salarie(ctx, &salarie);
    let dates = jours_du_mois(p.map_or(&[][..], |p| &p.dates[..]), mois);
    // Ce que la piece du mois atteste de lui (la paie d'un mois d'un permanent), sinon le contrat.
    let nombre = |de_la_paie: fn(&PaieVersee) -> Option<i64>, du_contrat: fn(&ContratAtteste) -> Option<i64>| p.and_then(de_la_paie).or_else(|| c.and_then(du_contrat));
    let (cachets, heures, jours) = (nombre(|p| p.cachets, |c| c.cachets), nombre(|p| p.heures, |c| c.heures), nombre(|p| p.jours, |c| c.jours));
    let bulletin = p.and_then(|p| p.bulletin.clone());
    let declare_par = match p.map(|p| p.declare_par) {
        Some(Declaration::Dsn) => "DSN",
        Some(Declaration::Bulletin) => "bulletin",
        None => "",
    };
    let par_aem = c.is_some_and(|c| c.atteste_par == Attestation::Aem);

    // Les montants, et d'ou chacun vient : le bulletin fait foi sur ceux qu'il imprime (`import::bulletins::completer`).
    let mut montants = Map::new();
    let mut sources = Map::new();
    let lus: [(&str, fn(&PaieVersee) -> Option<socle::num::Decimal>); 9] = [
        ("brut", |p| p.brut),
        ("net_imposable", |p| p.net_imposable),
        ("net_a_payer", |p| p.net_a_payer),
        ("pas", |p| p.pas),
        ("net_verse", |p| p.net_verse),
        ("cotisations_salariales", |p| p.cotisations_salariales),
        ("cotisations_patronales", |p| p.cotisations_patronales),
        ("cout_employeur", |p| p.cout_employeur),
        ("net_avant_impot", |p| p.net_avant_impot),
    ];
    for (k, lu) in lus {
        let mut v = p.and_then(lu).map(|d| d.to_string()).unwrap_or_default();
        let mut source = if bulletin.is_some() && !matches!(k, "net_verse") { "bulletin" } else { declare_par };
        if v.is_empty() && k == "brut" && paie.is_none() {
            v = c.and_then(|c| c.brut).map(|d| d.to_string()).unwrap_or_default();
            source = "AEM";
        }
        if !v.is_empty() {
            montants.insert(k.into(), json!(v));
            sources.insert(k.into(), json!(if source == "DSN" { "DSN" } else if source.is_empty() { "historique" } else { source }));
        }
    }

    // CE QUI MANQUE, case par case, et ce qui l'aurait donne.
    let mut manques: Vec<Value> = Vec::new();
    if salarie.is_empty() {
        manques.push(manque("salarie", "salarié", "aucune pièce importée ne rattache cette paie à une fiche".into()));
    } else if nom.is_none() {
        manques.push(manque("salarie", "salarié", format!("la fiche « {} » n'existe pas (ou plus) : à créer dans Configuration › Salariés", salarie)));
    }
    if numero.is_empty() && titre.is_empty() {
        manques.push(manque(
            "spectacle",
            "spectacle",
            "ni la DSN, ni l'AEM, ni le bulletin ne portent de numéro d'objet pour cette paie (un permanent n'en a pas)".into(),
        ));
    } else if !numero.is_empty() && titre.is_empty() {
        manques.push(manque(
            "spectacle",
            "titre du spectacle",
            format!("la DSN ne porte que le numéro d'objet {} : son titre se complète dans Configuration › Spectacles", numero),
        ));
    }
    if dates.is_empty() {
        let pourquoi = match (bulletin.is_some(), declare_par, par_aem) {
            (true, _, _) => "le bulletin n'imprime pas ses dates travaillées",
            (false, "DSN", _) => "la DSN ne déclare que le début et la fin du contrat ; le bulletin de cette paie les donnerait",
            (false, _, true) => "l'AEM ne porte que le début, la fin et le nombre de cachets ; le bulletin de cette paie les donnerait",
            _ => "aucune pièce importée ne les porte",
        };
        manques.push(manque("dates", "jours travaillés", pourquoi.into()));
    }
    if contrat.is_none() {
        manques.push(manque(
            "cachets",
            "cachets et heures",
            if bulletin.is_some() {
                "le bulletin ne les donne pas en nombres entiers lisibles, et aucune AEM n'atteste ce contrat".into()
            } else {
                "la DSN ne les déclare pas, et aucune AEM ni aucun bulletin n'atteste ce contrat".into()
            },
        ));
    }
    for (k, libelle, pourquoi) in [
        ("brut", "brut", "aucune pièce importée ne le porte"),
        ("net_imposable", "net imposable", "aucune pièce importée ne le porte"),
        (
            "net_a_payer",
            "net à payer",
            if declare_par == "DSN" {
                "la DSN ne le déclare pas (son 50.004 est le net versé) ; le bulletin de cette paie le donnerait"
            } else {
                "aucune pièce importée ne le porte ; le bulletin de cette paie le donnerait"
            },
        ),
        ("pas", "prélèvement à la source", "aucune pièce importée ne le porte"),
    ] {
        if !montants.contains_key(k) {
            manques.push(manque(k, libelle, pourquoi.into()));
        }
    }

    let documents = match &bulletin {
        Some(b) => json!({"couleur": "vert", "courte": "bulletin importé", "phrase": format!("bulletin de l'ancien logiciel{}, copié dans historique/bulletins/", {
            let n = b.numero.clone().unwrap_or_default();
            if n.is_empty() { String::new() } else { format!(" n° {}", n) }
        })}),
        None => json!({"couleur": "gris", "courte": "avant PuppetPaie", "phrase": "documents remis avant PuppetPaie : aucun bulletin importé pour cette paie"}),
    };
    // CE QUE LA CARTE FERMEE DIT : le spectacle, les cachets et les heures ; au survol, les pieces lues.
    let mut quoi = Vec::new();
    if !titre.is_empty() || !numero.is_empty() {
        quoi.push(if titre.is_empty() { numero.clone() } else { titre.clone() });
    }
    if let Some(n) = cachets.filter(|n| *n != 0) {
        let dit = socle::texte::virgule(socle::num::Decimal::from(n));
        quoi.push(format!("{} {}", dit, if n > 1 { "cachets" } else { "cachet" }));
    }
    if let Some(n) = heures.filter(|n| *n != 0) {
        quoi.push(format!("{} h", socle::texte::virgule(socle::num::Decimal::from(n))));
    }
    let quoi_dit = if quoi.is_empty() { "spectacle non lu".to_string() } else { quoi.join(" · ") };
    let mut lu = Vec::new();
    if declare_par == "DSN" {
        lu.push("DSN".to_string());
    }
    if let Some(b) = &bulletin {
        let n = b.numero.clone().unwrap_or_default();
        lu.push(if n.is_empty() { "bulletin".to_string() } else { format!("bulletin {}", n) });
    }
    if par_aem {
        lu.push("AEM".to_string());
    }
    let origine_dit = (!lu.is_empty()).then(|| format!("lu dans {}", lu.join(" + ")));
    let spectacle_dit = match (titre.is_empty(), numero.is_empty() || numero == titre) {
        (false, false) => format!("{} — n° {}", titre, numero),
        (false, true) => titre.clone(),
        (true, false) => format!("n° {}", numero),
        (true, true) => String::new(),
    };
    let net_titre = montants.contains_key("net_a_payer").then_some("net à payer");
    json!({
        "quoi_dit": quoi_dit, "origine_dit": origine_dit, "spectacle_dit": spectacle_dit, "net_titre": net_titre,
        "salarie": salarie,
        "nom": nom.clone().unwrap_or_else(|| salarie.clone()),
        "spectacle": {
            "cle": spectacle.as_ref().map(|s| s.0.clone()),
            "numero_objet": numero,
            "titre": titre,
        },
        "contrat": p.map(|p| p.contrat.clone()).unwrap_or_default(),
        "debut": debut,
        "fin": fin,
        "periode": periode_en_clair(&debut, &fin, mois),
        "dates": dates,
        "cachets": cachets,
        "heures": heures,
        "jours": jours,
        "montants": montants,
        "sources": sources,
        "lignes": p.map_or(json!([]), |p| json!(p.lignes)),
        "declare_par": if declare_par.is_empty() { Value::Null } else { json!(declare_par) },
        "atteste_par": c.map_or(Value::Null, |c| json!(c.atteste_par)),
        "bulletin": bulletin.map_or(Value::Null, |b| json!(b)),
        "etats": {
            "dpae": dpae,
            "signature": {"couleur": "gris", "courte": "avant PuppetPaie", "phrase": "contrat fait avant PuppetPaie : sa signature n'est pas suivie ici"},
            "documents": documents,
        },
        "manque": manques,
    })
}

/// LES ETAPES D'UN MOIS TENU AVANT (`bilan_du_mois`, le panneau du mois) : tout s'est fait ailleurs,
/// aucune etape n'a de bouton -- ni « Fermer le mois », ni « Deposer la DSN » (regle 4). La DSN dit ce qu'on en
/// sait (`compagnie::etat_dsn`) ; sans trace, elle reste en alerte, sans geste.
pub(crate) fn etapes(vue: &Value) -> Value {
    let cartes = vue["paies"].as_array().map_or(&[][..], |a| &a[..]);
    let n = cartes.len();
    let pl = |n: usize| if n > 1 { "s" } else { "" };
    let dpae_faites = cartes.iter().filter(|c| c["etats"]["dpae"]["couleur"] == json!("vert")).count();
    let bulletins = cartes.iter().filter(|c| c["bulletin"].is_object()).count();
    let ailleurs = n - dpae_faites;
    let retrouvees = format!("{} DPAE retrouvée{} dans le journal ou les certificats de l'Urssaf", dpae_faites, pl(dpae_faites));
    let sans_trace_ici = format!("{} DPAE faite{} ailleurs, sans trace relevée ici", ailleurs, pl(ailleurs));
    let dpae = match (dpae_faites, ailleurs) {
        (0, 0) => "Mois tenu avant PuppetPaie — aucune DPAE".to_string(),
        (_, 0) => format!("Mois tenu avant PuppetPaie — {}", retrouvees),
        (0, _) => format!("Mois tenu avant PuppetPaie — {}", sans_trace_ici),
        _ => format!("Mois tenu avant PuppetPaie — {} ; {}", retrouvees, sans_trace_ici),
    };
    let signature = match n {
        0 => "Mois tenu avant PuppetPaie — aucun contrat".to_string(),
        1 => "Mois tenu avant PuppetPaie — 1 contrat fait ailleurs : sa signature n'est pas suivie ici".to_string(),
        _ => format!("Mois tenu avant PuppetPaie — {} contrats faits ailleurs : leur signature n'est pas suivie ici", n),
    };
    let documents = match bulletins {
        0 => format!("Mois tenu avant PuppetPaie — aucun bulletin de l'ancien logiciel importé sur {} paie{}", n, pl(n)),
        b => format!("Mois tenu avant PuppetPaie — {} bulletin{} de l'ancien logiciel importé{} sur {} paie{}", b, pl(b), pl(b), n, pl(n)),
    };
    let dsn = &vue["dsn"];
    let sans_trace = dsn["etat"] == json!("sans_trace") || dsn["etat"] == json!("refusee");
    json!({
        "dpae": {"couleur": "vert", "phrase": dpae},
        "contrat-signature": {"couleur": "vert",
            "phrase": signature},
        "dsn": {
            // La couleur de chaque etape (`routes::couleur_de_l_etat`) : faite ailleurs, verte ; refusee, rouge ; sans trace
            // ici, tenue par l'ancien logiciel, une information.
            "couleur": if dsn["etat"] == json!("refusee") { "rouge" } else if sans_trace { "gris" } else { "vert" },
            "courte": dsn["courte"].clone(),
            "phrase": dsn["phrase"].clone(),
        },
        "documents": {"couleur": "vert",
            "phrase": documents},
    })
}

/// LES CARTES DES ENGAGEMENTS IMPORTES QUI TOUCHENT `mois`, qu'il soit tenu avant ce logiciel ou qu'il ait aussi des
/// engagements de la saisie : une carte par engagement importe, son contrat atteste et ce qui lui a ete verse ce
/// mois-la, dans l'ordre de leur debut.
pub(crate) fn cartes(ctx: &Contexte, mois: &str) -> R<Vec<Value>> {
    let importes = importes_du_mois(&ctx.racine, mois)?;
    if importes.is_empty() {
        return Ok(Vec::new());
    }
    let lignes = envois::journal::lu(&ctx.racine).map(|j| j.lignes)?;
    let embauches = etats::embauches_par_depot(&lignes);
    let (certifiees, _) = envois::reprise_journal::embauches_attestees(ctx);
    let mut cartes = Vec::new();
    for i in importes {
        let contrat = i.contrat_lu();
        let paie = i.paies_lues().into_iter().find(|p| p.mois == mois);
        let brut = &*i.objet().brut;
        let (s, d) = (texte(&brut, "salarie"), texte(&brut, "debut"));
        let dpae = dpae_de(&embauches, &certifiees, &s, &texte(&brut, "numero"), &d);
        cartes.push(carte(ctx, mois, paie.as_ref(), contrat.as_ref(), dpae));
    }
    cartes.sort_by(|a, b| (texte(a, "debut"), texte(a, "nom")).cmp(&(texte(b, "debut"), texte(b, "nom"))));
    Ok(cartes)
}

/// LE MOIS TENU AVANT, comme la page Paie le montre : `None` si un engagement de la saisie touche le mois ou si aucun
/// engagement importe ne le touche. Sinon `{"dsn", "paies", "totaux", "incompletes"}` : une carte par engagement
/// importe, son contrat atteste et ce qui lui a ete verse ce mois-la.
pub(crate) fn vue(ctx: &Contexte, mois: &str) -> R<Option<Value>> {
    if !importe(&ctx.racine, mois)? {
        return Ok(None);
    }
    let cartes = cartes(ctx, mois)?;

    // Les totaux du mois, sur ce qui est connu ; ce qui manque se compte a part.
    let mut totaux = Map::new();
    for k in ["brut", "net_a_payer", "cout_employeur"] {
        let mut somme = socle::num::Decimal::ZERO;
        let mut connus = 0usize;
        for c in &cartes {
            if let Some(v) = c["montants"].get(k).and_then(Value::as_str).and_then(socle::num::decimal_lu) {
                somme += v;
                connus += 1;
            }
        }
        totaux.insert(k.into(), json!({"montant": if connus > 0 { json!(somme.round_dp(2).to_string()) } else { Value::Null }, "connus": connus}));
    }
    let manques = cartes.iter().filter(|c| c["manque"].as_array().is_some_and(|l| !l.is_empty())).count();
    Ok(Some(json!({
        "mois": mois,
        "dsn": crate::compagnie::etat_dsn::du_mois(ctx, socle::mois::Mois::lire(mois)?, None)?.0,
        "paies": cartes,
        "totaux": totaux,
        "incompletes": manques,
    })))
}
