//! LES IMPREVUS D'UN SALARIE : ce qui arrive a un contrat en cours et le suspend -- un ARRET de travail
//! (bloc S21.G00.60, son signalement 04/05) ou une SUSPENSION d'une autre sorte (bloc S21.G00.65 : conge parental,
//! sabbatique, sans solde...). Un imprevu se saisit UNE FOIS, sur le salarie et le contrat qu'il suspend, avec ses dates ;
//! le moteur en tire tout le reste, mois par mois :
//!
//! - la ligne de paie d'un permanent (`lignes_du_mois`) : le salaire des jours couverts retire, le maintien de salaire
//!   saisi sur l'arret verse ; la saisie du mois n'a plus d'absence a taper ;
//! - le bloc 65 de chaque DSN dont le mois croise la suspension (`suspensions_du_mois`), comme le bloc 60 l'est deja
//!   pour l'arret (`arrets_du_mois`) : « en cas de suspension supra-mensuelle, la suspension doit etre declaree dans
//!   toutes les DSN mensuelles dont le Mois Principal declare presente une periode commune avec la suspension »
//!   (cahier technique DSN 2026.1, bloc S21.G00.60 p. 265 et bloc S21.G00.65 p. 281,
//!   `atelier/reference/norme-dsn/texte/bloc-S21.G00.60.txt` et `bloc-S21.G00.65.txt`) ;
//! - les cachets et services d'un intermittent qui tombent dans l'imprevu (`cachets_dans`, `avertir_les_jours_payes`) :
//!   le moteur les montre et avertit, il ne les retire pas (regle 1).
//!
//! UN SEUL MAGASIN : `imprevus/AAAA-MM.json` sous les donnees de la compagnie, au mois ou l'imprevu se range (le dernier
//! jour travaille d'un arret, le premier jour d'une suspension) : `{"_origine", "mois", "imprevus": [...]}`. Chaque
//! imprevu dit son `genre` (`arret` ou `suspension`) ; ses champs sont `CHAMPS_ARRET` ou `CHAMPS_SUSPENSION`, et tout
//! autre champ refuse.

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::conges::{avertir, flot, info};
use droit::contexte::Contexte;
use socle::date::{d, en_texte, Jour};
use crate::Avis;
use socle::fichiers::{repr, vrai};
use socle::num::{dec_avec_echelle, div, eur2, mul, Decimal, Resultat};

/// Le genre d'un arret de travail (bloc S21.G00.60).
pub const ARRET: &str = "arret";
/// Le genre d'une autre suspension du contrat (bloc S21.G00.65).
pub const SUSPENSION: &str = "suspension";
/// Le dossier du magasin, sous les donnees de la compagnie.
pub const DOSSIER: &str = "imprevus";

/// Les champs d'un arret, dans l'ordre ou ils s'ecrivent ; les `REQUIS_ARRET` premiers sont obligatoires. Ceux de la
/// norme (motif, dernier jour travaille, fin previsionnelle, subrogation, reprise, accident) sont ceux du bloc 60 et du
/// signalement (`signalement::verifier`) ; `jours_deja_indemnises` et `maintien` sont ceux du maintien de salaire d'un
/// permanent (D1226-4 ; le complement verse, mois par mois : `{"AAAA-MM": montant}`) ; `prolongations` garde chaque avis de
/// prolongation saisi (`moteur::routes::imprevus::arret_prolonger` : quand, la fin et la fin de subrogation d'avant et d'apres),
/// dont le moteur tire le signalement rectificatif qu'une subrogation prolongee appelle (`obligations::de_l_arret`).
pub const CHAMPS_ARRET: [&str; 20] = [
    "genre", "salarie", "contrat_debut", "contrat_numero", "motif", "dernier_jour_travaille", "fin_previsionnelle",
    "subrogation", "subrogation_debut", "subrogation_fin", "reprise", "reprise_motif", "date_accident",
    "jours_deja_indemnises", "maintien", "prolongations", "saisi_le", "qui", "note", "envois",
];
const REQUIS_ARRET: usize = 7;

/// Les champs d'une suspension (bloc 65 : motif, debut, fin), dans l'ordre ou ils s'ecrivent ; les `REQUIS_SUSPENSION`
/// premiers sont obligatoires.
pub const CHAMPS_SUSPENSION: [&str; 11] =
    ["genre", "salarie", "contrat_debut", "contrat_numero", "motif", "debut", "fin", "saisi_le", "qui", "note", "envois"];
const REQUIS_SUSPENSION: usize = 7;

const NUL: Value = Value::Null;

fn g<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).unwrap_or(&NUL)
}

fn s(v: &Value, k: &str) -> String {
    if vrai(g(v, k)) {
        en_texte(g(v, k))
    } else {
        String::new()
    }
}

/// Le genre d'un imprevu deja lu (`complet` l'a verifie).
pub fn genre(imp: &Value) -> &str {
    g(imp, "genre").as_str().unwrap_or("")
}

pub fn est_un_arret(imp: &Value) -> bool {
    genre(imp) == ARRET
}

pub fn est_une_suspension(imp: &Value) -> bool {
    genre(imp) == SUSPENSION
}

fn champs_du_genre(genre: &str) -> Option<(&'static [&'static str], usize)> {
    match genre {
        ARRET => Some((&CHAMPS_ARRET, REQUIS_ARRET)),
        SUSPENSION => Some((&CHAMPS_SUSPENSION, REQUIS_SUSPENSION)),
        _ => None,
    }
}

/// Un imprevu tel que le fichier le dit, verifie (un objet, un genre connu, aucun champ inconnu, les obligatoires
/// presents) et complete de ses valeurs par defaut, ses champs dans l'ordre du genre.
pub fn complet(v: &Value) -> Resultat<Value> {
    let Value::Object(m) = v else {
        return Err(format!("Imprevu : un objet est attendu, pas {}", socle::valeur::nom_type(v)));
    };
    let genre = m.get("genre").and_then(Value::as_str).unwrap_or("");
    let Some((champs, requis)) = champs_du_genre(genre) else {
        return Err(format!(
            "Imprevu de genre {} : un imprevu est un « {} » (arret de travail, bloc S21.G00.60) ou une « {} » (autre \
suspension du contrat, bloc S21.G00.65).",
            repr_valeur_courte(m.get("genre")),
            ARRET,
            SUSPENSION
        ));
    };
    if let Some(k) = m.keys().find(|k| !champs.contains(&k.as_str())) {
        return Err(format!("Imprevu ({}) : champ inconnu {}", genre, repr(k)));
    }
    let manque: Vec<String> = champs[..requis].iter().filter(|k| !m.contains_key(**k)).map(|k| repr(k)).collect();
    if !manque.is_empty() {
        return Err(format!("Imprevu ({}) : champ manquant {}", genre, manque.join(", ")));
    }
    let mut o = Map::new();
    for k in champs {
        let defaut = match *k {
            "qui" => json!("?"),
            "note" => json!(""),
            "envois" => json!([]),
            "maintien" => json!({}),
            "prolongations" => json!([]),
            _ => Value::Null,
        };
        o.insert(k.to_string(), m.get(*k).cloned().filter(|x| !(x.is_null() && matches!(*k, "envois" | "maintien" | "prolongations" | "note"))).unwrap_or(defaut));
    }
    if !o["envois"].is_array() {
        return Err(format!("Imprevu ({}) : `envois` n'est pas une liste", genre));
    }
    if genre == ARRET && !o["maintien"].is_object() {
        return Err("Arret : `maintien` est un objet {\"AAAA-MM\": montant}.".into());
    }
    if genre == ARRET && !o["prolongations"].is_array() {
        return Err("Arret : `prolongations` n'est pas une liste.".into());
    }
    Ok(Value::Object(o))
}

fn repr_valeur_courte(v: Option<&Value>) -> String {
    match v {
        None => "absent".into(),
        Some(x) => socle::fichiers::repr_valeur(x),
    }
}

// ------------------------------------------------------------------ le magasin

/// `imprevus/` sous les donnees de la compagnie.
pub fn dossier(racine: &Path) -> PathBuf {
    racine.join(DOSSIER)
}

fn chemin(racine: &Path, mois: &str) -> PathBuf {
    dossier(racine).join(format!("{}.json", mois))
}

/// `AAAA-MM.json`, et rien d'autre, est un fichier du magasin.
fn est_un_mois(nom: &str) -> bool {
    let b = nom.as_bytes();
    b.len() == 12 && b[4] == b'-' && nom.ends_with(".json") && b[..4].iter().chain(&b[5..7]).all(u8::is_ascii_digit)
}

/// Les imprevus d'un fichier deja lu.
fn du_contenu(contenu: &Value, chemin: &Path) -> Resultat<Vec<Value>> {
    let Value::Object(m) = contenu else {
        return Err(format!("{} : un objet est attendu, pas {}", chemin.display(), socle::valeur::nom_type(contenu)));
    };
    let liste = match m.get("imprevus") {
        None => return Ok(Vec::new()),
        Some(Value::Array(l)) => l,
        Some(autre) => return Err(format!("{} : `imprevus` n'est pas une liste ({})", chemin.display(), socle::valeur::nom_type(autre))),
    };
    liste.iter().map(|v| complet(v).map_err(|e| format!("{} : {}", chemin.display(), e))).collect()
}

/// Tous les imprevus de la compagnie, dans l'ordre des mois du magasin puis de leur saisie, verifies et complets. La
/// lecture se note au registre du calcul (`fichiers::lister`, `fichiers::lire_json`) : un imprevu saisi refait la paie.
pub fn tous(racine: &Path) -> Resultat<Vec<Value>> {
    let Some(fichiers) = socle::fichiers::lister(&dossier(racine)) else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    for f in fichiers {
        let nom = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if !est_un_mois(&nom) {
            continue;
        }
        out.extend(du_contenu(&socle::fichiers::lire_json(&f)?, &f)?);
    }
    Ok(out)
}

/// Les imprevus que le fichier d'un mois du magasin range, complets ; vide s'il n'existe pas.
pub fn du_fichier(racine: &Path, mois: &str) -> Resultat<Vec<Value>> {
    let p = chemin(racine, mois);
    if !socle::fichiers::existe(&p) {
        return Ok(Vec::new());
    }
    du_contenu(&socle::fichiers::lire_json(&p)?, &p)
}

/// Ecrit le fichier d'un mois du magasin.
pub fn ecrire(racine: &Path, mois: &str, imprevus: Vec<Value>) -> Resultat<()> {
    socle::ecriture::ecrire_json(
        &chemin(racine, mois),
        &json!({
            "_origine": format!(
                "Les imprevus ranges en {} : arrets de travail (au mois du dernier jour travaille) et suspensions (au mois \
de leur premier jour). Chacun se saisit une fois ; le moteur en tire la ligne de paie d'un permanent, le bloc 60 ou 65 \
de chaque DSN mensuelle dont le mois le croise, et, pour un arret, son signalement (nature 04, 05).",
                mois
            ),
            "mois": mois,
            "imprevus": imprevus,
        }),
        2,
    )
}

/// Le jour qui range un imprevu dans le magasin : le dernier jour travaille d'un arret, le premier jour d'une suspension.
pub fn jour_de_rangement(imp: &Value) -> Resultat<Jour> {
    if est_un_arret(imp) {
        d(g(imp, "dernier_jour_travaille"))
    } else {
        d(g(imp, "debut"))
    }
}

pub fn mois_de_rangement(imp: &Value) -> Resultat<String> {
    Ok(jour_de_rangement(imp)?.mois_iso())
}

// ------------------------------------------------------------------ les jours

/// Le premier jour d'absence : le lendemain du dernier jour travaille d'un arret (S21.G00.60.002), le debut d'une
/// suspension (S21.G00.65.002).
pub fn premier_jour(imp: &Value) -> Resultat<Jour> {
    if est_un_arret(imp) {
        d(g(imp, "dernier_jour_travaille"))?.plus_jours(1)
    } else {
        d(g(imp, "debut"))
    }
}

/// Le dernier jour d'absence : la veille de la reprise d'un arret, « premier jour travaille et paye suite a l'arret »
/// (S21.G00.60.010, p. 269), sinon sa fin previsionnelle (60.003) ; la fin d'une suspension (65.003).
pub fn dernier_jour(imp: &Value) -> Resultat<Jour> {
    if est_un_arret(imp) {
        if vrai(g(imp, "reprise")) {
            d(g(imp, "reprise"))?.plus_jours(-1)
        } else {
            d(g(imp, "fin_previsionnelle"))
        }
    } else {
        d(g(imp, "fin"))
    }
}

/// Une periode commune avec [debut, fin] : ce qui fait reporter l'imprevu dans la DSN de ce mois. « En cas de suspension
/// supra-mensuelle, la suspension doit etre declaree dans toutes les DSN mensuelles dont le Mois Principal declare
/// presente une periode commune avec la suspension » (bloc S21.G00.60, cahier technique DSN 2026.1, p. 265). Un arret
/// court de son dernier jour travaille (S21.G00.60.002) a son dernier jour d'absence (`dernier_jour`) : la veille de la
/// reprise, qui est le « premier jour travaille et paye suite a l'arret de travail » (S21.G00.60.010, p. 269), sinon la
/// fin previsionnelle (S21.G00.60.003, p. 267). Un arret repris le 1er du mois n'a donc aucun jour commun avec ce mois.
pub fn croise(imp: &Value, debut: &Jour, fin: &Jour) -> Resultat<bool> {
    if est_un_arret(imp) {
        return arret_croise(imp, debut, fin);
    }
    Ok(premier_jour(imp)? <= *fin && dernier_jour(imp)? >= *debut)
}

/// La meme regle pour un arret de travail, qu'il dise son genre ou non (un arret de la DSN, `dsn::Arret`).
pub fn arret_croise(arret: &Value, debut: &Jour, fin: &Jour) -> Resultat<bool> {
    let dernier = if vrai(g(arret, "reprise")) { d(g(arret, "reprise"))?.plus_jours(-1)? } else { d(g(arret, "fin_previsionnelle"))? };
    Ok(d(g(arret, "dernier_jour_travaille"))? <= *fin && dernier >= *debut)
}

/// Les imprevus d'un genre (ou de tous les genres) qui croisent le mois `AAAA-MM`.
pub fn du_mois(racine: &Path, mois: &str, genre_voulu: Option<&str>) -> Resultat<Vec<Value>> {
    let (debut, fin) = (socle::date::premier_du_mois(mois)?, socle::date::dernier_du_mois(mois)?);
    let mut out = Vec::new();
    for imp in tous(racine)? {
        if genre_voulu.is_some_and(|gv| gv != genre(&imp)) {
            continue;
        }
        if croise(&imp, &debut, &fin)? {
            out.push(imp);
        }
    }
    Ok(out)
}

/// L'imprevu suspend-il CE contrat ? Le meme salarie (la cle de sa fiche), la meme date de debut du contrat
/// (S21.G00.40.001), et le meme numero (40.009) quand l'un et l'autre en ont un : un contrat saisi avant sa premiere
/// production n'a pas encore de numero (il se fige a la production), et c'est sa date de debut qui le designe.
pub fn du_contrat(imp: &Value, cle: &str, numero: Option<&str>, debut: &Jour) -> Resultat<bool> {
    if s(imp, "salarie") != cle || d(g(imp, "contrat_debut"))? != *debut {
        return Ok(false);
    }
    let n_imp = s(imp, "contrat_numero");
    let n = numero.unwrap_or("");
    Ok(n_imp.is_empty() || n.is_empty() || n_imp == n)
}

// ------------------------------------------------------------- les nomenclatures

/// Le motif d'arret (`arrets-base.json`, `motifs_arret`), s'il existe.
pub fn motif_d_arret(code: &str) -> Resultat<Option<Value>> {
    let n = socle::fichiers::json_en_cache(&socle::fichiers::dossier_du_code().join("arrets-base.json"))?;
    Ok(n["motifs_arret"]["codes"].as_array().and_then(|l| l.iter().find(|c| c["code"].as_str() == Some(code))).cloned())
}

/// Le motif de suspension (`permanents-base.json`, section `suspensions`), s'il existe.
pub fn motif_de_suspension(code: &str) -> Resultat<Option<Value>> {
    let regles = droit::permanents::regles()?;
    Ok(regles["suspensions"]["motifs"].as_array().and_then(|l| l.iter().find(|m| m["code"].as_str() == Some(code))).cloned())
}

/// Ce que le motif d'une suspension interdit de declarer : un motif inconnu, ou le 200 (conges payes).
pub fn verifier_motif_de_suspension(code: &str, qui: &str) -> Resultat<Value> {
    let Some(m) = motif_de_suspension(code)? else {
        return Err(format!(
            "{} : motif de suspension {}. Les motifs connus sont dans donnees/permanents-base.json (section \
`suspensions`) ; un motif que le moteur ne connait pas ne se declare pas.",
            qui,
            if code.is_empty() { "absent".to_string() } else { format!("inconnu ({})", repr(code)) }
        ));
    };
    if vrai(g(&m, "_ne_pas_declarer")) {
        let regles = droit::permanents::regles()?;
        return Err(format!(
            "Motif de suspension {} ({}) : {} Les conges payes pris se saisissent dans le mois (« conges pris »).",
            code,
            en_texte(g(&m, "libelle")),
            en_texte(&regles["suspensions"]["_conges_payes_ne_sont_pas_une_suspension"])
        ));
    }
    Ok(m)
}

// ------------------------------------------------------------ le bloc 65 d'une paie

/// Les blocs S21.G00.65 d'une paie : chaque suspension du magasin qui suspend CE contrat et croise le mois que la DSN
/// declare, avec son motif et ses DEUX dates entieres (65.002 et 65.003) -- la meme dans chaque mois qu'elle traverse,
/// sans recopie (cahier technique DSN 2026.1, bloc S21.G00.65 p. 281, `atelier/reference/norme-dsn/texte/bloc-S21.G00.65.txt`).
pub fn suspensions_du_mois(ctx: &Contexte, paie: &crate::calcul::Paie, mois: &str) -> Resultat<Vec<Value>> {
    let cle = en_texte(g(&paie.salarie, "_cle"));
    let e = &paie.engagement;
    let qui = ctx.nomme(&cle);
    let mut out = Vec::new();
    let mut siennes = Vec::new();
    for imp in du_mois(&ctx.racine, mois, Some(SUSPENSION))? {
        if du_contrat(&imp, &cle, e.numero.as_deref(), &e.debut)? {
            siennes.push(imp);
        }
    }
    siennes.sort_by_key(|i| s(i, "debut"));
    for imp in siennes {
        let code = s(&imp, "motif");
        let motif = verifier_motif_de_suspension(&code, &qui)?;
        let (debut, fin) = (premier_jour(&imp)?, dernier_jour(&imp)?);
        if fin < debut {
            return Err(format!(
                "{} : corrigez les dates de son absence « {} », dans « Les imprévus » : elle finit le {}, avant son \
                 premier jour, le {}.",
                qui,
                en_texte(g(&motif, "libelle")),
                fin.jj_mm_aaaa(),
                debut.jj_mm_aaaa()
            ));
        }
        out.push(json!({"motif": code, "debut": debut.iso(), "fin": fin.iso()}));
    }
    Ok(out)
}

/// LA PERIODE ENTIERE SUSPENDUE SANS REMUNERATION : les suspensions du magasin (bloc 65, pas les arrets) qui suspendent
/// ce contrat -- la fiche `cle`, le contrat commence le `contrat_debut`, de numero `numero` -- dont le motif n'est pas
/// remunere (`permanents-base.json`, `suspensions.motifs`, `remunere`), quand elles couvrent a elles toutes chaque jour de
/// [debut, fin] ; vide sinon. Un mois traverse en partie n'est pas entierement suspendu.
pub fn suspendu_sans_remuneration(
    racine: &Path,
    cle: &str,
    numero: Option<&str>,
    contrat_debut: &Jour,
    debut: &Jour,
    fin: &Jour,
) -> Resultat<Vec<Value>> {
    let mut siennes = Vec::new();
    for imp in tous(racine)? {
        if !est_une_suspension(&imp) || !du_contrat(&imp, cle, numero, contrat_debut)? || !croise(&imp, debut, fin)? {
            continue;
        }
        let remunere = motif_de_suspension(&s(&imp, "motif"))?.is_none_or(|m| vrai(g(&m, "remunere")));
        if !remunere {
            siennes.push(imp);
        }
    }
    siennes.sort_by_key(|i| premier_jour(i).map(|j| j.iso()).unwrap_or_default());
    let mut couvert_jusqu_a = debut.plus_jours(-1)?;
    for imp in &siennes {
        if premier_jour(imp)? > couvert_jusqu_a.plus_jours(1)? {
            return Ok(Vec::new());
        }
        couvert_jusqu_a = couvert_jusqu_a.max(dernier_jour(imp)?);
    }
    Ok(if !siennes.is_empty() && couvert_jusqu_a >= *fin { siennes } else { Vec::new() })
}

/// LES JOURS DE CONGES QU'UNE SUSPENSION N'ACQUIERT PAS sur la presence [debut, fin] du contrat (`cle`, `numero`,
/// `contrat_debut`) : ceux des jours de ses suspensions dont le motif n'acquiert pas (`permanents-base.json`,
/// `suspensions.motifs`, `acquiert_conges`), au meme prorata des jours calendaires que l'acquisition
/// (`permanents::jours_acquis`). L3141-3 : 2,5 jours « par mois de travail effectif » ; L3141-5 et la CCN 3090, art.
/// 10.1.1, listent les periodes assimilees, et ni le conge parental, ni le sabbatique, ni le conge sans solde n'y sont
/// (`atelier/reference/sources/code-du-travail/articles-20261015.md`, `kali-3090-titres-vii-x-xi-annexes-20260924.md` ;
/// `suspension-non-remuneree/RAPPORT-20261015.md`, point 5). Un mois entierement suspendu n'acquiert rien ; un mois
/// suspendu en partie perd la part calendaire de la suspension (inference : L3141-4 compte en semaines de travail, le
/// moteur en jours calendaires, comme toute son acquisition). Les arrets de travail ne sont pas comptes ici.
pub fn jours_de_conges_non_acquis(
    racine: &Path,
    cle: &str,
    numero: Option<&str>,
    contrat_debut: &Jour,
    debut: &Jour,
    fin: &Jour,
) -> Resultat<Decimal> {
    let mut total = Decimal::ZERO;
    for imp in tous(racine)? {
        if !est_une_suspension(&imp) || !du_contrat(&imp, cle, numero, contrat_debut)? {
            continue;
        }
        let acquiert = motif_de_suspension(&s(&imp, "motif"))?.is_none_or(|m| vrai(g(&m, "acquiert_conges")));
        if acquiert {
            continue;
        }
        let (d, f) = (premier_jour(&imp)?.max(*debut), dernier_jour(&imp)?.min(*fin));
        if d <= f {
            total += droit::permanents::jours_acquis(&d, &f)?;
        }
    }
    Ok(total)
}

/// L'ANCIENNETE QUE LES SUSPENSIONS LAISSENT : le jour d'ou se compte l'anciennete du salarie `cle` au
/// `a`, `depuis` recule des jours de suspension qui n'y comptent pas, et ce qu'il y a a en dire. Pour les droits que le
/// salarie tient de son anciennete (le maintien de salaire, L1226-1 ; la prime d'anciennete de la 3090) :
///
/// - le conge parental d'education a temps plein (632) « est prise en compte pour moitie pour la determination des droits
///   que le salarie tient de son anciennete » (C. trav. L1225-54, `atelier/reference/sources/suspension-non-remuneree/
///   legi-extrait-code-du-travail-20261015.md`) : la moitie de ses jours ne compte pas (arrondie au jour, au profit du
///   salarie) ;
/// - une autre suspension non remuneree (sabbatique 639, sans solde 501...) : AUCUN TEXTE DEPOSE ne dit comment elle
///   compte ; le moteur ne la compte pas (le temps suspendu n'est pas du travail effectif, et L1225-54 n'accorde la moitie
///   qu'au conge parental : inference) et l'avertit ;
/// - une suspension remuneree (637) compte entiere ; un arret de travail n'est pas touche ici.
///
/// Tous les contrats du salarie comptent : l'anciennete est celle de l'entreprise (`date_anciennete`).
pub fn anciennete_depuis(racine: &Path, cle: &str, depuis: &Jour, a: &Jour) -> Resultat<(Jour, Vec<Avis>)> {
    let mut retranches = 0i64;
    let mut dit = Vec::new();
    for imp in tous(racine)? {
        if !est_une_suspension(&imp) || s(&imp, "salarie") != cle {
            continue;
        }
        let code = s(&imp, "motif");
        let Some(motif) = motif_de_suspension(&code)? else { continue };
        if vrai(g(&motif, "remunere")) {
            continue;
        }
        let (d, f) = (premier_jour(&imp)?.max(*depuis), dernier_jour(&imp)?.min(*a));
        if f < d {
            continue;
        }
        let jours = f.moins(&d) + 1;
        let libelle = en_texte(g(&motif, "libelle"));
        if g(&motif, "anciennete").as_str() == Some("moitie") {
            retranches += jours / 2;
            dit.push(info(format!(
                "{} : {} ({}) du {} au {} -- {} jour(s) compte(s) pour moitie dans l'anciennete (C. trav. L1225-54).",
                cle,
                libelle,
                code,
                d.jj_mm_aaaa(),
                f.jj_mm_aaaa(),
                jours
            )));
        } else {
            retranches += jours;
            dit.push(avertir(format!(
                "{} : {} ({}) du {} au {} -- {} jour(s) NON comptes dans l'anciennete : aucun texte depose ne dit comment \
cette suspension compte (le conge parental seul compte pour moitie, L1225-54). Si la convention ou un accord de la compagnie \
la compte, a reprendre a la main.",
                cle,
                libelle,
                code,
                d.jj_mm_aaaa(),
                f.jj_mm_aaaa(),
                jours
            )));
        }
    }
    Ok((depuis.plus_jours(retranches)?, dit))
}

/// Les arrets du magasin qui suspendent ce contrat et croisent [debut, fin] (le FCTU les reporte, bloc 60).
pub fn arrets_du_contrat(ctx: &Contexte, paie: &crate::calcul::Paie, debut: &Jour, fin: &Jour) -> Resultat<Vec<Value>> {
    let cle = en_texte(g(&paie.salarie, "_cle"));
    let mut out = Vec::new();
    for imp in tous(&ctx.racine)? {
        if est_un_arret(&imp) && du_contrat(&imp, &cle, paie.engagement.numero.as_deref(), &paie.engagement.debut)? && croise(&imp, debut, fin)? {
            out.push(imp);
        }
    }
    Ok(out)
}

// ------------------------------------------------------ les lignes d'un permanent

/// Ce que le mois d'un contrat paye au mois donne pour mesurer une absence (`conges::lignes_du_mois`) : les bornes du
/// contrat dans le mois, le salaire que la ligne « mois » paie, les heures d'un jour oeuvre et celles du mois au reel
/// (`conges::heures_par_jour_oeuvre`, `conges::jours_oeuvres`), et ce que vaut un jour ouvrable (l'assiette du maintien de
/// salaire que la loi donne, dit sans etre applique).
pub struct MoisDuContrat {
    pub debut: Jour,
    pub fin: Jour,
    pub salaire_du_mois: Decimal,
    pub heures_par_jour: Decimal,
    pub heures_du_mois: Decimal,
    pub taux_jour_ouvrable: Decimal,
}

/// UN TAUX SANS ARRONDI : le quotient tel quel, en texte (`num::dec` le relit sans perte), pour que la quantite fois le
/// taux, arrondi une fois au centime par la ligne (`calcul::lignes_brut`), donne le montant du quotient -- et le
/// salaire entier quand les heures sont toutes celles du mois. Un taux arrondi au centime AVANT de multiplier laissait
/// 0,07 EUR de salaire sur un mois entier d'absence (2 310 / 151,67 = 15,23, x 151,67 = 2 309,93) : aucun texte ne
/// l'arrondit (Cass. soc. 11/02/1982, ci-dessous, est un quotient).
fn taux_exact(montant: Decimal, quantite: Decimal) -> Value {
    Value::String(div(montant, quantite).normalize().to_string())
}

/// La cle de la fiche du salarie d'un engagement de saisie.
fn cle_de(ctx: &Contexte, e: &Value) -> Resultat<String> {
    let nom = en_texte(g(e, "salarie"));
    Ok(en_texte(g(&ctx.salarie(&nom)?, "_cle")))
}

/// Les imprevus du contrat d'un engagement de saisie qui croisent le mois, du premier au dernier jour.
pub fn du_contrat_dans_le_mois(ctx: &Contexte, e: &Value, mois: &str) -> Resultat<Vec<Value>> {
    // Un contrat sans date de debut ecrite ne se designe pas : aucun imprevu ne peut le nommer (S21.G00.40.001).
    if !vrai(g(e, "debut")) {
        return Ok(Vec::new());
    }
    let cle = cle_de(ctx, e)?;
    let debut = d(g(e, "debut"))?;
    let numero = s(e, "numero");
    let mut out = Vec::new();
    for imp in du_mois(&ctx.racine, mois, None)? {
        if du_contrat(&imp, &cle, Some(numero.as_str()), &debut)? {
            out.push(imp);
        }
    }
    out.sort_by_key(|i| premier_jour(i).map(|j| j.iso()).unwrap_or_default());
    Ok(out)
}

/// LES LIGNES QU'UN IMPREVU DONNE AU MOIS D'UN PERMANENT -- le seul endroit qui les tire. Chaque imprevu du contrat
/// retire le salaire de ses heures d'absence dans le mois (bornees par le contrat) ; un arret le retire sous
/// `absence_maladie` (le catalogue la marque `arret_de_travail`), une suspension non remuneree sous
/// `absence_non_remuneree`. Une suspension que `permanents-base.json` dit remuneree (637 : C. trav. L3142-2, « n'entrainent
/// pas de reduction de la remuneration ») ne retire rien. Le maintien de salaire d'un arret est celui SAISI SUR L'ARRET
/// pour ce mois (`maintien`) : la loi le dit (`maintien_possible`), elle ne le verse pas -- les indemnites journalieres a
/// deduire (D1226-5) ne se devinent pas.
///
/// LA RETENUE AU REEL DU MOIS. « La retenue par heure d'absence d'un salarie paye au mois doit etre en
/// principe egale au quotient du salaire mensuel par le nombre d'heures de travail dans l'entreprise pour le mois
/// considere », et non sur un horaire moyen (Cass. soc. 11/02/1982, n° 80-40.359 ; 14/05/1987, n° 84-43.854 :
/// `atelier/reference/sources/suspension-non-remuneree/cass-soc-19820211-80-40359-JURITEXT000007009242.xml` et
/// `cass-soc-19870514-84-43854-JURITEXT000007018764.xml` ; la DDETS en tire « salaire mensualise / heures programmees sur
/// le mois », `dreets-limousin-qr-assmat-retenues-absence-20150101.txt`). Les heures d'absence se comptent au reel
/// (fiche DSN 2640, « sans application d'une regle de trois visant a rapprocher le volume d'absence a la quotite de
/// travail mensualisee » ; fiche 2693, exemple 10 : septembre 2026 entier, 154 h = 22 x 7 h ; meme dossier) : les jours
/// oeuvres de l'absence fois les heures d'un jour (`conges::jours_oeuvres`, `conges::heures_par_jour_oeuvre` : l'hypothese
/// classique, du lundi au vendredi, est une inference nommee). La retenue est le salaire du mois fois ces heures sur les
/// heures du mois, au taux exact (`taux_exact`) : un mois entier d'absence retire exactement le salaire du mois.
pub fn lignes_du_mois(ctx: &Contexte, e: &Value, mois: &str, m: &MoisDuContrat) -> Resultat<(Vec<Value>, Vec<Avis>)> {
    let qui = crate::conges::qui(ctx, e);
    let ent = &ctx.entreprise;
    let (mut lignes, mut avert) = (Vec::new(), Vec::new());
    // Le taux de l'heure d'absence : le salaire du mois sur ses heures de travail (Cass. soc. 11/02/1982).
    let taux_horaire = if m.heures_du_mois > Decimal::ZERO { Some(taux_exact(m.salaire_du_mois, m.heures_du_mois)) } else { None };
    for imp in du_contrat_dans_le_mois(ctx, e, mois)? {
        let (premier, dernier) = (premier_jour(&imp)?, dernier_jour(&imp)?);
        let (debut, fin) = (premier.max(m.debut), dernier.min(m.fin));
        let oeuvres = if debut <= fin { Decimal::from(droit::conges::jours_oeuvres(&debut, &fin, Some(ent))?) } else { Decimal::ZERO };
        let heures = eur2(mul(m.heures_par_jour, oeuvres));
        let retenue = |rubrique: &str| -> Option<Value> {
            match &taux_horaire {
                Some(t) if heures > Decimal::ZERO => Some(json!({"rubrique": rubrique, "quantite": flot(heures), "taux": t})),
                _ => None,
            }
        };
        let au_reel = format!(
            "{} jour(s) oeuvre(s) d'absence ce mois-ci, {} h au reel sur {} h de travail du mois, retirees au quotient du \
salaire du mois",
            oeuvres, heures, m.heures_du_mois
        );
        let code = s(&imp, "motif");
        if est_un_arret(&imp) {
            let motif = motif_d_arret(&code)?.unwrap_or(Value::Null);
            lignes.extend(retenue("absence_maladie"));
            avert.push(info(format!(
                "{} : arret de travail ({}) du {} au {}, saisi une fois : {} ; le bloc S21.G00.60 part dans la DSN de \
chaque mois qu'il traverse.",
                qui,
                en_texte(g(&motif, "libelle")),
                premier.jj_mm_aaaa(),
                dernier.jj_mm_aaaa(),
                au_reel
            )));
            if vrai(g(&motif, "maintien_legal")) {
                avert.extend(maintien_possible(ctx, e, &imp, mois, m.taux_jour_ouvrable)?);
            }
            if let Some(montant) = g(&imp, "maintien").get(mois).filter(|x| vrai(x)) {
                let montant = dec_avec_echelle(montant)?;
                // Le montant saisi est verse tel quel : un taux arrondi avant de multiplier le changerait (`taux_exact`).
                let (q, t) = if heures > Decimal::ZERO { (heures, taux_exact(montant, heures)) } else { (Decimal::ONE, json!(flot(montant))) };
                lignes.push(json!({"rubrique": "maintien_salaire", "quantite": flot(q), "taux": t}));
            }
        } else {
            let motif = verifier_motif_de_suspension(&code, &qui)?;
            let libelle = en_texte(g(&motif, "libelle"));
            if vrai(g(&motif, "remunere")) {
                avert.push(info(format!(
                    "{} : suspension {} ({}) du {} au {} : un conge remunere, rien n'est retire du salaire ({}).",
                    qui,
                    code,
                    libelle,
                    premier.jj_mm_aaaa(),
                    dernier.jj_mm_aaaa(),
                    en_texte(g(&motif, "_remunere"))
                )));
            } else {
                lignes.extend(retenue("absence_non_remuneree"));
                avert.push(info(format!(
                    "{} : suspension {} ({}) du {} au {}, saisie une fois : {} ; le bloc S21.G00.65 la declare entiere \
dans la DSN de chaque mois qu'elle traverse.",
                    qui,
                    code,
                    libelle,
                    premier.jj_mm_aaaa(),
                    dernier.jj_mm_aaaa(),
                    au_reel
                )));
            }
        }
    }
    Ok((lignes, avert))
}

/// L'origine d'un arret pour le maintien de salaire, lue sur son motif (`arrets-base.json`, `origine`) : `at_mp` (accident
/// du travail, maladie professionnelle : pas de carence) ou `trajet` (un accident qui garde la sienne), D1226-3
/// (`atelier/reference/sources/code-du-travail/articles-20261015.md`).
pub fn origine(imp: &Value) -> Resultat<(bool, bool)> {
    let motif = motif_d_arret(&s(imp, "motif"))?.unwrap_or(Value::Null);
    Ok(match g(&motif, "origine").as_str() {
        Some("at_mp") => (true, false),
        Some("trajet") => (true, true),
        _ => (false, false),
    })
}

/// Ce que la loi donne pour cet arret-la, dit sans etre applique : L1226-1, D1226-1 a D1226-4 (le droit, la carence, les
/// tranches, les jours deja indemnises dans les douze mois, saisis sur l'arret), sur l'arret entier.
fn maintien_possible(ctx: &Contexte, e: &Value, imp: &Value, mois: &str, remuneration_journaliere: Decimal) -> Resultat<Vec<Avis>> {
    use droit::permanents::{anciennete_mois, droit_au_maintien, maintien_de_salaire, OptionsMaintien};
    let qui = crate::conges::qui(ctx, e);
    let nat = crate::conges::nature_de(e.get("nature"))?;
    let premier = premier_jour(imp)?;
    let fiche = ctx.salarie(&en_texte(g(e, "salarie")))?;
    let depuis = d(match fiche.get("date_anciennete").filter(|x| vrai(x)) {
        Some(v) => v,
        None => g(e, "debut"),
    })?;
    // L'anciennete que les suspensions laissent (L1225-54 : `anciennete_depuis`).
    let (depuis, sur_l_anciennete) = anciennete_depuis(&ctx.racine, &en_texte(g(&fiche, "_cle")), &depuis, &premier)?;
    let anciennete = anciennete_mois(&depuis, &premier);
    if let Some(refus) = droit_au_maintien(Some(&nat), anciennete)? {
        let mut dit = sur_l_anciennete;
        dit.push(avertir(format!("{} : pas de maintien de salaire pour cet arret -- {}", qui, refus)));
        return Ok(dit);
    }
    let jours = dernier_jour(imp)?.moins(&premier) + 1;
    let (at, trajet) = origine(imp)?;
    let deja = match g(imp, "jours_deja_indemnises") {
        x if vrai(x) => crate::conges::entier(x)?,
        _ => 0,
    };
    let r = maintien_de_salaire(
        anciennete,
        jours,
        remuneration_journaliere,
        &OptionsMaintien { deja_indemnises: deja, accident_du_travail: at, accident_de_trajet: trajet },
    )?;
    let texte = |k: &str| en_texte(&r[k]);
    let tranches: Vec<String> = r["tranches"]
        .as_array()
        .map_or(&[][..], |a| &a[..])
        .iter()
        .map(|x| format!("{} jour(s) a {} %", en_texte(&x["jours"]), en_texte(&x["taux"])))
        .collect();
    let tranches = if tranches.is_empty() { "aucun jour".to_string() } else { tranches.join(", ") };
    let carence = r["carence"].as_i64().unwrap_or(0);
    let mut dit = vec![info(format!(
        "{} : arret de {} jour(s) en tout. La loi donne {} apres {} jour(s) de carence ({}) -- soit {} EUR bruts AVANT \
deduction des indemnites journalieres (L1226-1, D1226-1 a D1226-3).",
        qui,
        jours,
        tranches,
        carence,
        if carence == 0 { "aucune, accident du travail ou maladie professionnelle" } else { "D1226-3" },
        texte("brut_avant_deduction")
    ))];
    let au_dela = r["jours_au_dela_du_droit"].as_i64().unwrap_or(0);
    if au_dela != 0 {
        dit.push(avertir(format!(
            "{} : {} jour(s) de cet arret depassent la duree d'indemnisation a laquelle l'anciennete donne droit (D1226-2).",
            qui, au_dela
        )));
    }
    if let Some(conv) = droit::clauses::maintien(ctx.entreprise.get("idcc").unwrap_or(&Value::Null), anciennete, at)? {
        dit.push(avertir(format!("{} : {}", qui, conv)));
    }
    if !g(imp, "maintien").get(mois).is_some_and(vrai) {
        dit.push(avertir(format!(
            "{} : le maintien de salaire de {} n'est pas saisi sur l'arret, donc rien n'est verse a ce titre. Le \
complement du est le brut ci-dessus moins les indemnites journalieres et la prevoyance, que la caisse communique : le \
porter sur l'arret, maintien du mois.",
            qui,
            crate::libelle_mois(mois)
        )));
    }
    dit.extend(sur_l_anciennete);
    Ok(dit)
}

// ------------------------------------------------------------- les jours payes

/// Les jours d'une ligne de saisie (`dates`), lus.
fn dates_de_ligne(l: &Value) -> Vec<Jour> {
    l.get("dates").and_then(Value::as_array).map_or_else(Vec::new, |a| a.iter().filter_map(|x| d(x).ok()).collect())
}

/// Le libelle d'une rubrique, ou son code si le catalogue ne la connait pas.
fn libelle_rubrique(racine: &Path, code: &str, jour: &Jour) -> String {
    droit::catalogue::rubrique(racine, code, jour).map(|r| r.libelle).unwrap_or_else(|_| code.to_string())
}

/// LES CACHETS ET SERVICES DE LA SAISIE QUI TOMBENT DANS UN IMPREVU : pour chaque mois saisi qu'il croise, les lignes des
/// engagements du MEME salarie (tout contrat : un arret de travail est une suspension « durant laquelle le salarie ne
/// peut exercer son activite », bloc S21.G00.60 p. 265) dont une date tombe entre son premier et son dernier jour
/// d'absence. Une ligne par ligne de saisie, avec les seules dates qui tombent dedans, pour qu'on les retire une a une.
/// Le moteur ne retire rien (regle 1).
pub fn cachets_dans(ctx: &Contexte, imp: &Value) -> Resultat<Vec<Value>> {
    let (premier, dernier) = (premier_jour(imp)?, dernier_jour(imp)?);
    let cle = s(imp, "salarie");
    let mut out = Vec::new();
    if dernier < premier {
        return Ok(out);
    }
    for mois in socle::date::mois_entre(&premier, &dernier) {
        let Some(data) = crate::stock::vue_du_mois(&ctx.racine, &mois)? else { continue };
        out.extend(cachets_de_la_saisie(ctx, &data, &mois, &cle, &premier, &dernier)?);
    }
    Ok(out)
}

fn cachets_de_la_saisie(ctx: &Contexte, data: &Value, mois: &str, cle: &str, premier: &Jour, dernier: &Jour) -> Resultat<Vec<Value>> {
    let mut out = Vec::new();
    for (i, e) in data.get("engagements").and_then(Value::as_array).map_or(&[][..], |a| &a[..]).iter().enumerate() {
        if crate::cle_de_l_engagement(ctx, e).as_deref() != Some(cle) {
            continue;
        }
        for (j, l) in e.get("lignes").and_then(Value::as_array).map_or(&[][..], |a| &a[..]).iter().enumerate() {
            let dedans: Vec<String> = dates_de_ligne(l).into_iter().filter(|x| premier <= x && x <= dernier).map(|x| x.iso()).collect();
            if dedans.is_empty() {
                continue;
            }
            let rubrique = s(l, "rubrique");
            out.push(json!({
                "mois": mois, "engagement": i, "numero": g(e, "numero"), "spectacle": g(e, "spectacle"),
                "ligne": j, "rubrique": rubrique, "libelle": libelle_rubrique(&ctx.racine, &rubrique, &ctx.lue_au),
                "dates": dedans, "quantite": g(l, "quantite"), "taux": g(l, "taux"),
            }));
        }
    }
    Ok(out)
}

/// UN JOUR PAYE PENDANT UN IMPREVU EST NOMME (regle 1 : le moteur montre, il ne retire rien). Pour chaque engagement de
/// la saisie (dans l'ordre de `engagements`), les avertissements de ses lignes datees d'un jour d'absence du salarie :
/// un arret de travail est une suspension « durant laquelle le salarie ne peut exercer son activite » (bloc S21.G00.60,
/// cahier technique DSN 2026.1 p. 265), une autre suspension, une raison « pour laquelle le contrat est suspendu »
/// (bloc S21.G00.65, p. 281).
pub fn avertir_les_jours_payes(ctx: &Contexte, data: &Value) -> Resultat<Vec<Vec<Avis>>> {
    let engagements = data.get("engagements").and_then(Value::as_array).map_or(&[][..], |a| &a[..]);
    let mut out: Vec<Vec<Avis>> = vec![Vec::new(); engagements.len()];
    let tous = tous(&ctx.racine)?;
    if tous.is_empty() {
        return Ok(out);
    }
    let mois = data.get("mois").map(en_texte).unwrap_or_default();
    for imp in &tous {
        let (premier, dernier) = (premier_jour(imp)?, dernier_jour(imp)?);
        for c in cachets_de_la_saisie(ctx, data, &mois, &s(imp, "salarie"), &premier, &dernier)? {
            let i = c["engagement"].as_u64().unwrap_or(0) as usize;
            let dates: Vec<String> = c["dates"].as_array().map_or_else(Vec::new, |a| a.iter().filter_map(|x| d(x).ok()).map(|x| x.jj_mm_aaaa()).collect());
            let quoi = if est_un_arret(imp) {
                let motif = motif_d_arret(&s(imp, "motif"))?.unwrap_or(Value::Null);
                format!(
                    "l'arret de travail ({}) du {} au {} : un salarie en arret ne peut exercer son activite (bloc S21.G00.60)",
                    en_texte(g(&motif, "libelle")),
                    premier.jj_mm_aaaa(),
                    dernier.jj_mm_aaaa()
                )
            } else {
                format!(
                    "la suspension {} du {} au {} : le contrat est suspendu (bloc S21.G00.65)",
                    s(imp, "motif"),
                    premier.jj_mm_aaaa(),
                    dernier.jj_mm_aaaa()
                )
            };
            if let Some(a) = out.get_mut(i) {
                a.push(avertir(format!(
                    "{} {} : {} le {} paye(s) pendant {}. Retirer ce jour de la ligne, ou corriger les dates de l'imprevu.",
                    s(imp, "salarie"),
                    if c["numero"].is_null() { String::new() } else { format!("(contrat {})", en_texte(&c["numero"])) },
                    en_texte(&c["libelle"]),
                    dates.join(", "),
                    quoi
                )));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_imprevu_se_complete_et_refuse_ce_qu_il_ne_connait_pas() {
        let a = complet(&json!({"genre": "arret", "salarie": "X", "contrat_debut": "2026-01-01", "contrat_numero": "N",
            "motif": "01", "dernier_jour_travaille": "2026-02-02", "fin_previsionnelle": "2026-02-10"}))
        .unwrap();
        let mut champs: Vec<String> = a.as_object().unwrap().keys().cloned().collect();
        champs.sort();
        let mut attendus: Vec<String> = CHAMPS_ARRET.iter().map(|c| c.to_string()).collect();
        attendus.sort();
        assert_eq!(champs, attendus, "un arret complete porte tous ses champs");
        assert_eq!((a["qui"].clone(), a["maintien"].clone(), a["envois"].clone()), (json!("?"), json!({}), json!([])));
        assert_eq!(a["prolongations"], json!([]));
        assert!(complet(&json!({"genre": "arret", "salarie": "X"})).unwrap_err().contains("champ manquant"));
        assert!(complet(&json!({"genre": "autre"})).unwrap_err().contains("genre"));
        let mut s = json!({"genre": "suspension", "salarie": "X", "contrat_debut": "2026-01-01", "contrat_numero": "N",
            "motif": "632", "debut": "2026-02-01", "fin": "2026-04-30"});
        assert!(complet(&s).is_ok());
        s["heures"] = json!(3);
        assert!(complet(&s).unwrap_err().contains("champ inconnu"));
    }

    #[test]
    fn les_jours_d_un_arret_vont_du_lendemain_du_dernier_jour_travaille_a_la_veille_de_la_reprise() {
        let mut a = json!({"genre": "arret", "dernier_jour_travaille": "2026-03-10", "fin_previsionnelle": "2026-03-20", "reprise": null});
        assert_eq!(premier_jour(&a).unwrap().iso(), "2026-03-11");
        assert_eq!(dernier_jour(&a).unwrap().iso(), "2026-03-20");
        a["reprise"] = json!("2026-03-15");
        assert_eq!(dernier_jour(&a).unwrap().iso(), "2026-03-14");
    }
}
