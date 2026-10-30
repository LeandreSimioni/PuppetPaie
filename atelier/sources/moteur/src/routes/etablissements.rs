//! LES AUTRES ETABLISSEMENTS DE LA COMPAGNIE (`etablissements` de la fiche entreprise) : ce que l'ecran en dit
//! (Configuration › Entreprise › Avance, « Autres etablissements ») et comment la fiche les date.
//!
//! La fiche porte une liste datee des etablissements ouverts : une version a chaque ouverture, a chaque modification et a
//! chaque fermeture. Toute la configuration est datee : une ligne qu'on modifie vaut a partir de la date d'effet que la
//! boite « Ce qui change » demande, et ce qui precede garde l'ancienne valeur ; une ligne qu'on retire ferme
//! l'etablissement a partir de cette date (une date au jour de son ouverture, ou avant, l'efface : il n'a jamais ouvert).
//! Une ligne neuve se date a son ouverture, et corriger une ouverture ne demande pas de date.
//!
//! Chacun dit son SIRET (du SIREN de la compagnie), son adresse, son ouverture et son code risque AT/MP : le taux est
//! determine par etablissement et le classement suit l'activite qui y est exercee (D. 242-6-1,
//! `atelier/reference/sources/code-de-la-securite-sociale/xml/D242-6-1.xml` ; arrete du 17/10/1995, art. 1, I, 1°,
//! `tarification-atmp/`). Un code laisse vide prend celui que la compagnie a pour l'annee de l'ouverture (INFERENCE :
//! l'activite de l'etablissement est celle de la compagnie tant que la notification de la CARSAT ne dit pas autre chose),
//! et l'ecran le dit (`atmp::lire`). Le taux saisi est « le taux net notifie de l'annee » (D. 242-6-15) : il porte l'annee
//! de sa date d'effet (`annee`) et ne vaut que pour elle (`droit::bareme::taux_atmp_d_un_etablissement`). Le taux laisse vide est
//! celui de l'arrete pour un risque « TC » (`droit::bareme::taux_atmp_applique`) ; sinon il se refuse (le controle des
//! etablissements, `droit::contexte::controler_etablissements`).

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use socle::date::Jour;
use socle::echec::R;
use socle::fichiers::vrai;
use socle::valeur::valeur_en_texte;

use crate::routes::fiches::{refus, sans_valeur};

fn texte(v: Option<&Value>) -> String {
    v.filter(|x| vrai(x)).map(valeur_en_texte).unwrap_or_default()
}

fn nic(e: &Value) -> String {
    texte(e.get("nic"))
}

fn nom(e: &Value) -> String {
    let l = texte(e.get("libelle"));
    if l.trim().is_empty() {
        let c = texte(e.get("commune_affichage").filter(|v| vrai(v)).or(e.get("commune")));
        if c.trim().is_empty() { nic(e) } else { c }
    } else {
        l
    }
}

/// Ce qu'une ligne dit, sans ce qui ne se date pas (son ouverture, son NIC qui se lit dans le SIRET, la forme lisible de
/// son adresse, les notes du moteur, l'annee du taux) ni les cases vides.
fn comparable(e: &Value) -> Value {
    let mut o = e.as_object().cloned().unwrap_or_default();
    o.retain(|k, v| !k.starts_with('_') && k != "ouverture" && k != "nic" && !k.ends_with("_affichage") && !sans_valeur(Some(v)));
    for cle in ["taux_atmp", "versement_mobilite"] {
        if let Some(Value::Object(t)) = o.get_mut(cle) {
            t.retain(|k, v| !k.starts_with('_') && k != "annee" && !sans_valeur(Some(v)));
        }
    }
    Value::Object(o)
}

fn atmp_comparable(e: &Value) -> Value {
    comparable(e).get("taux_atmp").cloned().unwrap_or(Value::Null)
}

/// Une ligne lisible : ce que la boite « Ce qui change » montre.
fn lisible(e: &Value) -> String {
    let t = e.get("taux_atmp").cloned().unwrap_or(Value::Null);
    let adresse = [texte(e.get("adresse_affichage").filter(|v| vrai(v)).or(e.get("adresse"))), texte(e.get("code_postal")), texte(e.get("commune_affichage").filter(|v| vrai(v)).or(e.get("commune")))]
        .into_iter()
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let mut morceaux = vec![nom(e), format!("SIRET {}", texte(e.get("siret"))), adresse];
    let code = texte(t.get("code_risque"));
    if !code.is_empty() {
        let taux = texte(t.get("valeur"));
        morceaux.push(format!("code risque {}{}", code, if taux.is_empty() { String::new() } else { format!(", taux {} %", taux.replace('.', ",")) }));
    }
    if let Some(vm) = e.get("versement_mobilite").filter(|v| v.is_object()) {
        morceaux.push(format!("versement mobilité {} %", texte(vm.get("taux")).replace('.', ",")));
    }
    morceaux.into_iter().filter(|s| !s.trim().is_empty()).collect::<Vec<_>>().join(" · ")
}

/// UN CHANGEMENT QUI SE DATE : un etablissement deja ouvert dont la ligne change (`apres`), ou qu'on retire (`apres` :
/// `None`, il ferme).
pub(crate) struct Changement {
    pub nom: String,
    pub avant: Value,
    pub apres: Option<Value>,
}

impl Changement {
    /// L'entree de la boite « Ce qui change » (`fiches::changements`).
    pub(crate) fn en_json(&self) -> Value {
        json!({
            "champ": "etablissements",
            "libelle": format!("Établissement « {} »", self.nom),
            "avant": lisible(&self.avant),
            "apres": self.apres.as_ref().map_or_else(|| "fermé".to_string(), lisible),
        })
    }
}

fn liste(v: Option<&Value>) -> Vec<Value> {
    v.and_then(Value::as_array).into_iter().flatten().filter(|e| e.is_object()).cloned().collect()
}

/// CE QUI SE DATE entre la liste `anciens` (les etablissements ouverts, tels que la fiche les dit) et la liste `nouveaux`
/// (telle que l'ecran l'envoie, derivee : `fiches::deriver`). Une ligne neuve ne se date pas (son ouverture la date), une
/// ouverture corrigee non plus.
pub(crate) fn a_dater(anciens: Option<&Value>, nouveaux: Option<&Value>) -> Vec<Changement> {
    let nouveaux = liste(nouveaux);
    let mut out = Vec::new();
    for a in liste(anciens) {
        match nouveaux.iter().find(|n| nic(n) == nic(&a)) {
            None => out.push(Changement { nom: nom(&a), avant: a.clone(), apres: None }),
            Some(n) if comparable(n) != comparable(&a) => out.push(Changement { nom: nom(&a), avant: a.clone(), apres: Some(n.clone()) }),
            Some(_) => {}
        }
    }
    out
}

/// La frise d'un etablissement : a partir de chaque jour, sa ligne (`None` : ferme).
type Frise = BTreeMap<Jour, Option<Value>>;

fn au(frise: &Frise, jour: &Jour) -> Option<Value> {
    frise.range(..=*jour).next_back().and_then(|(_, v)| v.clone())
}

/// ECRIRE LA LISTE QUE L'ECRAN DONNE (`liste`) dans la fiche entreprise `contenu`, datee : voir l'en-tete. `depuis` est la
/// date d'effet de la boite « Ce qui change » ; sans elle, un changement qui se date se refuse.
pub(crate) fn ecrire(
    ctx: &Contexte, contenu: &mut Map<String, Value>, donnees: &Map<String, Value>, liste_ecran: &Value, depuis: Option<Jour>, note: Option<&str>,
) -> R<()> {
    let siren = texte(donnees.get("siren")).trim().to_string();
    let mut nouvelles: Vec<(Jour, Value)> = Vec::new();
    let mut griefs: Vec<String> = Vec::new();
    for (i, e) in liste_ecran.as_array().into_iter().flatten().enumerate() {
        let Value::Object(o) = e else { continue };
        let mut o = o.clone();
        let nom = Some(texte(o.get("libelle"))).filter(|l| !l.trim().is_empty()).unwrap_or_else(|| format!("Établissement {}", i + 1));
        let siret = texte(o.get("siret"));
        if siret.chars().count() != 14 || !siret.chars().all(|c| c.is_ascii_digit()) {
            griefs.push(format!("{} : son SIRET (quatorze chiffres, sur l'avis de situation Sirene)", nom));
        } else if !siren.is_empty() && siret[..9] != siren {
            griefs.push(format!("{} : le SIRET {} n'est pas de la compagnie (son SIREN est {})", nom, siret, siren));
        }
        for (cle, mot) in [("adresse", "son adresse"), ("code_postal", "son code postal"), ("commune", "sa commune")] {
            if texte(o.get(cle)).trim().is_empty() {
                griefs.push(format!("{} : {}", nom, mot));
            }
        }
        if let Some(vm) = o.get("versement_mobilite").filter(|v| v.is_object()) {
            if vm.get("taux").and_then(Value::as_f64).is_some_and(|t| t > 0.0) && texte(vm.get("code_insee")).trim().chars().count() != 5 {
                griefs.push(format!("{} : le code INSEE de sa commune (cinq caractères), que la DSN déclare avec le versement mobilité", nom));
            }
        }
        let ouverture = match o.get("ouverture").filter(|v| vrai(v)).map(socle::date::d) {
            Some(Ok(j)) => j,
            _ => {
                griefs.push(format!("{} : sa date d'ouverture", nom));
                continue;
            }
        };
        let mut atmp = o.get("taux_atmp").and_then(Value::as_object).cloned().unwrap_or_default();
        let du_siege = ctx.taux_atmp_de(ouverture.an).and_then(Result::ok).and_then(|t| t.get("code_risque").filter(|c| vrai(c)).map(valeur_en_texte));
        let code = texte(atmp.get("code_risque")).trim().to_string();
        if code.is_empty() {
            match du_siege {
                Some(code) => {
                    atmp.insert("code_risque".into(), Value::String(code));
                    atmp.insert("_code_de_la_compagnie".into(), Value::Bool(true));
                }
                None => griefs.push(format!("{} : son code risque AT/MP, tel que la CARSAT le notifie", nom)),
            }
        } else if du_siege.as_deref() != Some(code.as_str()) {
            // Un autre code que celui de la compagnie : celui que la CARSAT a notifie.
            atmp.shift_remove("_code_de_la_compagnie");
        }
        o.insert("taux_atmp".into(), Value::Object(atmp));
        nouvelles.push((ouverture, Value::Object(o)));
    }
    if !griefs.is_empty() {
        return Err(refus(format!("Autres établissements : il manque {}. Rien n'a été enregistré.", griefs.join(" ; "))));
    }

    // L'histoire telle que la fiche la dit : une frise par etablissement.
    let versions: Vec<(Option<Jour>, Vec<Value>)> = match contenu.get("etablissements") {
        Some(v) => droit::contexte::valeurs_datees(&json!({"etablissements": v}), "etablissements")
            .map_err(|m| refus(m))?
            .into_iter()
            .map(|(j, l)| (j, liste(Some(l))))
            .collect(),
        None => Vec::new(),
    };
    let ouverts: Vec<Value> = versions.last().map(|(_, l)| l.clone()).unwrap_or_default();
    let mut frises: BTreeMap<String, Frise> = BTreeMap::new();
    for (i, (j, l)) in versions.iter().enumerate() {
        let Some(j) = j.or_else(|| versions.get(i + 1).and_then(|(x, _)| *x)) else { continue };
        for f in frises.values_mut() {
            f.insert(j, None);
        }
        for e in l {
            frises.entry(nic(e)).or_default().insert(j, Some(e.clone()));
        }
    }
    for f in frises.values_mut() {
        // Les fermetures repetees ne disent rien de plus.
        let mut avant: Option<Option<Value>> = None;
        f.retain(|_, v| {
            let garde = avant.as_ref() != Some(v);
            avant = Some(v.clone());
            garde
        });
    }

    let changements = a_dater(Some(&Value::Array(ouverts.clone())), Some(&Value::Array(nouvelles.iter().map(|(_, e)| e.clone()).collect())));
    let date_d_effet = |c: &Changement| -> R<Jour> {
        depuis.ok_or_else(|| {
            refus(format!(
                "Changer l'établissement « {} » demande sa date d'effet (« à partir du ») : ce qui précède garde l'ancienne \
valeur, et la date ne se devine pas. Rien n'a été enregistré.",
                c.nom
            ))
        })
    };
    for c in &changements {
        let d = date_d_effet(c)?;
        let n = nic(&c.avant);
        let f = frises.entry(n.clone()).or_default();
        match &c.apres {
            None => {
                let ouvert_le = f.iter().find(|(_, v)| v.is_some()).map(|(j, _)| *j);
                if ouvert_le.is_some_and(|o| d <= o) {
                    // Retire avant d'avoir ouvert : il n'a jamais ete.
                    frises.remove(&n);
                } else {
                    f.retain(|j, _| *j < d);
                    f.insert(d, None);
                }
            }
            Some(apres) => {
                let mut apres = apres.clone();
                // Le taux qui change est celui de l'annee de sa date d'effet (D. 242-6-15) ; inchange, il garde la sienne.
                let annee = if atmp_comparable(&apres) != atmp_comparable(&c.avant) {
                    Some(json!(d.an))
                } else {
                    c.avant.get("taux_atmp").and_then(|t| t.get("annee")).cloned()
                };
                poser_annee(&mut apres, annee);
                f.insert(d, Some(apres));
            }
        }
    }
    for (ouverture, ligne) in &nouvelles {
        let n = nic(ligne);
        let deja = ouverts.iter().any(|a| nic(a) == n);
        let f = frises.entry(n.clone()).or_default();
        if f.is_empty() {
            // Un etablissement neuf : il vaut a son ouverture, son taux pour l'annee de son ouverture.
            let mut l = ligne.clone();
            poser_annee(&mut l, Some(json!(ouverture.an)));
            f.insert(*ouverture, Some(l));
            continue;
        }
        if !deja {
            // Ferme, il rouvre a la date d'effet.
            let d = depuis.ok_or_else(|| {
                refus(format!(
                    "Rouvrir l'établissement « {} » demande sa date (« à partir du »). Rien n'a été enregistré.",
                    nom(ligne)
                ))
            })?;
            let mut l = ligne.clone();
            poser_annee(&mut l, Some(json!(d.an)));
            f.retain(|j, _| *j < d);
            f.insert(d, Some(l));
            continue;
        }
        // L'ouverture se corrige : la ligne vaut a partir d'elle, avec ce qu'elle disait alors.
        let premiere = f.iter().find(|(_, v)| v.is_some()).map(|(j, _)| *j);
        if premiere != Some(*ouverture) {
            let a_l_ouverture = au(f, ouverture).or_else(|| f.values().find_map(|v| v.clone()));
            f.retain(|j, _| j > ouverture);
            f.insert(*ouverture, a_l_ouverture);
        }
        let o = json!(ouverture.iso());
        for v in f.values_mut().flatten() {
            if let Some(m) = v.as_object_mut() {
                m.insert("ouverture".into(), o.clone());
            }
        }
    }

    // La liste datee : a chaque jour ou une frise change, les etablissements ouverts.
    let mut jours: Vec<Jour> = frises.values().flat_map(|f| f.keys().copied()).collect();
    jours.sort();
    jours.dedup();
    let mut datee = vec![json!({"valeur": []})];
    let mut precedente = Value::Array(Vec::new());
    for j in jours {
        let mut ouverts_ce_jour: Vec<(Jour, String, Value)> = frises
            .iter()
            .filter_map(|(n, f)| au(f, &j).map(|v| (v.get("ouverture").map(socle::date::d).and_then(Result::ok).unwrap_or(j), n.clone(), v)))
            .collect();
        ouverts_ce_jour.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
        let valeur = Value::Array(ouverts_ce_jour.into_iter().map(|(_, _, v)| v).collect());
        if valeur == precedente {
            continue;
        }
        let mut e = json!({"depuis": j.iso(), "valeur": valeur.clone()});
        if let Some(n) = note.filter(|n| !n.trim().is_empty() && Some(j) == depuis) {
            e["note"] = json!(n.trim());
        }
        datee.push(e);
        precedente = valeur;
    }

    // Le controle que toute lecture de la configuration fait (`droit::contexte`), sur ce qui est ouvert, avant d'ecrire :
    // contre le bareme de cette annee et de l'annee de chaque ouverture (une annee sans socle livre ne se controle pas).
    let fiche = json!({"siren": siren, "nic": texte(donnees.get("nic")), "etablissements": nouvelles.iter().map(|(_, e)| e.clone()).collect::<Vec<_>>()});
    let mut annees: Vec<i32> = std::iter::once(ctx.aujourdhui.an).chain(nouvelles.iter().map(|(o, _)| o.an)).collect();
    annees.sort();
    annees.dedup();
    for an in annees {
        if let Ok(bar) = ctx.bareme_fondu(an) {
            droit::contexte::controler_etablissements(&fiche, &bar).map_err(refus)?;
        }
    }
    contenu.insert("etablissements".into(), Value::Array(datee));
    Ok(())
}

/// L'annee du taux saisi (D. 242-6-15) ; un taux laisse vide (celui de l'arrete) n'en a pas.
fn poser_annee(ligne: &mut Value, annee: Option<Value>) {
    let Some(t) = ligne.get_mut("taux_atmp").and_then(Value::as_object_mut) else { return };
    match annee {
        Some(a) if t.get("valeur").is_some_and(|v| !v.is_null()) => {
            t.insert("annee".into(), a);
        }
        _ => {
            t.shift_remove("annee");
        }
    }
}

/// LA FICHE ENTREPRISE TELLE QUE L'ECRAN LA MONTRE : ses autres etablissements sont la derniere liste de la fiche
/// (`ouverture::etablissements_dits`), ceux qui ouvrent plus tard compris ; c'est contre elle qu'un enregistrement se
/// compare.
pub(crate) fn entreprise_montree(ctx: &Contexte) -> Value {
    let mut e = ctx.entreprise.clone();
    if let Some(o) = e.as_object_mut() {
        if o.contains_key("etablissements") || ctx.entreprise_brute().get("etablissements").is_some() {
            o.insert("etablissements".into(), Value::Array(crate::routes::ouverture::etablissements_dits(ctx)));
        }
    }
    e
}
