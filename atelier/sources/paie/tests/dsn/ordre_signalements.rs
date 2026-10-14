//! Le numero d'ordre d'un signalement, la plate-forme faisant foi (lot O1, decision de Leandre du 14/10/2026) :
//! `ordre_signalements::dernier_ordre_connu` releve les retours des trois derniers mois et prend le plus grand ordre que
//! la plate-forme (ses bilans de nature 11) et le journal de ce dossier connaissent ; sans reseau, le journal et les
//! bilans deja archives, et il le dit. Et la releve qui reconnait B1-208-10 sur un signalement.
//!
//! AUCUN RESEAU : le client parle a un faux net-entreprises local (`outils::serveur`), ou a un port ferme. Jamais
//! `None` pour le client : il irait chercher le vrai mot de passe.

use crate::outils::{compagnie, dossier_de_test, ecrire, http, lire, noms, port_ferme, serveur, Recu, Requete};
use paie::contexte::Contexte;
use paie::date::Jour;
use paie::ordre_signalements::dernier_ordre_connu;
use paie::releve_retours::{iso_de_naif, maintenant_naif, relever_retours, Adresses, Client};
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Le SIREN de `demo-3090-petite`.
const SIREN: &str = "123456923";

fn compagnie(nom: &str) -> PathBuf {
    compagnie::saisies("demo-3090-petite", &[], &dossier_de_test("ordre_signalements", nom).join("compagnie"))
}

fn contexte(racine: &Path) -> Contexte {
    Contexte::nouveau(racine, Some(Jour::lire("2026-10-14").unwrap())).unwrap()
}

/// Un signalement deja parti de CE dossier, note comme le depot le note (`detail.nature`, `detail.ordre`).
fn signalement_au_journal(racine: &Path, mode: &str, nature: &str, ordre: &str) {
    let mut j = lire(racine, "journal/depots.json");
    j.as_array_mut().unwrap().push(json!({
        "quoi": "dsn", "quand": "2026-09-27T10:00:00", "origine": "", "mois": "2026-09-27",
        "fichier": "SignalementFinContrat_AUTRE_20260927.dsn", "empreinte": null, "mode": mode, "qui": "ligne de commande",
        "accepte": true, "destinataire": "net-entreprises", "flux": "", "retours": [], "reponse": "", "trace": null,
        "detail": {"preuve": "", "type": "01", "nature": nature, "ordre": ordre},
    }));
    ecrire(racine, "journal/depots.json", &j);
}

/// Un bilan de nature 11 comme la plate-forme le rend (schema `v01r08`, celui du certificat du 24/09/2026) : un
/// certificat (`CCO`) ou, avec des anomalies, un bilan d'anomalies (`BAN`). Ni salarie ni contrat : il n'y en a pas.
fn bilan(flux: &str, essai_reel: &str, siren: &str, nature: &str, ordre: &str, anomalies: &[(&str, &str)]) -> String {
    let (type_rapport, etat) = if anomalies.is_empty() { ("CCO", "OK") } else { ("BAN", "KO") };
    let mut a = String::new();
    for (code, message) in anomalies {
        a += &format!(
            "<declaration_anomalie><description><code>{}</code><categorie>bloquant</categorie><message>{}</message><nombre>1</nombre></description></declaration_anomalie>",
            code, message
        );
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gipmds:rapport xmlns:gipmds=\"http://www.gip-mds.fr/\" profil=\"DSN\" type=\"{t}\" version=\"v01r08\">\
<envoi><envoi_identification><declarant><siret>{s}00010</siret><nom>FICTIF</nom><prenom>DECLARANT</prenom></declarant><idflux>{f}</idflux>\
<essai_reel>{e}</essai_reel><emetteur_siret>{s}00010</emetteur_siret></envoi_identification><envoi_bilan><envoi_etat>OK</envoi_etat></envoi_bilan></envoi>\
<declaration><declaration_identification><rang>1</rang><nature>{n}</nature><type>01</type><fraction>11</fraction><ordre>{o}</ordre>\
<SIREN>{s}</SIREN><nic_siege>00010</nic_siege><nic_affectation>00010</nic_affectation></declaration_identification>\
<declaration_bilan><etat>{et}</etat><declaration_contenu><salaries>1</salaries><contrats>1</contrats></declaration_contenu></declaration_bilan>{a}</declaration>\
</gipmds:rapport>",
        t = type_rapport, s = siren, f = flux, e = essai_reel, n = nature, o = ordre, et = etat, a = a
    )
}

/// La publication d'un retour : il y a douze heures, en 14 chiffres.
fn hier() -> String {
    let iso = iso_de_naif(maintenant_naif() - 43_200);
    iso.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// Un faux net-entreprises : l'authentification, la liste des retours par plage (seulement la tranche qui contient la
/// publication, 404 ailleurs : « rien sur cette plage »), le telechargement des bilans.
fn plateforme(retours: Vec<(&'static str, String)>) -> (String, Recu) {
    let publication = hier();
    serveur(move |q: &Requete| {
        let chemin = q.chemin.trim_start_matches('/');
        if chemin.starts_with("authentifier") {
            return Some(http(200, "text/plain", b"JETON-FICTIF"));
        }
        if let Some(plage) = chemin.strip_prefix("lister-retours-declarant/1.0/") {
            let bornes: Vec<&str> = plage.trim_end_matches('/').split('/').collect();
            let (debut, fin) = (bornes[0], bornes.get(1).copied().unwrap_or("99999999999999"));
            if !(publication.as_str() >= debut && publication.as_str() <= fin) || retours.is_empty() {
                return Some(http(404, "text/plain", b"Aucun retour sur cette plage"));
            }
            let mut xml = String::from("<retours>");
            for (i, (flux, _)) in retours.iter().enumerate() {
                xml += &format!(
                    "<flux><id>{}</id><retour><publication>{p}</publication><production>{p}</production><nature>11</nature><statut>OK</statut><url>{}/telecharger/{}</url></retour></flux>",
                    flux, q.base, i, p = publication
                );
            }
            xml += "</retours>";
            return Some(http(200, "application/xml", xml.as_bytes()));
        }
        if let Some(i) = chemin.strip_prefix("telecharger/") {
            let (_, contenu) = &retours[i.parse::<usize>().unwrap()];
            return Some(http(200, "application/xml", contenu.as_bytes()));
        }
        Some(http(404, "text/plain", b"inconnu"))
    })
}

/// Le client net-entreprises, pointe sur une adresse locale.
fn client(ctx: &Contexte, base: &str) -> Client {
    assert!(base.starts_with("http://127.0.0.1:"), "le faux serveur est local");
    let mut c = Client::dsn(ctx, "production", Some("faux-motdepasse".into())).unwrap();
    let service = c.adresses.service.clone();
    c.adresses = Adresses {
        authentification: format!("{}/authentifier/", base),
        retours: format!("{}/lister-retours-declarant/1.0/", base),
        retours_flux: format!("{}/lister-retours-flux/1.0/", base),
        depot: format!("{}/deposer-dsn/1.0/", base),
        service,
    };
    c
}

#[test]
fn l_ordre_suit_le_plus_grand_que_la_plate_forme_connait_quand_le_journal_en_connait_moins() {
    // Cahier technique DSN 2026.1, §1.4.3.2, p. 14 : un second signalement normal au meme SIRET et au meme ordre n'est pas
    // accepte. Le journal de CE dossier s'arrete a 6 ; la plate-forme a certifie un essai a l'ordre 9, depose d'ailleurs
    // (un autre poste, une copie) : le suivant est 10, pas 7. Un signalement d'un autre SIREN et une mensuelle (dont
    // l'ordre est celui du mois, p. 136) ne comptent pas.
    let racine = compagnie("plateforme");
    signalement_au_journal(&racine, "reel", "07", "6");
    let (base, _) = plateforme(vec![
        ("FLUX-ESSAI", bilan("FLUX-ESSAI", "01", SIREN, "07", "9", &[])),
        ("FLUX-AUTRE-SIREN", bilan("FLUX-AUTRE-SIREN", "01", "999999999", "04", "50", &[])),
        ("FLUX-MENSUELLE", bilan("FLUX-MENSUELLE", "02", SIREN, "01", "30", &[])),
    ]);
    let ctx = contexte(&racine);
    let connu = dernier_ordre_connu(&HashMap::new(), &ctx, Some(client(&ctx, &base))).unwrap();
    assert_eq!(connu.dernier, 9, "{:?}", connu);
    assert!(
        connu.avertissements.iter().any(|a| a.contains("ordre 9") && a.contains("FLUX-ESSAI") && a.contains("6")),
        "le moteur dit d'ou vient l'ordre : {:?}",
        connu.avertissements
    );
    // La piece est archivee, comme toute releve : elle est la preuve de ce qu'on a lu.
    assert!(noms(&racine.join("retours")).iter().any(|n| n.contains("_nature11_FLUX-ESSAI")), "{:?}", noms(&racine.join("retours")));
}

#[test]
fn sans_reseau_l_ordre_suit_le_journal_et_les_bilans_archives_et_le_dit() {
    // La plate-forme injoignable : le moteur prend ce que ce dossier sait (le journal, et les bilans deja archives d'une
    // releve passee), et le dit -- jamais en silence.
    let racine = compagnie("sans-reseau");
    signalement_au_journal(&racine, "reel", "04", "6");
    std::fs::create_dir_all(racine.join("retours")).unwrap();
    std::fs::write(
        racine.join("retours/20260920-101010_nature11_FLUX-ANCIEN.xml"),
        bilan("FLUX-ANCIEN", "02", SIREN, "05", "8", &[]),
    )
    .unwrap();
    let ctx = contexte(&racine);
    let connu = dernier_ordre_connu(&HashMap::new(), &ctx, Some(client(&ctx, &port_ferme()))).unwrap();
    assert_eq!(connu.dernier, 8, "{:?}", connu);
    assert!(connu.avertissements.iter().any(|a| a.contains("n'a pas pu etre consultee")), "{:?}", connu.avertissements);
}

#[test]
fn un_signalement_refuse_b1_208_10_se_reconnait_et_la_releve_dit_la_conduite_a_tenir() {
    // B1-208-10 sur un signalement : « Il existe deja une declaration normale ». Pour un signalement, la cle est (SIRET,
    // ordre) (p. 14) ; le bilan ne dit pas quel evenement porte deja cet ordre (PASSATION-O1, point 1). La releve le
    // reconnait et dit quoi faire : refaire a l'ordre libre, ou, si c'est le meme evenement deja accepte, un
    // annule-et-remplace a un ordre superieur (§1.4.3.4, p. 15).
    let racine = compagnie("b1-208-10");
    let (base, _) = plateforme(vec![(
        "FLUX-REFUSE",
        bilan("FLUX-REFUSE", "01", SIREN, "07", "1", &[("B1-208-10", "Il existe deja une declaration normale.")]),
    )]);
    let ctx = contexte(&racine);
    let fin = maintenant_naif();
    let releve = relever_retours(&ctx, &HashMap::new(), fin - 86_400, fin, false, "production", Some(client(&ctx, &base)), "").unwrap();
    let dit: Vec<&String> = releve.a_faire.iter().filter(|a| a.contains("B1-208-10")).collect();
    assert_eq!(dit.len(), 1, "{:?}", releve.a_faire);
    assert!(dit[0].contains("FLUX-REFUSE") && dit[0].contains("ordre 1"), "{}", dit[0]);
    assert!(dit[0].contains("annule-et-remplace") && dit[0].contains("p. 14"), "{}", dit[0]);
    // Une mensuelle refusee B1-208-10 n'est pas un signalement : la phrase ne la vise pas.
    let racine = compagnie("b1-208-10-mensuelle");
    let (base, _) = plateforme(vec![(
        "FLUX-MENSUELLE",
        bilan("FLUX-MENSUELLE", "01", SIREN, "01", "1", &[("B1-208-10", "Il existe deja une declaration normale.")]),
    )]);
    let ctx = contexte(&racine);
    let releve = relever_retours(&ctx, &HashMap::new(), fin - 86_400, fin, false, "production", Some(client(&ctx, &base)), "").unwrap();
    assert!(releve.a_faire.iter().all(|a| !a.contains("B1-208-10")), "{:?}", releve.a_faire);
}
