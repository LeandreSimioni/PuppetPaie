//! LES OS, GARDES QUOI QU'ON CHANGE DESSUS : chaque route dit en une phrase breve ce qu'elle fait, et l'exemple de sa
//! fiche est accepte ; la page et la ligne de commande ont le meme catalogue ; une crate ne prend qu'a celles du
//! dessous ; l'engagement ne lit pas le journal ; une seule fonction date une valeur de fiche ; un seul endroit
//! ecrit un fichier d'engagements.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use moteur::routes::catalogue::{catalogue, Acte, Ecran, Place, FICHES, GESTE_MAX};
use serde_json::{json, Value};

use crate::bulletins_fictifs::{a_heures, a_repetitions, page_a, pdf};
use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dossier_de_test, route, route_avec};
use moteur::compagnie::administrateur::Poste;
use moteur::interface::compagnies::ajouter_dans;
use moteur::compagnie::demande::VARIABLE_ESSAIS;

fn espace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

/// Les fichiers `.rs` sous `d`.
fn sources(d: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(d).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            v.extend(sources(&p));
        } else if p.extension().is_some_and(|x| x == "rs") {
            v.push(p);
        }
    }
    v
}

#[test]
fn chaque_route_dit_brievement_ce_qu_elle_fait() {
    let tout = catalogue();
    let mut vus = BTreeSet::new();
    for f in &tout {
        assert!(vus.insert(f.nom), "{} est deux fois au catalogue", f.nom);
        assert!(!f.geste.is_empty() && f.geste.chars().count() <= GESTE_MAX, "{} : le geste, en {} caracteres au plus : {}", f.nom, GESTE_MAX, f.geste);
        assert!(!f.rend.is_empty() && !f.refus.is_empty(), "{} : sa fiche dit ce qu'elle rend et ses refus", f.nom);
        assert!(serde_json::from_str::<Value>(f.exemple).is_ok_and(|v| v.is_object()), "{} : l'exemple n'est pas un objet JSON", f.nom);
    }
}

/// Les Treteaux du Banc au 02/11, avant son premier jour joue : une compagnie configuree, FULLER en saisie.
fn treteaux(test: &str) -> PathBuf {
    let d = dossier_de_test("structure", test);
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let debut: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"geste\":\"jour\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), debut.join("\n") + "\n").unwrap();
    rejouer_dossier(&sc, &d).1
}

/// Les exemples des fiches, joues dans l'ordre du catalogue sur une compagnie de test (Les Treteaux du Banc, au 02/11) :
/// chacun est accepte, et la phrase rendue tient dans sa place.
#[test]
fn l_exemple_de_chaque_fiche_est_accepte() {
    let c = treteaux("exemples");
    // Le serveur du filet et du développeur : un faux, sur cette machine ; il donne une identité de poste et accepte tout.
    let (filet, _) = crate::outils::serveur(|_| {
        Some(crate::outils::http_json(200, &json!({"poste": "0".repeat(32), "cle": "0".repeat(64), "confirme": false, "numero": 1})))
    });
    std::fs::write(c.join("config/diagnostic.json"), json!({"serveur": filet}).to_string()).unwrap();
    // Les bulletins d'un ancien logiciel, dans la boîte de l'import : un artiste (janvier 2026, sans la CASC-SVP) et un
    // permanent (mai 2025) dont aucune pièce ne dit la nature du contrat.
    let (mut artiste, mut permanent) = (a_repetitions(), a_heures());
    for a in [&mut artiste, &mut permanent] {
        a.id.siret = "32520440200011";
        a.id.entreprise[0] = "Les Tréteaux du Banc";
    }
    permanent.id.nom = "TARTEMPION";
    permanent.id.nir = "185079999912388";
    permanent.id.conges_spectacles = None;
    permanent.cotisations.retain(|c| c.2 != "conges_spectacles");
    std::fs::create_dir_all(c.join("import")).unwrap();
    pdf(vec![page_a(&artiste)], &c.join("import/artiste.pdf"));
    pdf(vec![page_a(&permanent)], &c.join("import/permanent.pdf"));
    // Un permanent en CDI, arrete deux fois (un accident du travail, un deuil d'enfant) : ce que seul un contrat paye au mois
    // appelle (le maintien de salaire), et ce que ces motifs font declarer hors DSN.
    let svoboda = json!({"nom": "salarie", "cle": null, "donnees": {"prenom_affichage": "Josef",
        "nom_affichage": "Svoboda", "nir": "1750544999001", "nir_cle": "19", "date_naissance": "1975-05-10", "lieu_naissance_affichage": "Villeneuve-sur-Loire",
        "departement_naissance": "44", "pays_naissance": "FR", "code_nationalite_dsn": "01", "adresse_affichage": "4 rue Kervégan", "code_postal": "44000",
        "commune_affichage": "Nantes", "email": "josef.svoboda@exemple.invalid", "emploi_affichage": "Directeur technique", "code_emploi_ft": "DIR210",
        "pcs_ese": "353C", "statut": "technicien", "statut_conventionnel": "04", "statut_categoriel_rc": "01", "date_anciennete": "2026-11-02", "sante_svp": {"niveau": "1"}, "actif": true}});
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let fiche_entreprise = gestes.lines().filter(|l| l.contains("\"route\":\"enregistrer-fiche\"") && l.contains("\"nom\":\"entreprise\"")).last().unwrap();
    let mut entreprise = serde_json::from_str::<Value>(fiche_entreprise).unwrap()["charge"].clone();
    entreprise["donnees"]["emploie_des_permanents"] = json!(true);
    entreprise["depuis"] = json!("2026-11-01");
    entreprise["confirme"] = json!(true);
    for (nom, charge) in [
        ("enregistrer-fiche", entreprise),
        ("organismes-enregistrer", json!({"contrats": {"prevoyance_permanents": {"libelle": "PREVOYANCE", "organisme": "P0983"}, "sante_permanents": {"libelle": "SANTE", "organisme": "P0983"}}})),
        ("enregistrer-fiche", svoboda),
        ("valider-engagement", json!({"mois": "2026-11", "saisie": {"salarie": "SVOBODA", "nature": "cdi", "debut": "2026-11-02", "salaire_mensuel": 3400, "quotite_contrat": 151.67}})),
        ("arret-enregistrer", json!({"salarie": "SVOBODA", "dernier_jour": "2026-11-02", "fin": "2026-11-10", "motif": "04", "date_accident": "2026-11-02", "subrogation": "02"})),
        ("arret-enregistrer", json!({"salarie": "SVOBODA", "dernier_jour": "2026-11-16", "fin": "2026-11-18", "motif": "19", "subrogation": "02"})),
    ] {
        let r = route(&c, nom, charge, "2026-11-02");
        assert_eq!(r["ok"], json!(true), "le permanent des exemples : {} : {}", nom, r);
    }
    // Le retrait de l'engagement en dernier : les exemples des imprevus portent sur celui que valider-engagement pose.
    let mut fiches = catalogue();
    fiches.sort_by_key(|f| f.nom == "retirer-engagement");
    // Un administrateur, pour son tableau et son dépôt groupé : la compagnie est rattachée à son registre, après les
    // exemples du mot de passe du logiciel (celui d'une compagnie seule).
    let inst = c.parent().unwrap().parent().unwrap();
    let registre = json!({"compagnies": [{"dossier": c.strip_prefix(inst).unwrap().to_string_lossy()}]});
    for f in &fiches {
        if (f.nom == "administrateur-tableau" || f.nom == "administrateur-deposer") && !inst.join("administrateur.json").exists() {
            std::fs::write(inst.join("administrateur.json"), registre.to_string()).unwrap();
        }
        // Un secret ne se lit jamais dans la charge : la fausse demande des essais le tape (`demande::canal_de_la_route`).
        let exemple = serde_json::from_str(f.exemple).unwrap();
        let r = match f.nom {
            "google-autoriser" => google_autoriser(&c, exemple),
            // La suppression efface une compagnie ajoutee a part, pas celle des exemples.
            "compagnie-supprimer" => {
                let autre = c.parent().unwrap().join("ajoutee-a-supprimer");
                crate::outils::copier(&c.join("config"), &autre.join("config"));
                std::fs::write(autre.join("COPIE-DE-DEVELOPPEMENT.json"), json!({"copie_de": "compagnie fictive ajoutee", "ajoutee": true}).to_string()).unwrap();
                let r = route(&autre, f.nom, exemple, "2026-11-02");
                assert!(!autre.exists(), "compagnie-supprimer : la compagnie est encore la");
                r
            }
            // Ce qui ne porte que sur une compagnie reelle, une fictive livree ou une compagnie creee en demo se joue dans
            // une installation a part.
            "compagnie-couleur" | "compagnie-remettre-a-zero" | "compagnies-garder" => route(&installation_a_part(&c, f.nom).1, f.nom, exemple, "2026-11-02"),
            "administrateur-rattacher" | "administrateur-detacher" => administrateur(&c, f.nom, exemple),
            _ => route_avec(&c, f.nom, exemple, "2026-11-02", &[(VARIABLE_ESSAIS, "exemple")]),
        };
        assert_eq!(r["ok"], json!(true), "{} refuse l'exemple de sa fiche : {}", f.nom, r);
        if f.phrase > 0 {
            let p = r["resultat"]["phrase"].as_str().unwrap_or_else(|| panic!("{} : la phrase manque : {}", f.nom, r));
            assert!(p.chars().count() <= f.phrase, "{} : la phrase passe sa place ({}) : {}", f.nom, f.phrase, p);
        }
    }
}


/// L'exemple de `google-autoriser`, sur un faux Google local (rien ne sort de la machine) : le test fait le navigateur
/// (il lit l'adresse de la fenetre de Google que le moteur dit sur sa sortie d'erreur, et renvoie un code a l'adresse
/// locale qu'elle nomme) ; le faux Google rend un jeton et l'adresse du compte.
fn google_autoriser(c: &Path, exemple: Value) -> Value {
    use std::io::{BufRead, Read, Write};
    use std::process::{Command, Stdio};
    let (google, _) = crate::outils::serveur(|r| {
        Some(match r.chemin.as_str() {
            "/token" => crate::outils::http_json(200, &json!({"refresh_token": "essai", "access_token": "essai", "scope": ""})),
            "/userinfo" => crate::outils::http_json(200, &json!({"email": "sauvegarde@exemple.invalid"})),
            _ => crate::outils::http(404, "text/plain", b""),
        })
    });
    // L'application du logiciel n'est livree qu'avec une version publiee : celle de la compagnie, inventee.
    std::fs::write(c.join("config/google.json"), json!({"client_id": "essai.apps.googleusercontent.com", "client_secret": "essai"}).to_string()).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_moteur"));
    cmd.args(["route", "--racine", c.to_str().unwrap(), "--date", "2026-11-02"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    crate::outils::bac_a_sable(&mut cmd, c, "2026-11-02");
    cmd.env(moteur::routes::coffre::VARIABLE_GOOGLE_ESSAIS, &google).env(VARIABLE_ESSAIS, "exemple");
    let mut p = cmd.spawn().unwrap();
    p.stdin.take().unwrap().write_all(json!({"route": "google-autoriser", "charge": exemple}).to_string().as_bytes()).unwrap();
    let (envoi, recu) = std::sync::mpsc::channel();
    let erreur = p.stderr.take().unwrap();
    std::thread::spawn(move || {
        for l in std::io::BufReader::new(erreur).lines().map_while(Result::ok) {
            let _ = envoi.send(l);
        }
    });
    let adresse = recu.iter().find_map(|l| l.split_once(": https://").map(|(_, u)| u.to_string())).expect("l'adresse de la fenetre de Google");
    let valeur = |cle: &str| adresse.split(['?', '&']).find_map(|kv| kv.strip_prefix(&format!("{}=", cle))).unwrap().to_string();
    let retour = valeur("redirect_uri").replace("%3A", ":").replace("%2F", "/");
    let hote = retour.trim_start_matches("http://").trim_end_matches('/');
    let mut navigateur = std::net::TcpStream::connect(hote).unwrap();
    // Une seule ecriture : le moteur repond et ferme des la premiere ligne lue.
    let requete = format!("GET /?code=essai&state={} HTTP/1.1\r\nHost: {}\r\n\r\n", valeur("state"), hote);
    let _ = navigateur.write_all(requete.as_bytes());
    let _ = navigateur.read_to_end(&mut Vec::new());
    let sortie = p.wait_with_output().unwrap().stdout;
    serde_json::from_slice(&sortie).unwrap()
}

/// Une installation debloquee a part, montee pour un exemple : une compagnie creee en demo (`fictive:ajoutee-demo`), une
/// fictive livree (`fictive:fictive-essai`), une compagnie reelle (`cie:la-compagnie`). Rend l'installation et la reelle.
fn installation_a_part(c: &Path, test: &str) -> (PathBuf, PathBuf) {
    let inst = crate::outils::installation(&dossier_de_test("structure", test), "2026.10.22");
    ajouter_dans(&inst, "Demo").unwrap();
    crate::outils::debloquer(&inst);
    let reelle = ajouter_dans(&inst, "La Compagnie").unwrap().racine;
    let livree = inst.join("fictives").join("fictive-essai");
    crate::outils::copier(&c.join("config"), &livree.join("config"));
    std::fs::write(livree.join("COPIE-DE-DEVELOPPEMENT.json"), json!({"copie_de": "compagnie fictive essai"}).to_string()).unwrap();
    (inst, reelle)
}

/// Les exemples de `administrateur-rattacher` et `administrateur-detacher`, dans une installation a part : la compagnie
/// reelle porte le mot de passe de l'administrateur, une seconde se rattache (puis se detache). Une compagnie reelle n'a
/// que le canal du poste (`demande::canal_de_la_route`) : les mots de passe que l'humain taperait sont ceux de la charge,
/// comme la page les donne, sur le coffre d'un poste de test.
fn administrateur(c: &Path, nom: &str, exemple: Value) -> Value {
    const ADMIN: &str = "administrateur-essai-1";
    let (inst, a) = installation_a_part(c, nom);
    let poste = Poste { dossier_cle: inst.join("cle-du-poste"), maintenant: "2026-11-02T10:00".into() };
    poste.coffre(&a).poser_ouverture(ADMIN, None).unwrap();
    let b = ajouter_dans(&inst, "Seconde").unwrap().racine;
    let mut charge = exemple;
    charge["administrateur"] = json!(ADMIN);
    if nom == "administrateur-detacher" {
        moteur::routes::coffre::administrateur(&b, "administrateur-rattacher", &json!({"administrateur": ADMIN}), &poste).unwrap();
        charge["nouveau"] = json!("propre-essai-1");
        charge["confirmation"] = json!("propre-essai-1");
    }
    match moteur::routes::coffre::administrateur(&b, nom, &charge, &poste) {
        Ok(v) => json!({"ok": true, "resultat": v}),
        Err(e) => json!({"ok": false, "erreur": e.message}),
    }
}

/// LES GESTES DE LA FENETRE ELLE-MEME (`window.pywebview.api.<nom>`, servis par `Fenetre::lanceur` : `NOMS_ACCUEIL` et
/// `NOMS_LOGICIEL` de `interface/mod.rs`) : aucun n'est un geste du logiciel, chacun avec ce qu'il fait. Fixe : un geste
/// de plus dans la fenetre fait echouer le test ; s'il est du logiciel, c'est une route.
const FENETRE: [(&str, &str); 11] = [
    ("choisir", "montrer la compagnie d'une tuile ; en ligne de commande, --racine"),
    ("verifier", "la deverrouiller dans cette fenetre ; en ligne de commande, la seance du coffre"),
    ("accueil", "revenir aux tuiles"),
    ("fermer", "fermer la fenetre"),
    ("redemarrer", "rouvrir la fenetre sur la version prete"),
    ("maj", "ou en est la recherche que la fenetre mene en fond (la version : maj-etat)"),
    ("ouvrir", "ouvrir dans le poste le chemin qu'une route a rendu"),
    ("choisir_dossier", "la boite du systeme qui choisit un dossier"),
    ("veille", "l'etat de la veille de la fenetre"),
    ("veille_etape", "l'etape de son passage en cours"),
    ("veille_maintenant", "un passage tout de suite : il appelle la route ouverture"),
];
/// Le seul geste qui n'existe qu'a l'ecran : brider ou debloquer le logiciel (la demo) ; la ligne de commande n'a pas de
/// bridage.
const ECRAN_SEUL: &str = "basculer";

/// Les fichiers de la page qui servent chaque place (depuis la racine du depot). L'accueil des compagnies, ses pages et
/// celles du mot de passe sont servies par la fenetre (`interface/pages/`).
fn fichiers_de(p: Place) -> &'static [&'static str] {
    match p {
        Place::Paie => &[
            "logiciel/interface/js/etats-engagements.js", "logiciel/interface/js/saisie-mois.js", "logiciel/interface/js/saisie-lignes.js",
            "logiciel/interface/js/actes.js", "logiciel/interface/js/imprevus.js", "logiciel/interface/js/rappels.js",
            "logiciel/interface/js/conseils.js", "logiciel/interface/js/fictives.js", "logiciel/interface/js/boites.js",
        ],
        Place::Fiches => &["logiciel/interface/js/configuration.js", "logiciel/interface/js/fiche.js", "logiciel/interface/js/conseils.js"],
        Place::Metiers | Place::Organismes | Place::Import => &["logiciel/interface/js/configuration.js"],
        Place::Bulletin => &["logiciel/interface/js/modeles-bulletin.js"],
        Place::ModelesContrat => &["logiciel/interface/js/modeles-contrat.js"],
        Place::Identite => &["logiciel/interface/js/identite.js"],
        Place::Coffre => &["logiciel/interface/js/coffre-compte.js", "atelier/sources/moteur/src/interface/pages/choix_mot_de_passe.html"],
        Place::Historique => &[
            "logiciel/interface/js/historique.js", "logiciel/interface/js/rappels.js", "logiciel/interface/js/exports.js",
            "logiciel/interface/js/corrections.js", "logiciel/interface/js/reprise-journal.js",
        ],
        Place::Exports => &["logiciel/interface/js/exports.js"],
        Place::Signaler => &["logiciel/interface/js/filet.js", "logiciel/interface/js/tuyau.js"],
        Place::Bandeau => &["logiciel/interface/js/demarrage.js", "logiciel/interface/js/veille.js", "logiciel/interface/js/mise-a-jour.js"],
        Place::Accueil => &[
            "atelier/sources/moteur/src/interface/pages/accueil.html", "atelier/sources/moteur/src/interface/pages/garder.html",
            "atelier/sources/moteur/src/interface/pages/choix_mot_de_passe.html",
        ],
    }
}

/// Ce que la page demande au moteur : `api("…"`, `appel("…"`, `appelerMoteur("…"`, `fetch("/api/…`.
fn appels_de_la_page(t: &str) -> Vec<String> {
    let mut v = Vec::new();
    for debut in ["api(\"", "appel(\"", "appelerMoteur(\"", "fetch(\"/api/"] {
        for morceau in t.split(debut).skip(1) {
            // Une route qui se compose (`"/api/" + route`) est celle d'un `api(...)` : elle n'a pas de nom ici.
            let nom = morceau.split(['"', '?']).next().unwrap();
            if !nom.is_empty() {
                v.push(nom.to_string());
            }
        }
    }
    v
}

/// L'ECRAN ET LE MOTEUR SONT EQUIVALENTS, sans exception : la page (`js/`, et les pages que sert la fenetre : l'accueil,
/// le mot de passe) ne parle au moteur que par une route du catalogue, ou par un geste de la fenetre elle-meme (`FENETRE`,
/// et `basculer`, le seul qui n'existe qu'a l'ecran) ; chaque route du catalogue a sa place (une place de la liste du
/// moteur, qui existe dans la page) et la page de sa place l'appelle, ou l'affiche sous « Avancé » depuis sa fiche ; une
/// lecture des ecrans est lue par la page ou par la fenetre ; les routes qui ecrivent d'un module ont une seule place ; et
/// chaque route se sert par `moteur route`.
#[test]
fn la_page_et_la_ligne_de_commande_ont_le_meme_catalogue() {
    let depot = espace().join("../..");
    let lu = |f: &str| std::fs::read_to_string(depot.join(f)).unwrap();
    let tout = catalogue();
    let noms: BTreeSet<&str> = tout.iter().map(|f| f.nom).collect();
    let mut page = String::new();
    let mut fichiers: Vec<PathBuf> = std::fs::read_dir(depot.join("logiciel/interface/js")).unwrap().flatten().map(|e| e.path()).collect();
    fichiers.extend(std::fs::read_dir(espace().join("moteur/src/interface/pages")).unwrap().flatten().map(|e| e.path()));
    let fenetre_rs = lu("atelier/sources/moteur/src/interface/mod.rs");
    let gestes = |cle: &str| -> Vec<String> {
        let liste = fenetre_rs.split(&format!("const {}: &str = r#\"", cle)).nth(1).and_then(|s| s.split("\"#").next()).unwrap();
        serde_json::from_str(liste).unwrap()
    };
    let mut de_la_fenetre: BTreeSet<String> = gestes("NOMS_ACCUEIL").into_iter().chain(gestes("NOMS_LOGICIEL")).collect();
    assert!(de_la_fenetre.remove(ECRAN_SEUL), "{} : le geste qui débride est celui de l'accueil", ECRAN_SEUL);
    let attendus: BTreeSet<String> = FENETRE.iter().map(|(n, _)| n.to_string()).collect();
    assert_eq!(
        de_la_fenetre, attendus,
        "la fenêtre ne sert que ses propres gestes (FENETRE) : un geste du logiciel est une route du catalogue, que la page appelle par `api(…)` ou `appel(…)`"
    );
    for p in &fichiers {
        let t = std::fs::read_to_string(p).unwrap();
        let nom_du_fichier = p.file_name().unwrap().to_string_lossy();
        for nom in appels_de_la_page(&t) {
            assert!(noms.contains(nom.as_str()), "{} appelle {}, absente du catalogue : mets ce geste au catalogue", nom_du_fichier, nom);
        }
        for morceau in t.split("pywebview.api.").skip(1) {
            let geste: String = morceau.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
            assert!(
                geste == ECRAN_SEUL || attendus.contains(&geste),
                "{} appelle le geste {} de la fenêtre : un geste du logiciel est une route du catalogue (fiche, place), que la page appelle par api(…) ou appel(…)",
                nom_du_fichier, geste
            );
        }
        page.push_str(&t);
    }
    let html = (lu("logiciel/interface/index.html") + &lu("atelier/sources/moteur/src/interface/pages/accueil.html")).replace("&amp;", "&");
    for p in Place::TOUTES {
        assert!(html.contains(&format!("data-place=\"{}\"", p.nom())), "la place « {} » n'est pas dans la page : nomme-la (data-place) là où la page l'ouvre", p.nom());
    }
    let mut fenetre = lu("atelier/sources/moteur/src/etat_garde.rs");
    for p in sources(&espace().join("moteur/src/interface")) {
        fenetre.push_str(&std::fs::read_to_string(p).unwrap());
    }
    let appelee = |nom: &str, t: &str| t.contains(&format!("\"{}\"", nom)) || t.contains(&format!("/api/{}", nom));
    for f in &tout {
        match f.ecran {
            Ecran::Dessine(p) => assert!(
                fichiers_de(p).iter().any(|x| appelee(f.nom, &lu(x))),
                "{} : aucune page de sa place ({}) ne l'appelle : donne à cette route sa place à l'écran (fiche, champ ecran) et appelle-la depuis cette page ({})",
                f.nom, p.nom(), fichiers_de(p).join(", ")
            ),
            Ecran::Auto(p) => {
                let porte = format!("data-auto=\"{}\"", p.nom());
                let configuration = p.nom().starts_with("Configuration › ") && html.contains("id=\"config-avance\"");
                assert!(f.avance, "{} s'affiche sous « Avancé » de sa place : sa fiche est avancée", f.nom);
                assert!(
                    configuration || html.contains(&porte) || fichiers_de(p).iter().any(|x| lu(x).contains(&porte)),
                    "{} : sa place ({}) n'a pas son « Avancé » (data-auto) : appelle js/avance.js là où la page l'ouvre", f.nom, p.nom()
                );
            }
            Ecran::Lecture => {
                assert!(f.acte == Acte::Lit, "{} écrit : donne à cette route sa place à l'écran (fiche, champ ecran)", f.nom);
                assert!(appelee(f.nom, &page) || appelee(f.nom, &fenetre), "{} : ni la page ni la fenêtre ne la lisent : donne-lui sa place à l'écran, ou retire-la", f.nom);
            }
        }
    }
    for module in FICHES {
        let places: BTreeSet<&str> = module.iter().filter(|f| f.acte != Acte::Lit).filter_map(|f| f.ecran.place()).map(Place::nom).collect();
        assert!(places.len() <= 1, "un module, une place : les routes qui écrivent de {:?} vont à {:?} ; range chacune dans le module de sa place", module.iter().map(|f| f.nom).collect::<Vec<_>>(), places);
    }
    let c = treteaux("servies");
    let inconnue = |r: &Value| r["erreur"].as_str().is_some_and(|m| m.starts_with("route inconnue"));
    assert!(inconnue(&route(&c, "pas-une-route", json!({}), "2026-11-02")));
    for nom in &noms {
        let r = route(&c, nom, json!({}), "2026-11-02");
        assert!(!inconnue(&r), "{} n'est pas servie par moteur route", nom);
    }
}

/// Les crates de l'espace, de bas en haut : chacune ne prend qu'a celles d'en dessous.
#[test]
fn une_crate_ne_prend_qu_a_celles_du_dessous() {
    const ORDRE: [&str; 7] = ["socle", "droit", "engagement", "documents", "declarations", "envois", "moteur"];
    for (rang, nom) in ORDRE.iter().enumerate() {
        let t = std::fs::read_to_string(espace().join(nom).join("Cargo.toml")).unwrap();
        let deps = t.split("\n[dependencies]").nth(1).unwrap_or("").split("\n[").next().unwrap();
        for l in deps.lines() {
            let dep = l.split([' ', '=']).next().unwrap_or("");
            if let Some(r) = ORDRE.iter().position(|x| *x == dep) {
                assert!(r < rang, "{} prend a {}, qui n'est pas en dessous", nom, dep);
            }
        }
    }
}

/// L'engagement ne lit pas le journal : le moteur le lit une fois par geste (`envois::journal::lu`) et lui passe ce qui
/// est parti.
#[test]
fn l_engagement_ne_lit_pas_le_journal() {
    for p in sources(&espace().join("engagement/src")) {
        for (n, l) in std::fs::read_to_string(&p).unwrap().lines().enumerate() {
            let code = l.split("//").next().unwrap();
            assert!(!code.contains("\"journal"), "{}:{} lit le journal : {}", p.display(), n + 1, l.trim());
        }
    }
}

/// Une valeur d'une fiche (entreprise, salarie, spectacle) se date a un seul endroit : `routes::fiches::ecrire_datee`,
/// que l'ecran, les gestes d'une case et l'import appellent. Ailleurs dans le moteur, `contexte::dater` ne date que le
/// fichier des organismes et la fiche d'une compagnie d'essai, posee neuve sous une autre identite.
#[test]
fn une_seule_fonction_date_une_valeur_de_fiche() {
    const AUTRES: [&str; 2] = ["moteur/src/compagnie/organismes.rs", "moteur/src/compagnie/essai.rs"];
    let fiches = espace().join("moteur/src/routes/fiches.rs");
    let texte = std::fs::read_to_string(&fiches).unwrap();
    let debut = texte.find("pub(crate) fn ecrire_datee(").unwrap();
    let fin = debut + texte[debut..].find("\n}\n").unwrap();
    assert_eq!(texte[..debut].matches("contexte::dater(").count() + texte[fin..].matches("contexte::dater(").count(), 0, "fiches.rs ne date que par `ecrire_datee`");
    for p in sources(&espace().join("moteur/src")).into_iter().filter(|p| *p != fiches) {
        let rel = p.strip_prefix(espace()).unwrap().to_string_lossy().into_owned();
        if std::fs::read_to_string(&p).unwrap().contains("contexte::dater(") {
            assert!(AUTRES.contains(&rel.as_str()), "{} date une valeur de fiche : il passe par `fiches::ecrire_datee`", rel);
        }
    }
}

/// Un seul endroit ecrit un fichier d'engagements : `engagement::stock::poser`. Hors de `stock.rs`, le chemin d'un tel
/// fichier (`stock::chemin`) ne se nomme que pour le lire : dans la cloture (l'archive de l'exercice) et l'apercu d'un
/// acte (le fichier designe est-il celui du mois ?).
#[test]
fn un_seul_endroit_ecrit_un_fichier_d_engagements() {
    const LECTEURS: [&str; 2] = ["moteur/src/compagnie/exercice_ecrit.rs", "moteur/src/commandes/actes.rs"];
    let stock = espace().join("engagement/src/stock.rs");
    assert_eq!(std::fs::read_to_string(&stock).unwrap().matches("ecrire_json(").count(), 1, "stock.rs n'ecrit que par `poser`");
    for crate_ in ["engagement", "documents", "declarations", "envois", "moteur"] {
        for p in sources(&espace().join(crate_).join("src")).into_iter().filter(|p| *p != stock) {
            if std::fs::read_to_string(&p).unwrap().contains("stock::chemin(") {
                let rel = p.strip_prefix(espace()).unwrap().to_string_lossy().into_owned();
                assert!(LECTEURS.contains(&rel.as_str()), "{} nomme le fichier d'un engagement : il passe par `stock`", rel);
            }
        }
    }
}
