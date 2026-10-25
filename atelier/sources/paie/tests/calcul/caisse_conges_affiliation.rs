//! Qui l'employeur declare a la caisse des Conges Spectacles, et ce que la declaration change au bulletin et a la DSN.
//!
//! Textes (atelier/reference/sources/caisse-conges-affiliation/LISEZ-MOI.md) : code du travail D. 7121-40 (« le personnel
//! artistique et technique ») et D. 7121-41 (« qu'ils n'ont pas employe de facon continue pendant les douze mois
//! precedant la demande de conge ») ; fiche Audiens (« quelle que soit [...] la nature du contrat de travail (CDD de
//! droit commun, CDD d'usage, etc) », la cotisation « sur chaque bulletin de paie d'artiste ou de technicien embauche
//! sous contrat a duree determinee de moins de 12 mois ») ; CCN 3090, art. 10.1.1 (« dans le cadre d'un meme contrat
//! de travail ») ; CSS L. 243-1-3, D. 243-0-2 (les majorations, 11,5 % ; 10 % pour la taxe d'apprentissage des seuls
//! intermittents de L. 6331-55) ; lettre circulaire Acoss 2013-019, annexe 1, R2 et R5.
//!
//! Sur une copie de la compagnie fictive figee `fictive-3090-grande` (compagnie inventee), dans le dossier temporaire des
//! tests : sa surcouche y recoit un versement mobilite fictif, la fiche de Deburau ce qu'un permanent demande (sante,
//! anciennete dans la branche), et son config/organismes.json le CTP 332 des permanents tel que le recompose
//! `donnees/organismes-base.json`. Rien n'est ecrit dans `tests/donnees/fictives/`.

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use paie::dsn::{generer, Options};
use engagement::Saisie;
use socle::num::Decimal;
use serde_json::{json, Value};
use crate::outils::{base81, chemin_unique, contrat, ctp, dec, ecrire, lire, valeur, versement, Copie};

/// La config de fictive-3090-grande, douze salaries depuis cinq ans : le versement mobilite est du (CGCT L. 2333-64), et la
/// formation des permanents passe a 1 % (code du travail L. 6331-3), que la compagnie ecrit dans sa surcouche ; Deburau
/// a la sante SVP et la prevoyance de branche. `recompose` : le fichier des organismes recompose (CTP 332 des permanents).
fn copie(recompose: bool) -> Copie {
    let c = Copie::config_de("fictive-3090-grande");
    let r = &c.0;
    let (mut bar, mut sal, mut orgs, mut eff) =
        (lire(r, "config/baremes/2026.json"), lire(r, "config/salaries.json"), lire(r, "config/organismes.json"), lire(r, "config/effectifs.json"));
    for an in eff["annees"].as_object_mut().unwrap().values_mut() {
        an["A01"]["valeur"] = json!(12.0);
    }
    bar["versement_mobilite"] = json!({"taux": 2.0, "code_insee": "45234", "autorite": "Autorite fictive", "source": "test"});
    bar["par_nature"] = json!({"permanent": {
        "cotisations_dsn": [{"bloc78": "03", "code81": "128", "taux": 1.0}],
        "rubriques_ajoutees": [{"id": "autres_4", "taux_pat": 1.0, "composants": [1.0]}]}});
    sal["DEBURAU"]["sante_svp"] = json!({"niveau": "1", "conjoint": false, "enfants": 0});
    sal["DEBURAU"]["prevoyance_branche_depuis"] = json!("2024-01-08");
    if recompose {
        for c in orgs["ctp_urssaf"].as_array_mut().unwrap() {
            if c["code"] == json!("332") && c["nature"] == json!("permanent") {
                c["assiette"] = json!("t1_x115_permanent");
            }
        }
    }
    ecrire(r, "config/effectifs.json", &eff);
    ecrire(r, "config/baremes/2026.json", &bar);
    ecrire(r, "config/salaries.json", &sal);
    ecrire(r, "config/organismes.json", &orgs);
    c
}

fn saisie(ctx: &Contexte, engagements: Value) -> Result<Saisie, String> {
    let chemin = chemin_unique("caisse-conges.json");
    let s = json!({"mois": "2026-10", "date_paiement": "2026-10-31", "engagements": engagements});
    std::fs::write(&chemin, s.to_string()).unwrap();
    let r = crate::outils::charger_fichier(ctx, &chemin);
    let _ = std::fs::remove_file(&chemin);
    r
}

/// Un CDDU de Deburau, un cachet de 100,00 le 20/10/2026 (sous le plafond du jour), du `debut` au `fin` signes.
fn cddu(numero: &str, debut: &str, fin: &str) -> Value {
    json!({"salarie": "Deburau", "spectacle": "266Z00910022", "debut": debut, "fin": fin, "numero": numero,
           "date_paiement": "2026-10-20",
           "lignes": [{"rubrique": "cachet", "quantite": 1, "taux": 100.0, "lieu": "Theatre fictif", "dates": ["2026-10-20"]}]})
}

/// Un contrat de Deburau paye au mois (2 400,00 en octobre 2026).
fn au_mois(nature: &str, numero: &str, fin: Option<&str>) -> Value {
    let mut e = json!({"salarie": "Deburau", "nature": nature, "debut": "2026-09-01", "numero": numero, "quotite_contrat": 151.67,
                       "lignes": [{"rubrique": "mois", "quantite": 1, "taux": 2400.0, "heures": 151.67}]});
    if let Some(f) = fin {
        e["fin"] = json!(f);
        e["motif_recours"] = json!("02");
    }
    e
}

/// La ligne `id` du bulletin : sa base, ou `None`.
fn base(p: &Paie, id: &str) -> Option<Decimal> {
    p.lignes.iter().find(|l| l.id == id).map(|l| l.base)
}

fn cotisation(v: &[(String, String)], code81: &str) -> bool {
    v.iter().any(|(k, x)| k == "S21.G00.81.001" && x == code81)
}

#[test]
fn un_cddu_de_douze_mois_n_est_pas_declare_a_la_caisse_des_son_premier_bulletin() {
    // Deux CDDU de Deburau qui commencent le 20/10/2026 : l'un finit le 18/10/2027 (moins de douze mois), l'autre le
    // 19/10/2027 (douze mois : le lendemain du terme est le 20/10/2027). Meme cachet de 100,00 en octobre 2026, sous le
    // plafond. Le premier garde tout ; le second, des ce premier bulletin, n'a ni la ligne de la caisse, ni le 066, ni
    // le 40.022, ni les majorations d'assiette (Acoss R5) -- et le reste du CDDU ne bouge pas.
    let copie = copie(true);
    let ctx = copie.contexte("2026-10-28");
    let s = saisie(&ctx, json!([cddu("DE3.26.10", "2026-10-20", "2027-10-18"), cddu("DE4.26.10", "2026-10-20", "2027-10-19")])).unwrap();
    let (court, long) = (&s.paies[0], &s.paies[1]);
    assert_eq!((court.brut, long.brut), (dec("100.00"), dec("100.00")));

    assert_eq!(base(court, "conges_spec"), Some(dec("100.00")), "le CDDU de moins de douze mois cotise a la caisse");
    assert_eq!(base(court, "autres_2"), Some(dec("111.50")), "FNAL majore");
    assert_eq!(base(court, "versement_mobilite"), Some(dec("111.50")));
    assert_eq!(base(court, "autres_3"), Some(dec("110.00")), "taxe d'apprentissage majoree de 10 % (L. 6331-55)");

    assert_eq!(base(long, "conges_spec"), None, "douze mois : plus de ligne de la caisse");
    assert_eq!(base(long, "autres_2"), Some(dec("100.00")), "FNAL sans majoration");
    assert_eq!(base(long, "versement_mobilite"), Some(dec("100.00")));
    assert_eq!(base(long, "autres_3"), Some(dec("100.00")));
    // Le reste est celui d'un CDDU : memes lignes, memes bases, hors la caisse et les trois majorations.
    for l in &court.lignes {
        if ["conges_spec", "autres_2", "autres_3", "versement_mobilite"].contains(&l.id.as_str()) {
            continue;
        }
        assert_eq!(base(long, &l.id), Some(l.base), "{} : le reste du CDDU ne change pas", l.id);
    }

    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    assert_eq!(valeur(&contrat(&dsn, "DE3.26.10"), "S21.G00.40.022").as_deref(), Some("970000000000F"));
    assert_eq!(valeur(&contrat(&dsn, "DE4.26.10"), "S21.G00.40.022"), None, "pas de caisse : pas de 40.022");
    let (vc, vl) = (versement(&dsn, "DE3.26.10"), versement(&dsn, "DE4.26.10"));
    assert!(cotisation(&vc, "066") && !cotisation(&vl, "066"), "la cotisation 066 de la caisse");
    assert_eq!(base81(&vc, "049"), Some(dec("111.50")));
    assert_eq!(base81(&vl, "049"), Some(dec("100.00")));
    assert_eq!(base81(&vc, "130"), Some(dec("110.00")));
    assert_eq!(base81(&vl, "130"), Some(dec("100.00")));
    // Le bordereau : le FNAL des artistes (334 sous 921) somme 111,50 et 100,00 ; la taxe d'apprentissage (992),
    // 110,00 et 100,00.
    assert_eq!(ctp(&dsn, "334", "921"), Some(dec("212")));
    assert_eq!(ctp(&dsn, "992", "920"), Some(dec("210")));
}

#[test]
fn un_artiste_en_cdd_de_droit_commun_de_trois_mois_est_declare_a_la_caisse() {
    // Deburau, comedien (COM010, annexe 10), en CDD de droit commun du 01/09 au 30/11/2026, 2 400,00 en octobre : sous
    // le plafond du mois. La caisse au bulletin (15,5 % de 2 400,00), le FNAL plafonne, le versement mobilite et la
    // taxe d'apprentissage majores de 11,5 % (D. 243-0-2, 1° : pas le 10 % des intermittents de L. 6331-55) ; dans la
    // DSN le 40.022, le 066, les 81.003 majores ; au bordereau, le 332 des permanents, le 992 et le 921 majores.
    let copie = copie(true);
    let ctx = copie.contexte("2026-10-28");
    let s = saisie(&ctx, json!([au_mois("cdd", "DE5.26.09", Some("2026-11-30"))])).unwrap();
    let p = &s.paies[0];
    assert_eq!(p.brut, dec("2400.00"));
    let l = p.lignes.iter().find(|l| l.id == "conges_spec").expect("la ligne de la caisse au bulletin");
    assert_eq!((l.base, l.montant_pat), (dec("2400.00"), dec("372.00")));
    assert_eq!(base(p, "autres_2"), Some(dec("2676.00")), "FNAL : la tranche 1 majoree");
    assert_eq!(base(p, "autres_3"), Some(dec("2676.00")), "taxe d'apprentissage majoree de 11,5 %");
    assert_eq!(base(p, "versement_mobilite"), Some(dec("2676.00")));

    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    let c = contrat(&dsn, "DE5.26.09");
    assert_eq!(valeur(&c, "S21.G00.40.007").as_deref(), Some("02"), "un CDD de droit commun reste en nature 02");
    assert_eq!(valeur(&c, "S21.G00.40.050"), None, "sans numero d'objet");
    assert_eq!(valeur(&c, "S21.G00.40.022").as_deref(), Some("970000000000F"));
    let v = versement(&dsn, "DE5.26.09");
    assert!(cotisation(&v, "066"), "la cotisation de la caisse au bloc 81");
    assert_eq!(base81(&v, "049"), Some(dec("2676.00")));
    assert_eq!(base81(&v, "130"), Some(dec("2676.00")));
    assert_eq!(base81(&v, "081"), Some(dec("2676.00")));
    assert_eq!(ctp(&dsn, "332", "921"), Some(dec("2676")), "FNAL plafonne des permanents, majore");
    assert_eq!(ctp(&dsn, "100", "921"), Some(dec("2400")), "la vieillesse plafonnee ne se majore pas");
    assert_eq!(ctp(&dsn, "992", "920"), Some(dec("2676")));
    assert_eq!(ctp(&dsn, "921", "920"), Some(dec("2676")), "versement mobilite d'un artiste");
}

#[test]
fn un_cdi_n_a_ni_la_caisse_ni_les_majorations() {
    // Le meme salaire en CDI : rien de la caisse, ni au bulletin, ni dans la DSN, ni au bordereau.
    let copie = copie(true);
    let ctx = copie.contexte("2026-10-28");
    let s = saisie(&ctx, json!([au_mois("cdi", "DE6.26.09", None)])).unwrap();
    let p = &s.paies[0];
    assert_eq!(base(p, "conges_spec"), None);
    assert_eq!(base(p, "autres_2"), Some(dec("2400.00")));
    assert_eq!(base(p, "autres_3"), Some(dec("2400.00")));
    assert_eq!(base(p, "versement_mobilite"), Some(dec("2400.00")));
    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    assert_eq!(valeur(&contrat(&dsn, "DE6.26.09"), "S21.G00.40.022"), None);
    let v = versement(&dsn, "DE6.26.09");
    assert!(!cotisation(&v, "066"));
    assert_eq!(base81(&v, "049"), Some(dec("2400.00")));
    assert_eq!(base81(&v, "130"), Some(dec("2400.00")));
    assert_eq!(ctp(&dsn, "332", "921"), Some(dec("2400")));
    assert_eq!(ctp(&dsn, "992", "920"), Some(dec("2400")));
}

#[test]
fn la_caisse_paie_les_conges_d_un_cdd_declare_l_employeur_ne_les_paie_pas_en_plus() {
    let copie = copie(true);
    let ctx = copie.contexte("2026-10-28");
    let mut e = au_mois("cdd", "DE5.26.09", Some("2026-11-30"));
    e["lignes"].as_array_mut().unwrap().push(json!({"rubrique": "indemnite_compensatrice_conges", "quantite": 2, "taux": 100.0}));
    let err = saisie(&ctx, json!([e])).map(|_| ()).unwrap_err();
    assert!(err.contains("caisse des Conges Spectacles") && err.contains("deux fois"), "{}", err);
}

/// #132 (V2f, « vu sans le faire ») : la caisse paie les conges d'un CDD declare (D. 7121-41) ; son bulletin ne montre
/// donc pas le compteur de conges de l'employeur. Celui d'un CDI, si.
#[test]
fn le_bulletin_d_un_cdd_declare_a_la_caisse_n_a_pas_le_compteur_de_conges_de_l_employeur() {
    let copie = copie(true);
    let ctx = copie.contexte("2026-10-28");
    let ent = ctx.entreprise_du_mois("2026-10").unwrap();
    let compteur = |e: Value| -> bool {
        let s = saisie(&ctx, json!([e])).unwrap();
        let d = paie::bulletin::jeux_et_description(&ctx, &s.paies, &ent, "2026-10", None, false, &Default::default()).unwrap();
        d.to_string().contains("CONGÉS PAYÉS")
    };
    assert!(compteur(au_mois("cdi", "DE6.26.09", None)), "le CDI garde son compteur");
    assert!(!compteur(au_mois("cdd", "DE5.26.09", Some("2026-11-30"))), "le CDD declare a la caisse n'en a pas");
}

/// LE CODE CONGES SPECTACLES DE LA DSN SE DEDUIT DU NUMERO D'ADHERENT (#236) : « 97 puis le numero d'adherent a la caisse
/// Conges spectacles (10 chiffres + 1 lettre). Ces 2 identifiants doivent etre accoles, sans espaces, ni tiret, etc… EX :
/// 970000000001A » (Audiens, fiche de specificites, `atelier/reference/sources/audiens-dsn/audiens-fiche-specificites-
/// conges-spectacles-20261023.txt`, l. 80-81). La fiche ne porte que le numero d'adherent ; une ancienne cle
/// `congesspectacles_affiliation` qui dirait autre chose n'est pas lue ; des blancs, un tiret ou une minuscule ne changent
/// pas la valeur (`forme_adherent`). Un numero d'adherent vraiment mal forme : la DSN refuse.
#[test]
fn le_code_de_la_caisse_en_dsn_est_97_suivi_du_numero_d_adherent() {
    let copie = copie(true);
    let r = &copie.0;
    let mut ent = lire(r, "config/entreprise.json");
    ent["congesspectacles_adherent"] = json!("0123 450001-f");
    ent["congesspectacles_affiliation"] = json!("979999999999Z");
    ecrire(r, "config/entreprise.json", &ent);
    let ctx = copie.contexte("2026-10-28");
    let s = saisie(&ctx, json!([cddu("DE3.26.10", "2026-10-20", "2026-10-20")])).unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    assert_eq!(valeur(&contrat(&dsn, "DE3.26.10"), "S21.G00.40.022").as_deref(), Some("970123450001F"));

    for mal_forme in ["", "0123450001", "0123450001-", "A123450001F", "97012345000F"] {
        ent["congesspectacles_adherent"] = json!(mal_forme);
        ecrire(r, "config/entreprise.json", &ent);
        let ctx = copie.contexte("2026-10-28");
        assert!(generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).is_err(), "{:?} : rien ne se devine", mal_forme);
    }
}
