//! LA BANQUE DES CONSEILS (`logiciel/donnees/conseils.json`, ticket 224) : un contrat, pas une forme. Toute règle vient
//! d'un texte déposé (atelier/CLAUDE.md, « Les sources ») : chaque source nomme un fichier qui existe dans le dépôt, et la
//! citation s'y lit à l'identique (à la casse, aux blancs, aux apostrophes et aux guillemets près ; « […] » sépare des
//! fragments). Un chiffre daté ne s'écrit pas en dur : les gabarits {bareme:…}, {minima:…} et {annee} se résolvent sur les
//! données livrées. Chaque `quand` est « toujours » ou une condition que le moteur sait évaluer (`conseils::CONDITIONS`).

use crate::outils::code_livre;
use paie::conseils::{rendre, Donnees, Valeurs, CONDITIONS};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

fn depot() -> PathBuf {
    code_livre().join("../..")
}

fn banque() -> Value {
    serde_json::from_str(&std::fs::read_to_string(code_livre().join("conseils.json")).unwrap()).unwrap()
}

fn conseils(b: &Value) -> Vec<Value> {
    b["conseils"].as_array().unwrap().clone()
}

fn sources(c: &Value) -> Vec<Value> {
    match &c["source"] {
        Value::Array(a) => a.clone(),
        o @ Value::Object(_) => vec![o.clone()],
        autre => panic!("{} : source illisible ({})", c["id"], autre),
    }
}

/// Le texte comparé : sans balises (seulement pour l'XML de LEGI), sans les marques de citation markdown en tête de
/// ligne, minuscules, apostrophes et guillemets d'une seule sorte, blancs réduits.
fn normaliser(t: &str, xml: bool) -> String {
    let mut s = String::with_capacity(t.len());
    for ligne in t.lines() {
        let l = if xml { ligne } else { ligne.trim_start().trim_start_matches('>') };
        s.push_str(l);
        s.push(' ');
    }
    if xml {
        let mut sans = String::with_capacity(s.len());
        let mut dedans = false;
        for c in s.chars() {
            match c {
                '<' => dedans = true,
                '>' if dedans => {
                    dedans = false;
                    sans.push(' ');
                }
                _ if !dedans => sans.push(c),
                _ => {}
            }
        }
        s = sans;
    }
    let s = s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'");
    let mut out = String::with_capacity(s.len());
    let mut blanc = false;
    for c in s.chars().flat_map(char::to_lowercase) {
        let c = match c {
            '\u{2019}' => '\'',
            '«' | '»' | '“' | '”' => '"',
            '\u{a0}' | '\u{202f}' | '|' | '•' | '\t' | '\r' | '\n' => ' ',
            '\u{ad}' | '\u{200b}' | '*' | '\\' => continue,
            autre => autre,
        };
        if c == ' ' {
            if !blanc {
                out.push(' ');
            }
            blanc = true;
        } else {
            out.push(c);
            blanc = false;
        }
    }
    out
}

/// Les fragments d'une citation : « […] », « ... » et « … » les séparent ; les très courts ne prouvent rien et se taisent.
fn fragments(citation: &str) -> Vec<String> {
    citation
        .replace("[…]", "\u{1}")
        .replace("...", "\u{1}")
        .replace('…', "\u{1}")
        .split('\u{1}')
        .map(|f| normaliser(f, false).trim().to_string())
        .filter(|f| f.chars().count() >= 8)
        .collect()
}

/// Le texte d'un article de l'entrepôt LEGI (`articles.jsonl.gz`), toutes ses versions déposées.
fn article_legi(chemin: &Path, code: &str, numero: &str, cache: &mut HashMap<(String, String), String>) -> String {
    if cache.is_empty() {
        let f = std::fs::File::open(chemin).unwrap();
        for ligne in BufReader::new(flate2::read::GzDecoder::new(f)).lines() {
            let ligne = ligne.unwrap();
            let v: Value = serde_json::from_str(&ligne).unwrap();
            let cle = (v["cle"][0].as_str().unwrap_or("").to_string(), v["cle"][1].as_str().unwrap_or("").to_string());
            let xml = v["xml"].as_str().or(v["contenu"].as_str()).unwrap_or("").to_string();
            cache.entry(cle).or_default().push_str(&xml);
        }
    }
    cache.get(&(code.to_string(), numero.to_string())).cloned().unwrap_or_default()
}

#[test]
fn chaque_source_est_un_texte_depose_et_sa_citation_s_y_lit() {
    let b = banque();
    let mut legi = HashMap::new();
    let mut textes: HashMap<PathBuf, String> = HashMap::new();
    let mut fautes = Vec::new();
    for c in conseils(&b) {
        let id = c["id"].as_str().unwrap().to_string();
        for s in sources(&c) {
            let fichier = s["fichier"].as_str().unwrap_or_else(|| panic!("{} : source sans fichier", id));
            let chemin = depot().join(fichier);
            assert!(chemin.is_file(), "{} : le fichier cité n'est pas déposé : {}", id, fichier);
            let cle_titre = match s.get("code") {
                Some(code) => format!("{}#{}", fichier, code.as_str().unwrap()),
                None => fichier.to_string(),
            };
            assert!(b["documents"].get(&cle_titre).is_some(), "{} : le document {} n'a pas de titre lisible", id, cle_titre);
            let texte = if fichier.ends_with(".jsonl.gz") {
                let code = s["code"].as_str().unwrap_or_else(|| panic!("{} : une source LEGI nomme son code", id));
                let numero = s["article"].as_str().unwrap();
                let xml = article_legi(&chemin, code, numero, &mut legi);
                assert!(!xml.is_empty(), "{} : l'article {} {} n'est pas dans l'entrepôt déposé", id, code, numero);
                normaliser(&xml, true)
            } else {
                textes.entry(chemin.clone()).or_insert_with(|| normaliser(&String::from_utf8_lossy(&std::fs::read(&chemin).unwrap()), false)).clone()
            };
            let citation = s["citation"].as_str().unwrap_or_else(|| panic!("{} : source sans citation", id));
            let fr = fragments(citation);
            assert!(!fr.is_empty(), "{} : une citation trop courte ne prouve rien : {}", id, citation);
            for f in fr {
                if !texte.contains(&f) {
                    fautes.push(format!("{} ({}) : « {} »", id, fichier, f));
                }
            }
        }
    }
    assert!(fautes.is_empty(), "citations introuvables dans le texte déposé :\n{}", fautes.join("\n"));
}

#[test]
fn chaque_chiffre_date_se_lit_dans_les_donnees_du_moteur() {
    // Les gabarits des conseils généraux se résolvent sur le barème de l'année livrée et sur les minima : un chiffre
    // daté (plafond de la Sécurité sociale, taux Congés Spectacles, abattement, minima conventionnels) n'est pas écrit
    // en dur, il est lu. Le texte rendu ne garde aucune accolade.
    let b = banque();
    let d = Donnees::de_l_annee(&code_livre(), 2026);
    let mut avec_donnees = 0;
    for c in conseils(&b).iter().filter(|c| c["quand"] == "toujours") {
        let gabarit = c["texte"].as_str().unwrap();
        let rendu = rendre(gabarit, &d, &Valeurs::new()).unwrap_or_else(|| panic!("{} : un gabarit ne se résout pas : {}", c["id"], gabarit));
        assert!(!rendu.contains('{') && !rendu.contains('}'), "{} : gabarit resté dans le texte : {}", c["id"], rendu);
        if gabarit.contains("{bareme:") || gabarit.contains("{minima:") {
            avec_donnees += 1;
        }
    }
    assert!(avec_donnees > 0);
    // Le plafond mensuel de 2026 lu dans le barème (BOSS § 680, 4 005 €), et 1,18 fois lui (annexe X, art. 30).
    let plafond = conseils(&b).into_iter().find(|c| c["id"] == "plafond-cumul-118").unwrap();
    let rendu = rendre(plafond["texte"].as_str().unwrap(), &d, &Valeurs::new()).unwrap();
    assert!(rendu.contains("4\u{a0}725,90 € en 2026"), "{}", rendu);
}

#[test]
fn chaque_condition_est_evaluee_par_le_moteur_et_les_points_tranches_sont_dits() {
    let b = banque();
    let mut ids = std::collections::BTreeSet::new();
    for c in conseils(&b) {
        let id = c["id"].as_str().unwrap();
        assert!(ids.insert(id.to_string()), "identifiant en double : {}", id);
        let quand = c["quand"].as_str().unwrap();
        assert!(quand == "toujours" || CONDITIONS.contains(&quand), "{} : condition que le moteur ne sait pas évaluer : {}", id, quand);
        assert!(!c["texte"].as_str().unwrap().starts_with("Le saviez-vous"), "{} : le titre est celui de l'écran", id);
    }
    for q in CONDITIONS {
        assert!(conseils(&b).iter().any(|c| c["quand"] == q), "condition sans conseil : {}", q);
    }
    for q in b["ordre_saisie"].as_array().unwrap() {
        assert!(CONDITIONS.contains(&q.as_str().unwrap()), "ordre_saisie : {}", q);
    }
    // Les deux points que Léandre a tranchés le 21/10/2026 : le délai des signalements (« cinq jours ouvrés », CSS
    // R. 133-14 II, en vigueur depuis le 01/07/2026, que la banque cite) ; l'ouverture du droit Congés Spectacles (le
    // conseil qui opposait D. 7121-31 à Audiens est retiré, remplacé par ce que finance la cotisation).
    for id in ["signalement-fin-contrat-cinq-jours", "signalement-arret-cinq-jours"] {
        let c = conseils(&b).into_iter().find(|c| c["id"] == id).unwrap_or_else(|| panic!("{} absent", id));
        assert!(c.get("suspendu").is_none(), "{} est tranché", id);
        assert!(sources(&c).iter().any(|s| s["article"] == "R133-14"), "{} cite R. 133-14", id);
    }
    assert!(conseils(&b).iter().all(|c| c["id"] != "conges-spectacles-droit-ouvre-1er-jour"));
}

#[test]
fn un_conseil_sur_un_salarie_le_nomme_par_son_prenom_sans_le_genrer() {
    // Léandre, 21/10/2026 : « le prénom seul, pas de pronom genré ». Les conseils de saisie et d'historique nomment le
    // salarié par {prenom} (jamais {qui}, « NOM Prénom ») ; aucun ne le désigne par « il », « elle », ni par un mot qui
    // s'accorde à son genre. « il » n'y est qu'impersonnel (« il manque », « il faut ») ; un pluriel (« ils » : les
    // contrats) ne désigne jamais le salarié.
    let interdits = ["elle", "indemnisé", "indemnisée", "payé", "payée", "sûr", "sûre", "celui", "celle"];
    for c in conseils(&banque()) {
        let quand = c["quand"].as_str().unwrap();
        if quand == "toujours" {
            continue;
        }
        let id = c["id"].as_str().unwrap();
        for champ in ["texte", "detail"] {
            let Some(t) = c[champ].as_str() else { continue };
            assert!(!t.contains("{qui}"), "{} : {{qui}} au lieu de {{prenom}}", id);
            // Hors des gabarits : `{heures|heure payée|heures payées}` accorde le mot au nombre d'heures, pas au salarié.
            let mut hors = String::new();
            let mut dedans = false;
            for ch in t.chars() {
                match ch {
                    '{' => dedans = true,
                    '}' => dedans = false,
                    _ if !dedans => hors.push(ch),
                    _ => {}
                }
            }
            let mots: Vec<String> = hors
                .split(|ch: char| !(ch.is_alphanumeric() || ch == '\'' || ch == '’'))
                .map(|m| m.to_lowercase().replace('’', "'"))
                .collect();
            for (i, m) in mots.iter().enumerate() {
                let m = m.rsplit('\'').next().unwrap_or(m);
                assert!(!interdits.contains(&m), "{} ({}) : « {} » genre le salarié : {}", id, champ, m, t);
                if m == "il" {
                    let suivant = mots.get(i + 1).map(String::as_str).unwrap_or("");
                    assert!(["manque", "faut", "reste"].contains(&suivant), "{} ({}) : « il {} » : {}", id, champ, suivant, t);
                }
            }
        }
        if quand.starts_with("saisie:") || quand == "historique:resume" {
            continue;
        }
        assert!(c["texte"].as_str().unwrap().contains("{prenom}"), "{} : le salarié n'est pas nommé", id);
    }
    // Les conseils de saisie nomment le salarié dans leur phrase courte.
    for c in conseils(&banque()).iter().filter(|c| c["quand"].as_str().unwrap().starts_with("saisie:")) {
        assert!(c["texte"].as_str().unwrap().contains("{prenom}"), "{} : la phrase courte ne nomme pas le salarié", c["id"]);
    }
}
