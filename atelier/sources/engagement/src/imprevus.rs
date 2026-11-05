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
use droit::permanents::MotifDeSuspension;
use droit::contexte::Contexte;
use droit::fiche::SalarieDuContrat;
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
/// signalement (`signalement::verifier`) ; `debut` est le premier jour que l'avis d'arret prescrit, d'ou se comptent
/// l'incapacite (CSS R323-1, 1°) et ce que la DSN declare (`dernier_jour_declare`) ; `jours_deja_indemnises` et les indemnites journalieres par
/// jour, brutes et nettes de CSG et CRDS (`ij_brute`, `ij_nette`, et `_majoree` au-dela du vingt-huitieme jour d'un accident du
/// travail) sont ceux du maintien de salaire d'un permanent (`maintien`) ; `prolongations` garde chaque avis de
/// prolongation saisi (`moteur::routes::imprevus::arret_prolonger` : quand, la fin et la fin de subrogation d'avant et d'apres),
/// dont le moteur tire le signalement rectificatif qu'une subrogation prolongee appelle (`obligations::de_l_arret`) ; `reprises`
/// garde chaque reprise notee ou effacee (`arret_reprise` : quand, la reprise et son motif d'avant et d'apres). Par ces deux
/// traces, l'arret se relit tel qu'il etait a un instant passe (`moteur::routes::imprevus::arret_a_l_instant`).
pub const CHAMPS_ARRET: [&str; 24] = [
    "genre", "salarie", "contrat_debut", "contrat_numero", "motif", "dernier_jour_travaille", "debut", "fin_previsionnelle",
    "subrogation", "subrogation_debut", "subrogation_fin", "reprise", "reprise_motif", "date_accident",
    "jours_deja_indemnises", "ij_brute", "ij_nette", "ij_brute_majoree", "ij_nette_majoree", "prolongations", "reprises", "saisi_le", "qui", "note",
];
const REQUIS_ARRET: usize = 8;

/// Les champs d'une suspension (bloc 65 : motif, debut, fin), dans l'ordre ou ils s'ecrivent ; les `REQUIS_SUSPENSION`
/// premiers sont obligatoires.
pub const CHAMPS_SUSPENSION: [&str; 10] =
    ["genre", "salarie", "contrat_debut", "contrat_numero", "motif", "debut", "fin", "saisi_le", "qui", "note"];
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
            "prolongations" | "reprises" => json!([]),
            _ => Value::Null,
        };
        o.insert(k.to_string(), m.get(*k).cloned().filter(|x| !(x.is_null() && matches!(*k, "prolongations" | "reprises" | "note"))).unwrap_or(defaut));
    }
    if genre == ARRET {
        for k in ["ij_brute", "ij_nette", "ij_brute_majoree", "ij_nette_majoree"] {
            if vrai(&o[k]) && dec_avec_echelle(&o[k]).map_or(true, |x| x <= Decimal::ZERO) {
                return Err(socle::echec::a_la_cle("Un montant par jour de l'arrêt doit être positif.", k, en_texte(&o[k])));
            }
        }
    }
    for k in ["prolongations", "reprises"] {
        if genre == ARRET && !o[k].is_array() {
            return Err(socle::echec::charge(k, "une liste"));
        }
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

/// Les imprevus d'un fichier deja lu.
fn du_contenu(contenu: &Value, chemin: &Path) -> Resultat<Vec<Value>> {
    let Value::Object(m) = contenu else {
        return Err(socle::echec::abime(chemin, format!("un objet est attendu, pas {}", socle::valeur::nom_type(contenu))));
    };
    let liste = match m.get("imprevus") {
        None => return Ok(Vec::new()),
        Some(Value::Array(l)) => l,
        Some(autre) => return Err(socle::echec::abime(chemin, format!("imprevus : une liste est attendue, pas {}", socle::valeur::nom_type(autre)))),
    };
    liste.iter().map(|v| complet(v).map_err(|e| socle::echec::du_disque(chemin, &e))).collect()
}

/// Tous les imprevus de la compagnie, dans l'ordre des mois du magasin puis de leur saisie, verifies et complets. La
/// lecture se note au registre du calcul (`fichiers::lister`, `fichiers::lire_json`) : un imprevu saisi refait la paie.
pub fn tous(racine: &Path) -> Resultat<Vec<Value>> {
    let Some(fichiers) = socle::fichiers::lister(&dossier(racine)) else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    for f in fichiers {
        let nom = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if !nom.strip_suffix(".json").is_some_and(|m| socle::mois::Mois::lire(m).is_ok()) {
            continue;
        }
        out.extend(du_contenu(&socle::fichiers::lire_json(&f)?, &f)?);
    }
    Ok(out)
}

/// LES IMPREVUS D'UN SALARIE (`cle`, la cle de sa fiche), dans l'ordre du magasin : ce que le calcul de ses paies lit,
/// et rien d'autre. La lecture se note au registre du calcul en une entree, `imprevus racine:imprevus <cle>`, qui vaut
/// l'empreinte de ces seuls imprevus (`empreinte_du_salarie`) : un imprevu saisi pour un autre salarie ne refait pas
/// ses paies.
pub fn du_salarie(racine: &Path, cle: &str) -> Resultat<Vec<Value>> {
    let siens = siens_sans_noter(racine, cle)?;
    socle::fichiers::noter_valeur(&entree_du_salarie(cle), empreinte_de(&siens));
    Ok(siens)
}

/// L'entree du registre que `du_salarie` note.
pub fn entree_du_salarie(cle: &str) -> String {
    format!("{} racine:{} {}", DOSSIER, DOSSIER, cle)
}

/// Le salarie d'une entree `du_salarie`, s'il en est une.
pub fn salarie_de_l_entree(k: &str) -> Option<&str> {
    k.strip_prefix(DOSSIER)?.strip_prefix(" racine:")?.strip_prefix(DOSSIER)?.strip_prefix(' ')
}

/// Ce que vaut aujourd'hui l'entree `du_salarie` de `cle` : l'empreinte de ses imprevus, relus sans rien noter.
pub fn empreinte_du_salarie(racine: &Path, cle: &str) -> Resultat<String> {
    Ok(empreinte_de(&siens_sans_noter(racine, cle)?))
}

fn empreinte_de(imprevus: &[Value]) -> String {
    socle::fichiers::sha256_hex(Value::Array(imprevus.to_vec()).to_string().as_bytes())
}

/// Les imprevus de `cle`, lus mois par mois du magasin sans se noter (`fichiers::json_partage`).
fn siens_sans_noter(racine: &Path, cle: &str) -> Resultat<Vec<Value>> {
    let mut fichiers: Vec<PathBuf> = match std::fs::read_dir(dossier(racine)) {
        Ok(l) => l.flatten().map(|f| f.path()).filter(|p| p.file_stem().and_then(|n| n.to_str()).is_some_and(|m| socle::mois::Mois::lire(m).is_ok()) && p.extension().is_some_and(|x| x == "json")).collect(),
        Err(_) => return Ok(Vec::new()),
    };
    fichiers.sort();
    let mut out = Vec::new();
    for f in fichiers {
        let Some(contenu) = socle::fichiers::json_partage(&f)? else { continue };
        out.extend(du_contenu(&contenu, &f)?.into_iter().filter(|imp| s(imp, "salarie") == cle));
    }
    Ok(out)
}

/// Les imprevus d'un genre (ou de tous les genres) d'un salarie (`du_salarie`) qui croisent le mois `AAAA-MM`.
pub fn du_mois_du_salarie(racine: &Path, mois: &str, genre_voulu: Option<&str>, cle: &str) -> Resultat<Vec<Value>> {
    croisant(du_salarie(racine, cle)?, mois, genre_voulu)
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

// ------------------------------------------------------------------ les jours

/// LE DERNIER JOUR TRAVAILLE QUE LA DSN DECLARE D'UN ARRET (S21.G00.60.002) : « la veille de la date de debut de la
/// prescription de l'arret de travail, sauf [...] arret prescrit lors d'un jour travaille : le dernier jour travaille
/// correspond alors a la date de l'arret » (cahier technique DSN 2026.1, p. 266-267,
/// `atelier/reference/norme-dsn/texte/bloc-S21.G00.60.txt`). Un arret prescrit un jour travaille a son debut egal au
/// dernier jour travaille saisi ; sinon la DSN declare la veille de son debut, meme un dimanche : les organismes en lisent
/// le debut de l'arret, « Date du dernier jour travaille S21.G00.60.002 + 1 [...] Aucune autre donnee ne permet d'avoir la
/// date de debut de l'arret reelle en DSN » (fiche consigne 568, `atelier/reference/sources/net-entreprises-base-connaissances/
/// net-entreprises-de-la-dsn-a-l-attestation-employeur-v3-2-20230328.txt`, l. 1683).
pub fn dernier_jour_declare(arret: &Value) -> Resultat<Jour> {
    let (travaille, debut) = (d(g(arret, "dernier_jour_travaille"))?, d(g(arret, "debut"))?);
    Ok(if debut <= travaille { travaille } else { debut.plus_jours(-1)? })
}

/// Le premier jour d'absence : le lendemain du dernier jour que la DSN declare d'un arret (`dernier_jour_declare`) -- le
/// debut que l'avis prescrit, ou son lendemain quand il est prescrit un jour travaille --, d'ou se compte « le quatrieme
/// jour de l'incapacite de travail » (CSS R323-1, 1°) ; le debut d'une suspension (S21.G00.65.002).
pub fn premier_jour(imp: &Value) -> Resultat<Jour> {
    if est_un_arret(imp) {
        dernier_jour_declare(imp)?.plus_jours(1)
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
/// court du dernier jour travaille qu'il declare (S21.G00.60.002, `dernier_jour_declare`) a son dernier jour d'absence (`dernier_jour`) : la veille de la
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
    Ok(dernier_jour_declare(arret)? <= *fin && dernier >= *debut)
}

/// Les imprevus d'un genre (ou de tous les genres) qui croisent le mois `AAAA-MM`.
pub fn du_mois(racine: &Path, mois: &str, genre_voulu: Option<&str>) -> Resultat<Vec<Value>> {
    croisant(tous(racine)?, mois, genre_voulu)
}

fn croisant(imprevus: Vec<Value>, mois: &str, genre_voulu: Option<&str>) -> Resultat<Vec<Value>> {
    let (debut, fin) = (socle::mois::Mois::lire(mois)?.premier_jour(), socle::mois::Mois::lire(mois)?.dernier_jour());
    let mut out = Vec::new();
    for imp in imprevus {
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

// ------------------------------------------------------------- les motifs de suspension

/// Le motif de suspension (`permanents-base.json`, section `suspensions`), s'il existe.
pub fn motif_de_suspension(code: &str) -> Resultat<Option<MotifDeSuspension>> {
    droit::permanents::motif_de_suspension(code)
}

/// Ce que le motif d'une suspension interdit de declarer : un motif inconnu, ou le 200 (conges payes).
pub fn verifier_motif_de_suspension(code: &str, qui: &str) -> Resultat<MotifDeSuspension> {
    let Some(m) = motif_de_suspension(code)? else {
        return Err(socle::echec::cle(
            format!(
                "{} : motif de suspension {}, que le logiciel ne connaît pas : le choisir dans la liste des motifs.",
                qui,
                if code.is_empty() { "absent".to_string() } else { format!("inconnu ({})", repr(code)) }
            ),
            "motif",
        ));
    };
    if let Some(pourquoi) = &m.ne_se_declare_pas {
        return Err(format!(
            "Motif de suspension {} ({}) : {} Les conges payes pris se saisissent dans le mois (« conges pris »).",
            code, m.libelle, pourquoi
        ));
    }
    Ok(m)
}

// ------------------------------------------------------------ le bloc 65 d'une paie

/// Les blocs S21.G00.65 d'une paie : chaque suspension du magasin qui suspend CE contrat et croise le mois que la DSN
/// declare, avec son motif et ses DEUX dates entieres (65.002 et 65.003) -- la meme dans chaque mois qu'elle traverse,
/// sans recopie (cahier technique DSN 2026.1, bloc S21.G00.65 p. 281, `atelier/reference/norme-dsn/texte/bloc-S21.G00.65.txt`).
pub fn suspensions_du_mois(ctx: &Contexte, paie: &crate::calcul::Paie, mois: &str) -> Resultat<Vec<Value>> {
    let cle = paie.salarie.id.to_string();
    let e = &paie.engagement;
    let qui = ctx.nomme(&cle);
    let mut out = Vec::new();
    let mut siennes = Vec::new();
    for imp in du_mois_du_salarie(&ctx.racine, mois, Some(SUSPENSION), &cle)? {
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
                motif.libelle,
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
    for imp in du_salarie(racine, cle)? {
        if !est_une_suspension(&imp) || !du_contrat(&imp, cle, numero, contrat_debut)? || !croise(&imp, debut, fin)? {
            continue;
        }
        let remunere = motif_de_suspension(&s(&imp, "motif"))?.is_none_or(|m| m.remunere);
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
    for imp in du_salarie(racine, cle)? {
        if !est_une_suspension(&imp) || !du_contrat(&imp, cle, numero, contrat_debut)? {
            continue;
        }
        let acquiert = motif_de_suspension(&s(&imp, "motif"))?.is_none_or(|m| m.acquiert_conges);
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
    for imp in du_salarie(racine, cle)? {
        if !est_une_suspension(&imp) || s(&imp, "salarie") != cle {
            continue;
        }
        let code = s(&imp, "motif");
        let Some(motif) = motif_de_suspension(&code)? else { continue };
        if motif.remunere {
            continue;
        }
        let (d, f) = (premier_jour(&imp)?.max(*depuis), dernier_jour(&imp)?.min(*a));
        if f < d {
            continue;
        }
        let jours = f.moins(&d) + 1;
        let libelle = motif.libelle.clone();
        if motif.anciennete == Some(droit::permanents::AncienneteDeSuspension::Moitie) {
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
    let cle = paie.salarie.id.to_string();
    let mut out = Vec::new();
    for imp in du_salarie(&ctx.racine, &cle)? {
        if est_un_arret(&imp) && du_contrat(&imp, &cle, paie.engagement.numero.as_deref(), &paie.engagement.debut)? && croise(&imp, debut, fin)? {
            out.push(imp);
        }
    }
    Ok(out)
}

// ------------------------------------------------------ les lignes d'un permanent

/// Ce que le mois d'un contrat paye au mois donne pour mesurer une absence (`conges::lignes_du_mois`) : les bornes du
/// contrat dans le mois, le salaire que la ligne « mois » paie et les jours travailles payes qu'elle porte
/// (`droit::periodes::part_du_mois`), l'horaire du salarie qui les compte et les heures d'un jour
/// (`droit::conges::heures_par_jour`), et ce que vaut un jour ouvrable (l'assiette du maintien de salaire que la loi
/// donne, dit sans etre applique).
pub struct MoisDuContrat<'a> {
    pub debut: Jour,
    pub fin: Jour,
    pub fin_mois: Jour,
    pub salaire_du_mois: Decimal,
    pub heures_par_jour: Decimal,
    pub jours_payes: i64,
    pub horaire: &'a droit::periodes::Horaire,
    pub taux_jour_ouvrable: Decimal,
}

/// UN TAUX SANS ARRONDI : le quotient tel quel, en texte (`num::dec` le relit sans perte), pour que la quantite fois le
/// taux, arrondi une fois au centime par la ligne (`calcul::lignes_brut`), donne le montant du quotient -- et le
/// salaire entier quand les heures sont toutes celles du mois. Un taux arrondi au centime AVANT de multiplier laissait
/// 0,07 EUR de salaire sur un mois entier d'absence (2 310 / 151,67 = 15,23, x 151,67 = 2 309,93) : aucun texte ne
/// l'arrondit (Cass. soc. 11/02/1982, ci-dessous, est un quotient).
fn taux_exact(montant: Decimal, quantite: Decimal) -> crate::Chiffre {
    crate::Chiffre::texte_de(div(montant, quantite).normalize())
}

/// LA FICHE DU SALARIE D'UN IMPREVU, rattachee a l'etablissement du contrat qu'il suspend (`calcul::etablissement_du_contrat`) :
/// un signalement d'arret porte l'etablissement de la derniere DSN mensuelle du contrat (cahier technique DSN 2026.1,
/// § 2.5.1, p. 67). Le contrat se lit dans la saisie du mois ou il commence ; introuvable, la fiche telle quelle.
pub fn fiche_du_salarie(ctx: &Contexte, imp: &Value) -> Resultat<SalarieDuContrat> {
    let fiche = SalarieDuContrat::de(ctx.salarie(&s(imp, "salarie"))?);
    let cle = fiche.id.to_string();
    let Some(debut) = Some(g(imp, "contrat_debut")).filter(|v| vrai(v)).map(d).transpose()? else { return Ok(fiche) };
    let mois = debut.mois_civil().to_string();
    let Some(data) = crate::stock::vue_du_mois(&ctx.racine, &mois)? else { return Ok(fiche) };
    for e in &data.engagements {
        let Some(debut_dit) = e.debut.as_deref().filter(|x| !x.is_empty()) else { continue };
        if Jour::lire(debut_dit)? != debut || e.cle(ctx).as_deref() != Some(cle.as_str()) {
            continue;
        }
        let (n_imp, n) = (s(imp, "contrat_numero"), e.numero.clone().unwrap_or_default());
        if !n_imp.is_empty() && !n.is_empty() && n_imp != n {
            continue;
        }
        let mut f = fiche;
        f.etablissement = e.etablissement.as_deref().map(str::trim).filter(|n| !n.is_empty()).map(str::to_string);
        return Ok(f);
    }
    Ok(fiche)
}

/// Les imprevus du contrat qui croisent le mois, du premier au dernier jour : le contrat de `salarie` (le nom que la carte
/// ecrit) commence le `debut`, de numero `numero` (vide avant la validation).
pub fn du_contrat_dans_le_mois(ctx: &Contexte, salarie: &str, debut: &Jour, numero: &str, mois: &str) -> Resultat<Vec<Value>> {
    let cle = ctx.cle_du_salarie(salarie)?.to_string();
    let mut out = Vec::new();
    for imp in du_mois_du_salarie(&ctx.racine, mois, None, &cle)? {
        if du_contrat(&imp, &cle, Some(numero), debut)? {
            out.push(imp);
        }
    }
    out.sort_by_key(|i| premier_jour(i).map(|j| j.iso()).unwrap_or_default());
    Ok(out)
}

/// LES LIGNES QU'UN IMPREVU DONNE AU MOIS D'UN PERMANENT -- le seul endroit qui les tire. Chaque imprevu du contrat
/// retire le salaire de ses heures d'absence dans le mois (bornees par le contrat) ; un arret le retire sous
/// la rubrique de son motif (`arrets-base.json`, `rubrique` ; le catalogue la marque `arret_de_travail`), une suspension non remuneree sous
/// `absence_non_remuneree`. Une suspension que `permanents-base.json` dit remuneree (637 : C. trav. L3142-2, « n'entrainent
/// pas de reduction de la remuneration ») ne retire rien. Le maintien de salaire d'un arret est celui SAISI SUR L'ARRET
/// pour ce mois (`maintien`) : la loi le dit (`droit::maintien::legal`), elle ne le verse pas -- les indemnites journalieres a
/// deduire (D1226-5) ne se devinent pas.
///
/// LA RETENUE AU REEL DU MOIS. « La retenue par heure d'absence d'un salarie paye au mois doit etre en
/// principe egale au quotient du salaire mensuel par le nombre d'heures de travail dans l'entreprise pour le mois
/// considere », et non sur un horaire moyen (Cass. soc. 11/02/1982, n° 80-40.359 ; 14/05/1987, n° 84-43.854 :
/// `atelier/reference/sources/suspension-non-remuneree/cass-soc-19820211-80-40359-JURITEXT000007009242.xml` et
/// `cass-soc-19870514-84-43854-JURITEXT000007018764.xml` ; la DDETS en tire « salaire mensualise / heures programmees sur
/// le mois », `dreets-limousin-qr-assmat-retenues-absence-20150101.txt`). Les heures d'absence se comptent au reel
/// (fiche DSN 2640, « sans application d'une regle de trois visant a rapprocher le volume d'absence a la quotite de
/// travail mensualisee » ; fiche 2693, exemple 10 : septembre 2026 entier, 154 h = 22 x 7 h ; exemple 4 : les jours
/// feries comptent parmi les heures du mois ; meme dossier) : les jours travailles de l'absence, comptes comme ceux du
/// salaire du mois (`droit::periodes::part_du_mois`, sur l'horaire du salarie : ses jours travailles, les feries qu'il ne
/// voit pas payes deja hors de son salaire), fois les heures d'un jour (`droit::conges::heures_par_jour`). La retenue est
/// le salaire du mois fois ces heures sur les heures payees du mois, au taux exact (`taux_exact`) : un mois entier
/// d'absence retire exactement le salaire du mois.
///
/// Rend aussi les jours travailles que les absences retirees ote du mois (un arret, une suspension non remuneree) :
/// `conges::lignes_du_mois` en tire les jours travailles du mois.
pub fn lignes_du_mois(ctx: &Contexte, e: &Value, mois: &str, m: &MoisDuContrat) -> Resultat<(Vec<crate::LignePaie>, Vec<Avis>, i64)> {
    let qui = crate::conges::qui(ctx, e);
    let (mut lignes, mut avert, mut absents) = (Vec::new(), Vec::new(), 0i64);
    let nat = crate::conges::nature_de(e.get("nature"))?;
    let mut maintien = crate::maintien::DuMois::default();
    // Le taux de l'heure d'absence : le salaire du mois sur ses heures de travail (Cass. soc. 11/02/1982).
    let heures_du_mois = eur2(mul(m.heures_par_jour, Decimal::from(m.jours_payes)));
    let taux_horaire = if heures_du_mois > Decimal::ZERO { Some(taux_exact(m.salaire_du_mois, heures_du_mois)) } else { None };
    // Un contrat sans date de debut ecrite ne se designe pas : aucun imprevu ne peut le nommer (S21.G00.40.001).
    let imprevus = if vrai(g(e, "debut")) { du_contrat_dans_le_mois(ctx, &en_texte(g(e, "salarie")), &d(g(e, "debut"))?, &s(e, "numero"), mois)? } else { Vec::new() };
    for imp in imprevus {
        let (premier, dernier) = (premier_jour(&imp)?, dernier_jour(&imp)?);
        let (debut, fin) = (premier.max(m.debut), dernier.min(m.fin));
        let jours = if debut <= fin {
            droit::periodes::part_du_mois(&debut, &fin, &m.fin_mois, droit::periodes::RegleDuMoisIncomplet::HoraireReel, m.horaire)?.jours
        } else {
            0
        };
        let heures = eur2(mul(m.heures_par_jour, Decimal::from(jours)));
        let retenue = |rubrique: &str| -> Option<crate::LignePaie> {
            match &taux_horaire {
                Some(t) if heures > Decimal::ZERO => Some(crate::LignePaie::de(rubrique, crate::Chiffre::de_f64(flot(heures)), t.clone())),
                _ => None,
            }
        };
        let au_reel = format!(
            "{} d'absence ce mois-ci, {} h sur les {} h de travail du mois, retirées au prorata du salaire du mois",
            if jours > 1 { format!("{} jours travaillés", jours) } else { format!("{} jour travaillé", jours) },
            socle::texte::virgule(heures),
            socle::texte::virgule(heures_du_mois)
        );
        let code = s(&imp, "motif");
        if est_un_arret(&imp) {
            let motif = droit::arrets::motif_d_arret(&code)?;
            // Le bulletin nomme l'absence d'apres le motif de l'arret (S21.G00.60.001) : sa rubrique est celle du motif
            // (`arrets-base.json`, `rubrique`).
            let rubrique = motif.as_ref().and_then(|m| m.rubrique.clone()).unwrap_or_default();
            if rubrique.is_empty() {
                return Err(socle::echec::cle(format!(
                    "{} : l'arret du {} au {} a pour motif {} ({}), que le logiciel ne paie pas : aucune ligne de bulletin ne \
nomme cette absence. Rien n'est calcule.",
                    qui,
                    premier.jj_mm_aaaa(),
                    dernier.jj_mm_aaaa(),
                    code,
                    motif.as_ref().map_or("inconnu de la norme", |m| m.libelle.as_str())
                ), "arrets-base.json"));
            }
            lignes.extend(retenue(&rubrique).map(|l| crate::LignePaie { arret: Some(true), ..l }));
            absents += jours;
            avert.push(info(format!(
                "{} : arrêt de travail ({}) du {} au {} : {} ; la DSN de chaque mois qu'il traverse le déclare.",
                qui,
                motif.as_ref().map_or("inconnu de la norme", |m| m.libelle.as_str()),
                premier.jj_mm_aaaa(),
                dernier.jj_mm_aaaa(),
                au_reel
            )));
            if motif.as_ref().is_some_and(|m| m.origine.is_some()) {
                crate::maintien::ajouter(ctx, e, &imp, &nat, mois, m, &mut maintien)?;
            }
        } else {
            let motif = verifier_motif_de_suspension(&code, &qui)?;
            let libelle = motif.libelle.clone();
            if motif.remunere {
                avert.push(info(format!(
                    "{} : absence ({}) du {} au {} : un congé rémunéré, rien n'est retiré du salaire ({}).",
                    qui,
                    libelle,
                    premier.jj_mm_aaaa(),
                    dernier.jj_mm_aaaa(),
                    motif.texte()
                )));
            } else {
                lignes.extend(retenue("absence_non_remuneree"));
                absents += jours;
                avert.push(info(format!(
                    "{} : absence ({}) du {} au {} : {} ; la DSN de chaque mois qu'elle traverse la déclare.",
                    qui,
                    libelle,
                    premier.jj_mm_aaaa(),
                    dernier.jj_mm_aaaa(),
                    au_reel
                )));
            }
        }
    }
    lignes.extend(crate::maintien::lignes(&maintien, mois));
    avert.extend(maintien.avis);
    Ok((lignes, avert, absents))
}

// ------------------------------------------------------------- les jours payes

/// Les jours d'une ligne de saisie (`dates`), lus.
fn dates_de_ligne(l: &crate::LignePaie) -> Vec<Jour> {
    l.dates.iter().filter_map(|x| x.lu().ok()).collect()
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
    for mois in socle::mois::Mois::entre(&premier, &dernier).iter().map(socle::mois::Mois::to_string) {
        let Some(data) = crate::stock::vue_du_mois(&ctx.racine, &mois)? else { continue };
        out.extend(cachets_de_la_saisie(ctx, &data, &mois, &cle, &premier, &dernier)?);
    }
    Ok(out)
}

fn cachets_de_la_saisie(ctx: &Contexte, data: &crate::saisie::VueDuMois, mois: &str, cle: &str, premier: &Jour, dernier: &Jour) -> Resultat<Vec<Value>> {
    let mut out = Vec::new();
    for (i, e) in data.engagements.iter().enumerate() {
        if e.cle(ctx).as_deref() != Some(cle) {
            continue;
        }
        for (j, l) in e.lignes().iter().enumerate() {
            let dedans: Vec<String> = dates_de_ligne(l).into_iter().filter(|x| premier <= x && x <= dernier).map(|x| x.iso()).collect();
            if dedans.is_empty() {
                continue;
            }
            let rubrique = l.code().to_string();
            // TRAVAIL A FAIRE (chaine A) : le jour paye pendant un imprevu a sa forme.
            out.push(json!({
                "mois": mois, "engagement": i, "numero": e.numero, "spectacle": e.spectacle,
                "ligne": j, "rubrique": rubrique, "libelle": libelle_rubrique(&ctx.racine, &rubrique, &ctx.lue_au),
                "dates": dedans, "quantite": l.quantite, "taux": l.taux,
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
pub fn avertir_les_jours_payes(ctx: &Contexte, data: &crate::saisie::VueDuMois) -> Resultat<Vec<Vec<Avis>>> {
    let engagements = &data.engagements;
    let mut out: Vec<Vec<Avis>> = vec![Vec::new(); engagements.len()];
    // Les imprevus des seuls salaries de la saisie (`du_salarie`).
    let mut salaries: Vec<String> = Vec::new();
    for cle in engagements.iter().filter_map(|e| e.cle(ctx)) {
        if !salaries.iter().any(|s| *s == cle) {
            salaries.push(cle.into());
        }
    }
    let mut siens = Vec::new();
    for cle in &salaries {
        siens.extend(du_salarie(&ctx.racine, cle)?);
    }
    let mois = data.mois.to_string();
    for imp in &siens {
        let (premier, dernier) = (premier_jour(imp)?, dernier_jour(imp)?);
        for c in cachets_de_la_saisie(ctx, data, &mois, &s(imp, "salarie"), &premier, &dernier)? {
            let i = c["engagement"].as_u64().unwrap_or(0) as usize;
            let dates: Vec<String> = c["dates"].as_array().map_or_else(Vec::new, |a| a.iter().filter_map(|x| d(x).ok()).map(|x| x.jj_mm_aaaa()).collect());
            let quoi = if est_un_arret(imp) {
                let motif = droit::arrets::motif_d_arret(&s(imp, "motif"))?;
                format!(
                    "l'arret de travail ({}) du {} au {} : un salarie en arret ne peut exercer son activite (bloc S21.G00.60)",
                    motif.as_ref().map_or("inconnu de la norme", |m| m.libelle.as_str()),
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
            "motif": "01", "dernier_jour_travaille": "2026-02-02", "debut": "2026-02-03", "fin_previsionnelle": "2026-02-10"}))
        .unwrap();
        let mut champs: Vec<String> = a.as_object().unwrap().keys().cloned().collect();
        champs.sort();
        let mut attendus: Vec<String> = CHAMPS_ARRET.iter().map(|c| c.to_string()).collect();
        attendus.sort();
        assert_eq!(champs, attendus, "un arret complete porte tous ses champs");
        assert_eq!((a["qui"].clone(), a["ij_brute"].clone()), (json!("?"), Value::Null));
        assert_eq!(a["prolongations"], json!([]));
        assert!(complet(&json!({"genre": "arret", "salarie": "X"})).unwrap_err().contains("champ manquant"));
        assert!(complet(&json!({"genre": "autre"})).unwrap_err().contains("genre"));
        let mut s = json!({"genre": "suspension", "salarie": "X", "contrat_debut": "2026-01-01", "contrat_numero": "N",
            "motif": "632", "debut": "2026-02-01", "fin": "2026-04-30"});
        assert!(complet(&s).is_ok());
        s["heures"] = json!(3);
        assert!(complet(&s).unwrap_err().contains("champ inconnu"));
    }

    /// S21.G00.60.002 : « la veille de la date de debut de la prescription de l'arret de travail, sauf [...] arret prescrit
    /// lors d'un jour travaille : le dernier jour travaille correspond alors a la date de l'arret » (bloc-S21.G00.60.txt,
    /// p. 266-267) ; l'absence va du lendemain a la veille de la reprise (S21.G00.60.010).
    #[test]
    fn la_dsn_declare_la_veille_du_debut_prescrit_et_l_absence_court_de_ce_debut() {
        let mut a = json!({"genre": "arret", "dernier_jour_travaille": "2026-03-06", "debut": "2026-03-09", "fin_previsionnelle": "2026-03-20", "reprise": null});
        assert_eq!((dernier_jour_declare(&a).unwrap().iso(), premier_jour(&a).unwrap().iso()), ("2026-03-08".into(), "2026-03-09".into()));
        a["debut"] = json!("2026-03-06");
        assert_eq!((dernier_jour_declare(&a).unwrap().iso(), premier_jour(&a).unwrap().iso()), ("2026-03-06".into(), "2026-03-07".into()));
        assert_eq!(dernier_jour(&a).unwrap().iso(), "2026-03-20");
        a["reprise"] = json!("2026-03-15");
        assert_eq!(dernier_jour(&a).unwrap().iso(), "2026-03-14");
    }
}
