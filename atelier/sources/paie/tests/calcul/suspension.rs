//! Un mois entier de suspension non remuneree d'un permanent (15/10/2026) : la retenue au reel du mois, la sante qui se
//! suspend (3090) ou que la fiche entreprise dit (ailleurs), les conges qui ne s'acquierent pas, l'anciennete du conge
//! parental pour moitie. Les textes : `atelier/reference/sources/suspension-non-remuneree/` (`RAPPORT-20261015.md` et
//! `LISEZ-MOI.md`). Sur des copies de `fictive-3090-grande` (Nicolet, CDI NI.24.01 a 2 310 EUR pour 151,67 h) et de
//! `fictive-1285-grande` (Nadar, CDI NA.22.01 a 3 010 EUR pour 151,67 h), dont le test ecrit le magasin `imprevus/`.

use crate::outils::{compagnie, dossier_de_test, ecrire, lire};
use engagement::calcul::Paie;
use droit::contexte::Contexte;
use socle::date::Jour;
use socle::num::Decimal;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::str::FromStr;

fn copie(fictive: &str, nom: &str) -> PathBuf {
    compagnie::saisies(fictive, &["2026-09"], &dossier_de_test("calcul_suspension", nom).join("compagnie"))
}

fn jour(s: &str) -> Jour {
    Jour::lire(s).unwrap()
}

fn contexte(racine: &Path) -> Contexte {
    Contexte::nouveau(racine, Some(jour("2026-10-15"))).unwrap()
}

fn charger(racine: &Path) -> Result<Vec<Paie>, String> {
    let ctx = contexte(racine);
    engagement::charger(&ctx, "2026-09").map(|s| s.paies)
}

fn paie_de(racine: &Path, cle: &str) -> Paie {
    charger(racine).unwrap().into_iter().find(|p| p.salarie["_cle"] == json!(cle)).unwrap()
}

fn suspension(cle: &str, debut_contrat: &str, numero: &str, motif: &str, debut: &str, fin: &str) -> Value {
    json!({
        "genre": "suspension", "salarie": cle, "contrat_debut": debut_contrat, "contrat_numero": numero,
        "motif": motif, "debut": debut, "fin": fin,
    })
}

fn nicolet(motif: &str, debut: &str, fin: &str) -> Value {
    suspension("NICOLET", "2024-03-01", "NI.24.01", motif, debut, fin)
}

fn d(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

fn montant_de(p: &Paie, code: &str) -> Decimal {
    p.remuneration.iter().filter(|l| l.code == code).fold(Decimal::ZERO, |a, l| a + l.montant)
}

fn cotise_a_la_sante(p: &Paie) -> bool {
    p.lignes.iter().any(|l| l.id == "mutuelle")
}

/// LA RETENUE D'UN MOIS ENTIER EST LE SALAIRE ENTIER. « La retenue par heure d'absence d'un salarie paye au mois doit
/// etre en principe egale au quotient du salaire mensuel par le nombre d'heures de travail dans l'entreprise pour le
/// mois considere » (Cass. soc. 11/02/1982, n° 80-40.359, `cass-soc-19820211-80-40359-JURITEXT000007009242.xml`) ; les
/// heures d'absence au reel : septembre 2026 entier, « 154 heures (22 x 7 heures) » (fiche DSN 2693, exemple 10,
/// `net-entreprises-fiche2693-volumes-activite-exemples-20260929.txt`). Le brut du mois est 0,00 -- pas 0,07 EUR d'un taux
/// horaire arrondi --, et les garanties de sante 3090 sont suspendues : ni cotisation ni part patronale (accord sante du
/// 19/07/2024, art. 4.2, `atelier/reference/sources/kali-3090-accord-sante-permanents-20240719-20261011.md`), donc rien a
/// reintegrer, un net fiscal et un net social a 0,00.
#[test]
fn un_mois_entier_de_conge_parental_retire_tout_le_salaire_et_suspend_la_sante_3090() {
    let racine = copie("fictive-3090-grande", "mois-entier");
    ecrire(&racine, "imprevus/2026-09.json", &json!({"mois": "2026-09", "imprevus": [nicolet("632", "2026-09-01", "2026-12-31")]}));
    let p = paie_de(&racine, "NICOLET");
    let absence: Vec<&engagement::calcul::LigneBrut> = p.remuneration.iter().filter(|l| l.code == "absence_non_remuneree").collect();
    assert_eq!(absence.len(), 1, "{:?}", p.remuneration);
    assert_eq!(absence[0].quantite, d("154"), "22 jours oeuvres x 7 h");
    assert_eq!(absence[0].montant, d("-2310.00"));
    assert_eq!(p.brut, d("0.00"));
    assert_eq!(p.heures_absence, d("154"));
    assert_eq!(p.engagement.heures, None, "l'activite remuneree d'un mois entierement absent est nulle (fiche 2640)");
    assert!(!cotise_a_la_sante(&p), "{:?}", p.lignes.iter().map(|l| &l.id).collect::<Vec<_>>());
    assert!(p.salarie[droit::bareme::SANTE_SUSPENDUE].as_str().is_some_and(|t| t.contains("4.2")));
    assert_eq!((p.net_imposable, p.net_social, p.net_a_payer), (d("0.00"), d("0.00"), d("0.00")));
    // La prevoyance s'assoit sur la remuneration (tranche A) : rien non plus.
    assert!(p.lignes.iter().all(|l| l.montant_sal.is_zero()), "{:?}", p.lignes.iter().map(|l| (&l.id, l.montant_sal)).collect::<Vec<_>>());
}

/// UNE SEMAINE D'ABSENCE AU QUOTIENT DU MOIS : 5 jours oeuvres (lundi 07 au vendredi 11/09/2026), 35 h au reel, sur les
/// 154 h de septembre : 2 310 x 35 / 154 = 525,00 EUR (Cass. soc. 11/02/1982 ; 14/05/1987, n° 84-43.854,
/// `cass-soc-19870514-84-43854-JURITEXT000007018764.xml`). Le mois n'est pas entierement suspendu : la sante reste due.
/// Un arret de travail se retire de meme (lundi 21 au vendredi 25/09).
#[test]
fn une_semaine_d_absence_se_retire_au_quotient_du_mois() {
    let racine = copie("fictive-3090-grande", "semaine");
    let arret = json!({
        "genre": "arret", "salarie": "NICOLET", "contrat_debut": "2024-03-01", "contrat_numero": "NI.24.01", "motif": "01",
        "dernier_jour_travaille": "2026-09-18", "fin_previsionnelle": "2026-09-25", "subrogation": "02",
    });
    ecrire(&racine, "imprevus/2026-09.json", &json!({"imprevus": [nicolet("501", "2026-09-07", "2026-09-11"), arret]}));
    let p = paie_de(&racine, "NICOLET");
    assert_eq!(montant_de(&p, "absence_non_remuneree"), d("-525.00"));
    // L'arret court du samedi 19 au vendredi 25 : cinq jours oeuvres, les memes 35 h.
    assert_eq!(montant_de(&p, "absence_maladie"), d("-525.00"));
    assert_eq!(p.brut, d("1260.00"));
    assert_eq!(p.heures_absence, d("70"));
    assert!(cotise_a_la_sante(&p), "un mois suspendu en partie garde sa sante");
}

/// HORS DE LA 3090, LA FICHE ENTREPRISE DIT CE QUE FAIT LE CONTRAT DE SANTE. L'accord sante 1285 du 30/01/2026 ne parle
/// que de la suspension « indemnisee » (`atelier/reference/sources/kali-1285-accord-frais-de-sante-20260130-20261011.md`) ;
/// pour le non indemnise, le maintien est facultatif (instruction DSS/3C/5B/2021/127, II.B,
/// `dss-instruction-3c5b-2021-127-suspension-contrat-prevoyance-20210617.txt`) : sans `sante_suspension_non_indemnisee`,
/// le mois refuse (regle 1) ; « suspendue », rien sur la paie ; « maintenue », la paie cotise -- la part salariale fait un
/// net social negatif, licite (BOSS, FAQ Q37, `atelier/reference/sources/montant-net-social/`).
#[test]
fn en_1285_le_mois_entierement_suspendu_attend_la_fiche_entreprise() {
    let racine = copie("fictive-1285-grande", "1285");
    ecrire(
        &racine,
        "imprevus/2026-09.json",
        &json!({"imprevus": [suspension("NADAR", "2022-01-03", "NA.22.01", "639", "2026-09-01", "2027-02-28")]}),
    );
    let e = charger(&racine).err().expect("sans la fiche, le mois refuse");
    assert!(e.contains(droit::bareme::SANTE_SUSPENSION), "{}", e);
    // « Pour demarrer » le demande, parce qu'une suspension non remuneree est saisie (et seulement alors).
    let manque = |r: &Path| paie::demarrage::manques(&contexte(r)).iter().any(|m| m["cle"] == json!(droit::bareme::SANTE_SUSPENSION));
    assert!(manque(&racine));
    assert!(!manque(&copie("fictive-1285-grande", "1285-sans-suspension")));

    let poser = |valeur: &str| {
        let mut ent = lire(&racine, "config/entreprise.json");
        ent[droit::bareme::SANTE_SUSPENSION] = json!(valeur);
        ecrire(&racine, "config/entreprise.json", &ent);
    };
    poser("suspendue");
    let p = paie_de(&racine, "NADAR");
    assert_eq!(p.brut, d("0.00"));
    assert!(!cotise_a_la_sante(&p));
    assert_eq!((p.net_imposable, p.net_social), (d("0.00"), d("0.00")));

    poser("maintenue");
    let p = paie_de(&racine, "NADAR");
    assert_eq!(p.brut, d("0.00"));
    assert!(cotise_a_la_sante(&p));
    assert!(p.net_social < Decimal::ZERO, "la part salariale maintenue : un net social negatif ({})", p.net_social);
}

/// LES CONGES NE S'ACQUIERENT PAS SUR UN MOIS SANS TRAVAIL : 2,5 jours « par mois de travail effectif » (L3141-3), et ni
/// le conge parental, ni le sabbatique, ni le conge sans solde ne sont des periodes assimilees (L3141-5,
/// `atelier/reference/sources/code-du-travail/articles-20261015.md` ; CCN 3090, art. 10.1.1,
/// `kali-3090-titres-vii-x-xi-annexes-20260924.md`). Le compteur du bulletin de septembre n'acquiert rien, la ou le mois
/// sans suspension en acquiert 2,5.
#[test]
fn un_mois_entierement_suspendu_n_acquiert_aucun_conge() {
    let sans = copie("fictive-3090-grande", "conges-sans");
    let p = paie_de(&sans, "NICOLET");
    let a = engagement::conges::compteur_au_bulletin(&contexte(&sans), &p, "2026-09").unwrap().acquis;

    let avec = copie("fictive-3090-grande", "conges-avec");
    ecrire(&avec, "imprevus/2026-09.json", &json!({"imprevus": [nicolet("639", "2026-09-01", "2026-12-31")]}));
    let p = paie_de(&avec, "NICOLET");
    let b = engagement::conges::compteur_au_bulletin(&contexte(&avec), &p, "2026-09").unwrap().acquis;
    assert_eq!(a - b, d("2.5"), "sans suspension {}, avec {}", a, b);
}

/// L'ANCIENNETE : « La duree du conge parental d'education a temps plein est prise en compte pour moitie pour la
/// determination des droits que le salarie tient de son anciennete » (C. trav. L1225-54,
/// `legi-extrait-code-du-travail-20261015.md`). Un an de conge parental recule le jour d'ou se compte l'anciennete de
/// 182 jours (la moitie de 365, au profit du salarie) ; un an de conge sabbatique, faute de texte, ne compte pas du tout,
/// et le moteur le dit.
#[test]
fn le_conge_parental_compte_pour_moitie_dans_l_anciennete() {
    let racine = copie("fictive-3090-grande", "anciennete");
    ecrire(&racine, "imprevus/2025-01.json", &json!({"imprevus": [nicolet("632", "2025-01-01", "2025-12-31")]}));
    let (depuis, dit) = engagement::imprevus::anciennete_depuis(&racine, "NICOLET", &jour("2024-03-01"), &jour("2026-09-30")).unwrap();
    assert_eq!(depuis.iso(), "2024-08-30", "182 jours de moins");
    assert!(dit.iter().all(|a| a.info) && dit.iter().any(|a| a.texte.contains("L1225-54")), "{:?}", dit.iter().map(|a| &a.texte).collect::<Vec<_>>());

    ecrire(&racine, "imprevus/2025-01.json", &json!({"imprevus": [nicolet("639", "2025-01-01", "2025-12-31")]}));
    let (depuis, dit) = engagement::imprevus::anciennete_depuis(&racine, "NICOLET", &jour("2024-03-01"), &jour("2026-09-30")).unwrap();
    assert_eq!(depuis.iso(), "2025-03-01", "365 jours de moins");
    assert!(dit.iter().any(|a| !a.info && a.texte.contains("aucun texte")), "{:?}", dit.iter().map(|a| &a.texte).collect::<Vec<_>>());
}
