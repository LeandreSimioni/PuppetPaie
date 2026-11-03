//! LES OS, GARDES QUOI QU'ON CHANGE DESSUS : chaque route dit en une phrase breve ce qu'elle fait, et l'exemple de sa
//! fiche est accepte ; la page et la ligne de commande ont le meme catalogue ; une crate ne prend qu'a celles du
//! dessous ; l'engagement ne lit pas le journal ; une seule fonction date une valeur de fiche ; un seul endroit
//! ecrit un fichier d'engagements ; une seule fonction lit une paie, une seule l'ecrit.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use moteur::routes::catalogue::{catalogue, Acte, Ecran, Place, FICHES};
use serde_json::{json, Value};

use crate::bulletins_fictifs::{a_heures, a_repetitions, page_a, pdf};
use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dossier_de_test, route, route_avec};
use moteur::compagnie::administrateur::Poste;
use moteur::interface::compagnies::{ajouter_venant, Venue};
use moteur::compagnie::demande::VARIABLE_ESSAIS;

/// La longueur d'un geste, en caracteres : une phrase breve.
const GESTE_MAX: usize = 80;

/// Toutes les places de l'ecran (`catalogue::Place`).
const PLACES: [Place; 14] = [
    Place::Paie, Place::Fiches, Place::Metiers, Place::Organismes, Place::Bulletin, Place::ModelesContrat, Place::Identite,
    Place::Coffre, Place::Import, Place::Historique, Place::Exports, Place::Signaler, Place::Bandeau, Place::Accueil,
];


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

/// CE QUE L'ECRAN AFFICHE EST FAIT POUR L'ECRAN (`routes::affiche`) : le bouton de chaque route sous « Avancé » et le
/// libelle de chaque champ, tels que le catalogue les rend a la page (`routes --avance`), et le libelle, l'aide et les
/// choix de chaque case de `donnees/champs-base.json` : brefs, sans fichier, chemin ni terme de code ; un libelle ou un
/// bouton commence par une capitale. Les phrases que rendent les routes passent la meme verification
/// (`l_exemple_de_chaque_fiche_est_accepte`). Le sens long, qui peut nommer un fichier, est celui du LLM (`sens`, `geste`).
#[test]
fn ce_que_l_ecran_affiche_est_fait_pour_l_ecran() {
    use moteur::routes::affiche::{defaut, COURT};
    let mut fautes = Vec::new();
    let mut voir = |ou: String, t: &str, place: usize, court: bool| {
        if let Some(d) = defaut(t, place, court) {
            fautes.push(format!("{} : « {} » : {}", ou, t, d.corriger()));
        }
    };
    for f in moteur::routes::catalogue::en_json(true)["routes"].as_array().unwrap() {
        let nom = f["nom"].as_str().unwrap();
        if let Some(b) = f["bouton"].as_str() {
            voir(format!("le bouton de {}", nom), b, COURT, true);
        }
        for c in ["charge", "cases", "cases_de_la_fin"].iter().flat_map(|k| f[*k].as_array().unwrap()) {
            voir(format!("le libellé de {}.{}", nom, c["nom"].as_str().unwrap()), c["libelle"].as_str().unwrap(), COURT, true);
        }
    }
    fn cases<'a>(v: &'a Value, ou: &str, out: &mut Vec<(String, &'a Value)>) {
        match v {
            Value::Object(o) if o.contains_key("cle") && o.contains_key("libelle") => {
                let ici = format!("{}.{}", ou, o["cle"].as_str().unwrap_or("?"));
                out.push((ici.clone(), v));
                for x in o.values() {
                    cases(x, &ici, out);
                }
            }
            Value::Object(o) => o.iter().filter(|(k, _)| !k.starts_with('_')).for_each(|(k, x)| cases(x, &format!("{}{}", ou, k), out)),
            Value::Array(a) => a.iter().for_each(|x| cases(x, ou, out)),
            _ => {}
        }
    }
    let base: Value = serde_json::from_str(&std::fs::read_to_string(espace().join("../../logiciel/donnees/champs-base.json")).unwrap()).unwrap();
    let mut toutes = Vec::new();
    cases(&base, "", &mut toutes);
    // Une case que l'ecran ne montre pas (`ecran: false`, et ce qu'elle porte) est pour le LLM.
    let cachees: Vec<String> = toutes.iter().filter(|(_, c)| c["ecran"] == json!(false)).map(|(ou, _)| ou.clone()).collect();
    for (ou, c) in toutes.iter().filter(|(ou, _)| !cachees.iter().any(|x| ou == x || ou.starts_with(&format!("{}.", x)))) {
        voir(format!("champs-base.json {} : libelle", ou), c["libelle"].as_str().unwrap_or(""), CASE, true);
        if let Some(a) = c["aide"].as_str().filter(|a| !a.is_empty()) {
            voir(format!("champs-base.json {} : aide", ou), a, AIDE, false);
        }
        // Un choix sans libelle s'affiche tel qu'il s'ecrit.
        for v in c["choix"].as_array().into_iter().flatten().filter_map(Value::as_str).filter(|v| !v.is_empty()) {
            let l = c["libelles"][v].as_str().unwrap_or(v);
            voir(format!("champs-base.json {} : le choix « {} »", ou, v), l, CHOIX, false);
        }
        if let Some(l) = c["libelles"][""].as_str() {
            voir(format!("champs-base.json {} : le choix vide", ou), l, CHOIX, false);
        }
    }
    let mut chemins = Vec::new();
    // LES PHRASES DE REFUS que les sources construisent : la page les affiche, le LLM les lit (`refus_rs`).
    for crate_ in ["socle", "droit", "engagement", "documents", "declarations", "envois", "moteur"] {
        let atelier = espace().join("moteur/src/atelier");
        for p in sources(&espace().join(crate_).join("src")).into_iter().filter(|p| !p.starts_with(&atelier)) {
            let rel = p.strip_prefix(espace()).unwrap().to_string_lossy().into_owned();
            for (ligne, phrase, chemin) in refus_rs(&std::fs::read_to_string(&p).unwrap()) {
                if chemin {
                    chemins.push(format!("{}:{} : le refus « {} » compose un chemin : il va dans un champ à part (`socle::echec::avec`)", rel, ligne, phrase));
                } else {
                    voir(format!("{}:{} : le refus", rel, ligne), &phrase, usize::MAX, false);
                }
            }
        }
    }
    fautes.extend(chemins);
    assert!(fautes.is_empty(), "ce que l'écran affiche n'est pas fait pour l'écran : récris-le bref, en mots, dans le moteur (le sens long reste au LLM ; un chemin, une clé, un fichier vont dans un champ à part du refus, `socle::echec::avec`) :\n{}", fautes.join("\n"));
}

/// LES PHRASES DE REFUS d'un source Rust (hors commentaires et tests) : le litteral que construit un refus, en premier
/// argument de `Err(…)`, `ok_or(…)`, `ok_or_else(|| …)`, `map_err(|e| …)`, `Echec::from(…)`, `message: …`, `avec(…)`,
/// ou en second de `erreur(classe, …)`, `noter_et(classe, …)` (directement ou par `format!`). Chacune : sa ligne, sa
/// phrase (une valeur composee `{}` y devient un mot ; `{:?}` reste du code) et si ses valeurs portent un chemin
/// (`.display()`).
fn refus_rs(t: &str) -> Vec<(usize, String, bool)> {
    let c: Vec<char> = t.chars().collect();
    // Les modules de tests (`#[cfg(test)]` puis le bloc qui suit) : leurs litteraux ne sont pas des refus.
    let mut essais: Vec<(usize, usize)> = Vec::new();
    let marque: Vec<char> = "#[cfg(test)]".chars().collect();
    let mut i = 0;
    while i + marque.len() <= c.len() {
        if c[i..i + marque.len()] == marque[..] {
            let mut j = i + marque.len();
            while j < c.len() && c[j] != '{' && c[j] != ';' {
                j += 1;
            }
            let mut profondeur = 0;
            while j < c.len() {
                match c[j] {
                    '{' => profondeur += 1,
                    '}' => {
                        profondeur -= 1;
                        if profondeur <= 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            essais.push((i, j));
            i = j;
        }
        i += 1;
    }
    let mut out = Vec::new();
    for (ligne, debut, fin, l) in litteraux_rs_places(t) {
        if essais.iter().any(|(a, b)| *a <= debut && debut <= *b) {
            continue;
        }
        let avant: String = c[debut.saturating_sub(160)..debut].iter().filter(|x| !x.is_whitespace()).collect();
        let avant = avant.strip_suffix("format!(").unwrap_or(&avant);
        let avant = avant.strip_suffix('&').unwrap_or(avant);
        let premier = ["Err(", "ok_or(", "Echec::from(", "message:", "echec::avec(", "echec::a_la_cle(", "echec::cle(", "ok_or_else(||"].iter().any(|s| avant.ends_with(s))
            || avant.rsplit_once("map_err(|").is_some_and(|(_, r)| r.ends_with('|') && r[..r.len() - 1].chars().all(|x| x.is_alphanumeric() || x == '_'));
        let second = ["erreur(", "noter_et("].iter().any(|f| avant.rsplit_once(f).is_some_and(|(_, r)| r.ends_with(',') && r.matches(',').count() == 1 && !r.replace("Some(", "").contains('(')));
        if !(premier || second) {
            continue;
        }
        // Les valeurs composees : jusqu'a la parenthese qui ferme l'appel.
        let mut profondeur = 0i32;
        let mut j = fin;
        let mut args = String::new();
        let mut dans = false;
        while j < c.len() {
            let x = c[j];
            if dans {
                if x == '\\' {
                    j += 1;
                } else if x == '"' {
                    dans = false;
                }
            } else if x == '"' {
                dans = true;
            } else if "([{".contains(x) {
                profondeur += 1;
            } else if ")]}".contains(x) {
                profondeur -= 1;
                if profondeur < 0 {
                    break;
                }
            } else if x == ';' && profondeur == 0 {
                break;
            }
            if !dans {
                args.push(x);
            }
            j += 1;
        }
        out.push((ligne, phrase_de_refus(&l), args.contains(".display()")));
    }
    out
}

/// Le litteral d'un refus tel que l'ecran le lit : les echappements rendus, une valeur composee `{…}` en mot (`x`) ;
/// `{:?}` (la forme de code d'une valeur) garde son accolade.
fn phrase_de_refus(l: &str) -> String {
    let c: Vec<char> = l.chars().collect();
    let mut s = String::new();
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            '\\' => {
                match c.get(i + 1) {
                    Some('n') | Some('t') => s.push(' '),
                    Some('u') => {
                        let fin = c[i..].iter().position(|x| *x == '}').map_or(c.len(), |p| i + p);
                        let hex: String = c[i + 3..fin].iter().collect();
                        s.push(u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32).unwrap_or(' '));
                        i = fin - 1;
                    }
                    Some(&y) => s.push(y),
                    None => {}
                }
                i += 2;
            }
            '{' if c.get(i + 1) == Some(&'{') => {
                s.push('{');
                i += 2;
            }
            '}' if c.get(i + 1) == Some(&'}') => {
                s.push('}');
                i += 2;
            }
            '{' => {
                let fin = c[i..].iter().position(|x| *x == '}').map_or(c.len(), |p| i + p);
                let dedans: String = c[i + 1..fin].iter().collect();
                s.push_str(if dedans.ends_with('?') { "{" } else { "x" });
                i = fin + 1;
            }
            x => {
                s.push(x);
                i += 1;
            }
        }
    }
    s
}

/// La place du libelle d'une case de fiche (une ligne du formulaire), de son aide et d'un choix de sa liste, en caracteres.
const CASE: usize = 60;
const AIDE: usize = 500;
const CHOIX: usize = 100;

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
/// chacun est accepte, rend sa phrase (ce que `moteur route --texte` imprime, jamais du JSON), et celle-ci tient dans sa
/// place.
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
        ("arret-enregistrer", json!({"salarie": "SVOBODA", "dernier_jour": "2026-11-02", "debut": "2026-11-03", "fin": "2026-11-10", "motif": "04", "date_accident": "2026-11-02"})),
        ("arret-enregistrer", json!({"salarie": "SVOBODA", "dernier_jour": "2026-11-16", "debut": "2026-11-17", "fin": "2026-11-18", "motif": "19"})),
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
    let mut phrases_fautives = Vec::new();
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
            // Le dossier portable : une installation a part qui porte deja les trois systemes (rien ne se telecharge).
            "portable-completer" => {
                let (inst, reelle) = installation_a_part(&c, f.nom);
                for s in moteur::compagnie::maj::SYSTEMES {
                    let p = moteur::compagnie::maj::programme_de(&inst.join("logiciel"), s, "puppetpaie");
                    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                    std::fs::write(p, "").unwrap();
                }
                route(&reelle, f.nom, exemple, "2026-11-02")
            }
            _ => route_avec(&c, f.nom, exemple, "2026-11-02", &[(VARIABLE_ESSAIS, "exemple")]),
        };
        assert_eq!(r["ok"], json!(true), "{} refuse l'exemple de sa fiche : {}", f.nom, r);
        assert!(r["resultat"]["phrase"].as_str().is_some_and(|p| !p.trim().is_empty()) && !moteur::service::en_texte(f.nom, &r).trim_start().starts_with(['{', '[']), "{} : sa phrase manque (moteur route --texte ne rend que des phrases, celles que l'écran lit) : {}", f.nom, r);
        for p in moteur::routes::phrases_rendues(&r["resultat"]) {
            if let Some(d) = moteur::routes::affiche::defaut(p, usize::MAX, false) {
                phrases_fautives.push(format!("{} : « {} » : {}", f.nom, p, d.corriger()));
            }
        }
        if f.phrase > 0 {
            let n = moteur::routes::plus_longue_phrase(&r["resultat"]);
            assert!(n <= f.phrase, "{} : une phrase qu'elle rend (la sienne, ou celle d'une de ses lignes) passe sa place ({}) : {} caractères : {}", f.nom, f.phrase, n, r);
        }
    }
    assert!(phrases_fautives.is_empty(), "des phrases que rendent les routes ne sont pas faites pour l'écran (un fichier, un chemin, un terme de code) : récris-les en mots, dans le moteur :\n{}", phrases_fautives.join("\n"));
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
    ajouter_venant(&inst, "Demo", Venue::Neuve).unwrap();
    crate::outils::debloquer(&inst);
    let reelle = ajouter_venant(&inst, "La Compagnie", Venue::Neuve).unwrap().racine;
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
    let b = ajouter_venant(&inst, "Seconde", Venue::Neuve).unwrap().racine;
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
const FENETRE: [(&str, &str); 12] = [
    ("choisir", "montrer la compagnie d'une tuile ; en ligne de commande, --racine"),
    ("verifier", "la deverrouiller dans cette fenetre ; en ligne de commande, la seance du coffre"),
    ("accueil", "revenir aux tuiles"),
    ("fermer", "fermer la fenetre"),
    ("redemarrer", "rouvrir la fenetre sur la version prete"),
    ("maj", "ou en est la recherche que la fenetre mene en fond (la version : maj-etat)"),
    ("site", "ouvrir dans le navigateur du poste le site de l'editeur, et lui seul (le lien de l'auteur)"),
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
    for p in PLACES {
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
            Ecran::Auto(p, _) => {
                assert!(f.charge.iter().all(|c| c.forme != moteur::routes::catalogue::Forme::Objet), "{} s'affiche sous « Avancé » : aucun champ de sa charge n'est un objet, que la page ne saisit pas d'une case", f.nom);
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
    // Une route inconnue se refuse en nommant la route a part (`routes::servir`).
    let inconnue = |r: &Value| r["ok"] == json!(false) && r.get("route").is_some_and(Value::is_string);
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

/// Une seule fonction lit un fichier de paies, une seule l'ecrit : dans `engagement::paies`, `fichier` (`stock::lire_dans`)
/// et `ecrire` (`stock::poser_dans`), sur le dossier des paies, que rien d'autre ne nomme. Hors de `paies.rs`, le fichier
/// des paies d'un mois ne se nomme que pour le lire tel qu'il est sur le disque : dans la cloture (l'archive de
/// l'exercice) et l'etat garde de l'ecran (sa signature).
#[test]
fn une_seule_fonction_lit_une_paie_et_une_seule_l_ecrit() {
    const LECTEURS: [&str; 2] = ["moteur/src/compagnie/exercice_ecrit.rs", "moteur/src/etat_garde.rs"];
    let paies = espace().join("engagement/src/paies.rs");
    let stock = espace().join("engagement/src/stock.rs");
    let texte = std::fs::read_to_string(&paies).unwrap();
    assert_eq!(texte.matches("stock::lire_dans(").count(), 1, "paies.rs ne lit un fichier de paies que par `fichier`");
    assert_eq!(texte.matches("stock::poser_dans(").count(), 1, "paies.rs n'ecrit un fichier de paies que par `ecrire`");
    for crate_ in ["engagement", "documents", "declarations", "envois", "moteur"] {
        for p in sources(&espace().join(crate_).join("src")).into_iter().filter(|p| *p != paies && *p != stock) {
            let t = std::fs::read_to_string(&p).unwrap();
            let rel = p.strip_prefix(espace()).unwrap().to_string_lossy().into_owned();
            assert!(!t.contains("Dossier::Paies"), "{} nomme le dossier des paies : il lit et ecrit une paie par `engagement::paies`", rel);
            if t.contains("paies::chemin(") || t.contains("paies::DOSSIER") {
                assert!(LECTEURS.contains(&rel.as_str()), "{} nomme le fichier des paies d'un mois : il passe par `engagement::paies`", rel);
            }
        }
    }
}

/// LA ROUTE REND LA PHRASE QUE L'ON LIT : la page (`logiciel/interface/js/`, `index.html`, et les pages que sert la
/// fenetre, `interface/pages/`) n'ecrit que des mots d'interface (le nom d'un bouton, d'une colonne, d'un onglet, d'un
/// champ) : un texte qu'elle montre a au plus `MOTS_D_INTERFACE` mots et, s'il en a plusieurs, aucune ponctuation de
/// phrase (`.` `:` `!` `…` ; un point d'interrogation clot une question d'interface, « Un problème, une idée ? »). Un
/// texte est ce qu'un litteral montre entre deux balises (un gabarit `${…}` ne le coupe pas : une phrase composee autour
/// d'une valeur reste une phrase), ou un attribut qui se lit (`title`, `placeholder`, `aria-label`, `alt`) ; un mot porte
/// une lettre et rien d'un code (ni le point d'un selecteur) ; un texte qui porte `[]{}=<>` est du code.
#[test]
fn la_page_n_ecrit_aucune_phrase() {
    let depot = espace().join("../..");
    let mut fichiers: Vec<PathBuf> = std::fs::read_dir(depot.join("logiciel/interface/js")).unwrap().flatten().map(|e| e.path()).collect();
    fichiers.push(depot.join("logiciel/interface/index.html"));
    fichiers.extend(std::fs::read_dir(espace().join("moteur/src/interface/pages")).unwrap().flatten().map(|e| e.path()));
    fichiers.sort();
    let mut phrases = Vec::new();
    for p in &fichiers {
        let t = std::fs::read_to_string(p).unwrap();
        let textes: Vec<String> = if p.extension().is_some_and(|x| x == "js") {
            litteraux_js(&t).iter().flat_map(|s| textes_html(s)).chain(tables_js(&t)).collect()
        } else {
            // Une page HTML : ses textes, et les litteraux de ses scripts (hors JSON).
            let t = t.replace("{{", "\u{1}").replace("}}", "\u{1}");
            let scripts: Vec<String> = t.split("<script").skip(1).filter(|s| !s.starts_with(" type=\"application/json\"")).filter_map(|s| s.split_once('>')).map(|(_, s)| s.split("</script>").next().unwrap_or("").to_string()).collect();
            textes_html(&t)
                .into_iter()
                .chain(scripts.iter().flat_map(|s| litteraux_js(s)).flat_map(|s| textes_html(&s)))
                .chain(scripts.iter().flat_map(|s| tables_js(s)))
                .collect()
        };
        for texte in textes {
            if let Some(phrase) = phrase(&texte) {
                phrases.push(format!("{} : « {} »", p.file_name().unwrap().to_string_lossy(), phrase));
            }
        }
    }
    assert!(
        phrases.is_empty(),
        "la page écrit {} phrase(s) : chacune doit venir d'une route (la phrase de la réponse du geste qui l'appelle, que la page affiche), ou disparaître ; la page n'écrit que des mots d'interface ({} mots au plus, sans ponctuation de phrase) :\n{}",
        phrases.len(), MOTS_D_INTERFACE, phrases.join("\n")
    );
}

/// Le plus de mots qu'ait un mot d'interface (« Envoyer à la signature », « Code emploi France Travail »).
const MOTS_D_INTERFACE: usize = 4;

/// Un mot se lie-t-il a une valeur dans `t` (\u{1}) ? Le mot et la valeur ne sont separes que par des blancs, une
/// parenthese ou un guillemet (« du … », « travail (…) ») ; un tiret, un point median, deux-points ou une barre les
/// separent (un titre et son nom, « Prolonger l'arrêt — … » ; une case et sa valeur, « Nom : … ») ; un mot colle a la
/// valeur (« …px », « …h ») est une unite, pas un mot qui s'y lie. Un choix de texte (\u{2}) compose toujours ; un
/// accord (\u{3}) colle a un mot (« salarié… ») aussi.
fn composee(t: &str) -> bool {
    let c: Vec<char> = t.chars().collect();
    let lien = |x: char| x.is_whitespace() || "()«»\"'".contains(x);
    for (i, x) in c.iter().enumerate() {
        match *x {
            '\u{2}' => return true,
            '\u{3}' if i > 0 && c[i - 1].is_alphabetic() => return true,
            '\u{1}' => {}
            _ => continue,
        }
        for sens in [-1i64, 1] {
            let mut j = i as i64 + sens;
            let mut blanc = false;
            while j >= 0 && (j as usize) < c.len() && lien(c[j as usize]) {
                blanc = true;
                j += sens;
            }
            if !(blanc && j >= 0 && (j as usize) < c.len() && c[j as usize].is_alphabetic()) {
                continue;
            }
            // Apres la valeur, son unite (« … h », « … Ko ») : deux lettres au plus, ou un multiple d'octet.
            let mot: String = c[j as usize..].iter().take_while(|x| x.is_alphabetic()).collect();
            if sens == 1 && (mot.chars().count() <= 2 || ["Ko", "Mo", "Go"].contains(&mot.as_str())) {
                continue;
            }
            return true;
        }
    }
    false
}

/// `Some(le texte)` si le texte est une phrase : plus de `MOTS_D_INTERFACE` mots, ou plusieurs mots et une ponctuation
/// de phrase, ou un texte compose autour d'une valeur (`composee`).
fn phrase(texte: &str) -> Option<String> {
    let brut = texte.replace("&nbsp;", " ").replace("&#160;", " ");
    let t = brut.replace(['\u{1}', '\u{2}', '\u{3}'], " ");
    if t.chars().any(|c| "[]{}=<>".contains(c)) {
        return None;
    }
    // Un mot porte une lettre, et rien d'un code : ni `/\_$#;|&*`, ni le point d'un selecteur en tete.
    let mots: Vec<&str> = t
        .split_whitespace()
        .filter(|m| m.chars().any(char::is_alphabetic) && !m.chars().any(|c| "/\\_$#;|&*".contains(c)) && !m.starts_with('.'))
        .collect();
    let c: Vec<char> = t.chars().collect();
    let ponctue = (1..c.len()).any(|i| {
        ".:!…".contains(c[i]) && c.get(i + 1).is_none_or(|y| y.is_whitespace()) && (c[i - 1].is_alphanumeric() || "»)".contains(c[i - 1]) || (c[i - 1] == ' ' && c[i] != '.'))
    });
    let compose = !mots.is_empty() && composee(&brut);
    (compose || mots.len() > MOTS_D_INTERFACE || mots.len() > 1 && ponctue)
        .then(|| texte.replace(['\u{1}', '\u{2}', '\u{3}'], "…").split_whitespace().collect::<Vec<_>>().join(" "))
}

/// Les litteraux de chaine d'un source JS, hors commentaires et expressions regulieres ; un gabarit garde la place de
/// chaque `${…}` (\u{1}), et ses litteraux interieurs sont des litteraux a part.
fn litteraux_js(t: &str) -> Vec<String> {
    let c: Vec<char> = t.chars().collect();
    let mut v = Vec::new();
    let (mut i, mut avant, mut choix) = (0, '(', false);
    while i < c.len() {
        let x = c[i];
        if x == '/' && c.get(i + 1) == Some(&'/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
        } else if x == '/' && c.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < c.len() && !(c[i] == '*' && c[i + 1] == '/') {
                i += 1;
            }
            i += 2;
        } else if x == '/' && "(,=:[!&|?{};+".contains(avant) {
            let mut classe = false;
            i += 1;
            while i < c.len() && (c[i] != '/' || classe) {
                match c[i] {
                    '\\' => i += 1,
                    '[' => classe = true,
                    ']' => classe = false,
                    _ => {}
                }
                i += 1;
            }
            i += 1;
            avant = 'r';
        } else if x == '"' || x == '\'' {
            let (s, j) = chaine(&c, i);
            // Une concatenation (`"mots " + valeur`) garde la place de la valeur, comme un gabarit.
            let apres = c[j.min(c.len())..].iter().find(|y| !y.is_whitespace()) == Some(&'+');
            // Un choix de texte selon une valeur (`a ? "Envoyer" : "Préparer"`) se marque \u{2}.
            let branche = avant == '?' || (avant == ':' && choix);
            choix = avant == '?';
            let marque = if branche && texte_humain(&s) { "\u{2}" } else { "" };
            let s = format!("{}{}{}{}", marque, if avant == '+' { "\u{1}" } else { "" }, s, if apres { "\u{1}" } else { "" });
            v.push(s);
            (i, avant) = (j, 's');
        } else if x == '`' {
            let (s, j) = gabarit(&c, i + 1, &mut v);
            let branche = avant == '?' || (avant == ':' && choix);
            choix = avant == '?';
            let s = if branche && texte_humain(&s) { format!("\u{2}{}", s) } else { s };
            v.push(s);
            (i, avant) = (j, 's');
        } else {
            if !x.is_whitespace() {
                avant = x;
            }
            i += 1;
        }
    }
    v
}

/// La chaine entre guillemets qui commence en `i`, et ou elle finit.
fn chaine(c: &[char], mut i: usize) -> (String, usize) {
    let q = c[i];
    let mut s = String::new();
    i += 1;
    while i < c.len() && c[i] != q {
        if c[i] == '\\' {
            i += 1;
        }
        if let Some(&y) = c.get(i) {
            s.push(if y == 'n' && c[i - 1] == '\\' { '\n' } else { y });
        }
        i += 1;
    }
    (s, i + 1)
}

/// Le gabarit qui commence en `i` (apres l'accent grave) : son texte, chaque `${…}` a sa place, et ou il finit. Les
/// litteraux du code de chaque `${…}` vont dans `v`. La place d'une valeur est \u{1} ; celle d'un choix de texte selon
/// une valeur (`a ? "Envoyer" : "Préparer"`) est \u{2} ; celle d'un accord (`n > 1 ? "s" : ""`) est \u{3}.
fn gabarit(c: &[char], mut i: usize, v: &mut Vec<String>) -> (String, usize) {
    let mut s = String::new();
    // Dans une balise, un choix est celui d'un attribut (` selected`, une classe) : il ne s'y lit pas.
    let mut dans_balise = false;
    while i < c.len() && c[i] != '`' {
        if c[i] == '\\' {
            if let Some(&y) = c.get(i + 1) {
                s.push(y);
            }
            i += 2;
        } else if c[i] == '$' && c.get(i + 1) == Some(&'{') {
            i += 2;
            let mut profondeur = 1;
            let (mut ternaire, mut branches) = (false, Vec::new());
            // Par niveau de parentheses (un appel, une liste) : un choix y a-t-il commence ? Un argument, ou ce qui
            // precede le `?`, n'est pas une branche ; une virgule ferme le choix de son niveau.
            let mut niveaux = vec![false];
            while i < c.len() && profondeur > 0 {
                if c[i] == '/' && c.get(i + 1) == Some(&'/') {
                    while i < c.len() && c[i] != '\n' {
                        i += 1;
                    }
                    continue;
                }
                match c[i] {
                    '{' => profondeur += 1,
                    '}' => profondeur -= 1,
                    '(' | '[' if profondeur == 1 => niveaux.push(false),
                    ')' | ']' if profondeur == 1 && niveaux.len() > 1 => {
                        niveaux.pop();
                    }
                    ',' if profondeur == 1 => *niveaux.last_mut().unwrap() = false,
                    '?' if profondeur == 1 && ternaire_en(c, i) => {
                        ternaire = true;
                        *niveaux.last_mut().unwrap() = true;
                    }
                    _ => {}
                }
                let branche = profondeur == 1 && *niveaux.last().unwrap();
                if c[i] == '`' {
                    let (t, j) = gabarit(c, i + 1, v);
                    if branche { branches.push(t) } else { v.push(t) }
                    i = j;
                } else if c[i] == '"' || c[i] == '\'' {
                    let (t, j) = chaine(c, i);
                    if branche { branches.push(t) } else { v.push(t) }
                    i = j;
                } else {
                    i += 1;
                }
            }
            // Dans une balise, seul un attribut qui se lit (`title`, `placeholder`…) montre le texte qu'il choisit.
            let se_lit = !dans_balise || attribut_qui_se_lit(&s);
            s.push(match () {
                _ if !se_lit => '\u{1}',
                _ if ternaire && branches.iter().any(|b| texte_humain(b)) => '\u{2}',
                _ if ternaire && branches.iter().any(|b| accord(b)) => '\u{3}',
                _ => '\u{1}',
            });
            // Les textes qu'un choix offre se lisent chacun, marques comme choisis.
            v.extend(branches.into_iter().map(|b| if ternaire && se_lit && texte_humain(&b) { format!("\u{2}{}", b) } else { b }));
        } else {
            match c[i] {
                '<' if c.get(i + 1).is_some_and(|y| y.is_ascii_alphabetic() || *y == '/') => dans_balise = true,
                '>' => dans_balise = false,
                _ => {}
            }
            s.push(c[i]);
            i += 1;
        }
    }
    (s, i + 1)
}

/// Le gabarit `s`, arrete dans une balise, est-il dans la valeur d'un attribut qui se lit (`title="…`) ?
fn attribut_qui_se_lit(s: &str) -> bool {
    let balise = &s[s.rfind('<').unwrap_or(0)..];
    let Some(q) = balise.rfind(['"', '\'']) else { return false };
    let g = balise[q..].chars().next().unwrap();
    let tete = &balise[..q];
    balise.matches(g).count() % 2 == 1
        && ["title=", "placeholder=", "aria-label=", "alt="]
            .iter()
            .any(|a| tete.strip_suffix(a).is_some_and(|r| r.ends_with(char::is_whitespace)))
}

/// Le `?` en `i` est-il celui d'un choix (`a ? b : c`) ? Ni `?.`, ni `??`.
fn ternaire_en(c: &[char], i: usize) -> bool {
    c.get(i + 1).is_some_and(|y| *y != '.' && *y != '?') && (i == 0 || c[i - 1] != '?')
}

/// Un litteral qui se lit : des lettres, sans balise, et deux mots, une capitale, une lettre accentuee, un signe de
/// l'ecran ou une apostrophe (« Envoyer », « l'arrêt », « · contrat ») ; un nom de code ou un attribut (`primaire`,
/// `a_faire`, `arret-signaler`, ` selected`) n'en est pas un.
fn texte_humain(t: &str) -> bool {
    let t = t.trim();
    !t.contains('<')
        && t.chars().any(char::is_alphabetic)
        && (t.split_whitespace().filter(|m| 2 * m.chars().filter(|c| c.is_alphabetic()).count() > m.chars().count()).count() > 1
            || t.chars().any(|c| c.is_uppercase() || !c.is_ascii() || c == '\''))
}

/// Les textes qu'une table de la page associe a une valeur : un objet litteral indexe par une valeur (`{…}[cle]`, ou
/// `const NOMS = {…}` puis `NOMS[cle]`) dont une propriete vaut un litteral qui se lit (`texte_humain`). Chacun se rend
/// marque comme choisi (\u{2}) : la page choisirait un texte selon une valeur.
fn tables_js(t: &str) -> Vec<String> {
    let c: Vec<char> = t.chars().collect();
    let mut v = Vec::new();
    let mut ouvertes: Vec<usize> = Vec::new();
    let (mut i, mut avant) = (0, '(');
    let mut jetes = Vec::new();
    while i < c.len() {
        let x = c[i];
        if x == '/' && c.get(i + 1) == Some(&'/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
        } else if x == '/' && c.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < c.len() && !(c[i] == '*' && c[i + 1] == '/') {
                i += 1;
            }
            i += 2;
        } else if x == '/' && "(,=:[!&|?{};+".contains(avant) {
            let mut classe = false;
            i += 1;
            while i < c.len() && (c[i] != '/' || classe) {
                match c[i] {
                    '\\' => i += 1,
                    '[' => classe = true,
                    ']' => classe = false,
                    _ => {}
                }
                i += 1;
            }
            i += 1;
            avant = 'r';
        } else if x == '"' || x == '\'' {
            (i, avant) = (chaine(&c, i).1, 's');
        } else if x == '`' {
            (i, avant) = (gabarit(&c, i + 1, &mut jetes).1, 's');
        } else {
            match x {
                '{' => ouvertes.push(i),
                '}' => {
                    if let Some(debut) = ouvertes.pop() {
                        let apres = c[i + 1..].iter().find(|y| !y.is_whitespace() && **y != ')');
                        let avant_txt: String = c[..debut].iter().collect();
                        let avant_txt = avant_txt.trim_end();
                        let nom: String = avant_txt
                            .strip_suffix('=')
                            .map(|a| a.trim_end().chars().rev().take_while(|y| y.is_alphanumeric() || *y == '_').collect::<Vec<_>>().into_iter().rev().collect())
                            .unwrap_or_default();
                        let indexee = apres == Some(&'[') || (!nom.is_empty() && t.contains(&format!("{}[", nom)));
                        if indexee {
                            v.extend(valeurs_de_table(&c[debut + 1..i]).into_iter().filter(|s| texte_humain(s)).map(|s| format!("\u{2}{}", s)));
                        }
                    }
                }
                _ => {}
            }
            if !x.is_whitespace() {
                avant = x;
            }
            i += 1;
        }
    }
    v
}

/// Les valeurs litterales (une chaine juste apres `:`) d'un corps d'objet, a son premier niveau.
fn valeurs_de_table(c: &[char]) -> Vec<String> {
    let mut v = Vec::new();
    let mut jetes = Vec::new();
    let (mut i, mut profondeur, mut deux_points) = (0, 0, false);
    while i < c.len() {
        match c[i] {
            '"' | '\'' => {
                let (s, j) = chaine(c, i);
                if profondeur == 0 && deux_points {
                    v.push(s);
                }
                deux_points = false;
                i = j;
                continue;
            }
            '`' => {
                i = gabarit(c, i + 1, &mut jetes).1;
                deux_points = false;
                continue;
            }
            '{' | '[' | '(' => profondeur += 1,
            '}' | ']' | ')' => profondeur -= 1,
            ':' if profondeur == 0 => deux_points = true,
            y if !y.is_whitespace() => deux_points = false,
            _ => {}
        }
        i += 1;
    }
    v
}

/// Un accord : une terminaison de trois lettres au plus (« s », « es », « nt »).
fn accord(t: &str) -> bool {
    (1..=3).contains(&t.chars().count()) && t.chars().all(|c| c.is_ascii_lowercase())
}

/// Les textes qu'un fragment HTML montre : ce qui est entre deux balises, et les attributs qui se lisent ; ni les
/// commentaires, ni `<script>`, ni `<style>`.
fn textes_html(t: &str) -> Vec<String> {
    let c: Vec<char> = t.chars().collect();
    let mut v = Vec::new();
    let mut texte = String::new();
    let mut i = 0;
    while i < c.len() {
        if c[i] != '<' || !c.get(i + 1).is_some_and(|y| y.is_ascii_alphabetic() || *y == '/' || *y == '!') {
            texte.push(c[i]);
            i += 1;
            continue;
        }
        v.push(std::mem::take(&mut texte));
        if c[i + 1..].starts_with(&['!', '-', '-']) {
            while i + 2 < c.len() && !(c[i] == '-' && c[i + 1] == '-' && c[i + 2] == '>') {
                i += 1;
            }
            i += 3;
            continue;
        }
        let debut = i;
        let mut guillemet: Option<char> = None;
        while i < c.len() && (guillemet.is_some() || c[i] != '>') {
            match guillemet {
                Some(g) if c[i] == g => guillemet = None,
                None if c[i] == '"' || c[i] == '\'' => guillemet = Some(c[i]),
                _ => {}
            }
            i += 1;
        }
        let balise: String = c[debut..i.min(c.len())].iter().collect();
        for attribut in [" title=", " placeholder=", " aria-label=", " alt="] {
            for m in balise.split(attribut).skip(1) {
                let mut it = m.chars();
                if let Some(g @ ('"' | '\'')) = it.next() {
                    v.push(it.take_while(|y| *y != g).collect());
                }
            }
        }
        let nom: String = balise.chars().skip(1).take_while(char::is_ascii_alphanumeric).collect();
        i += 1;
        if nom == "script" || nom == "style" {
            let reste: String = c[i.min(c.len())..].iter().collect();
            i += reste[..reste.find(&format!("</{}", nom)).unwrap_or(reste.len())].chars().count();
        }
    }
    v.push(texte);
    v
}

/// UN SEUL ENDROIT ECRIT UN MONTANT TEL QU'ON LE LIT : `socle::texte::euros` (« 1 234,50 € »), et une quantite par
/// `socle::texte::virgule`. Hors de `socle/src/texte.rs`, aucun litteral des sources ne met « € » ou « EUR » derriere
/// une valeur qu'il compose (`{}`, `{:.2}`…), ni ne commence par un blanc suivi de « € » (le suffixe qu'on colle a un
/// nombre) : c'est le signe d'une fonction qui refait la mise en forme. Un fichier dont une norme impose la forme d'un
/// montant (un format d'organisme, l'imprime d'un autre logiciel) l'ecrit sans « € », ou porte, sur la ligne du
/// litteral, la marque `// forme imposée :` suivie de la norme.
#[test]
fn un_seul_endroit_ecrit_un_montant() {
    let maison = espace().join("socle/src/texte.rs");
    let mut fautes = Vec::new();
    for crate_ in ["socle", "droit", "engagement", "documents", "declarations", "envois", "moteur"] {
        for p in sources(&espace().join(crate_).join("src")).into_iter().filter(|p| *p != maison) {
            let t = std::fs::read_to_string(&p).unwrap();
            let lignes: Vec<&str> = t.lines().collect();
            for (ligne, l) in litteraux_rs(&t) {
                if lignes.get(ligne - 1).is_some_and(|x| x.contains("// forme imposée :")) {
                    continue;
                }
                if met_des_euros(&l) {
                    let rel = p.strip_prefix(espace()).unwrap().to_string_lossy().into_owned();
                    fautes.push(format!("{}:{} : « {} »", rel, ligne, l));
                }
            }
        }
    }
    assert!(
        fautes.is_empty(),
        "{} litteral(aux) mettent un montant en forme : un montant s'ecrit par `socle::texte::euros` (ou `euros_de` pour une valeur), une quantite par `socle::texte::virgule` ; un format impose par une norme porte `// forme imposée : <la norme>` sur sa ligne :\n{}",
        fautes.len(), fautes.join("\n")
    );
}

/// Le litteral `l` met-il « € » ou « EUR » derriere une valeur qu'il compose, ou est-il un suffixe « € » ?
fn met_des_euros(l: &str) -> bool {
    let blanc = |c: char| c == ' ' || c == '\u{a0}' || c == '\u{202f}';
    let l = l.replace("\\u{a0}", "\u{a0}").replace("\\u{202f}", "\u{202f}").replace("\\u{20ac}", "€");
    if l.starts_with(blanc) && l.trim_start_matches(blanc).starts_with('€') {
        return true;
    }
    let mut reste = l.as_str();
    while let Some(i) = reste.find('}') {
        let avant = &reste[..i];
        reste = &reste[i + 1..];
        if !avant.contains('{') || avant.ends_with('}') {
            continue;
        }
        let suite = reste.trim_start_matches(blanc);
        let eur = suite.starts_with("EUR") && !suite[3..].starts_with(|c: char| c.is_alphanumeric() || c == '_');
        if suite.starts_with('€') || eur {
            return true;
        }
    }
    false
}

/// Les litteraux de chaine d'un source Rust (hors commentaires), avec la ligne ou chacun commence ; une fin de ligne
/// echappee (`\` puis le blanc qui suit) est retiree, comme le fait le compilateur.
fn litteraux_rs(t: &str) -> Vec<(usize, String)> {
    jetons_rs(t).into_iter().filter(|j| !j.3).map(|(l, _, _, _, s)| (l, s)).collect()
}

/// Les memes, avec ou chacun commence (son guillemet) et finit (apres le sien), en caracteres.
fn litteraux_rs_places(t: &str) -> Vec<(usize, usize, usize, String)> {
    jetons_rs(t).into_iter().filter(|j| !j.3).map(|(l, o, f, _, s)| (l, o, f, s)).collect()
}

/// Les commentaires d'un source Rust (`//`, `///`, `//!`, `/* */`), avec la ligne de chacun.
fn commentaires_rs(t: &str) -> Vec<(usize, String)> {
    jetons_rs(t).into_iter().filter(|j| j.3).map(|(l, _, _, _, s)| (l, s)).collect()
}

/// Les litteraux de chaine et les commentaires d'un source Rust : `(ligne, debut, fin, commentaire ?, texte)`, debut et
/// fin en caracteres.
fn jetons_rs(t: &str) -> Vec<(usize, usize, usize, bool, String)> {
    let c: Vec<char> = t.chars().collect();
    let (mut v, mut i, mut ligne) = (Vec::new(), 0, 1);
    while i < c.len() {
        match c[i] {
            '\n' => {
                ligne += 1;
                i += 1;
            }
            '/' if c.get(i + 1) == Some(&'/') => {
                let debut = i;
                while i < c.len() && c[i] != '\n' {
                    i += 1;
                }
                v.push((ligne, debut, i, true, c[debut..i].iter().collect()));
            }
            '/' if c.get(i + 1) == Some(&'*') => {
                let (debut, l) = (i, ligne);
                i += 2;
                while i + 1 < c.len() && !(c[i] == '*' && c[i + 1] == '/') {
                    ligne += (c[i] == '\n') as usize;
                    i += 1;
                }
                i += 2;
                v.push((l, debut, i.min(c.len()), true, c[debut..i.min(c.len())].iter().collect()));
            }
            '\'' => {
                // Un caractere ('"', '\'', '\u{a0}') ; sinon une duree de vie.
                if c.get(i + 1) == Some(&'\\') {
                    i += 2;
                    while i < c.len() && c[i] != '\'' {
                        i += 1;
                    }
                    i += 1;
                } else if c.get(i + 2) == Some(&'\'') {
                    i += 3;
                } else {
                    i += 1;
                }
            }
            'r' if (c.get(i + 1) == Some(&'"') || c.get(i + 1) == Some(&'#')) && (i == 0 || !(c[i - 1].is_alphanumeric() || c[i - 1] == '_')) => {
                let mut j = i + 1;
                let mut dieses = 0;
                while c.get(j) == Some(&'#') {
                    dieses += 1;
                    j += 1;
                }
                if c.get(j) != Some(&'"') {
                    i += 1;
                    continue;
                }
                let (debut, ouvre) = (ligne, i);
                let mut s = String::new();
                j += 1;
                while j < c.len() && !(c[j] == '"' && (0..dieses).all(|k| c.get(j + 1 + k) == Some(&'#'))) {
                    ligne += (c[j] == '\n') as usize;
                    s.push(c[j]);
                    j += 1;
                }
                i = j + 1 + dieses;
                v.push((debut, ouvre, i, false, s));
            }
            '"' => {
                let (debut, ouvre) = (ligne, i);
                let mut s = String::new();
                i += 1;
                while i < c.len() && c[i] != '"' {
                    if c[i] == '\\' && c.get(i + 1) == Some(&'\n') {
                        i += 1;
                        while i < c.len() && c[i].is_whitespace() {
                            ligne += (c[i] == '\n') as usize;
                            i += 1;
                        }
                        continue;
                    }
                    if c[i] == '\\' {
                        s.push(c[i]);
                        i += 1;
                    }
                    if let Some(&x) = c.get(i) {
                        ligne += (x == '\n') as usize;
                        s.push(x);
                    }
                    i += 1;
                }
                i += 1;
                v.push((debut, ouvre, i, false, s));
            }
            _ => i += 1,
        }
    }
    v
}

/// LE CODE DIT CE QUI EST, JAMAIS COMMENT ON Y EST ARRIVE : aucun commentaire des sources Rust de l'espace (et de leurs
/// tests), du CSS, du HTML et des scripts de la page ne porte une marque d'histoire (`HISTOIRE` : le numero d'une demande, son
/// demandeur, ce qui a change). Le CSS et le HTML ne citent aucun texte depose : une date y est celle d'une decision
/// (`DATE`). L'histoire va au journal du depot prive ; un commentaire d'histoire s'efface, ou se reecrit en son pourquoi.
#[test]
fn le_code_ne_raconte_pas_son_histoire() {
    const HISTOIRE: &str = r"(?i)(?:^|[^&\w])#\d{2,3}\b|\bticket|\bl[ée]andre\b|\bd[ée]sormais\b|\bavant, on\b|\bcomme le python\b";
    const DATE: &str = r"\b\d{1,2}/\d{1,2}(?:/20\d{2})?\b";
    let histoire = regex::Regex::new(HISTOIRE).unwrap();
    let date = regex::Regex::new(DATE).unwrap();
    let mut fautes = Vec::new();
    let mut noter = |p: &Path, commentaires: Vec<(usize, String)>, page: bool| {
        for (n, c) in commentaires {
            if histoire.is_match(&c) || (page && date.is_match(&c)) {
                let rel = p.canonicalize().unwrap().strip_prefix(depot()).unwrap().to_string_lossy().into_owned();
                fautes.push(format!("{}:{} : {}", rel, n, c.lines().next().unwrap_or("").trim()));
            }
        }
    };
    for e in std::fs::read_dir(espace()).unwrap().flatten() {
        for d in ["src", "tests"].map(|d| e.path().join(d)).into_iter().filter(|d| d.is_dir()) {
            for p in sources(&d) {
                noter(&p, commentaires_rs(&std::fs::read_to_string(&p).unwrap()), false);
            }
        }
    }
    for p in fichiers_de_la_page() {
        let t = std::fs::read_to_string(&p).unwrap();
        let c = if p.extension().is_some_and(|x| x == "css") { commentaires_c(&t, false) } else { commentaires_html(&t) };
        noter(&p, c, true);
    }
    for p in std::fs::read_dir(depot().join("logiciel/interface/js")).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "js")) {
        noter(&p, commentaires_c(&std::fs::read_to_string(&p).unwrap(), true), false);
    }
    assert!(
        fautes.is_empty(),
        "{} commentaire(s) racontent l'histoire du code : un commentaire dit le pourquoi d'une regle (le texte depose, la contrainte), jamais d'ou elle vient ; l'histoire va au journal du depot prive :\n{}",
        fautes.len(),
        fautes.join("\n")
    );
}

/// LA PAGE SE CHARGE : chaque script se lit sans erreur de syntaxe (`node --check`), et ses scripts, classiques
/// (`index.html`, `<script src>`), partagent une seule
/// portee. Un nom declare deux fois au premier niveau (`function`, `const`, `let`, `class`), dans un fichier ou d'un
/// fichier a l'autre, fait refuser le second fichier entier au navigateur, et tout ce qu'il porte manque a l'ecran.
#[test]
fn chaque_script_de_la_page_se_charge() {
    let decl = regex::Regex::new(r"(?m)^(?:async\s+)?(?:function\*?|const|let|class)\s+([A-Za-z_$][\w$]*)").unwrap();
    let mut vus: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    let mut mal_ecrits = Vec::new();
    for p in std::fs::read_dir(depot().join("logiciel/interface/js")).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "js")) {
        let t = std::fs::read_to_string(&p).unwrap();
        let nom = p.file_name().unwrap().to_string_lossy().into_owned();
        let syntaxe = std::process::Command::new("node").arg("--check").arg(&p).output().expect("node, pour lire chaque script de la page comme le navigateur le lit");
        if !syntaxe.status.success() {
            mal_ecrits.push(format!("{} : {}", nom, String::from_utf8_lossy(&syntaxe.stderr).lines().find(|l| l.contains("Error")).unwrap_or("")));
        }
        for c in decl.captures_iter(&t) {
            let ligne = t[..c.get(0).unwrap().start()].matches('\n').count() + 1;
            vus.entry(c[1].to_string()).or_default().push(format!("{}:{}", nom, ligne));
        }
    }
    assert!(mal_ecrits.is_empty(), "{} script(s) de la page que le navigateur refuserait (node --check) :\n{}", mal_ecrits.len(), mal_ecrits.join("\n"));
    let doubles: Vec<String> = vus.into_iter().filter(|(_, l)| l.len() > 1).map(|(n, l)| format!("{} : {}", n, l.join(", "))).collect();
    assert!(doubles.is_empty(), "{} nom(s) declares deux fois au premier niveau des scripts de la page : le navigateur refuserait le second fichier ; renommer l'un, ou ne garder qu'une declaration :\n{}", doubles.len(), doubles.join("\n"));
}

/// ON NE COUPE JAMAIS UN TEXTE : le CSS de la page (ses feuilles, et les `<style>` des pages que sert la fenetre) n'a ni
/// `text-overflow`, ni `line-clamp`, ni `overflow: hidden` ou `clip` (sur un axe ou les deux). Ce qui ne tient pas
/// defile (`overflow: auto`) ou passe a la ligne ; ce qui deborde se voit, et une phrase trop longue s'ecrit plus brève
/// dans le moteur.
#[test]
fn la_page_ne_coupe_aucun_texte() {
    let coupe = regex::Regex::new(r"(?i)text-overflow|line-clamp|overflow(?:-x|-y)?\s*:\s*(?:hidden|clip)").unwrap();
    let mut fautes = Vec::new();
    for p in fichiers_de_la_page() {
        let t = std::fs::read_to_string(&p).unwrap();
        let feuilles: Vec<(usize, String)> = if p.extension().is_some_and(|x| x == "css") {
            vec![(1, t.clone())]
        } else {
            blocs(&t, "<style", "</style>")
        };
        for (debut, f) in feuilles {
            for (n, l) in sans_commentaires_c(&f).lines().enumerate() {
                if coupe.is_match(l) {
                    let rel = p.canonicalize().unwrap().strip_prefix(depot()).unwrap().to_string_lossy().into_owned();
                    fautes.push(format!("{}:{} : {}", rel, debut + n, l.trim()));
                }
            }
        }
    }
    assert!(
        fautes.is_empty(),
        "{} regle(s) CSS coupent un texte : ce qui ne tient pas defile (`overflow: auto`) ou passe a la ligne, jamais ne se cache :\n{}",
        fautes.len(),
        fautes.join("\n")
    );
}

fn depot() -> PathBuf {
    espace().join("../..").canonicalize().unwrap()
}

/// Le CSS et le HTML de la page : les feuilles et `index.html` de `logiciel/interface/`, et les pages que sert la fenetre
/// (`moteur/src/interface/pages/`).
fn fichiers_de_la_page() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(depot().join("logiciel/interface")).unwrap().flatten().map(|e| e.path()).collect();
    v.extend(std::fs::read_dir(espace().join("moteur/src/interface/pages")).unwrap().flatten().map(|e| e.path()));
    v.retain(|p| p.extension().is_some_and(|x| x == "css" || x == "html"));
    v.sort();
    v
}

/// Ce qui est entre `ouvre` (jusqu'a son `>`) et `ferme`, avec la ligne du fichier ou chaque bloc commence.
fn blocs(t: &str, ouvre: &str, ferme: &str) -> Vec<(usize, String)> {
    let mut v = Vec::new();
    let mut reste = 0;
    while let Some(i) = t[reste..].find(ouvre) {
        let Some(j) = t[reste + i..].find('>') else { break };
        let debut = reste + i + j + 1;
        let fin = t[debut..].find(ferme).map_or(t.len(), |k| debut + k);
        v.push((t[..debut].matches('\n').count() + 1, t[debut..fin].to_string()));
        reste = fin;
    }
    v
}

/// Les commentaires d'un HTML : les `<!-- -->`, et ceux des `<style>` et des `<script>` qu'il porte.
fn commentaires_html(t: &str) -> Vec<(usize, String)> {
    let mut v = blocs(t, "<!--", "-->");
    for (ouvre, ferme, ligne) in [("<style", "</style>", false), ("<script", "</script>", true)] {
        for (debut, b) in blocs(t, ouvre, ferme) {
            v.extend(commentaires_c(&b, ligne).into_iter().map(|(n, c)| (debut + n - 1, c)));
        }
    }
    v
}

/// Les commentaires `/* */` (et `//` si `ligne`, pour un script) d'un CSS ou d'un script, avec la ligne de chacun ; une
/// chaine (`'`, `"`, `` ` ``) n'en porte pas.
fn commentaires_c(t: &str, ligne: bool) -> Vec<(usize, String)> {
    let c: Vec<char> = t.chars().collect();
    let (mut v, mut i, mut n) = (Vec::new(), 0, 1);
    while i < c.len() {
        match c[i] {
            '\n' => n += 1,
            '/' if c.get(i + 1) == Some(&'*') => {
                let (debut, l) = (i, n);
                i += 2;
                while i + 1 < c.len() && !(c[i] == '*' && c[i + 1] == '/') {
                    n += (c[i] == '\n') as usize;
                    i += 1;
                }
                i += 1;
                v.push((l, c[debut..(i + 1).min(c.len())].iter().collect()));
            }
            '/' if ligne && c.get(i + 1) == Some(&'/') => {
                let debut = i;
                while i + 1 < c.len() && c[i + 1] != '\n' {
                    i += 1;
                }
                v.push((n, c[debut..=i].iter().collect()));
            }
            // Une expression reguliere d'un script (`/[&<>"]/g`) : apres un operateur ou une ouverture, pas une division.
            '/' if ligne && c[..i].iter().rev().find(|x| !x.is_whitespace()).is_none_or(|x| "(,=:[!&|?{};".contains(*x)) => {
                let mut classe = false;
                i += 1;
                while i < c.len() && c[i] != '\n' && (classe || c[i] != '/') {
                    match c[i] {
                        '\\' => i += 1,
                        '[' => classe = true,
                        ']' => classe = false,
                        _ => {}
                    }
                    i += 1;
                }
            }
            q @ ('\'' | '"' | '`') => {
                i += 1;
                while i < c.len() && c[i] != q {
                    i += (c[i] == '\\') as usize;
                    n += (c.get(i) == Some(&'\n')) as usize;
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    v
}

/// Un CSS sans ses commentaires, chaque fin de ligne gardee (les numeros de ligne restent justes).
fn sans_commentaires_c(t: &str) -> String {
    let mut s = t.to_string();
    for (_, c) in commentaires_c(t, false) {
        s = s.replacen(&c, &c.chars().filter(|x| *x == '\n').collect::<String>(), 1);
    }
    s
}

/// UN SEUL CHEMIN POUR CHAQUE FICHIER DE LA COMPAGNIE : un dossier de la compagnie (`config/`, `journal/`, `retours/` et
/// leurs sous-dossiers) et chacun de leurs fichiers ne se nomment, dans le code du logiciel (hors tests), qu'a un seul
/// endroit : la constante ou la fonction qui en donne le chemin, au plus bas niveau de crate qui le sert ; les autres
/// l'appellent. Un nom, c'est un litteral qui n'est qu'un chemin sous l'un de ces dossiers (`"config/entreprise.json"`), un
/// `.join("<dossier>")`, ou un `.join("<nom>")` qui suit un dossier. Le garde ne lit pas une phrase qui cite un fichier, ni un
/// chemin compose ailleurs (`racine.join(CONFIG).join("x.json")` se lit ; `cfg.join("x.json")`, `format!("{}/x.json", ..)` non).
#[test]
fn un_seul_endroit_nomme_un_fichier_de_la_compagnie() {
    let chemin = regex::Regex::new(r#""((?:config|journal|retours)(?:/[\w.-]+)+)/?""#).unwrap();
    // Un dossier par son nom (`.join("config")`) ou par sa constante (`.join(socle::fichiers::CONFIG)`, qui ne le nomme
    // pas), puis ce qui s'y joint.
    let joint = regex::Regex::new(r#"\.join\((?:"(config|journal|retours)"|(?:\w+::)*(CONFIG|JOURNAL|RETOURS))\)((?:\s*\.join\("[\w.-]+"\))*)"#).unwrap();
    let suite = regex::Regex::new(r#"\.join\("([\w.-]+)"\)"#).unwrap();
    let mut noms: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for e in std::fs::read_dir(espace()).unwrap().flatten() {
        let src = e.path().join("src");
        if !src.is_dir() {
            continue;
        }
        for p in sources(&src) {
            let code = sans_tests(&std::fs::read_to_string(&p).unwrap());
            let code: String = code.lines().map(|l| if l.trim_start().starts_with("//") { "" } else { l }).collect::<Vec<_>>().join("\n");
            let rel = p.strip_prefix(espace()).unwrap().to_string_lossy().into_owned();
            let ligne = |i: usize| code[..i].matches('\n').count() + 1;
            for c in chemin.captures_iter(&code) {
                let m = c.get(1).unwrap();
                noms.entry(m.as_str().to_string()).or_default().push(format!("{}:{}", rel, ligne(m.start())));
            }
            for c in joint.captures_iter(&code) {
                let ou = format!("{}:{}", rel, ligne(c.get(0).unwrap().start()));
                let mut nom = match (c.get(1), c.get(2)) {
                    (Some(d), _) => {
                        noms.entry(d.as_str().to_string()).or_default().push(ou.clone());
                        d.as_str().to_string()
                    }
                    (_, Some(k)) => k.as_str().to_lowercase(),
                    _ => unreachable!(),
                };
                for s in suite.captures_iter(&c[3]) {
                    nom = format!("{}/{}", nom, &s[1]);
                    noms.entry(nom.clone()).or_default().push(ou.clone());
                }
            }
        }
    }
    let fautes: Vec<String> = noms.iter().filter(|(_, v)| v.len() > 1).map(|(k, v)| format!("{} : {}", k, v.join(", "))).collect();
    assert!(
        fautes.is_empty(),
        "{} chemin(s) de la compagnie se nomment a plusieurs endroits : un seul (la constante ou la fonction qui donne le chemin, au plus bas niveau de crate qui le sert), que les autres appellent :\n{}",
        fautes.len(),
        fautes.join("\n")
    );
}
/// TOUT CE QUI EST PUBLIC SERT LE LOGICIEL : chaque `pub fn`, `pub const`, `pub static` et `pub type` d'une crate de
/// l'espace a un appelant dans le code du logiciel (le `src/` d'une crate, la fenetre et l'atelier compris), hors de sa
/// definition et hors des tests. Les crates sont des bibliotheques : le compilateur ne dit rien d'un `pub` mort. Une
/// fonction que seul un test appelle teste ce que le logiciel n'emploie pas : le test passe par ce que le logiciel
/// emploie, ou part avec elle. Le garde lit les noms, pas les types : un nom repris ailleurs (`nouveau`, `lire`) compte
/// comme un appel, meme d'une autre fonction.
#[test]
fn tout_ce_qui_est_public_sert_le_logiciel() {
    let def = regex::Regex::new(r"(?m)^\s*pub (?:const fn|async fn|unsafe fn|fn|const|static|type) ([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    let mot = regex::Regex::new(r"[A-Za-z_][A-Za-z0-9_]*").unwrap();
    let mut codes = Vec::new();
    for e in std::fs::read_dir(espace()).unwrap().flatten() {
        let src = e.path().join("src");
        if src.is_dir() {
            for p in sources(&src) {
                let t = std::fs::read_to_string(&p).unwrap();
                let code = sans_tests(&t);
                let sans: String = code.lines().map(|l| l.split("//").next().unwrap()).collect::<Vec<_>>().join("\n");
                codes.push((p.strip_prefix(espace()).unwrap().to_string_lossy().into_owned(), sans));
            }
        }
    }
    let mut vus: std::collections::BTreeMap<&str, usize> = Default::default();
    for (_, c) in &codes {
        for m in mot.find_iter(c) {
            *vus.entry(m.as_str()).or_default() += 1;
        }
    }
    let mut morts = Vec::new();
    for (rel, c) in &codes {
        for d in def.captures_iter(c) {
            let nom = d.get(1).unwrap().as_str();
            if vus.get(nom).copied().unwrap_or(0) < 2 {
                let ligne = c[..d.get(0).unwrap().start()].matches('\n').count() + 1;
                morts.push(format!("{}:{} `{}`", rel, ligne, nom));
            }
        }
    }
    assert!(
        morts.is_empty(),
        "{} `pub` sans appelant dans le logiciel : ce que rien n'appelle part (avec le test qui l'appelait seul) ; si le logiciel en a besoin, c'est un chemin debranche, a rebrancher :\n{}",
        morts.len(),
        morts.join("\n")
    );
}

/// Le code d'un source Rust sans ses parties `#[cfg(test)]` (un module, une fonction ou un `use` a la marge).
fn sans_tests(t: &str) -> String {
    let mut v = Vec::new();
    let mut lignes = t.lines();
    while let Some(l) = lignes.next() {
        if l != "#[cfg(test)]" {
            v.push(l);
            continue;
        }
        if let Some(item) = lignes.next() {
            if !item.trim_end().ends_with(';') {
                for x in lignes.by_ref() {
                    if x == "}" {
                        break;
                    }
                }
            }
        }
    }
    v.join("\n")
}
