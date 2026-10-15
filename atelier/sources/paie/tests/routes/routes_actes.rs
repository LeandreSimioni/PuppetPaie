//! Les actes du mois par la route : `dpae` (flux, zip et recapitulatif ecrits, rien de depose), `arret-signaler`,
//! `fin-contrat-signaler`, `fin-contrat-documents`, et le numero d'ordre et le contact des signalements. Sur
//! `fictive-3090-petite` (config, journal, mars 2026 et ses sorties) ; aucun reseau, aucun depot.

use crate::outils::{compagnie, dossier_de_test, ecrire, lire, noms, refus, reussi};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn compagnie(nom: &str) -> PathBuf {
    compagnie::config_et_mois("fictive-3090-petite", &["2026-03"], &dossier_de_test("routes_actes", nom).join("compagnie"))
}

fn route(racine: &Path, nom: &str, charge: Value) -> Value {
    crate::outils::route(racine, nom, charge, "2026-03-20")
}

/// Le journal sans aucune ligne de DPAE : les embauches des fictives y sont toutes deja parties.
fn sans_dpae_au_journal(racine: &Path) {
    let mut j = lire(racine, "journal/depots.json");
    j.as_array_mut().unwrap().retain(|x| x["quoi"] != json!("dpae"));
    ecrire(racine, "journal/depots.json", &j);
}

/// Les DPAE de mars 2026 que la fictive a deja ecrites (le complet et un partiel) : on part d'un dossier sans elles.
fn sans_dpae_ecrites(racine: &Path) {
    for n in noms(&racine.join("sorties/2026-03")) {
        if n.starts_with("DPAE_") {
            std::fs::remove_file(racine.join("sorties/2026-03").join(n)).unwrap();
        }
    }
}

/// Les engagements portent la cle du salarie en capitales (la route compare `salarie` a la cle).
fn engagements_en_capitales(racine: &Path, mois: &str) {
    let rel = format!("paies/{}.json", mois);
    let mut s = lire(racine, &rel);
    for e in s["engagements"].as_array_mut().unwrap() {
        let n = e["salarie"].as_str().unwrap().to_uppercase();
        e["salarie"] = json!(n);
    }
    ecrire(racine, &rel, &s);
}

#[test]
fn dpae_refuse_un_mois_sans_saisie_une_selection_introuvable_et_ce_qui_est_deja_parti() {
    let racine = compagnie("dpae-refus");
    let (_, m) = refus(&route(&racine, "dpae", json!({"mois": "2031-01"})));
    assert!(m.contains("2031-01"), "{}", m);
    // le journal des fictives dit que mars 2026 est deja parti : rien ne s'ecrit
    let (_, m) = refus(&route(&racine, "dpae", json!({"mois": "2026-03"})));
    assert!(m.contains("Deja declaree"), "{}", m);
    sans_dpae_au_journal(&racine);
    sans_dpae_ecrites(&racine);
    let avant = noms(&racine.join("sorties/2026-03"));
    let (_, m) = refus(&route(&racine, "dpae", json!({"mois": "2026-03", "paies": [{"salarie": "PERSONNE", "debut": "2026-03-24"}]})));
    assert!(m.contains("ne sont pas dans la saisie"), "{}", m);
    let (_, m) = refus(&route(&racine, "dpae", json!({"mois": "2026-03", "paies": []})));
    assert!(m.contains("Aucune embauche retenue"), "{}", m);
    assert_eq!(noms(&racine.join("sorties/2026-03")), avant, "un refus n'ecrit rien");
}

#[test]
fn dpae_refuse_deux_lignes_identiques_d_une_saisie_et_dit_lesquelles() {
    // Retours Windows, D : deux lignes identiques declaraient deux fois la meme embauche, sans avertissement.
    let racine = compagnie("dpae-doublon");
    sans_dpae_au_journal(&racine);
    sans_dpae_ecrites(&racine);
    let avant = noms(&racine.join("sorties/2026-03"));
    let mut s = lire(&racine, "paies/2026-03.json");
    let premiere = s["engagements"][0].clone();
    s["engagements"].as_array_mut().unwrap().push(premiere);
    ecrire(&racine, "paies/2026-03.json", &s);
    let (_, m) = refus(&route(&racine, "dpae", json!({"mois": "2026-03"})));
    assert!(m.starts_with("Deux lignes identiques dans la saisie de 2026-03 declareraient deux fois la meme embauche : BRIOCHE au 2026-03-24."), "{}", m);
    assert!(m.contains("Corrigez la saisie") && m.ends_with("Rien n'est ecrit."), "{}", m);
    // La selection de cette embauche est refusee de la meme facon.
    let (_, m) = refus(&route(&racine, "dpae", json!({"mois": "2026-03", "paies": [{"salarie": "BRIOCHE", "debut": "2026-03-24"}]})));
    assert!(m.contains("BRIOCHE au 2026-03-24"), "{}", m);
    assert_eq!(noms(&racine.join("sorties/2026-03")), avant, "un refus n'ecrit rien");
    // Une ligne qui differe (un autre lieu) n'est pas un doublon : le refus ne vise que l'identique.
    let mut s = lire(&racine, "paies/2026-03.json");
    let n = s["engagements"].as_array().unwrap().len();
    s["engagements"][n - 1]["lignes"][0]["lieu"] = json!("Ailleurs");
    ecrire(&racine, "paies/2026-03.json", &s);
    reussi(&route(&racine, "dpae", json!({"mois": "2026-03"})));
}

#[test]
fn dpae_ecrit_le_flux_son_zip_et_le_recapitulatif_et_ne_depose_rien() {
    let racine = compagnie("dpae-mois");
    sans_dpae_au_journal(&racine);
    sans_dpae_ecrites(&racine);
    let journal_avant = std::fs::read(racine.join("journal/depots.json")).unwrap();
    let r = route(&racine, "dpae", json!({"mois": "2026-03"}));
    let res = reussi(&r);
    assert_eq!(res["libelle"], json!("Mars 2026"), "{}", r);
    assert_eq!(res["lignes"].as_array().unwrap().len(), 3, "{}", r);
    let dossier = racine.join("sorties/2026-03");
    let flux = std::fs::read(dossier.join("DPAE_Mars2026.txt")).unwrap();
    assert_eq!(flux.len(), 3 * 990, "un enregistrement de 990 caracteres par embauche");
    assert!(dossier.join("DPAE_Mars2026.txt.zip").exists());
    assert!(dossier.join("DPAE_Mars2026_recapitulatif.txt").exists());
    assert_eq!(std::fs::read(racine.join("journal/depots.json")).unwrap(), journal_avant, "rien n'est depose, rien n'est note");
    // Retours Windows, E : la phrase du depot dit ce qui est vrai (le moteur depose lui-meme), pas « le depot reste manuel ».
    let detail = res["etapes"][0]["detail"].as_str().unwrap();
    assert!(!detail.contains("reste manuel") && !detail.contains("ne connait pas le mot de passe"), "{}", detail);
    assert!(detail.contains("Le moteur depose lui-meme : en test, sans rien demander ; en reel, avec le mot de passe du logiciel, tape par l'utilisateur au moment du depot."), "{}", detail);
    // Retours Windows, E (suite) : aucune commande proposee ; le geste est celui de la fenetre.
    assert!(detail.ends_with("Le geste : dans la fenetre, l'action « Envoyer les DPAE… » du mois (le test d'abord, puis l'envoi du fichier teste)."), "{}", detail);
    assert!(res["etapes"].as_array().unwrap().iter().all(|e| e["commande"].is_null() && e["detail"].as_str().unwrap().contains("Le geste : ")), "{}", r);
    // on est le 20 mars : l'embauche du 24 est dans quatre jours
    assert_eq!(res["lignes"][0]["etat"], json!("ok"), "{}", r);
    assert_eq!(res["lignes"][0]["jours_restants"], json!(4), "{}", r);
}

#[test]
fn dpae_d_une_selection_va_dans_un_fichier_a_cote_et_se_reprend_a_l_identique() {
    let racine = compagnie("dpae-partiel");
    sans_dpae_au_journal(&racine);
    sans_dpae_ecrites(&racine);
    let charge = json!({"mois": "2026-03", "paies": [{"salarie": "BRIOCHE", "debut": "2026-03-24"}]});
    reussi(&route(&racine, "dpae", charge.clone()));
    let dossier = racine.join("sorties/2026-03");
    let partiels: Vec<String> = noms(&dossier).into_iter().filter(|n| n.starts_with("DPAE_Mars2026_2") && n.ends_with(".txt") && !n.contains("recapitulatif")).collect();
    assert_eq!(partiels.len(), 1, "{:?}", noms(&dossier));
    assert!(!dossier.join("DPAE_Mars2026.txt").exists(), "le complet du mois n'est pas ecrit par une selection");
    assert_eq!(std::fs::read(dossier.join(&partiels[0])).unwrap().len(), 990);
    reussi(&route(&racine, "dpae", charge));
    let apres: Vec<String> = noms(&dossier).into_iter().filter(|n| n.starts_with("DPAE_Mars2026_2") && n.ends_with(".txt") && !n.contains("recapitulatif")).collect();
    assert_eq!(apres, partiels, "le meme contenu reprend le meme fichier");
}

#[test]
fn arret_signaler_refuse_un_arret_inconnu_et_produit_le_fichier_sans_toucher_au_magasin() {
    let racine = compagnie("arret");
    let (_, m) = refus(&route(&racine, "arret-signaler", json!({"salarie": "BRIOCHE", "dernier_jour": "2026-03-25"})));
    assert!(m.contains("Aucun arret"), "{}", m);
    engagements_en_capitales(&racine, "2026-03");
    reussi(&route(
        &racine,
        "arret-enregistrer",
        json!({"salarie": "BRIOCHE", "dernier_jour": "2026-03-25", "fin": "2026-03-26", "motif": "01", "subrogation": "02"}),
    ));
    let magasin = std::fs::read(racine.join("imprevus/2026-03.json")).unwrap();
    let r = route(&racine, "arret-signaler", json!({"salarie": "BRIOCHE", "dernier_jour": "2026-03-25"}));
    let res = reussi(&r);
    assert_eq!(res["nom"], json!("SignalementArret_BRIOCHE_20260325.dsn"), "{}", r);
    assert_eq!(res["ordre"], json!(1), "{}", r);
    assert!(racine.join("sorties/2026-03/SignalementArret_BRIOCHE_20260325.dsn").exists());
    assert_eq!(std::fs::read(racine.join("imprevus/2026-03.json")).unwrap(), magasin, "produire ne note aucun envoi");
}

#[test]
fn fin_de_contrat_refuse_ce_qui_manque_et_les_documents_s_ecrivent() {
    let racine = compagnie("fin");
    let (_, m) = refus(&route(&racine, "fin-contrat-signaler", json!({"mois": "2031-01", "salarie": "BRIOCHE"})));
    assert!(m.contains("Aucune saisie"), "{}", m);
    // Un nom qui ne designe aucune fiche se refuse comme partout dans le moteur (`Contexte::salarie`).
    let (_, m) = refus(&route(&racine, "fin-contrat-documents", json!({"mois": "2026-03", "salarie": "PERSONNE"})));
    assert!(m.contains("inconnu") && m.contains("PERSONNE"), "{}", m);
    engagements_en_capitales(&racine, "2026-03");
    let (_, m) = refus(&route(&racine, "fin-contrat-signaler", json!({"mois": "2026-03", "salarie": "BRIOCHE"})));
    assert!(m.contains("motif_rupture"), "{}", m);
    assert!(!racine.join("sorties/2026-03/SignalementFinContrat_BRIOCHE_20260326.dsn").exists());
    let r = route(&racine, "fin-contrat-documents", json!({"mois": "2026-03", "salarie": "BRIOCHE"}));
    let res = reussi(&r);
    assert_eq!(res["noms"], json!(["CertificatTravail_BRIOCHE_20260326.pdf", "RecuSoldeToutCompte_BRIOCHE_20260326.pdf"]), "{}", r);
    for rel in ["CertificatTravail_BRIOCHE_20260326.pdf", "RecuSoldeToutCompte_BRIOCHE_20260326.pdf"] {
        let pdf = std::fs::read(racine.join("sorties/2026-03").join(rel)).unwrap();
        assert!(pdf.starts_with(b"%PDF"), "{}", rel);
    }
    assert_eq!(res["avertissements"].as_array().unwrap().len(), 3);
}

// ------------------------------------------------------------------ le numero d'ordre des signalements (V1h, point 3)

/// Un signalement REEL deja parti, note au journal des envois comme le depot le note (`detail.nature`, `detail.ordre`).
fn signalement_parti(racine: &Path, nature: &str, ordre: Value, accepte: bool) {
    let mut j = lire(racine, "journal/depots.json");
    j.as_array_mut().unwrap().push(json!({
        "quoi": "dsn", "quand": "2026-02-27T10:00:00", "origine": "", "mois": "2026-02-27",
        "fichier": "SignalementFinContrat_AUTRE_20260227.dsn", "empreinte": null, "mode": "reel", "qui": "interface",
        "accepte": accepte, "destinataire": "net-entreprises", "flux": "", "retours": [], "reponse": "", "trace": null,
        "detail": {"preuve": "fenetre", "type": "01", "nature": nature, "ordre": ordre},
    }));
    ecrire(racine, "journal/depots.json", &j);
}

/// La valeur d'une rubrique dans un fichier DSN (latin-1, une rubrique par ligne).
fn rubrique(chemin: &Path, code: &str) -> String {
    let t: String = std::fs::read(chemin).unwrap().iter().map(|&o| o as char).collect();
    let l = t.lines().find(|l| l.starts_with(&format!("{},", code))).unwrap_or_else(|| panic!("{} absente de {}", code, chemin.display()));
    l[code.len() + 1..].trim_matches('\'').to_string()
}

#[test]
fn un_arret_se_numerote_a_la_suite_de_tous_les_signalements_deja_partis() {
    // Cahier technique DSN 2026.1, p. 136 : « Pour un signalement, ce numero ne doit pas etre remis a zero afin qu'il soit
    // unique quel que soit le mois d'emission. » Une fin de contrat est partie a l'ordre 6 : le premier arret de BRIOCHE
    // part au 7, pas au 1 (le rang parmi les envois de CET arret, que le moteur prenait).
    let racine = compagnie("ordre-arret");
    engagements_en_capitales(&racine, "2026-03");
    signalement_parti(&racine, "07", json!("6"), true);
    // Un envoi que le point de depot n'a pas recu ne prend aucun numero.
    signalement_parti(&racine, "04", json!(null), false);
    reussi(&route(
        &racine,
        "arret-enregistrer",
        json!({"salarie": "BRIOCHE", "dernier_jour": "2026-03-25", "fin": "2026-03-26", "motif": "01", "subrogation": "02"}),
    ));
    let r = route(&racine, "arret-signaler", json!({"salarie": "BRIOCHE", "dernier_jour": "2026-03-25"}));
    assert_eq!(reussi(&r)["ordre"], json!(7), "{}", r);
    assert_eq!(rubrique(&racine.join("sorties/2026-03/SignalementArret_BRIOCHE_20260325.dsn"), "S20.G00.05.004"), "7");
}

#[test]
fn sans_la_plate_forme_l_ordre_d_un_signalement_suit_le_journal_et_la_route_le_dit() {
    // Lot O1 : avant d'ecrire un signalement, le moteur releve les retours de la plate-forme, qui fait foi. Ici elle ne
    // se consulte pas (le bac a sable ne laisse aucun mot de passe) : l'ordre suit le journal, et la route le dit.
    let racine = compagnie("ordre-sans-plateforme");
    engagements_en_capitales(&racine, "2026-03");
    signalement_parti(&racine, "07", json!("6"), true);
    reussi(&route(
        &racine,
        "arret-enregistrer",
        json!({"salarie": "BRIOCHE", "dernier_jour": "2026-03-25", "fin": "2026-03-26", "motif": "01", "subrogation": "02"}),
    ));
    let r = route(&racine, "arret-signaler", json!({"salarie": "BRIOCHE", "dernier_jour": "2026-03-25"}));
    let res = reussi(&r);
    assert_eq!(res["ordre"], json!(7), "{}", r);
    let dit = res["avertissements"].as_array().unwrap().iter().filter_map(|a| a.as_str()).any(|a| a.contains("n'a pas pu etre consultee") && a.contains("Mot de passe net-entreprises absent"));
    assert!(dit, "{}", r);
}

#[test]
fn un_signalement_parti_sans_son_ordre_refuse_le_suivant_et_dit_ou_l_ecrire() {
    let racine = compagnie("ordre-illisible");
    engagements_en_capitales(&racine, "2026-03");
    signalement_parti(&racine, "07", json!(null), true);
    reussi(&route(
        &racine,
        "arret-enregistrer",
        json!({"salarie": "BRIOCHE", "dernier_jour": "2026-03-25", "fin": "2026-03-26", "motif": "01", "subrogation": "02"}),
    ));
    let (_, m) = refus(&route(&racine, "arret-signaler", json!({"salarie": "BRIOCHE", "dernier_jour": "2026-03-25"})));
    assert!(m.contains("sans son numero d'ordre") && m.contains("\"ordre\": N") && m.contains("depots.json"), "{}", m);
    assert!(!racine.join("sorties/2026-03/SignalementArret_BRIOCHE_20260325.dsn").exists());
}

#[test]
fn les_amorcages_d_un_lot_prennent_chacun_un_ordre_a_la_suite_de_ceux_deja_partis() {
    // Deux salaries sans taux dans le meme lot partaient tous deux a l'ordre 1 : le second serait refuse (p. 14).
    let racine = compagnie("ordre-amorcage");
    sans_dpae_au_journal(&racine);
    sans_dpae_ecrites(&racine);
    let _ = std::fs::remove_file(racine.join("config/pas.json"));
    signalement_parti(&racine, "08", json!(3), true);
    let r = route(&racine, "dpae", json!({"mois": "2026-03"}));
    // Retours Windows, E (suite) : l'etape de l'amorcage se fait elle aussi depuis la fenetre, sans commande proposee.
    let etapes = reussi(&r)["etapes"].as_array().unwrap();
    assert_eq!(etapes[1]["cle"], json!("amorcage"), "{}", r);
    assert!(etapes.iter().all(|e| e["commande"].is_null()), "{}", r);
    assert!(etapes[1]["detail"].as_str().unwrap().contains("le depot depuis la fenetre"), "{}", r);
    let dossier = racine.join("sorties/2026-03");
    let mut ordres: Vec<i64> = noms(&dossier)
        .into_iter()
        .filter(|n| n.starts_with("Amorcage") && n.ends_with(".dsn"))
        .map(|n| rubrique(&dossier.join(n), "S20.G00.05.004").parse().unwrap())
        .collect();
    ordres.sort();
    assert!(ordres.len() >= 2, "le cas veut au moins deux amorcages : {:?}", noms(&dossier));
    assert_eq!(ordres, (4..4 + ordres.len() as i64).collect::<Vec<_>>());
}

#[test]
fn un_signalement_reel_a_un_ordre_deja_pris_ne_part_pas() {
    // Ecrit a l'ordre 1, puis une fin de contrat part a l'ordre 1 avant lui : il se refait (p. 14 : le second signalement
    // normal au meme SIRET et au meme ordre n'est pas accepte). Refuse avant toute preuve : aucun mot de passe n'y change rien.
    let racine = compagnie("ordre-depot");
    engagements_en_capitales(&racine, "2026-03");
    reussi(&route(
        &racine,
        "arret-enregistrer",
        json!({"salarie": "BRIOCHE", "dernier_jour": "2026-03-25", "fin": "2026-03-26", "motif": "01", "subrogation": "02"}),
    ));
    let r = route(&racine, "arret-signaler", json!({"salarie": "BRIOCHE", "dernier_jour": "2026-03-25"}));
    assert_eq!(reussi(&r)["ordre"], json!(1), "{}", r);
    signalement_parti(&racine, "07", json!("1"), true);
    let fichier = racine.join("sorties/2026-03/SignalementArret_BRIOCHE_20260325.dsn");
    let (_, m) = refus(&route(&racine, "deposer", json!({"quoi": "dsn", "fichier": fichier.to_string_lossy(), "reel": true})));
    assert!(m.contains("porte l'ordre 1") && m.contains("il partira a l'ordre 2") && m.contains("Rien n'est parti"), "{}", m);
}

// ------------------------------------------------------------------ le contact emetteur des signalements (V1h, point 2)

#[test]
fn le_contact_d_un_signalement_est_le_declarant_avec_sa_civilite_ou_rien_ne_s_ecrit() {
    let racine = compagnie("contact");
    engagements_en_capitales(&racine, "2026-03");
    reussi(&route(
        &racine,
        "arret-enregistrer",
        json!({"salarie": "BRIOCHE", "dernier_jour": "2026-03-25", "fin": "2026-03-26", "motif": "01", "subrogation": "02"}),
    ));
    let mut e = lire(&racine, "config/entreprise.json");
    e["declarant_dsn"] = json!({"nom_prenom": "MOLIERE JEAN", "tel": "0100000099", "email": "ADMINISTRATION@DEMO.INVALID", "qualite": "04"});
    ecrire(&racine, "config/entreprise.json", &e);
    let (_, m) = refus(&route(&racine, "arret-signaler", json!({"salarie": "BRIOCHE", "dernier_jour": "2026-03-25"})));
    assert!(m.contains("civilité") && m.contains("S10.G00.02.001"), "{}", m);
    let fichier = racine.join("sorties/2026-03/SignalementArret_BRIOCHE_20260325.dsn");
    assert!(!fichier.exists());
    e["declarant_dsn"]["civilite"] = json!("01");
    ecrire(&racine, "config/entreprise.json", &e);
    reussi(&route(&racine, "arret-signaler", json!({"salarie": "BRIOCHE", "dernier_jour": "2026-03-25"})));
    // Le signataire de la fictive est SAND GEORGE : il ne se melange plus au courriel du declarant.
    assert_eq!(
        ["S10.G00.02.001", "S10.G00.02.002", "S10.G00.02.004"].map(|c| rubrique(&fichier, c)),
        ["01", "MOLIERE JEAN", "ADMINISTRATION@DEMO.INVALID"].map(String::from)
    );
}
