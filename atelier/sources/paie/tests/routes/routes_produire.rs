//! La route `produire` : un mois pret s'ecrit (bulletins numerotes a la suite, DSN constituee le jour meme, contact du
//! declarant, emplois multiples), un mois pas pret, hors delai ou clos refuse sans rien ecrire, un mois depose repart en
//! annule-et-remplace, un numero de contrat se lit et ne s'ecrase pas. Sur `fictive-3090-grande` : config, journal, toutes
//! ses saisies, ses DSN deposees, et les sorties entieres des seuls mois que le test relit.

use crate::outils::{compagnie, copier, fictive, dossier_de_test, ecrire, lire, noms};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn compagnie(nom: &str, sorties: &[&str]) -> PathBuf {
    let racine = compagnie::config_et_mois("fictive-3090-grande", sorties, &dossier_de_test("routes_produire", nom).join("compagnie"));
    copier(&fictive("fictive-3090-grande").join("engagements"), &racine.join("engagements"));
    // Les DSN deposees, sans les PDF : la production relit l'identite declaree.
    let sorties = fictive("fictive-3090-grande").join("sorties");
    for m in noms(&sorties) {
        for f in noms(&sorties.join(&m)).into_iter().filter(|f| f.ends_with(".dsn")) {
            std::fs::create_dir_all(racine.join("sorties").join(&m)).unwrap();
            std::fs::copy(sorties.join(&m).join(&f), racine.join("sorties").join(&m).join(&f)).unwrap();
        }
    }
    racine
}

fn route(racine: &Path, jour: &str, charge: Value) -> Value {
    route_nommee(racine, jour, "produire", charge)
}

fn route_nommee(racine: &Path, jour: &str, nom: &str, charge: Value) -> Value {
    crate::outils::route(racine, nom, charge, jour)
}

fn charge(racine: &Path, mois: &str) -> Value {
    json!({"mois": mois, "saisie": crate::outils::saisie(racine, &mois)})
}

/// Le mois est pret : pour chaque intermittent qui commence ce mois-la, sa DPAE est partie et son contrat est
/// revenu signe (octobre 2026 : quatre embauches le 6).
fn mois_pret(racine: &Path, mois: &str) {
    let saisie = crate::outils::saisie(racine, &mois);
    let mut j = lire(racine, "journal/depots.json");
    let l = j.as_array_mut().unwrap();
    let modele = l.iter().find(|x| x["quoi"] == json!("dpae") && x["mode"] == json!("reel")).unwrap().clone();
    let contrat = l.iter().find(|x| x["quoi"] == json!("contrat-recu")).unwrap().clone();
    let gens: Vec<(String, String)> = saisie["engagements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e.get("spectacle").is_some() && e["debut"].as_str().unwrap().starts_with(mois))
        .map(|e| (e["salarie"].as_str().unwrap().to_uppercase(), e["debut"].as_str().unwrap().to_string()))
        .collect();
    let mut d = modele;
    d["mois"] = json!(mois);
    d["detail"]["embauches"] = json!(gens.iter().map(|(g, deb)| json!({"salarie": g, "debut": deb, "fin": deb})).collect::<Vec<_>>());
    l.push(d);
    for (g, deb) in &gens {
        let mut c = contrat.clone();
        c["mois"] = json!(mois);
        c["detail"]["salarie"] = json!(g);
        c["detail"]["debut"] = json!(deb);
        l.push(c);
    }
    ecrire(racine, "journal/depots.json", &j);
}

#[test]
fn un_mois_deja_depose_repart_en_annule_et_remplace() {
    let r = compagnie("ar", &["2026-03"]);
    let avant = lire(&r, "config/compteurs.json");
    let rep = route(&r, "2026-04-10", charge(&r, "2026-03"));
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let res = &rep["resultat"];
    assert_eq!(res["mois"], json!("2026-03"));
    let fichiers: Vec<String> = res["fichiers"].as_array().unwrap().iter().map(|f| f["nom"].as_str().unwrap_or_default().to_string()).collect();
    assert!(fichiers.iter().any(|n| n == "DSN_Mars2026_AR2.dsn"), "{:?}", fichiers);
    assert!(res["informations"].as_array().unwrap().iter().any(|i| i.as_str().unwrap().contains("ANNULE-ET-REMPLACE")));
    // Retours Windows, F : constituee le jour ou ce second fichier est fait, pas a la fin de periode du mois.
    assert_eq!(constitution_de(&r, "sorties/2026-03/DSN_Mars2026_AR2.dsn"), "10042026");
    // les numeros sont figes dans les engagements, la DSN est notee
    assert_eq!(crate::outils::saisie(&r, "2026-03")["numeros_figes"], json!(true));
    assert!(lire(&r, "journal/dsn-ecrites.json").get("2026-03/DSN_Mars2026_AR2.dsn").is_some());
    assert_ne!(lire(&r, "config/compteurs.json")["2026"]["bulletin"], avant["2026"]["bulletin"]);
    // CE QUI SE DEPOSE ENSUITE, C'EST L'ANNULE-ET-REMPLACE (#253) : la DSN deja partie reste dans le dossier, mais la DSN
    // du mois -- celle que la boite « Deposer la DSN » ouvre et que l'etape DSN nomme -- est la nouvelle.
    let f = crate::outils::route(&r, "fichiers-deposables", json!({"mois": "2026-03"}), "2026-04-10");
    let noms: Vec<&str> = f["resultat"]["dsn"].as_array().unwrap().iter().map(|x| x["nom"].as_str().unwrap()).collect();
    assert_eq!(noms.first(), Some(&"DSN_Mars2026_AR2.dsn"), "{:?}", noms);
    assert!(noms.contains(&"DSN_Mars2026.dsn"), "{:?}", noms);
    // L'etat du mois, que lisent l'etape DSN et la boite « Deposer » (`fichiers.dsn[0]`), le dit de meme.
    let e = crate::outils::route(&r, "etat-du-mois", json!({"mois": "2026-03"}), "2026-04-10");
    assert_eq!(e["resultat"]["fichiers"]["dsn"][0]["nom"], json!("DSN_Mars2026_AR2.dsn"), "{}", e["resultat"]["fichiers"]);
    // Et la boite « Deposer la DSN » ouverte sur ce fichier montre un annule-et-remplace integral (type 03, cahier
    // technique DSN, S20.G00.05.002), d'ordre 2 : c'est lui qui se teste et part.
    let chemin = e["resultat"]["fichiers"]["dsn"][0]["chemin"].clone();
    let a = crate::outils::route(&r, "depot-apercu", json!({"quoi": "dsn", "fichier": chemin}), "2026-04-10");
    assert_eq!((&a["resultat"]["type_declaration"], &a["resultat"]["ordre"]), (&json!("03"), &json!("2")), "{}", a);
}

#[test]
fn un_mois_pas_pret_dit_ce_qui_manque_et_n_ecrit_rien() {
    let r = compagnie("manques", &[]);
    let compteurs = std::fs::read(r.join("config/compteurs.json")).unwrap();
    let rep = route(&r, "2026-11-10", charge(&r, "2026-10"));
    assert_eq!(rep["ok"], json!(false));
    assert_eq!(rep["classe"], json!("MoisPasPret"));
    let manques = rep["manques"].as_array().expect("les manques portent la donnee");
    assert_eq!(manques.len(), 8);
    assert!(manques.iter().any(|m| m["quoi"] == json!("dpae") && m["salarie"] == json!("CLAIRON")));
    assert!(manques.iter().any(|m| m["quoi"] == json!("contrat")));
    assert!(!r.join("sorties/2026-10").exists());
    assert_eq!(std::fs::read(r.join("config/compteurs.json")).unwrap(), compteurs);
}

#[test]
fn un_mois_hors_delai_est_refuse_avant_toute_ecriture() {
    let r = compagnie("delai", &["2026-08"]);
    let avant = noms(&r.join("sorties/2026-08"));
    let rep = route(&r, "2026-10-06", charge(&r, "2026-08"));
    assert_eq!(rep["ok"], json!(false));
    assert!(rep["erreur"].as_str().unwrap().contains("2026-08"), "{}", rep);
    assert_eq!(noms(&r.join("sorties/2026-08")), avant);
    // `forcer` ne passe pas un annule-et-remplace dont le delai est echu
    let mut c = charge(&r, "2026-08");
    c["forcer"] = json!(true);
    let rep = route(&r, "2026-10-06", c);
    assert_eq!(rep["ok"], json!(false));
    assert_eq!(noms(&r.join("sorties/2026-08")), avant);
}

#[test]
fn un_exercice_clos_ne_se_produit_pas() {
    let r = compagnie("clos", &[]);
    let rep = route(&r, "2025-07-10", charge(&r, "2025-06"));
    assert_eq!(rep["ok"], json!(false));
    assert!(rep["erreur"].as_str().unwrap().contains("cloture"), "{}", rep);
}

/// La date de constitution (S20.G00.05.007) d'une DSN ecrite, `JJMMAAAA`.
fn constitution_de(racine: &Path, rel: &str) -> String {
    let octets = std::fs::read(racine.join(rel)).unwrap();
    let texte = String::from_utf8_lossy(&octets).into_owned();
    let ligne = texte.lines().find(|l| l.starts_with("S20.G00.05.007,")).unwrap_or_else(|| panic!("pas de S20.G00.05.007 dans {}", rel));
    ligne.trim_end().trim_start_matches("S20.G00.05.007,").trim_matches('\'').to_string()
}

#[test]
fn un_envoi_de_test_produit_une_dsn_de_test() {
    let r = compagnie("test", &["2026-03"]);
    let mut c = charge(&r, "2026-03");
    c["test"] = json!(true);
    let rep = route(&r, "2026-04-10", c);
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    assert_eq!(rep["resultat"]["envoi_test"], json!(true));
    assert!(rep["resultat"]["avertissements"].as_array().unwrap().iter().any(|a| a.as_str().unwrap().contains("ENVOI DE TEST"))
        || rep["resultat"]["informations"].as_array().unwrap().iter().any(|a| a.as_str().unwrap().contains("ENVOI DE TEST")));
}

#[test]
fn un_mois_neuf_numerote_date_et_contacte_puis_le_suivant_continue_la_numerotation() {
    let r = compagnie("neuf", &[]);
    mois_pret(&r, "2026-10");
    let avant = lire(&r, "config/compteurs.json");
    let oct = route(&r, "2026-11-10", charge(&r, "2026-10"));
    assert_eq!(oct["ok"], json!(true), "{}", oct);
    let res = &oct["resultat"];
    assert_eq!(res["paies"].as_array().unwrap().len(), 6);
    // Premiere DSN du mois : normale, sans annule-et-remplace.
    let f = "sorties/2026-10/DSN_Octobre2026.dsn";
    assert!(r.join(f).is_file());
    assert!(!noms(&r.join("sorties/2026-10")).iter().any(|n| n.contains("_AR")));
    // Numerotation continue (Leandre, 06/10/2026) : le dernier bulletin emis etait le 56, les six du mois sont les
    // 57 a 62, sans trou.
    assert_eq!(avant["2026"]["bulletin"], json!(56));
    assert_eq!(res["numeros"]["bulletin_premier"], json!(57));
    assert_eq!(res["numeros"]["bulletin_dernier"], json!(62));
    assert_eq!(lire(&r, "config/compteurs.json")["2026"]["bulletin"], json!(62));
    let contrats: Vec<&str> = res["paies"].as_array().unwrap().iter().map(|p| p["numero_contrat"].as_str().unwrap()).collect();
    assert!(contrats.contains(&"BI4.26.08"), "{:?}", contrats);
    // Les numeros de contrat sont figes dans la saisie enregistree.
    let saisie = crate::outils::saisie(&r, "2026-10");
    assert_eq!(saisie["numeros_figes"], json!(true));
    assert!(saisie["engagements"].as_array().unwrap().iter().all(|e| e.get("numero").is_some()));
    // Retours Windows, F : constituee le jour ou elle est produite (le 10/11), pas a la fin de periode la plus tardive.
    assert_eq!(constitution_de(&r, f), "10112026");
    // CT DSN 2026.1, p. 132 : la copie figee des fictives dit « madame » de son contact, la DSN aussi.
    assert_eq!(rubrique_de(&r, f, "S10.G00.02.001"), "02");
    assert_eq!(rubrique_de(&r, f, "S10.G00.02.002"), "LECOUVREUR ADRIENNE");

    // Le mois suivant : les bulletins et les contrats d'une meme personne se suivent, sans trou ni double.
    mois_pret(&r, "2026-11");
    let nov = route(&r, "2026-12-10", charge(&r, "2026-11"));
    assert_eq!(nov["ok"], json!(true), "{}", nov);
    let (o, n) = (&oct["resultat"]["numeros"], &nov["resultat"]["numeros"]);
    assert_eq!(n["bulletin_premier"].as_i64().unwrap(), o["bulletin_dernier"].as_i64().unwrap() + 1);
    // les numeros de contrat d'une meme personne se suivent aussi : un par engagement neuf
    let mut vus: Vec<String> = Vec::new();
    for res in [&oct["resultat"], &nov["resultat"]] {
        for p in res["paies"].as_array().unwrap() {
            let num = p["numero_contrat"].as_str().unwrap().to_string();
            if !vus.contains(&num) {
                vus.push(num);
            }
        }
    }
    let mut par_personne: std::collections::HashMap<String, Vec<i64>> = Default::default();
    for num in &vus {
        let (tete, rang) = num.rsplit_once('.').unwrap();
        if tete.starts_with("BI4.26") || tete.starts_with("CL6.26") {
            par_personne.entry(tete.split('.').next().unwrap().to_string()).or_default().push(rang.parse().unwrap());
        }
    }
    for (_, rangs) in par_personne {
        let mut tries = rangs.clone();
        tries.sort();
        tries.dedup();
        assert_eq!(tries.len(), rangs.len(), "numero de contrat en double");
        assert!(tries.windows(2).all(|w| w[1] == w[0] + 1), "trou dans {:?}", tries);
    }
}

/// Octobre de la grande avec un JUMEAU : un second contrat de Biancolelli qui commence le meme jour que le premier
/// (le 6, un seul cachet), juste apres lui dans le mois, et chacun valide. Rend le rang du premier.
fn octobre_avec_un_jumeau(racine: &Path, numero_du_jumeau: Option<&str>) -> usize {
    let mut saisie = crate::outils::saisie(racine, "2026-10");
    let engs = saisie["engagements"].as_array_mut().unwrap();
    let i = engs.iter().position(|e| e["salarie"] == json!("Biancolelli")).unwrap();
    let mut jumeau = engs[i].clone();
    jumeau["fin"] = json!("2026-10-06");
    jumeau["date_paiement"] = json!("2026-10-06");
    jumeau["lignes"][0]["quantite"] = json!(1);
    jumeau["lignes"][0]["dates"] = json!(["2026-10-06"]);
    jumeau["lignes"][1]["quantite"] = json!(1);
    if let Some(n) = numero_du_jumeau {
        jumeau["numero"] = json!(n);
    }
    engs.insert(i + 1, jumeau);
    crate::outils::poser_saisie(racine, "2026-10", &saisie);
    crate::outils::tout_valider(racine, "2026-10", "2026-11-10");
    i
}

/// Les contrats signes des deux jumeaux, chacun sous SON numero (`mois_pret` les a poses sans) : le journal parle
/// d'un contrat par son numero.
fn signes_sous(racine: &Path, numeros: [&str; 2]) {
    let mut j = lire(racine, "journal/depots.json");
    let mut n = numeros.iter();
    for l in j.as_array_mut().unwrap() {
        if l["quoi"] == json!("contrat-recu") && l["mois"] == json!("2026-10") && l["detail"]["salarie"] == json!("BIANCOLELLI") {
            l["detail"]["numero"] = json!(n.next().unwrap());
        }
    }
    assert!(n.next().is_none(), "deux contrats signes pour les deux jumeaux");
    ecrire(racine, "journal/depots.json", &j);
}

/// Le numero que la production donnera au `k`-ieme contrat neuf de Biancolelli en 2026.
fn numero_neuf(racine: &Path, k: i64) -> String {
    let n = lire(racine, "config/compteurs.json")["2026"]["contrat_BIANCOLELLI"].as_i64().unwrap() + k;
    format!("BI4.26.{:02}", n)
}

#[test]
fn deux_contrats_du_meme_jour_se_figent_chacun_sous_son_numero() {
    // Deux contrats du meme salarie et du meme jour : chacun recoit le sien, et le garde.
    let r = compagnie("jumeaux", &[]);
    let i = octobre_avec_un_jumeau(&r, None);
    mois_pret(&r, "2026-10");
    signes_sous(&r, [&numero_neuf(&r, 1), &numero_neuf(&r, 2)]);
    let rep = route(&r, "2026-11-10", charge(&r, "2026-10"));
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let paies = rep["resultat"]["paies"].as_array().unwrap();
    let (a, b) = (paies[i]["numero_contrat"].as_str().unwrap(), paies[i + 1]["numero_contrat"].as_str().unwrap());
    assert_ne!(a, b, "deux contrats, deux numeros");
    // Le fichier dit ce qui a ete declare : chaque engagement porte le numero de SA paie.
    let saisie = crate::outils::saisie(&r, "2026-10");
    let engs = saisie["engagements"].as_array().unwrap();
    assert_eq!(engs.len(), paies.len());
    for (e, p) in engs.iter().zip(paies) {
        assert_eq!(e["numero"], p["numero_contrat"], "{}", saisie);
    }
}

#[test]
fn un_numero_deja_pose_et_different_n_est_jamais_ecrase() {
    // Le jumeau porte deja son numero (parti a la signature) ; la saisie de l'ecran ne le porte pas. Le numero SE LIT
    // (regle 4) : le jumeau part sous BI4.26.99, et l'engagement le garde.
    let r = compagnie("jumeaux-conflit", &[]);
    let i = octobre_avec_un_jumeau(&r, Some("BI4.26.99"));
    mois_pret(&r, "2026-10");
    signes_sous(&r, [&numero_neuf(&r, 1), "BI4.26.99"]);
    let premier = numero_neuf(&r, 1);
    let mut c = charge(&r, "2026-10");
    c["saisie"]["engagements"][i + 1].as_object_mut().unwrap().remove("numero");
    let rep = route(&r, "2026-11-10", c);
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let paies = rep["resultat"]["paies"].as_array().unwrap();
    assert_eq!(paies[i]["numero_contrat"], json!(premier));
    assert_eq!(paies[i + 1]["numero_contrat"], json!("BI4.26.99"));
    let saisie = crate::outils::saisie(&r, "2026-10");
    assert_eq!(saisie["engagements"][i + 1]["numero"], json!("BI4.26.99"), "{}", saisie);
    assert_eq!(saisie["engagements"][i]["numero"], json!(premier), "{}", saisie);
}

#[test]
fn la_dsn_produite_declare_les_engagements_enregistres_meme_quand_l_ecran_les_range_autrement() {
    // Ce qui part est ce que les engagements valides du mois disent, dans leur ordre : un ecran qui les range autrement (le
    // regroupement de la page) ne change ni la DSN ni son empreinte, et le depot n'est pas refuse (« la paie a change »).
    let r = compagnie("ordre-ecran", &[]);
    mois_pret(&r, "2026-10");
    let mut c = charge(&r, "2026-10");
    let engs = c["saisie"]["engagements"].as_array_mut().unwrap();
    engs.reverse();
    let ordre_ecran: Vec<Value> = engs.iter().map(|e| e["salarie"].clone()).collect();
    let ordre_fichier: Vec<Value> = crate::outils::saisie(&r, "2026-10")["engagements"].as_array().unwrap().iter().map(|e| e["salarie"].clone()).collect();
    assert_ne!(ordre_ecran, ordre_fichier);
    let rep = route(&r, "2026-11-10", c);
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    // Le fichier enregistre est celui que la DSN declare : rien n'a change depuis qu'elle est ecrite.
    let b = route_nommee(&r, "2026-11-10", "bilan-du-mois", json!({"mois": "2026-10"}));
    assert_eq!(b["ok"], json!(true), "{}", b);
    assert_eq!(b["resultat"]["dsn"]["etape"], json!("ecrite"), "{}", b["resultat"]["dsn"]);
    // Ce qui est declare est dans l'ordre des engagements, et chacun porte le numero de sa paie.
    let paies = rep["resultat"]["paies"].as_array().unwrap();
    let saisie = crate::outils::saisie(&r, "2026-10");
    assert_eq!(saisie["numeros_figes"], json!(true));
    let engs = saisie["engagements"].as_array().unwrap();
    assert_eq!(engs.iter().map(|e| e["salarie"].clone()).collect::<Vec<_>>(), ordre_fichier);
    assert_eq!(engs.len(), paies.len());
    for (e, p) in engs.iter().zip(paies) {
        assert_eq!(e["numero"], p["numero_contrat"], "{}", saisie);
    }
}

// ------------------------------------------------------------------ le contact emetteur (V1h, point 2)

fn rubrique_de(racine: &Path, rel: &str, code: &str) -> String {
    let texte: String = std::fs::read(racine.join(rel)).unwrap().iter().map(|&o| o as char).collect();
    let ligne = texte.lines().find(|l| l.starts_with(&format!("{},", code))).unwrap_or_else(|| panic!("pas de {} dans {}", code, rel));
    ligne.trim_end()[code.len() + 1..].trim_matches('\'').to_string()
}

/// La personne que les organismes contactent n'est plus celle qui signe : l'administrateur fait la paie, la directrice signe.
fn contact_administrateur(racine: &Path, civilite: Option<&str>) {
    let mut e = lire(racine, "config/entreprise.json");
    let mut d = json!({"nom_prenom": "MOLIERE JEAN", "tel": "0100000099", "email": "ADMINISTRATION@DEMO.INVALID", "qualite": "04"});
    if let Some(c) = civilite {
        d["civilite"] = json!(c);
    }
    e["declarant_dsn"] = d;
    ecrire(racine, "config/entreprise.json", &e);
}

#[test]
fn le_contact_de_la_dsn_est_une_seule_personne_avec_sa_civilite() {
    // Cahier technique DSN 2026.1, p. 132 : S10.G00.02 est l'« Acteur a contacter », 02.001 « 01 - monsieur / 02 - madame »,
    // 02.002 « Nom, prenom de l'agent de l'emetteur ». Le moteur ecrivait 01 en dur et le nom du signataire (ici la
    // directrice, LECOUVREUR ADRIENNE) a cote du courriel et du telephone du declarant.
    let r = compagnie("contact", &[]);
    mois_pret(&r, "2026-10");
    contact_administrateur(&r, Some("01"));
    let rep = route(&r, "2026-11-10", charge(&r, "2026-10"));
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let f = "sorties/2026-10/DSN_Octobre2026.dsn";
    assert_eq!(
        ["S10.G00.02.001", "S10.G00.02.002", "S10.G00.02.004", "S10.G00.02.005", "S20.G00.07.001"].map(|c| rubrique_de(&r, f, c)),
        ["01", "MOLIERE JEAN", "ADMINISTRATION@DEMO.INVALID", "0100000099", "MOLIERE JEAN"].map(String::from)
    );
}

#[test]
fn sans_civilite_du_contact_la_dsn_refuse_et_dit_ou_la_saisir() {
    let r = compagnie("contact-sans-civilite", &[]);
    mois_pret(&r, "2026-10");
    contact_administrateur(&r, None);
    let rep = route(&r, "2026-11-10", charge(&r, "2026-10"));
    assert_eq!(rep["ok"], json!(false), "{}", rep);
    let m = rep["erreur"].as_str().unwrap_or_default();
    assert!(m.contains("civilité") && m.contains("Qui les organismes contactent") && m.contains("S10.G00.02.001"), "{}", m);
    assert!(!r.join("sorties/2026-10/DSN_Octobre2026.dsn").exists());
}

// ------------------------------------------------------------------ les emplois multiples (V1h, point 7)

/// `S21.G00.40.036` de chaque contrat, par nom de salarie (`S21.G00.30.002`), dans l'ordre du fichier.
fn emplois_multiples_par_salarie(racine: &Path, rel: &str) -> Vec<(String, String)> {
    let texte: String = std::fs::read(racine.join(rel)).unwrap().iter().map(|&o| o as char).collect();
    let mut nom = String::new();
    let mut v = Vec::new();
    for l in texte.lines() {
        let valeur = || l.split_once(',').unwrap().1.trim_end().trim_matches('\'').to_string();
        if l.starts_with("S21.G00.30.002,") {
            nom = valeur();
        } else if l.starts_with("S21.G00.40.036,") {
            v.push((nom.clone(), valeur()));
        }
    }
    v
}

#[test]
fn un_salarie_qui_tient_deux_emplois_dans_le_mois_est_en_emplois_multiples() {
    // Cahier technique DSN 2026.1, p. 199 : « Cette rubrique précise si le salarié a plusieurs emplois chez un même
    // employeur au cours d'une même période de paie » (02 - emplois multiples). Le moteur ecrivait 01 en constante.
    // Biancolelli joue (son emploi de comedienne) puis assiste la mise en scene (MET042) en octobre : 02 sur ses deux
    // contrats. Clairon a deux contrats du meme emploi : 01. Les autres, un contrat : 01.
    // à trancher, §226 : deux contrats du meme emploi en 01 est une inference ; la norme ne tranche pas ce cas.
    let r = compagnie("emplois-multiples", &[]);
    let mut s = crate::outils::saisie(&r, "2026-10");
    let engagements = s["engagements"].as_array_mut().unwrap();
    for (qui, metier) in [("Biancolelli", Some("MET042")), ("Clairon", None)] {
        let mut second = engagements.iter().find(|e| e["salarie"] == json!(qui)).unwrap().clone();
        second["debut"] = json!("2026-10-20");
        second["fin"] = json!("2026-10-21");
        second["date_paiement"] = json!("2026-10-21");
        second["lignes"] = json!([{"rubrique": "cachet", "quantite": 2, "taux": 190.0, "lieu": "Theatre fictif de Saumur", "dates": ["2026-10-20", "2026-10-21"]}]);
        if let Some(m) = metier {
            second["metier"] = json!(m);
        }
        engagements.push(second);
    }
    crate::outils::poser_saisie(&r, "2026-10", &s);
    crate::outils::tout_valider(&r, "2026-10", "2026-11-10");
    mois_pret(&r, "2026-10");
    let rep = route(&r, "2026-11-10", charge(&r, "2026-10"));
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let vu = emplois_multiples_par_salarie(&r, "sorties/2026-10/DSN_Octobre2026.dsn");
    let de = |qui: &str| -> Vec<String> { vu.iter().filter(|(n, _)| n == qui).map(|(_, c)| c.clone()).collect() };
    assert_eq!(de("BIANCOLELLI"), ["02", "02"], "{:?}", vu);
    assert_eq!(de("CLAIRON"), ["01", "01"], "{:?}", vu);
    assert!(vu.iter().filter(|(n, _)| n != "BIANCOLELLI").all(|(_, c)| c == "01"), "{:?}", vu);
}
