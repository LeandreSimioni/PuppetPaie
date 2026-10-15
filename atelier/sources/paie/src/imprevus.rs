//! LES IMPREVUS D'UN SALARIE (15/10/2026) : ce qui arrive a un contrat en cours et le suspend -- un ARRET de travail
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
//! autre champ refuse. Le dossier `arrets/` d'avant le 15/10/2026 n'est plus lu : sa presence refuse (`refuser_l_ancien`).

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::conges::{avertir, flot, info};
use crate::contexte::Contexte;
use crate::date::{d, en_texte, Jour};
use crate::engagement::Avis;
use crate::fichiers::{repr, vrai};
use crate::num::{dec_py, div, eur2, mul, Decimal, Resultat};

/// Le genre d'un arret de travail (bloc S21.G00.60).
pub const ARRET: &str = "arret";
/// Le genre d'une autre suspension du contrat (bloc S21.G00.65).
pub const SUSPENSION: &str = "suspension";
/// Le dossier du magasin, sous les donnees de la compagnie.
pub const DOSSIER: &str = "imprevus";

/// Les champs d'un arret, dans l'ordre ou ils s'ecrivent ; les `REQUIS_ARRET` premiers sont obligatoires. Ceux de la
/// norme (motif, dernier jour travaille, fin previsionnelle, subrogation, reprise, accident) sont ceux du bloc 60 et du
/// signalement (`signalement::verifier`) ; `jours_deja_indemnises` et `maintien` sont ceux du maintien de salaire d'un
/// permanent (D1226-4 ; le complement verse, mois par mois : `{"AAAA-MM": montant}`).
pub const CHAMPS_ARRET: [&str; 19] = [
    "genre", "salarie", "contrat_debut", "contrat_numero", "motif", "dernier_jour_travaille", "fin_previsionnelle",
    "subrogation", "subrogation_debut", "subrogation_fin", "reprise", "reprise_motif", "date_accident",
    "jours_deja_indemnises", "maintien", "saisi_le", "qui", "note", "envois",
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
        return Err(format!("Imprevu : un objet est attendu, pas {}", crate::etats::nom_type(v)));
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
            _ => Value::Null,
        };
        o.insert(k.to_string(), m.get(*k).cloned().filter(|x| !(x.is_null() && matches!(*k, "envois" | "maintien" | "note"))).unwrap_or(defaut));
    }
    if !o["envois"].is_array() {
        return Err(format!("Imprevu ({}) : `envois` n'est pas une liste", genre));
    }
    if genre == ARRET && !o["maintien"].is_object() {
        return Err("Arret : `maintien` est un objet {\"AAAA-MM\": montant}.".into());
    }
    Ok(Value::Object(o))
}

fn repr_valeur_courte(v: Option<&Value>) -> String {
    match v {
        None => "absent".into(),
        Some(x) => crate::fichiers::repr_valeur(x),
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

/// LE DOSSIER `arrets/` N'EST PLUS LU (15/10/2026, regle du 14/10 : pas de compatibilite). Un arret qu'il garderait ne
/// serait ni reporte ni retire de la paie : rien ne se lit tant qu'il est la, et la phrase dit le geste.
fn refuser_l_ancien(racine: &Path) -> Resultat<()> {
    if crate::fichiers::est_dossier(&racine.join("arrets")) {
        return Err(format!(
            "Le dossier {} n'est plus lu : depuis le 15/10/2026, arrets de travail et suspensions sont des IMPREVUS, \
dans {}. Pour chaque arrets/AAAA-MM.json : le deplacer en imprevus/AAAA-MM.json, y renommer la cle \"arrets\" en \
\"imprevus\", ajouter \"genre\": \"arret\" a chaque arret ; puis supprimer le dossier arrets/. Rien n'est lu d'ici la.",
            racine.join("arrets").display(),
            dossier(racine).display()
        ));
    }
    Ok(())
}

/// Les imprevus d'un fichier deja lu.
fn du_contenu(contenu: &Value, chemin: &Path) -> Resultat<Vec<Value>> {
    let Value::Object(m) = contenu else {
        return Err(format!("{} : un objet est attendu, pas {}", chemin.display(), crate::etats::nom_type(contenu)));
    };
    let liste = match m.get("imprevus") {
        None => return Ok(Vec::new()),
        Some(Value::Array(l)) => l,
        Some(autre) => return Err(format!("{} : `imprevus` n'est pas une liste ({})", chemin.display(), crate::etats::nom_type(autre))),
    };
    liste.iter().map(|v| complet(v).map_err(|e| format!("{} : {}", chemin.display(), e))).collect()
}

/// Tous les imprevus de la compagnie, dans l'ordre des mois du magasin puis de leur saisie, verifies et complets. La
/// lecture se note au registre du calcul (`fichiers::lister`, `fichiers::lire_json`) : un imprevu saisi refait la paie.
pub fn tous(racine: &Path) -> Resultat<Vec<Value>> {
    refuser_l_ancien(racine)?;
    let Some(fichiers) = crate::fichiers::lister(&dossier(racine)) else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    for f in fichiers {
        let nom = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if !est_un_mois(&nom) {
            continue;
        }
        out.extend(du_contenu(&crate::fichiers::lire_json(&f)?, &f)?);
    }
    Ok(out)
}

/// Les imprevus que le fichier d'un mois du magasin range, complets ; vide s'il n'existe pas.
pub fn du_fichier(racine: &Path, mois: &str) -> Resultat<Vec<Value>> {
    refuser_l_ancien(racine)?;
    let p = chemin(racine, mois);
    if !crate::fichiers::existe(&p) {
        return Ok(Vec::new());
    }
    du_contenu(&crate::fichiers::lire_json(&p)?, &p)
}

/// Ecrit le fichier d'un mois du magasin.
pub fn ecrire(racine: &Path, mois: &str, imprevus: Vec<Value>) -> Resultat<()> {
    crate::ecriture::ecrire_json(
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

/// Une periode commune avec [debut, fin] : ce qui fait reporter l'imprevu dans la DSN de ce mois. Pour un arret, la
/// regle du bloc 60 telle que la DSN l'applique (`dsn::Arret::couvre`).
pub fn croise(imp: &Value, debut: &Jour, fin: &Jour) -> Resultat<bool> {
    if est_un_arret(imp) {
        return crate::dsn::Arret { v: imp.clone() }.couvre(debut, fin);
    }
    Ok(premier_jour(imp)? <= *fin && dernier_jour(imp)? >= *debut)
}

/// Les imprevus d'un genre (ou de tous les genres) qui croisent le mois `AAAA-MM`.
pub fn du_mois(racine: &Path, mois: &str, genre_voulu: Option<&str>) -> Resultat<Vec<Value>> {
    let (debut, fin) = (crate::date::premier_du_mois(mois)?, crate::date::dernier_du_mois(mois)?);
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
    let n = crate::fichiers::json_en_cache(&crate::fichiers::dossier_du_code().join("arrets-base.json"))?;
    Ok(n["motifs_arret"]["codes"].as_array().and_then(|l| l.iter().find(|c| c["code"].as_str() == Some(code))).cloned())
}

/// Le motif de suspension (`permanents-base.json`, section `suspensions`), s'il existe.
pub fn motif_de_suspension(code: &str) -> Resultat<Option<Value>> {
    let regles = crate::permanents::regles()?;
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
        let regles = crate::permanents::regles()?;
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
    let numero = e.numero.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| "?".into());
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
        verifier_motif_de_suspension(&code, &format!("Contrat {}", numero))?;
        let (debut, fin) = (premier_jour(&imp)?, dernier_jour(&imp)?);
        if fin < debut {
            return Err(format!(
                "Contrat {} : suspension {} du {} au {} -- elle finit avant de commencer.",
                numero,
                code,
                debut.jj_mm_aaaa(),
                fin.jj_mm_aaaa()
            ));
        }
        out.push(json!({"motif": code, "debut": debut.iso(), "fin": fin.iso()}));
    }
    Ok(out)
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
/// contrat dans le mois, sa quotite, son taux horaire, les jours ouvrables du mois et ce que vaut l'un d'eux.
pub struct MoisDuContrat {
    pub debut: Jour,
    pub fin: Jour,
    pub quotite: Decimal,
    pub taux_horaire: Decimal,
    pub ouvrables_du_mois: Decimal,
    pub taux_jour_ouvrable: Decimal,
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
/// retire le salaire de ses jours d'absence dans le mois (bornes par le contrat), en heures : la quotite au prorata des
/// jours ouvrables (la mesure des conges pris et des absences, `conges::lignes_du_mois`) ; un arret le retire sous
/// `absence_maladie` (le catalogue la marque `arret_de_travail`), une suspension non remuneree sous
/// `absence_non_remuneree`. Une suspension que `permanents-base.json` dit remuneree (637 : C. trav. L3142-2, « n'entrainent
/// pas de reduction de la remuneration ») ne retire rien. Le maintien de salaire d'un arret est celui SAISI SUR L'ARRET
/// pour ce mois (`maintien`) : la loi le dit (`maintien_possible`), elle ne le verse pas -- les indemnites journalieres a
/// deduire (D1226-5) ne se devinent pas.
pub fn lignes_du_mois(ctx: &Contexte, e: &Value, mois: &str, m: &MoisDuContrat) -> Resultat<(Vec<Value>, Vec<Avis>)> {
    let numero = crate::conges::numero(e);
    let ent = &ctx.entreprise;
    let (mut lignes, mut avert) = (Vec::new(), Vec::new());
    for imp in du_contrat_dans_le_mois(ctx, e, mois)? {
        let (premier, dernier) = (premier_jour(&imp)?, dernier_jour(&imp)?);
        let (debut, fin) = (premier.max(m.debut), dernier.min(m.fin));
        let ouvres = if debut <= fin { Decimal::from(crate::conges::jours_ouvrables(&debut, &fin, Some(ent))?) } else { Decimal::ZERO };
        let heures = eur2(div(mul(m.quotite, ouvres), m.ouvrables_du_mois));
        let code = s(&imp, "motif");
        if est_un_arret(&imp) {
            let motif = motif_d_arret(&code)?.unwrap_or(Value::Null);
            if heures > Decimal::ZERO {
                lignes.push(json!({"rubrique": "absence_maladie", "quantite": flot(heures), "taux": flot(m.taux_horaire)}));
            }
            avert.push(info(format!(
                "{} : arret de travail ({}) du {} au {}, saisi une fois : {} jour(s) ouvrable(s) d'absence ce mois-ci, retires \
du salaire ; le bloc S21.G00.60 part dans la DSN de chaque mois qu'il traverse.",
                numero,
                en_texte(g(&motif, "libelle")),
                premier.jj_mm_aaaa(),
                dernier.jj_mm_aaaa(),
                ouvres
            )));
            if vrai(g(&motif, "maintien_legal")) {
                avert.extend(maintien_possible(ctx, e, &imp, mois, m.taux_jour_ouvrable)?);
            }
            if let Some(montant) = g(&imp, "maintien").get(mois).filter(|x| vrai(x)) {
                let montant = dec_py(montant)?;
                let (q, t) = if heures > Decimal::ZERO { (heures, eur2(div(montant, heures))) } else { (Decimal::ONE, montant) };
                lignes.push(json!({"rubrique": "maintien_salaire", "quantite": flot(q), "taux": flot(t)}));
            }
        } else {
            let motif = verifier_motif_de_suspension(&code, &numero)?;
            let libelle = en_texte(g(&motif, "libelle"));
            if vrai(g(&motif, "remunere")) {
                avert.push(info(format!(
                    "{} : suspension {} ({}) du {} au {} : un conge remunere, rien n'est retire du salaire ({}).",
                    numero,
                    code,
                    libelle,
                    premier.jj_mm_aaaa(),
                    dernier.jj_mm_aaaa(),
                    en_texte(g(&motif, "_remunere"))
                )));
            } else {
                if heures > Decimal::ZERO {
                    lignes.push(json!({"rubrique": "absence_non_remuneree", "quantite": flot(heures), "taux": flot(m.taux_horaire)}));
                }
                avert.push(info(format!(
                    "{} : suspension {} ({}) du {} au {}, saisie une fois : {} jour(s) ouvrable(s) ce mois-ci, retires du \
salaire ; le bloc S21.G00.65 la declare entiere dans la DSN de chaque mois qu'elle traverse.",
                    numero,
                    code,
                    libelle,
                    premier.jj_mm_aaaa(),
                    dernier.jj_mm_aaaa(),
                    ouvres
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
    use crate::permanents::{anciennete_mois, droit_au_maintien, maintien_de_salaire, OptionsMaintien};
    let numero = crate::conges::numero(e);
    let nat = crate::conges::nature_de(e.get("nature"))?;
    let premier = premier_jour(imp)?;
    let fiche = ctx.salarie(&en_texte(g(e, "salarie")))?;
    let depuis = d(match fiche.get("date_anciennete").filter(|x| vrai(x)) {
        Some(v) => v,
        None => g(e, "debut"),
    })?;
    let anciennete = anciennete_mois(&depuis, &premier);
    if let Some(refus) = droit_au_maintien(Some(&nat), anciennete)? {
        return Ok(vec![avertir(format!("{} : pas de maintien de salaire pour cet arret -- {}", numero, refus))]);
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
        numero,
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
            numero, au_dela
        )));
    }
    if let Some(conv) = crate::clauses::maintien(ctx.entreprise.get("idcc").unwrap_or(&Value::Null), anciennete, at)? {
        dit.push(avertir(format!("{} : {}", numero, conv)));
    }
    if !g(imp, "maintien").get(mois).is_some_and(vrai) {
        dit.push(avertir(format!(
            "{} : le maintien de salaire de {} n'est pas saisi sur l'arret, donc rien n'est verse a ce titre. Le \
complement du est le brut ci-dessus moins les indemnites journalieres et la prevoyance, que la caisse communique : le \
porter sur l'arret, maintien du mois.",
            numero,
            crate::engagement::libelle_mois(mois)
        )));
    }
    Ok(dit)
}

// ------------------------------------------------------------- les jours payes

/// Les jours d'une ligne de saisie (`dates`), lus.
fn dates_de_ligne(l: &Value) -> Vec<Jour> {
    l.get("dates").and_then(Value::as_array).map_or_else(Vec::new, |a| a.iter().filter_map(|x| d(x).ok()).collect())
}

/// Le libelle d'une rubrique, ou son code si le catalogue ne la connait pas.
fn libelle_rubrique(racine: &Path, code: &str) -> String {
    crate::catalogue::rubrique(racine, code).map(|r| r.libelle).unwrap_or_else(|_| code.to_string())
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
    for mois in crate::date::mois_entre(&premier, &dernier) {
        let p = ctx.racine.join("paies").join(format!("{}.json", mois));
        if !p.exists() {
            continue;
        }
        let data = crate::engagement::lire_saisie(&p)?;
        out.extend(cachets_de_la_saisie(ctx, &data, &mois, &cle, &premier, &dernier)?);
    }
    Ok(out)
}

fn cachets_de_la_saisie(ctx: &Contexte, data: &Value, mois: &str, cle: &str, premier: &Jour, dernier: &Jour) -> Resultat<Vec<Value>> {
    let mut out = Vec::new();
    for (i, e) in data.get("engagements").and_then(Value::as_array).map_or(&[][..], |a| &a[..]).iter().enumerate() {
        if crate::engagement::cle_de_l_engagement(ctx, e).as_deref() != Some(cle) {
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
                "ligne": j, "rubrique": rubrique, "libelle": libelle_rubrique(&ctx.racine, &rubrique),
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
        assert_eq!(a.as_object().unwrap().keys().cloned().collect::<Vec<_>>(), CHAMPS_ARRET.to_vec());
        assert_eq!((a["qui"].clone(), a["maintien"].clone(), a["envois"].clone()), (json!("?"), json!({}), json!([])));
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
