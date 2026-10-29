//! `moteur arrivee [--json]` : l'arrivee d'une compagnie, etape par etape -- ce qui est pret, ce qui manque, quoi lancer.
//! Rien n'est ecrit.
//!
//! `arrivee` ne fait RIEN : elle LIT et dresse la liste des etapes, dans l'ordre ou une compagnie les franchit, chacune
//! avec son etat (`fait`, `a_faire`, `sans_objet`) et la commande qui la fait avancer. Cette liste n'existait pas encore
//! dans le moteur Rust : elle est portee ici, depuis le Python.

use serde_json::{json, Map, Value};

use super::analyse::{Args, Opt, Spec};
use super::rendu::{enveloppe, rendre_indent};
use super::Cadre;
use droit::contexte::{effectif_de, Contexte};
use socle::echec::Echec;

pub static SPEC: Spec = Spec { commande: "arrivee", positionnels: &[], options: &[Opt::drapeau("json")] };

const FAIT: &str = "fait";
const A_FAIRE: &str = "a_faire";
const SANS_OBJET: &str = "sans_objet";

struct Etape {
    cle: &'static str,
    quoi: String,
    etat: &'static str,
    detail: String,
    commandes: Vec<String>,
}

impl Etape {
    fn json(&self) -> Value {
        json!({"cle": self.cle, "quoi": self.quoi, "etat": self.etat, "detail": self.detail, "commandes": self.commandes})
    }
}

fn ko(m: String) -> Echec {
    Echec { classe: None, message: m }
}

/// `"%g" % v` de Python, la virgule pour separateur decimal (`effectif._nombre`).
fn nombre(v: f64) -> String {
    let t = if v == v.trunc() && v.abs() < 1e6 {
        format!("{}", v as i64)
    } else {
        // six chiffres significatifs, zeros de queue retires
        let s = format!("{:.6e}", v);
        let (mant, exp) = s.split_once('e').unwrap_or((&s, "0"));
        let exp: i32 = exp.parse().unwrap_or(0);
        if (-4..6).contains(&exp) {
            let decimales = (5 - exp).max(0) as usize;
            let f = format!("{:.*}", decimales, v);
            if f.contains('.') { f.trim_end_matches('0').trim_end_matches('.').to_string() } else { f }
        } else {
            let m = if mant.contains('.') { mant.trim_end_matches('0').trim_end_matches('.') } else { mant };
            format!("{}e{}{:02}", m, if exp < 0 { '-' } else { '+' }, exp.abs())
        }
    };
    t.replace('.', ",")
}

/// `Effectif.decrire`.
fn decrire(eff: &droit::contexte::Effectif, an: i32) -> String {
    match eff.valeur(an) {
        None => "effectif inconnu".to_string(),
        Some(v) => {
            let avant = eff.annees.keys().filter(|a| **a < an).max();
            match avant {
                None => format!("aucun salarie avant la premiere embauche ({})", eff.premiere_embauche.map(|a| a.to_string()).unwrap_or_else(|| "None".into())),
                Some(a) => format!("effectif {} en {}", nombre(v), a),
            }
        }
    }
}

fn erreurs(nom: &str, fiche: &Value) -> Result<Vec<Value>, Echec> {
    let m = fiche.as_object().cloned().unwrap_or_default();
    let anomalies = declarations::champs::verifier_fiche(nom, &m)?;
    Ok(anomalies.into_iter().filter(|a| a.get("gravite").and_then(Value::as_str) == Some("erreur")).collect())
}

fn txt(v: &Value, k: &str) -> String {
    v.get(k).map(super::rendu::texte).unwrap_or_default()
}

fn etapes(ctx: &Contexte, aujourdhui: socle::date::Jour) -> Result<Vec<Etape>, Echec> {
    let an = aujourdhui.an;
    let mut out: Vec<Etape> = Vec::new();

    // 1. La fiche entreprise.
    let ent = &ctx.entreprise;
    let err = erreurs("entreprise", ent)?;
    let sans_siren = txt(ent, "siren").trim().is_empty() || ent.get("siren").map_or(true, |v| !socle::fichiers::vrai(v));
    let a_faire = !err.is_empty() || sans_siren;
    out.push(Etape {
        cle: "entreprise",
        quoi: "La fiche de l'entreprise (SIRET, convention, licence, organismes)".into(),
        etat: if a_faire { A_FAIRE } else { FAIT },
        detail: if !err.is_empty() {
            err.iter().take(5).map(|a| format!("{} : {}", txt(a, "champ"), txt(a, "message"))).collect::<Vec<_>>().join("; ")
        } else {
            format!("SIREN {}, IDCC {}", txt(ent, "siren"), super::rendu::champ(ent, "idcc"))
        },
        commandes: if err.is_empty() { vec![] } else { vec!["Configuration › Entreprise (ou `moteur fiche`)".into()] },
    });

    // 2. L'import du passe : fait des qu'un engagement importe est ecrit ; a faire quand la boite `import/` porte des
    // pieces que rien n'a reprises.
    let importes = engagement::importe::tous(&ctx.racine).map_err(Echec::from)?.len();
    let boite = ctx.racine.join("import");
    let en_attente = std::fs::read_dir(&boite).map_or(0, |l| l.flatten().filter(|e| !e.file_name().to_string_lossy().starts_with('.')).count());
    let quoi_import = "Reprendre l'historique (ce qui a été déclaré avant)";
    if importes > 0 {
        out.push(Etape {
            cle: "import",
            quoi: quoi_import.into(),
            etat: FAIT,
            detail: format!("{} engagement(s) importé(s)", importes),
            commandes: vec![],
        });
    } else if en_attente > 0 {
        out.push(Etape {
            cle: "import",
            quoi: quoi_import.into(),
            etat: A_FAIRE,
            detail: format!("{} fichier(s) dans {}, pas encore repris", en_attente, boite.display()),
            commandes: vec!["Configuration › Reprendre l'historique (ou `moteur importer`, puis `moteur importer --ecrire`)".into()],
        });
    } else {
        out.push(Etape {
            cle: "import",
            quoi: quoi_import.into(),
            etat: SANS_OBJET,
            detail: "rien à reprendre : les fiches se saisissent dans Configuration (ou déposer les bulletins et les DSN d'avant dans import/)".into(),
            commandes: vec![],
        });
    }

    // 3. Les salaries.
    let fiches: Vec<(&String, &Value)> = ctx
        .salaries
        .iter()
        .filter(|(k, v)| !k.starts_with('_') && v.is_object())
        .filter(|(_, v)| v.get("actif").map_or(true, socle::fichiers::vrai))
        .collect();
    let mut a_completer: Vec<(String, Vec<Value>)> = Vec::new();
    for (k, v) in &fiches {
        let e = erreurs("salarie", v)?;
        if !e.is_empty() {
            a_completer.push(((*k).clone(), e));
        }
    }
    a_completer.sort_by(|a, b| a.0.cmp(&b.0));
    let detail = if fiches.is_empty() {
        "aucun salarié".to_string()
    } else {
        format!(
            "{} actif(s){}",
            fiches.len(),
            if a_completer.is_empty() {
                String::new()
            } else {
                format!(
                    " ; à compléter : {}",
                    a_completer
                        .iter()
                        .map(|(k, v)| format!("{} ({})", k, v.iter().take(3).map(|a| txt(a, "champ")).collect::<Vec<_>>().join(", ")))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        )
    };
    let salaries_a_faire = fiches.is_empty() || !a_completer.is_empty();
    out.push(Etape {
        cle: "salaries",
        quoi: "Les fiches des salariés actifs".into(),
        etat: if salaries_a_faire { A_FAIRE } else { FAIT },
        detail,
        commandes: if salaries_a_faire { vec!["Configuration › Salariés".into()] } else { vec![] },
    });

    // 4. L'effectif : les 118 de l'Urssaf, ou la premiere embauche.
    let eff = effectif_de(&ctx.racine, ent).map_err(ko)?;
    match eff.atteint(11, an) {
        Ok(_) => out.push(Etape { cle: "effectif", quoi: "L'effectif (seuils de 11 et 50 salariés)".into(), etat: FAIT, detail: decrire(&eff, an), commandes: vec![] }),
        Err(_) => {
            let manquent: Vec<String> = (an - 5..an).filter(|a| eff.annee(*a).is_none()).map(|a| a.to_string()).collect();
            let detail = format!(
                "effectif inconnu pour {} : l'Urssaf le renvoie en CRM de nature 118, que le SI DSN ne sert que quelques mois ; une compagnie qui a embauché pour la première fois il y a moins de cinq ans n'a qu'à renseigner `premiere_embauche`. Le 118 d'une année arrive l'été suivant (celui de 2025 le 05/08/2026) ; une plage trop large peut être refusée par le service : la découper",
                manquent.join(", ")
            );
            let depuis = aujourdhui.plus_jours(-180).map_err(ko)?;
            out.push(Etape {
                cle: "effectif",
                quoi: "L'effectif (seuils de 11 et 50 salariés)".into(),
                etat: A_FAIRE,
                detail,
                commandes: vec![format!("moteur retours --depuis {}", depuis.jj_mm_aaaa()), "Configuration › Entreprise › première embauche".into()],
            });
        }
    }

    // 5. Le taux AT/MP de la compagnie. Lu dans la SURCOUCHE, pas dans le bareme fondu : celui-ci refuse tant que
    // l'effectif n'est pas connu, et c'est justement ce qu'on examine.
    let surcouche = ctx.racine.join("config").join("baremes").join(format!("{}.json", an));
    let taux = std::fs::read(&surcouche).ok().and_then(|o| String::from_utf8(o).ok()).and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v.get("taux_atmp").cloned());
    let valeur = taux.as_ref().filter(|t| t.is_object()).and_then(|t| t.get("valeur")).filter(|v| !v.is_null()).cloned();
    out.push(Etape {
        cle: "atmp",
        quoi: format!("Le taux AT/MP notifié par la Carsat ({})", an),
        etat: if valeur.is_some() { FAIT } else { A_FAIRE },
        detail: match &valeur {
            Some(v) => format!("{} %", super::rendu::texte(v)),
            None => format!("absent de config/baremes/{}.json : il se lit sur la notification (compte AT/MP de net-entreprises) et ne se devine pas", an),
        },
        commandes: if valeur.is_some() { vec![] } else { vec![format!("config/baremes/{}.json : \"taux_atmp\": {{\"valeur\": …}}", an)] },
    });

    // 6. Les taux de PAS.
    let cles: Vec<String> = fiches.iter().map(|(k, _)| (*k).clone()).collect();
    let diag = if fiches.is_empty() { Vec::new() } else { droit::pas::diagnostic(ctx, Some(&cles), None).map_err(ko)? };
    let manquants: Vec<&Value> = diag.iter().filter(|d| matches!(d.get("situation").and_then(Value::as_str), Some("inconnu" | "perime"))).collect();
    let mut cmds = Vec::new();
    if !manquants.is_empty() {
        if crate::routes::fiches::pieces_importees(ctx).is_dir() {
            cmds.push("moteur pas --amorcer".to_string());
        }
        cmds.push("moteur pas --demander --motif 07".to_string());
    }
    out.push(Etape {
        cle: "pas",
        quoi: "Les taux de prélèvement à la source".into(),
        etat: if fiches.is_empty() { SANS_OBJET } else if !manquants.is_empty() { A_FAIRE } else { FAIT },
        detail: if fiches.is_empty() {
            "sans salarié, rien à demander".to_string()
        } else if !manquants.is_empty() {
            format!("{} salarié(s) sans taux valide : {}", manquants.len(), manquants.iter().map(|d| txt(d, "salarie")).collect::<Vec<_>>().join(", "))
        } else {
            format!("{} taux valide(s) ou barème voulu", diag.len())
        },
        commandes: cmds,
    });

    // 7. Les mots de passe des organismes (poses, jamais lus).
    let coffre = crate::compagnie::coffre::Coffre::du_poste(&ctx.racine, "");
    let poses: Vec<(String, bool)> = coffre
        .etat()
        .map_err(|e| ko(e.message))?
        .iter()
        .map(|s| (txt(s, "service"), s.get("pose").is_some_and(socle::fichiers::vrai) || s.get("dans_environnement").is_some_and(socle::fichiers::vrai)))
        .collect();
    let manque: Vec<&str> = ["net-entreprises", "urssaf"].into_iter().filter(|s| poses.iter().any(|(n, p)| n == s && !*p)).collect();
    out.push(Etape {
        cle: "coffre",
        quoi: "Les accès net-entreprises et Urssaf (DSN, DPAE, retours)".into(),
        etat: if manque.is_empty() { FAIT } else { A_FAIRE },
        detail: if manque.is_empty() { "posés".to_string() } else { format!("à poser : {}", manque.join(", ")) },
        commandes: manque.iter().map(|s| format!("moteur coffre poser {}", s)).collect(),
    });
    Ok(out)
}

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    let liste = match etapes(&cadre.ctx, cadre.aujourdhui) {
        Ok(l) => l,
        Err(e) => return cadre.plantage(e),
    };
    let pret = liste.iter().all(|e| e.etat != A_FAIRE);
    if args.drapeau("json") {
        let mut r = Map::new();
        r.insert("etapes".into(), Value::Array(liste.iter().map(Etape::json).collect()));
        r.insert("pret".into(), json!(pret));
        rendre_indent(&enveloppe("arrivee", Value::Object(r)), 1);
        return 0;
    }
    for (i, e) in liste.iter().enumerate() {
        let signe = match e.etat {
            FAIT => "ok ",
            A_FAIRE => "A FAIRE",
            _ => "-- ",
        };
        dire!("{}. [{}] {}", i + 1, signe, e.quoi);
        if !e.detail.is_empty() {
            dire!("      {}", e.detail);
        }
        for c in &e.commandes {
            dire!("      -> {}", c);
        }
    }
    if pret {
        0
    } else {
        1
    }
}
