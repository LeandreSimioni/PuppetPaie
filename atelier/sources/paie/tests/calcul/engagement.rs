//! L'`Engagement` contre `engagement.json` : les fichiers de mois des quatre compagnies fictives et leur reprise, des
//! engagements et des saisies fabriques (refus compris), la reprise d'un contrat a cheval ; sans oracle : les jumeaux,
//! les heures d'un permanent, un numero sur deux salaries. `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit l'oracle.
//! Les `permanents` de l'oracle sont une entree : les lignes de permanent de chaque mois, a la place de
//! `conges::lignes_du_mois`.

use crate::outils::{copier, fictive, dossier_de_test, lire_oracle, oracle};
use paie::contexte::Contexte;
use paie::date::{d, Jour};
use paie::engagement::{engagements, engagements_du_fichier, suivre_contrat, Avis, Engagement};
use serde_json::{json, Map, Value};
use std::cell::Cell;
use std::path::Path;

fn contexte(racine: &Path) -> Contexte {
    Contexte::nouveau(racine, Some(d(&json!("2026-09-27")).unwrap())).unwrap()
}

fn iso(v: &[Jour]) -> Value {
    json!(v.iter().map(|j| j.iso()).collect::<Vec<_>>())
}

/// `{"ok": v}` ou `{"refus": message}`.
fn essai(r: Result<Value, String>) -> Value {
    match r {
        Ok(v) => json!({"ok": v}),
        Err(e) => json!({"refus": e}),
    }
}

/// La forme canonique d'un engagement et ce qu'il dit de lui-meme.
fn releve(e: &Engagement) -> Value {
    let jours = e.dates_du_contrat();
    let mut lieux: Vec<Jour> = jours.clone();
    lieux.push(e.debut);
    let par_lieu = |p: bool| {
        Value::Array(e.dates_par_lieu(p).into_iter().map(|(l, ds)| json!([l, iso(&ds)])).collect())
    };
    let nombre = |o: Option<f64>| match o {
        None => Ok(Value::Null),
        Some(x) => paie::num::dec_f64(x).map(|v| Value::String(paie::engagement::nombre(v))),
    };
    let mut extras = Map::new();
    extras.insert("dates_du_contrat".into(), essai(Ok(iso(&jours))));
    extras.insert("lieux_joues".into(), essai(Ok(json!(e.lieux_joues()))));
    extras.insert("dates_par_lieu".into(), essai(Ok(par_lieu(true))));
    extras.insert("dates_par_lieu_mois".into(), essai(Ok(par_lieu(false))));
    extras.insert("modalite_temps_travail".into(), essai(e.modalite_temps_travail().map(Value::String)));
    extras.insert("quotite_reference".into(), essai(e.quotite_reference().and_then(nombre)));
    extras.insert("a_cheval".into(), essai(e.a_cheval().map(Value::Bool)));
    extras.insert("planning_connu".into(), essai(Ok(Value::Bool(e.planning_connu()))));
    extras.insert(
        "lieu_de".into(),
        essai(Ok(json!(lieux.iter().map(|j| e.lieu_de(j)).collect::<Vec<_>>()))),
    );
    json!({"canon": essai(e.canon()), "extras": extras})
}

fn sans_permanents(_: &Contexte, _: &Value, _: &str) -> Result<(Vec<Value>, Vec<Avis>), String> {
    Err("lignes d'un permanent non fournies".into())
}

fn reprise(ctx: &Contexte, e: &mut Engagement) -> Result<Value, String> {
    let avis = suivre_contrat(ctx, e)?;
    Ok(json!({"numero": e.numero, "jours_contrat": e.jours_contrat, "avis": avis.iter().map(|a| json!([a.info, a.texte])).collect::<Vec<_>>()}))
}

#[test]
fn les_fichiers_de_mois_et_leur_reprise() {
    let mut mois = lire_oracle("engagement.json")["mois"].clone();
    let mut engs_n = 0;
    for c in mois.as_array_mut().unwrap() {
        let racine = fictive(c["racine"].as_str().unwrap());
        let fichier = racine.join("paies").join(c["fichier"].as_str().unwrap());
        let ctx = contexte(&racine);
        let permanents = c["permanents"].as_array().unwrap().clone();
        let appel = Cell::new(0);
        let lignes = |_: &Contexte, _: &Value, _: &str| {
            let i = appel.get();
            appel.set(i + 1);
            Ok((permanents[i].as_array().unwrap().clone(), Vec::new()))
        };
        let engs = match engagements_du_fichier(&ctx, &fichier, &lignes) {
            Err(e) => {
                c["refus"] = json!(e);
                continue;
            }
            Ok((_, engs, _)) => engs,
        };
        engs_n += engs.len();
        c["engagements"] = Value::Array(engs.iter().map(releve).collect());
        // La reprise d'un contrat a cheval ; un mensuel garde ce que l'oracle en dit (il ne se reprend pas).
        if let Some(attendu) = c["reprise"].as_array().cloned() {
            let vus: Vec<Value> = engs
                .into_iter()
                .zip(attendu)
                .map(|(mut e, a)| if e.nat().unwrap().mensuelle() { a } else { reprise(&ctx, &mut e).unwrap() })
                .collect();
            c["reprise"] = Value::Array(vus);
        }
    }
    assert!(engs_n >= 300, "{}", engs_n);
    oracle("engagement.json", "/mois", &mois);
}

#[test]
fn les_engagements_fabriques_et_leurs_refus() {
    let mut directs = lire_oracle("engagement.json")["directs"].clone();
    let racine = fictive("fictive-1285-grande");
    let mut refus = 0;
    for c in directs.as_array_mut().unwrap() {
        c["attendu"] = Engagement::nouveau(&racine, &c["kwargs"]).map_or_else(|m| json!({"refus": m}), |e| releve(&e));
        refus += c["attendu"].get("refus").is_some() as usize;
    }
    // un refus par cas `refus_*`
    assert!(refus >= 30, "{}", refus);
    oracle("engagement.json", "/directs", &directs);
}

#[test]
fn les_saisies_fabriquees_et_leurs_refus() {
    let mut saisies = lire_oracle("engagement.json")["saisies"].clone();
    let mut refus = 0;
    for c in saisies.as_array_mut().unwrap() {
        let mut ctx = contexte(&fictive(c["racine"].as_str().unwrap()));
        ctx.entreprise.as_object_mut().unwrap().insert("emploie_des_permanents".into(), c["permanents"].clone());
        c["attendu"] = engagements(&ctx, &c["data"], &sans_permanents).map_or_else(|m| json!({"refus": m}), |(v, _)| Value::Array(v.iter().map(releve).collect()));
        refus += c["attendu"].get("refus").is_some() as usize;
    }
    assert!(refus >= 40, "{}", refus);
    oracle("engagement.json", "/saisies", &saisies);
}

#[test]
fn le_contrat_a_cheval_recopie_le_premier_mois() {
    let mut reprises = lire_oracle("engagement.json")["reprises"].clone();
    for c in reprises.as_array_mut().unwrap() {
        let tmp = dossier_de_test("engagement", c["nom"].as_str().unwrap());
        copier(&fictive(c["config"].as_str().unwrap()).join("config"), &tmp.join("config"));
        std::fs::create_dir_all(tmp.join("paies")).unwrap();
        for (m, data) in c["fichiers"].as_object().unwrap() {
            std::fs::write(tmp.join("paies").join(format!("{}.json", m)), data.to_string()).unwrap();
        }
        let ctx = contexte(&tmp);
        for (m, attendu) in c["attendu"].as_object_mut().unwrap() {
            let chemin = tmp.join("paies").join(format!("{}.json", m));
            *attendu = (|| -> Result<Value, String> {
                let (_, engs, _) = engagements_du_fichier(&ctx, &chemin, &sans_permanents)?;
                engs.into_iter().map(|mut e| reprise(&ctx, &mut e)).collect::<Result<Vec<_>, _>>().map(Value::Array)
            })()
            .unwrap_or_else(|m| json!({"refus": m}));
        }
    }
    oracle("engagement.json", "/reprises", &reprises);
}

/// Deux contrats a cheval de Brioche qui commencent le meme jour (les JUMEAUX) : `BR1.26.01` (5 jours) et
/// `BR1.26.02` (2 jours), ouverts en septembre. Octobre ne porte que le second.
#[test]
fn le_mois_suivant_d_un_jumeau_recopie_son_premier_mois_et_pas_celui_de_l_autre() {
    let tmp = dossier_de_test("engagement", "jumeaux");
    copier(&fictive("fictive-3090-petite").join("config"), &tmp.join("config"));
    std::fs::create_dir_all(tmp.join("paies")).unwrap();
    let ecrire = |mois: &str, engagements: Value| {
        let v = json!({"mois": mois, "spectacle": "ubu-roi", "taux_cachet": 130.0, "engagements": engagements});
        std::fs::write(tmp.join("paies").join(format!("{}.json", mois)), v.to_string()).unwrap();
    };
    let contrat = |numero: Option<&str>, fin: &str, dates: Value, planning: Value| {
        let mut e = json!({"salarie": "BRIOCHE", "spectacle": "ubu-roi", "debut": "2026-09-28", "fin": fin,
                           "lignes": [{"rubrique": "cachet", "quantite": dates.as_array().unwrap().len(), "taux": 130.0,
                                       "dates": dates, "planning": planning}]});
        if let Some(n) = numero {
            e["numero"] = json!(n);
        }
        e
    };
    ecrire(
        "2026-09",
        json!([
            contrat(Some("BR1.26.01"), "2026-10-10", json!(["2026-09-28", "2026-09-29"]), json!(["2026-10-01", "2026-10-05", "2026-10-10"])),
            contrat(Some("BR1.26.02"), "2026-10-02", json!(["2026-09-28"]), json!(["2026-10-02"])),
        ]),
    );
    let ctx = contexte(&tmp);
    let octobre = |numero: Option<&str>| -> Result<Engagement, String> {
        ecrire("2026-10", json!([contrat(numero, "2026-10-02", json!(["2026-10-02"]), json!([]))]));
        let (_, mut engs, _) = engagements_du_fichier(&ctx, &tmp.join("paies/2026-10.json"), &sans_permanents)?;
        let mut e = engs.remove(0);
        suivre_contrat(&ctx, &mut e)?;
        Ok(e)
    };
    // Par son numero, octobre retrouve SON premier mois : ses deux jours, pas les cinq du jumeau.
    let e = octobre(Some("BR1.26.02")).unwrap();
    assert_eq!((e.numero.as_deref(), e.jours_contrat), (Some("BR1.26.02"), 2));
    // Sans numero, deux contrats du meme jour en septembre : on ne devine pas lequel, on refuse.
    let refus = octobre(None).expect_err("deux jumeaux en septembre, aucun numero en octobre");
    assert!(refus.contains("BR1.26.01") && refus.contains("BR1.26.02"), "{}", refus);
}

// ------------------------------------------------- les heures d'un permanent

/// Un mois de permanent de la fictive (inventee), lu par `engagements` avec les
/// vraies lignes du mois, puis calcule.
fn mois_permanent(e: Value) -> (Engagement, paie::calcul::Paie) {
    let ctx = contexte(&fictive("fictive-1285-grande"));
    let data = json!({"mois": "2026-03", "date_paiement": "2026-03-31", "engagements": [e]});
    let (engs, _) = engagements(&ctx, &data, &paie::conges::lignes_du_mois).unwrap();
    let eng = engs.into_iter().next().unwrap();
    let p = paie::calcul::calculer(&ctx, &eng).unwrap();
    (eng, p)
}

/// Les blocs S21.G00.53 sous la remuneration 002 : (type, valeur, unite).
fn activites_dsn(p: &paie::calcul::Paie) -> Vec<(String, String, String)> {
    let mut f = paie::dsn::Fichier::nouveau();
    let (debut, fin) = (Jour::lire("2026-03-01").unwrap(), Jour::lire("2026-03-31").unwrap());
    paie::dsn::remunerations(&mut f, p, &debut, &fin, 1).unwrap();
    let valeur = |l: &str| l.split_once(",'").unwrap().1.trim_end_matches('\'').to_string();
    let l53: Vec<&String> = f.lignes.iter().filter(|l| l.starts_with("S21.G00.53.")).collect();
    l53.chunks(3).map(|c| (valeur(c[0]), valeur(c[1]), valeur(c[2]))).collect()
}

fn bloc(t: &str, v: &str, u: &str) -> (String, String, String) {
    (t.to_string(), v.to_string(), u.to_string())
}

#[test]
fn un_mois_incomplet_de_permanent_compte_ses_heures_une_fois() {
    // Du 12 au 31 mars, 20 jours sur 31 a 151,67 h : 97,85 h, et pas
    // 151,67 x (20/31) x (20/31) = 63,13 h.
    let (eng, p) = mois_permanent(json!({
        "salarie": "Dorval", "nature": "cdi", "debut": "2026-03-12", "numero": "DO.26.03",
        "salaire_mensuel": 2200.0, "quotite_contrat": 151.67
    }));
    assert_eq!(eng.heures, Some(97.85), "{:?}", eng.lignes);
    assert_eq!(p.heures, crate::outils::dec("97.85"));
    let rg = p.reduction_generale.as_ref().expect("la reduction generale d'un salaire de 2 200 EUR");
    assert_eq!(rg.heures, crate::outils::dec("97.85"));
    assert_eq!(activites_dsn(&p), vec![bloc("01", "97.85", "10")]);
}

#[test]
fn une_absence_retire_ses_heures_de_ce_que_la_dsn_declare() {
    // 20 h d'arret maladie sur un mois plein de 151,67 h : 131,67 h de
    // travail remunere, et 20 h d'absence, chacune comptee une fois.
    let (eng, p) = mois_permanent(json!({
        "salarie": "Dorval", "nature": "cdi", "debut": "2025-04-01", "numero": "DO.25.01",
        "salaire_mensuel": 2200.0, "quotite_contrat": 151.67,
        "absences": [{"motif": "maladie", "debut": "2026-03-16", "fin": "2026-03-18", "heures": 20}]
    }));
    assert_eq!(eng.heures, Some(131.67), "{:?}", eng.lignes);
    assert_eq!(p.heures_travaillees, crate::outils::dec("131.67"));
    assert_eq!(p.heures_absence, crate::outils::dec("20"));
    assert_eq!(activites_dsn(&p), vec![bloc("01", "131.67", "10"), bloc("02", "20.00", "10")]);
}

// ------------------------------------------------- un numero, un salarie (#139)

/// Un numero de contrat est celui d'UN contrat, donc d'un salarie (regle 4). Un salarie coche en plus dans un
/// bloc deja numerote recevait le numero du bloc : la saisie qui le porte sur deux salaries se refuse, en nommant
/// le numero et les deux salaries, quelle que soit la face qui l'a ecrite.
#[test]
fn un_numero_de_contrat_porte_par_deux_salaries_se_refuse() {
    let ctx = contexte(&fictive("fictive-3090-petite"));
    let mut data = crate::outils::lire(&fictive("fictive-3090-petite"), "paies/2026-08.json");
    assert!(paie::engagement::charger_donnees(&ctx, &data).is_ok());
    let numero = data["engagements"][0]["numero"].clone();
    data["engagements"][1]["numero"] = numero.clone();
    let refus = paie::engagement::charger_donnees(&ctx, &data).err().expect("un numero sur deux salaries");
    let (a, b) = (data["engagements"][0]["salarie"].as_str().unwrap(), data["engagements"][1]["salarie"].as_str().unwrap());
    let bas = refus.to_lowercase();
    assert!(refus.contains(numero.as_str().unwrap()) && bas.contains(&a.to_lowercase()) && bas.contains(&b.to_lowercase()), "{}", refus);
}

// ------------------------------------------------- le taux du cachet ne se devine pas (lot F1)

/// REGLE 1 : un engagement sans ligne de cachet n'a de taux de cachet que celui qu'il ecrit, ou que son mois ecrit.
/// Ni l'un ni l'autre : aucun (`null`), et non plus 130 EUR, un chiffre qu'aucun texte ne fixe (le Python le tenait de
/// CoolPaie ; `routes_saisie::dernier_taux` ne le propose plus depuis R12). Les deux entrees du moteur :
/// `Engagement::nouveau` (l'engagement seul) et `engagements` (le fichier du mois).
#[test]
fn un_engagement_sans_cachet_ni_taux_ecrit_n_a_aucun_taux_de_cachet() {
    let racine = fictive("fictive-1285-grande");
    let taux = |e: &Engagement| e.canon().unwrap()["taux_cachet"].clone();
    // Un mois du contrat sans cachet (`lignes: []`), et un CDI paye au mois : aucun taux de cachet n'est ecrit.
    let a_cheval = json!({"salarie": "X", "spectacle": "S", "dates_travail": [], "lignes": [],
                          "debut": "2026-09-20", "fin": "2026-11-05", "mois": "2026-10"});
    assert_eq!(taux(&Engagement::nouveau(&racine, &a_cheval).unwrap()), Value::Null);
    let cdi = json!({"salarie": "X", "spectacle": null, "dates_travail": [], "nature": "cdi", "debut": "2022-01-03",
                     "mois": "2026-10", "quotite_contrat": 151.67,
                     "lignes": [{"rubrique": "mois", "quantite": 1, "taux": 2950.5, "heures": 151.67}]});
    assert_eq!(taux(&Engagement::nouveau(&racine, &cdi).unwrap()), Value::Null);
    // Le fichier du mois : ni l'engagement ni le mois ne disent de taux, ou le mois l'ecrit `null` (ce que la page
    // ecrit quand le dossier n'a encore aucun cachet).
    let ctx = contexte(&racine);
    let mut data = json!({"mois": "2026-10", "spectacle": "Zerbinette",
                          "engagements": [{"salarie": "NADAR", "lignes": [], "debut": "2026-09-20", "fin": "2026-11-05"}]});
    let lu = |data: &Value| taux(&engagements(&ctx, data, &sans_permanents).unwrap().0[0]);
    assert_eq!(lu(&data), Value::Null);
    data["taux_cachet"] = Value::Null;
    assert_eq!(lu(&data), Value::Null);
    // Ce qui est ecrit reste ce qui compte : le taux du mois, puis celui de l'engagement.
    data["taux_cachet"] = json!(150.0);
    assert_eq!(lu(&data), json!("150"));
    data["engagements"][0]["taux_cachet"] = json!(165.0);
    assert_eq!(lu(&data), json!("165"));
}

// ------------------------------------------------- une ligne qui demande ses heures les dit (#125)

/// REGLE 1, ticket #125 : le service de repetition et la journee n'ont pas de forfait d'heures
/// (`logiciel/donnees/rubriques-base.json`, `_heures` : « partout ailleurs les heures sont saisies, parce qu'elles sont
/// un fait elles aussi ») ; seul le cachet en a un. Une telle ligne qui tait ses heures, ou les dit nulles, se refuse
/// comme une saisie pas encore finie, au lieu de payer et de declarer zero heure. La meme ligne qui les dit passe, et
/// la paie les compte : 2 services de 4 h, 8 h. Sur `fictive-3090-petite` (copie figee), aout 2026, Brioche.
#[test]
fn une_ligne_qui_demande_ses_heures_ne_passe_pas_sans_elles() {
    let racine = fictive("fictive-3090-petite");
    let ctx = contexte(&racine);
    let avec = |ligne: Value| {
        let mut data = crate::outils::lire(&racine, "paies/2026-08.json");
        data["engagements"][0]["debut"] = json!("2026-08-19");
        data["engagements"][0]["fin"] = json!("2026-08-20");
        data["engagements"][0]["lignes"] = json!([ligne]);
        engagements(&ctx, &data, &paie::conges::lignes_du_mois)
    };
    let dates = json!(["2026-08-19", "2026-08-20"]);
    for (rubrique, heures) in [("repetition", None), ("repetition", Some(json!(0))), ("journee", None), ("journee", Some(Value::Null))] {
        let mut ligne = json!({"rubrique": rubrique, "quantite": 2, "taux": 120.0, "dates": dates});
        if let Some(h) = heures.clone() {
            ligne["heures"] = h;
        }
        let refus = avec(ligne).err().unwrap_or_else(|| panic!("{} sans heures ({:?}) passe", rubrique, heures));
        assert!(refus.starts_with("L'engagement 1 de 2026-08 : sa ligne 1 ") && refus.contains("ne dit pas ses heures"), "{}", refus);
        assert!(paie::engagement::saisie_incomplete(&refus), "{}", refus);
    }
    // Une ligne de quantite nulle ne paie rien et ne compte rien : elle n'a pas d'heures a dire.
    let (engs, _) = avec(json!({"rubrique": "repetition", "quantite": 0, "taux": 120.0, "dates": dates})).unwrap();
    assert_eq!(engs[0].heures, None);
    // Avec ses heures, la ligne passe et la paie les compte.
    let (engs, _) = avec(json!({"rubrique": "repetition", "quantite": 2, "taux": 120.0, "heures": 4, "dates": dates})).unwrap();
    assert_eq!(engs[0].heures, Some(8.0));
    let p = paie::calcul::calculer(&ctx, &engs[0]).unwrap();
    assert_eq!(p.heures, crate::outils::dec("8"));
}
