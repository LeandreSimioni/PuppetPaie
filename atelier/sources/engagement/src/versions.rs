//! LES VERSIONS D'UN ENGAGEMENT, DITES EN MOTS. Un engagement modifie apres un envoi garde la version sur laquelle l'envoi
//! est parti (`etats`) ; l'ecran de l'engagement montre alors une ligne (« Modifié le 12/11, après l'envoi du contrat »)
//! qui ouvre la frise : par version, sa date, ce qui a change depuis la precedente (« Cachets : 4 → 5 ») et ce qui est
//! parti sur elle. Un engagement jamais modifie apres un envoi n'a pas de frise.

use serde::Serialize;
use socle::forme::NombreLu;
use crate::{Chiffre, LignePaie};

use crate::departs::{Depart, Departs, Envoi};
use crate::etats::{ses_departs, ARefaire, Valide};
use crate::saisie::{Carte, Champ, Groupe, Valeur};
use crate::stock::{Saisie, Version};
use droit::contexte::Contexte;
use socle::date::Jour;
use socle::mois::{Graphie, Mois};
use socle::num::Resultat;
use socle::texte::{euros, virgule};

/// « 14/11 » d'un jour ISO ; tel quel s'il ne se lit pas.
fn jour_court(iso: &str) -> String {
    let jour = match iso.get(..10) {
        Some(j) => j,
        None => iso,
    };
    match Jour::lire(jour) {
        Ok(j) => j.jj_mm(),
        Err(_) => iso.to_string(),
    }
}

/// Ce qui change dans les jours d'une ligne, et seulement cela : « Jours : + 16/11 », « Jours : − 14/11 »,
/// « Jours : + 16/11 · − 14/11 ». `None` si la ligne a les memes jours.
fn jours_changes(x: &LignePaie, y: &LignePaie) -> Option<String> {
    let js = |l: &LignePaie| {
        let mut j: Vec<String> = l.dates.iter().map(|d| d.jour().to_string()).filter(|d| !d.is_empty()).collect();
        j.sort();
        j.dedup();
        j
    };
    let (a, b) = (js(x), js(y));
    let dire = |signe: &str, xs: Vec<&String>| (!xs.is_empty()).then(|| format!("{} {}", signe, xs.iter().map(|d| jour_court(d)).collect::<Vec<_>>().join(", ")));
    let parts: Vec<String> = [dire("+", b.iter().filter(|d| !a.contains(d)).collect()), dire("−", a.iter().filter(|d| !b.contains(d)).collect())]
        .into_iter()
        .flatten()
        .collect();
    (!parts.is_empty()).then(|| format!("Jours : {}", parts.join(" · ")))
}

/// Le premier et le dernier jour d'une carte : ses bornes dites, sinon ses jours travailles (`Carte::debut_lu`,
/// `Carte::dates`).
fn bornes(ctx: &Contexte, s: &Carte) -> (String, String) {
    let fin = s.fin.clone().filter(|f| !f.is_empty());
    // Sans fin dite, le dernier jour de ses dates ; aucun sans date.
    let fin = match fin {
        Some(f) => f,
        None => s.dates(&ctx.racine).iter().map(|j| j.jour().to_string()).max().into_iter().collect(),
    };
    (s.debut_lu(&ctx.racine), fin)
}

/// Le libelle d'une rubrique au catalogue, ou son code.
fn libelle(ctx: &Contexte, code: &str) -> (String, String) {
    match droit::catalogue::rubrique(&ctx.racine, code, &ctx.lue_au) {
        Ok(r) => (r.libelle, r.unite),
        Err(_) => (code.to_string(), String::new()),
    }
}

/// « Cachets », « Heures de travail », « Services de répétition » : le premier mot du libelle au pluriel.
fn au_pluriel(libelle: &str) -> String {
    let (premier, reste) = match libelle.split_once(' ') {
        Some(p_r) => p_r,
        None => (libelle, ""),
    };
    let pluriel = if premier.ends_with(['s', 'x', 'z']) { premier.to_string() } else { format!("{}s", premier) };
    if reste.is_empty() { pluriel } else { format!("{} {}", pluriel, reste) }
}

/// Ce que dit le taux d'une rubrique : « Salaire du cachet », « Salaire de l'heure » ; le libelle pour un forfait.
fn nom_du_taux(libelle: &str, unite: &str) -> String {
    match unite {
        "cachet" => "Salaire du cachet".into(),
        "heure" => "Salaire de l'heure".into(),
        "jour" => "Salaire de la journée".into(),
        "semaine" => "Salaire de la semaine".into(),
        "service" => "Salaire du service".into(),
        _ => format!("{}, montant", libelle),
    }
}

/// Les clauses de l'engagement hors lignes et bornes : leur nom a l'ecran.
const CLAUSES: [(Champ, &str); 13] = [
    (Champ::Salarie, "Salarié"),
    (Champ::Spectacle, "Spectacle"),
    (Champ::Lieu, "Lieu"),
    (Champ::Etablissement, "Établissement"),
    (Champ::Nature, "Nature du contrat"),
    (Champ::MotifRecours, "Motif de recours"),
    (Champ::SalaireMensuel, "Salaire mensuel"),
    (Champ::QuotiteContrat, "Quotité du contrat"),
    (Champ::ModaliteTempsTravail, "Temps de travail"),
    (Champ::Metier, "Métier"),
    (Champ::Emploi, "Emploi"),
    (Champ::Cadre, "Cadre"),
    (Champ::Numero, "Numéro du contrat"),
];

/// Une valeur en mots : en euros (`euros`), un nombre a la virgule, un oui-non, un texte ; « — » sans valeur.
fn en_mots(v: Option<&Valeur>, en_euros: bool) -> String {
    let nombre = |n: &NombreLu| socle::num::dec_texte(&n.to_string());
    match v {
        None => "—".into(),
        Some(Valeur::Oui(b)) => if *b { "oui" } else { "non" }.into(),
        Some(Valeur::Nombre(n)) => match (nombre(n), en_euros) {
            (Ok(x), true) => euros(x),
            (Ok(x), false) => virgule(x),
            (Err(_), _) => n.to_string(),
        },
        Some(Valeur::Texte(t)) if en_euros => match t.trim().replace(',', ".").parse() {
            Ok(x) => euros(x),
            Err(_) => t.clone(),
        },
        Some(Valeur::Texte(t)) => t.clone(),
    }
}

/// Un nombre d'une ligne de paie, en valeur simple.
fn du_chiffre(c: &Option<Chiffre>) -> Option<Valeur> {
    match c.as_ref()? {
        Chiffre::Nombre(n) => Some(Valeur::Nombre(n.clone())),
        Chiffre::Texte(t) => Some(Valeur::Texte(t.clone())),
    }
}

/// La quantite d'une ligne, en flottant quand elle est un nombre.
fn quantite_nombre(l: &LignePaie) -> Option<f64> {
    match &l.quantite {
        Some(Chiffre::Nombre(n)) => n.as_f64(),
        _ => None,
    }
}

/// Une version, ou un mois d'une version, que l'autre n'a pas (avant la premiere, un mois de plus) ne dit rien : `rien`.
fn ou_rien<'a, T>(x: Option<&'a T>, rien: &'a T) -> &'a T {
    match x {
        Some(x) => x,
        None => rien,
    }
}

/// CE QUI A CHANGE d'une saisie a l'autre, en mots, dans l'ordre ou l'on lit un engagement : les bornes, chaque ligne
/// (son nombre, son taux, ses jours, son lieu), puis les autres clauses ; chaque terme qui change a une date (« À partir
/// du 01/09 : Salaire mensuel : … → … ») ; et les faits de chaque mois suivant d'un contrat qui deborde.
pub fn changements(ctx: &Contexte, avant: Option<&Saisie>, apres: Option<&Saisie>) -> Vec<String> {
    let vide = Saisie::default();
    let (a, b) = (ou_rien(avant, &vide), ou_rien(apres, &vide));
    let mut out = changements_du_mois(ctx, &a.carte, &b.carte);
    // Les termes au `jour` : ceux du premier mois, et chaque avenant venu jusqu'a lui.
    let au = |s: &Saisie, jour: &Jour| {
        let mut c = s.carte.du_groupe(&[Groupe::Terme]);
        for x in s.avenants.iter().filter(|x| x.depuis <= *jour) {
            c.recouvrir(&x.termes);
            for k in &x.retires {
                c.ote(*k);
            }
        }
        c
    };
    let mut jours: Vec<Jour> = a.avenants.iter().chain(&b.avenants).map(|x| x.depuis).collect();
    jours.sort();
    jours.dedup();
    for jour in jours {
        // Les termes que l'un ou l'autre change ce jour-la, tels qu'ils sont ce jour-la.
        let termes: Vec<Champ> = a
            .avenants
            .iter()
            .chain(&b.avenants)
            .filter(|x| x.depuis == jour)
            .flat_map(|x| Champ::des(&[Groupe::Terme]).filter(|k| x.termes.dit(*k)).chain(x.retires.iter().copied()).collect::<Vec<_>>())
            .collect();
        let tels = |s: &Saisie| {
            let (tous, mut c) = (au(s, &jour), Carte::default());
            for k in &termes {
                c.prendre(&tous, *k);
            }
            c
        };
        for c in changements_du_mois(ctx, &tels(a), &tels(b)) {
            out.push(format!("À partir du {} : {}", jour.jj_mm(), c));
        }
    }
    let mut mois: Vec<&socle::mois::Mois> = a.faits.keys().chain(b.faits.keys()).collect();
    mois.sort();
    mois.dedup();
    let rien = Carte::default();
    for m in mois {
        for c in changements_du_mois(ctx, ou_rien(a.faits.get(m), &rien), ou_rien(b.faits.get(m), &rien)) {
            out.push(format!("En {} : {}", socle::mois::Mois::en_clair(&m.to_string()), c));
        }
    }
    out
}

/// Un terme du contrat qui change a un jour : sa valeur convenue jusque-la (`avant`) et la nouvelle (`apres`) ; `None` :
/// le terme n'est pas dit.
#[derive(Debug, Clone, PartialEq)]
pub struct TermeChange {
    pub terme: Champ,
    pub jour: Jour,
    pub avant: Option<Valeur>,
    pub apres: Option<Valeur>,
}

/// LES TERMES QUI CHANGENT entre la saisie sur laquelle le contrat a ete convenu (`convenue`) et celle d'aujourd'hui
/// (`presente`), jour par jour : chaque jour ou un terme du contrat (`etats::CLAUSES` qu'un avenant peut dater) ne
/// vaut pas ce qui etait convenu. L'ancienne valeur est celle que le contrat disait ce jour-la, ou celle qu'un changement
/// plus tot du meme terme vient de poser ; un changement qui ne change rien ne se dit pas.
pub fn termes_changes(convenue: &Saisie, presente: &Saisie) -> Vec<TermeChange> {
    // Les jours ou un avenant de `s` change le terme `k`, et la valeur qu'il lui donne.
    let datees = |s: &Saisie, k: Champ| -> Vec<(Jour, Option<Valeur>)> {
        s.avenants.iter().filter(|x| x.termes.dit(k) || x.retires.contains(&k)).map(|x| (x.depuis, x.termes.valeur_du(k))).collect()
    };
    let au = |s: &Saisie, k: Champ, jour: &Jour| -> Option<Valeur> {
        match datees(s, k).into_iter().filter(|(d, _)| d <= jour).last() {
            Some((_, v)) => v,
            None => s.carte.valeur_du(k),
        }
    };
    let pareil = |a: &Option<Valeur>, b: &Option<Valeur>| socle::ecriture::canonique(a) == socle::ecriture::canonique(b);
    let mut out = Vec::new();
    for k in crate::etats::CLAUSES.iter().copied().filter(|k| k.groupe() == Groupe::Terme) {
        let mut jours: Vec<Jour> = datees(convenue, k).into_iter().chain(datees(presente, k)).map(|(d, _)| d).collect();
        jours.sort();
        jours.dedup();
        let mut dernier: Option<(Jour, Option<Valeur>)> = None;
        for j in jours {
            let (c, p) = (au(convenue, k, &j), au(presente, k, &j));
            let avant = match &dernier {
                Some((dj, v)) if pareil(&au(convenue, k, dj), &c) => v.clone(),
                _ => c,
            };
            if pareil(&avant, &p) {
                continue;
            }
            out.push(TermeChange { terme: k, jour: j, avant, apres: p.clone() });
            dernier = Some((j, p));
        }
    }
    out.sort_by_key(|t| t.jour);
    out
}

/// CE QUI A CHANGE DEPUIS LA VERSION `n` de l'engagement valide `v` jusqu'a son present, en mots (`changements`) : ce
/// que dit la frise, et ce que dit le renvoi des documents partis sur la version `n`. Vide si `n` est le present ou ne se
/// lit pas.
pub fn depuis(ctx: &Contexte, v: &Valide, n: u64) -> Vec<String> {
    match (v.version_numero(n), &v.objet().brut.valide) {
        (Some(passee), Some(present)) if v.version() != Some(n) => changements(ctx, passee.saisie(), present.saisie.as_ref()),
        _ => Vec::new(),
    }
}

fn changements_du_mois(ctx: &Contexte, a: &Carte, b: &Carte) -> Vec<String> {
    let mut out = Vec::new();
    let ((da, fa), (db, fb)) = (bornes(ctx, a), bornes(ctx, b));
    if da != db {
        out.push(format!("Début : {} → {}", jour_court(&da), jour_court(&db)));
    }
    if fa != fb {
        out.push(format!("Fin : {} → {}", jour_court(&fa), jour_court(&fb)));
    }
    let (la, lb) = (a.lignes(), b.lignes());
    let code = |l: &LignePaie| l.code().to_string();
    // Les lignes se rapprochent par leur rubrique, dans l'ordre : la deuxieme ligne de cachets de l'une va avec la
    // deuxieme de l'autre.
    let mut vues: Vec<String> = Vec::new();
    for l in la.iter().chain(lb.iter()) {
        if !vues.contains(&code(l)) {
            vues.push(code(l));
        }
    }
    for c in vues {
        let (lib, unite) = libelle(ctx, &c);
        let xs: Vec<&LignePaie> = la.iter().filter(|l| code(l) == c).collect();
        let ys: Vec<&LignePaie> = lb.iter().filter(|l| code(l) == c).collect();
        for k in 0..xs.len().max(ys.len()) {
            match (xs.get(k), ys.get(k)) {
                (Some(_), None) => out.push(format!("{} : retiré", lib)),
                (None, Some(y)) => out.push(format!("{} : ajouté ({})", lib, en_mots(du_chiffre(&y.quantite).as_ref(), false))),
                (Some(x), Some(y)) => {
                    let (qx, qy) = (quantite_nombre(x), quantite_nombre(y));
                    if qx != qy {
                        out.push(format!(
                            "{} : {} → {}",
                            au_pluriel(&lib),
                            en_mots(du_chiffre(&x.quantite).as_ref(), false),
                            en_mots(du_chiffre(&y.quantite).as_ref(), false)
                        ));
                    }
                    let (tx, ty) = (en_mots(du_chiffre(&x.taux).as_ref(), true), en_mots(du_chiffre(&y.taux).as_ref(), true));
                    if tx != ty {
                        out.push(format!("{} : {} → {}", nom_du_taux(&lib, &unite), tx, ty));
                    }
                    if let Some(j) = jours_changes(x, y) {
                        out.push(j);
                    }
                    let lieu = |l: &LignePaie| en_mots(l.lieu.clone().map(Valeur::Texte).as_ref(), false);
                    let (lx, ly) = (lieu(x), lieu(y));
                    if lx != ly {
                        out.push(format!("Lieu : {} → {}", lx, ly));
                    }
                }
                (None, None) => {}
            }
        }
    }
    for (cle, nom) in CLAUSES {
        let en_euros = cle == Champ::SalaireMensuel;
        let (x, y) = (en_mots(a.valeur_du(cle).as_ref(), en_euros), en_mots(b.valeur_du(cle).as_ref(), en_euros));
        if x != y {
            out.push(format!("{} : {} → {}", nom, x, y));
        }
    }
    out
}

/// Un depart en mots, sans sa date : « le contrat », « l'avenant n° 1 », « la DPAE », « les documents de novembre 2026 ».
/// Une DSN se nomme par le mois de la DSN qui est partie : la correction d'un mois passe portee par la DSN d'un mois
/// suivant (`Depart::par`) est « la DSN de décembre 2026 (correction de novembre 2026) ».
pub(crate) fn quoi_du_depart(d: &Depart) -> String {
    match (d.envoi, d.mois.as_deref()) {
        (Envoi::Contrat, _) => match &d.avenant {
            Some(a) => format!("l'avenant n° {}", a),
            None => "le contrat".to_string(),
        },
        (Envoi::Dpae, _) => "la DPAE".to_string(),
        (Envoi::Documents, Some(m)) => format!("les documents {}", de(&socle::mois::Mois::en_clair(m))),
        (Envoi::Dsn, Some(m)) => match d.par.as_deref() {
            Some(p) => format!("la DSN {} (correction {})", de(&socle::mois::Mois::en_clair(p)), de(&socle::mois::Mois::en_clair(m))),
            None => format!("la DSN {}", de(&socle::mois::Mois::en_clair(m))),
        },
        (Envoi::Signalement, _) => "un signalement".to_string(),
        (_, None) => "un envoi".to_string(),
    }
}

/// « du contrat », « de la DPAE », « des documents de novembre 2026 », « d'un signalement », « de l'avenant n° 1 » :
/// le complement d'« après l'envoi », élidé devant une voyelle.
fn complement(quoi: &str) -> String {
    if let Some(r) = quoi.strip_prefix("le ") {
        format!("du {}", r)
    } else if let Some(r) = quoi.strip_prefix("les ") {
        format!("des {}", r)
    } else {
        de(quoi)
    }
}

/// « le contrat, la DPAE et les documents de novembre 2026 ».
fn enumerer(xs: &[String]) -> String {
    match xs {
        [] => String::new(),
        [x] => x.clone(),
        [debut @ .., dernier] => format!("{} et {}", debut.join(", "), dernier),
    }
}

/// LA FRISE D'UN ENGAGEMENT VALIDE, s'il a ete modifie apres un envoi : la phrase de la ligne sous la carte et, de la
/// plus ancienne a la presente, chaque version.
#[derive(Debug, Clone, Serialize)]
pub struct Frise {
    pub phrase: String,
    pub versions: Vec<VersionDeLaFrise>,
}

/// Une version, dans la frise : son numero, quand elle a ete validee, son titre et son jour tels que la frise les lit, ce
/// qui a change depuis la precedente, et ce qui est parti sur elle.
#[derive(Debug, Clone, Serialize)]
pub struct VersionDeLaFrise {
    pub version: Option<u64>,
    pub le: String,
    pub jour: String,
    pub titre_dit: String,
    pub jour_dit: String,
    pub changements: Vec<String>,
    pub partis: Vec<String>,
}

/// LA FRISE de l'engagement valide `v` (`Frise`). `None` : aucune version passee, jamais modifie apres un envoi, ou la
/// paie de `mois` est restee celle de la version d'avant (un contrat au mois change a partir d'un mois suivant : ce
/// mois-ci reste tel qu'il est parti).
pub fn frise(ctx: &Contexte, v: &Valide, departs: &Departs, mois: &str) -> Resultat<Option<Frise>> {
    let passees = v.versions();
    let paie_du_mois = |x: Version| -> Resultat<_> { Ok(crate::etats::paie_ecrite_de_la_version(v.objet(), x, mois)?.map(|p| p.paie.clone())) };
    let present = v.objet().brut.valide.as_ref().map(Version::Presente);
    let Some((a, p)) = passees.last().map(Version::Passee).zip(present) else { return Ok(None) };
    let avant = paie_du_mois(a)?;
    if avant.is_some() && avant == paie_du_mois(p)? {
        return Ok(None);
    }
    let siens = ses_departs(ctx, v.objet(), departs);
    let toutes: Vec<(Option<Version>, String)> = passees
        .iter()
        .map(|x| (Some(Version::Passee(x)), x.valide_le.clone()))
        .chain(std::iter::once((present, v.le().to_string())))
        .collect();
    let mut versions = Vec::new();
    let mut precedente: Option<Option<Version>> = None;
    for (x, le) in &toutes {
        let n = x.and_then(|x| x.numero());
        let mut partis: Vec<String> = siens
            .iter()
            .filter(|d| Some(d.version) == n)
            .map(|d| format!("{}, le {}", quoi_du_depart(d), jour_court(&d.le)))
            .collect();
        partis.dedup();
        // La premiere version n'a rien d'avant quoi changer.
        let changes = match precedente {
            Some(p) => changements(ctx, p.and_then(|p| p.saisie()), x.and_then(|x| x.saisie())),
            None => Vec::new(),
        };
        let titre_dit = match n {
            Some(n) => format!("Version {}", n),
            None => "Version".to_string(),
        };
        versions.push(VersionDeLaFrise { version: n, le: le.clone(), jour: jour_court(le), titre_dit, jour_dit: format!("le {}", jour_court(le)), changements: changes, partis });
        precedente = Some(*x);
    }
    Ok(Some(Frise { phrase: phrase_de_la_frise(ctx, v, departs, siens)?, versions }))
}

/// LA LIGNE SOUS LA CARTE : quand l'engagement a change, a partir de quel mois si ce n'est pas son premier, et ce qui
/// etait parti que le changement touche (`etats::a_refaire`), groupe par sorte, les mois qui se suivent en une etendue :
/// « Modifié le 20/01, à partir de septembre 2025, après l'envoi du contrat, des documents et des DSN de septembre à
/// décembre 2025 ». Le detail, envoi par envoi, est dans la frise.
fn phrase_de_la_frise(ctx: &Contexte, v: &Valide, departs: &Departs, siens: &[Depart]) -> Resultat<String> {
    let mut phrase = format!("Modifié le {}", jour_court(v.le()));
    if let Some(m) = premier_mois_change(v)?.filter(|m| m.as_str() != v.objet().premier_mois()) {
        phrase.push_str(&format!(", à partir de {}", socle::mois::Mois::en_clair(&m)));
    }
    let (mut contrat, mut dpae) = (false, false);
    let (mut documents, mut dsn) = (std::collections::BTreeSet::new(), std::collections::BTreeSet::new());
    for a in crate::etats::a_refaire(ctx, v, departs)? {
        match a {
            ARefaire::Avenant { .. } => contrat = true,
            ARefaire::Dpae { .. } => dpae = true,
            ARefaire::Bulletin { mois, .. } => {
                documents.insert(Mois::lire(&mois)?);
            }
            ARefaire::Dsn { mois, .. } => {
                dsn.insert(Mois::lire(&mois)?);
            }
        }
    }
    let mut quoi = Vec::new();
    // Le contrat, ou son dernier avenant : celui que le changement rend faux.
    if let Some(d) = siens.iter().filter(|d| contrat && d.envoi == Envoi::Contrat).last() {
        quoi.push(complement(&quoi_du_depart(d)));
    }
    if dpae {
        quoi.push(complement("la DPAE"));
    }
    if !documents.is_empty() && documents == dsn {
        quoi.push("des documents".to_string());
        quoi.push(format!("des DSN {}", de_mois(&documents)));
    } else {
        if !documents.is_empty() {
            quoi.push(format!("des documents {}", de_mois(&documents)));
        }
        if !dsn.is_empty() {
            quoi.push(format!("des DSN {}", de_mois(&dsn)));
        }
    }
    if !quoi.is_empty() {
        phrase.push_str(&format!(", après l'envoi {}", enumerer(&quoi)));
    }
    Ok(phrase)
}

/// Le premier mois dont la paie a change au dernier changement de l'engagement valide `v` (sa derniere version passee
/// contre son present) ; `None` si aucune ne se lit autrement, ou sans version passee.
fn premier_mois_change(v: &Valide) -> Resultat<Option<String>> {
    let (Some(passee), Some(valide)) = (v.versions().last(), v.objet().brut.valide.as_ref()) else { return Ok(None) };
    let (avant, present) = (Version::Passee(passee), Version::Presente(valide));
    let paie = |x: Version, m: &str| -> Resultat<_> { Ok(crate::etats::paie_ecrite_de_la_version(v.objet(), x, m)?.map(|p| p.paie.clone())) };
    for m in crate::paies::mois_ecrits(v.objet())? {
        if paie(avant, &m)? != paie(present, &m)? {
            return Ok(Some(m));
        }
    }
    Ok(None)
}

/// « de septembre à décembre 2025 », « d'avril 2025 et de juin 2025 », « de novembre 2025 à février 2026 » : des mois
/// `AAAA-MM`, ceux qui se suivent en une etendue.
fn de_mois(mois: &std::collections::BTreeSet<Mois>) -> String {
    let mut etendues: Vec<(Mois, Mois)> = Vec::new();
    for m in mois {
        match etendues.last_mut() {
            Some((_, fin)) if fin.suivant() == *m => *fin = *m,
            _ => etendues.push((*m, *m)),
        }
    }
    let dites: Vec<String> = etendues
        .iter()
        .map(|(a, b)| {
            let (la, lb) = (a.libelle(Graphie::Accentuee), b.libelle(Graphie::Accentuee));
            let texte = if a == b {
                la
            } else if a.annee() == b.annee() {
                format!("{} à {}", a.nom(Graphie::Accentuee), lb)
            } else {
                format!("{} à {}", la, lb)
            };
            de(&texte)
        })
        .collect();
    enumerer(&dites)
}

/// « de mars 2025 », « d'avril 2025 » : la preposition, elidee devant une voyelle.
fn de(texte: &str) -> String {
    if texte.starts_with(['a', 'e', 'i', 'o', 'u', 'é', 'h']) {
        format!("d'{}", texte)
    } else {
        format!("de {}", texte)
    }
}
