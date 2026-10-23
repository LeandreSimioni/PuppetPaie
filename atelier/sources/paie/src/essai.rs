//! La COMPAGNIE D'ESSAI (lot E1, 15/10/2026) : un vrai depot de TEST -- net-entreprises pour la DSN et ses signalements,
//! l'Urssaf pour la DPAE -- depuis une machine qui n'a pas `prive/` (une seance dans le nuage, le poste d'un autre
//! developpeur). Inventaire : `prive/journal/finitions/TOPO-VARIABLES-ENVIRONNEMENT.md` du depot prive (14/10/2026).
//!
//! `paie essai monter [DOSSIER] [--depuis FICTIVE]` copie une compagnie FICTIVE livree (ses salaries inventes, ses paies,
//! son bareme, ses organismes) dans DOSSIER, hors de tout depot Git, et lui donne l'IDENTITE d'une vraie compagnie, lue dans
//! UNE variable d'environnement, `PUPPETPAIE_COMPAGNIE` : un objet JSON sur une ligne, fragment de `config/entreprise.json`,
//! avec EXACTEMENT les cles de `CLES` (decide par Leandre le 14/10/2026). Le reste de la seance nomme cette compagnie par
//! `PUPPETPAIE_DONNEES=DOSSIER` et se sert des commandes ordinaires (`paie mois`, `paie depot-dsn --test`, `paie dpae`,
//! `paie deposer-dpae --test`, `paie retours`).
//!
//! POURQUOI UNE COPIE, ET PAS LA VARIABLE LUE PARTOUT. Sur le poste, une variable posee par megarde ecraserait la vraie fiche ;
//! ici, la variable n'est lue qu'au montage, et seulement pour ECRIRE la fiche de la copie (regle 1 : rien ne se devine).
//!
//! CE QUI REND LE REEL IMPOSSIBLE PAR CE CHEMIN, TROIS FOIS :
//! - la copie porte le marqueur `COPIE-DE-DEVELOPPEMENT.json`, dont `copie_de` dit « compagnie d'essai » et NON
//!   « compagnie fictive » : ses depots de test vont donc au reseau (une fictive les joue), et la derniere porte avant le
//!   reseau (`depot_acte::refuser_si_copie`) refuse toute DSN ou DPAE reelle ;
//! - la copie n'a pas de coffre : `verifier_preuve` refuse un acte reel faute de mot de passe du logiciel (« Aucun mot de passe
//!   du logiciel n'est pose ») ;
//! - dans `paie-atelier`, tout acte reel est refuse avant toute autre question.
//!
//! CE QUI NE S'AFFICHE JAMAIS : une valeur de la variable. Ses refus nomment la CLE (« `declarant_dsn.civilite` manque »,
//! « `siren` : 9 chiffres attendus »), jamais ce qu'elle contient ; le montage dit « posee », pas plus. Les fichiers ecrits
//! dans la copie (la fiche, les DSN, les DPAE) portent l'identite : ils restent dans DOSSIER, que le montage refuse de placer
//! dans un depot Git et qu'il ferme par un `.gitignore`.
//!
//! CE QU'ON PREND DE LA FICTIVE, ET CE QU'ON N'EN PREND PAS. Tout ce qui n'est pas l'identite vient d'elle : salaries (NIR
//! inventes, cles justes), contrats, paies, spectacles, `organismes.json`, `banque`, bareme, convention. On n'en prend ni
//! son marqueur, ni ses retours (ce sont ceux de SON SIREN), ni son horloge (elle ne joue que dans une fictive), ni ses PDF ;
//! de sa fiche, tout ce qui dit son identite est remplace ou retire (`historique` compris : il pourrait redonner, a une
//! date passee, la raison sociale de la fictive). Les DSN deja ecrites sont gardees : celle d'un mois relit celles des mois
//! d'avant (numeros de contrat, regle 4). Le journal est garde SAUF ses depots de DSN : ceux-la ont ete joues sous le SIREN
//! de la fictive, la plate-forme ne les connait pas sous celui de la variable, et ils feraient refuser la DSN du mois
//! (« deja partie ») ou partir un essai a un ordre que rien ne justifie. Ses DPAE et ses contrats revenus signes restent :
//! ce sont eux qui disent qu'un mois peut se fermer (une compagnie d'essai ne depose aucune DPAE en reel).
//!
//! INFERENCES, A CONFIRMER AU PREMIER DEPOT DE TEST (son bilan fait foi, regle 10) : que la plate-forme d'essai accepte un
//! en-tete reel sur un contenu fictif (NIR inventes a cle juste, references d'adhesion et organismes de la fictive) ; que
//! la civilite, la qualite et l'adresse ne sont pas confrontees au repertoire SIRENE.

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

/// La variable : l'identite de la compagnie, un objet JSON sur une ligne.
pub const VARIABLE: &str = "PUPPETPAIE_COMPAGNIE";

/// Le marqueur d'une copie (`commandes::compagnie`, `interface::compagnies`).
const MARQUEUR: &str = "COPIE-DE-DEVELOPPEMENT.json";

/// Le debut de `copie_de` d'une compagnie d'essai. Ne commence PAS par « compagnie fictive » (`compagnies::MARQUE_FICTIVE`) :
/// une fictive joue ses depots, une compagnie d'essai les fait en test, pour de vrai.
pub const MARQUE_ESSAI: &str = "compagnie d'essai";

/// La fictive copiee quand `--depuis` ne dit rien : celle qui a du monde CHAQUE mois (des permanents et des intermittents,
/// IDCC 3090), pour qu'une DSN d'essai porte des individus quel que soit le mois.
pub const FICTIVE_PAR_DEFAUT: &str = "fictive-3090-grande";

/// Le nom du dossier quand la commande n'en donne pas : dans le dossier temporaire du systeme.
pub const DOSSIER_PAR_DEFAUT: &str = "puppetpaie-essai";

/// Les cles de la variable, et celles de ses trois objets (Leandre, 14/10/2026). Une cle qui manque ou qui est en trop est
/// refusee : une faute de frappe ne passe pas en silence.
pub const CLES: [&str; 11] =
    ["siren", "nic", "raison_sociale", "adresse", "code_postal", "commune", "naf", "medecine_travail", "declarant", "declarant_dsn", "api_dpae"];
const CLES_DECLARANT: [&str; 2] = ["nom", "prenom"];
const CLES_DECLARANT_DSN: [&str; 5] = ["civilite", "nom_prenom", "tel", "email", "qualite"];
const CLES_API_DPAE: [&str; 4] = ["nom", "prenom", "siret", "service"];
/// Celles qui peuvent etre vides : le SIRET du compte DPAE (par defaut, celui de la compagnie, comme `Client::dpae`) et son
/// code de service (facultatif dans l'adresse d'authentification, `releve_retours::adresses_dpae`).
const PEUVENT_ETRE_VIDES: [&str; 2] = ["api_dpae.siret", "api_dpae.service"];

/// Les rubriques de la DSN que chaque cle remplit (`dsn.rs` : l'en-tete S10.G00.01, le contact S10.G00.02 et S20.G00.07,
/// l'entreprise S21.G00.06 et l'etablissement S21.G00.11).
const RUBRIQUES: [(&str, &[&str]); 12] = [
    ("siren", &["S10.G00.01.001", "S21.G00.06.001"]),
    ("nic", &["S10.G00.01.002", "S21.G00.06.002", "S21.G00.11.001"]),
    ("raison_sociale", &["S10.G00.01.003"]),
    ("adresse", &["S10.G00.01.004", "S21.G00.06.004", "S21.G00.11.003"]),
    ("code_postal", &["S10.G00.01.005", "S21.G00.06.005", "S21.G00.11.004"]),
    ("commune", &["S10.G00.01.006", "S21.G00.06.006", "S21.G00.11.005"]),
    ("naf", &["S21.G00.06.003", "S21.G00.11.002"]),
    ("declarant_dsn.civilite", &["S10.G00.02.001"]),
    ("declarant_dsn.nom_prenom", &["S10.G00.02.002", "S20.G00.07.001"]),
    ("declarant_dsn.email", &["S10.G00.02.004", "S20.G00.07.003"]),
    ("declarant_dsn.tel", &["S10.G00.02.005", "S20.G00.07.002"]),
    ("declarant_dsn.qualite", &["S20.G00.07.004"]),
];

/// Ce que la fiche de la fictive dit de SON identite, et qui ne doit pas survivre sous celle de la variable : ce que la
/// variable remplace, plus ce qui en derive (l'affichage, le complement d'adresse, le telephone de repli de la DPAE).
const RETIREES: [&str; 8] = ["siret", "nom_affichage", "adresse_affichage", "commune_affichage", "complement_adresse", "tel", "etablissements", "_source"];

/// Ce que la copie ne prend pas de la fictive (relatif a sa racine).
const NON_COPIES: [&str; 4] = [MARQUEUR, "FICTIVE.md", "retours", "config/horloge.json"];

/// Une compagnie d'essai : son marqueur le dit.
pub fn est_essai(racine: &Path) -> bool {
    std::fs::read_to_string(racine.join(MARQUEUR))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("copie_de").and_then(Value::as_str).map(str::to_string))
        .is_some_and(|c| c.starts_with(MARQUE_ESSAI))
}

// ------------------------------------------------------------------ l'identite

/// L'identite lue dans la variable, validee. Ne s'affiche jamais (pas de `Debug`).
pub struct Identite(Map<String, Value>);

fn chiffres(t: &str, n: usize) -> bool {
    t.len() == n && t.bytes().all(|b| b.is_ascii_digit())
}

/// Les cles d'un objet, contre celles attendues : ce qui manque, ce qui est en trop (les NOMS, prefixes de `chemin`).
fn confronter(o: &Map<String, Value>, attendues: &[&str], chemin: &str, manques: &mut Vec<String>, inconnues: &mut Vec<String>) {
    let nom = |k: &str| if chemin.is_empty() { k.to_string() } else { format!("{}.{}", chemin, k) };
    manques.extend(attendues.iter().filter(|k| !o.contains_key(**k)).map(|k| nom(k)));
    inconnues.extend(o.keys().filter(|k| !attendues.contains(&k.as_str())).map(|k| nom(k)));
}

impl Identite {
    /// Lit et valide le texte de la variable. Le refus nomme la variable et la cle, JAMAIS une valeur : meme l'erreur de
    /// syntaxe ne rend que sa ligne et sa colonne.
    pub fn lire(texte: &str) -> Result<Identite, String> {
        let v: Value = serde_json::from_str(texte).map_err(|e| {
            format!("{} n'est pas un JSON lisible (ligne {}, colonne {}) : un objet sur une ligne est attendu. Rien n'est ecrit.", VARIABLE, e.line(), e.column())
        })?;
        let Value::Object(racine) = v else {
            return Err(format!("{} n'est pas un objet JSON ({{\"siren\": ..., ...}}). Rien n'est ecrit.", VARIABLE));
        };
        let (mut manques, mut inconnues) = (Vec::new(), Vec::new());
        confronter(&racine, &CLES, "", &mut manques, &mut inconnues);
        for (cle, attendues) in [("declarant", &CLES_DECLARANT[..]), ("declarant_dsn", &CLES_DECLARANT_DSN[..]), ("api_dpae", &CLES_API_DPAE[..])] {
            match racine.get(cle) {
                Some(Value::Object(o)) => confronter(o, attendues, cle, &mut manques, &mut inconnues),
                Some(_) => return Err(format!("{} : `{}` doit etre un objet ({}). Rien n'est ecrit.", VARIABLE, cle, attendues.join(", "))),
                None => {}
            }
        }
        if !manques.is_empty() || !inconnues.is_empty() {
            let mut p = Vec::new();
            if !manques.is_empty() {
                p.push(format!("manque : {}", manques.join(", ")));
            }
            if !inconnues.is_empty() {
                p.push(format!("cle(s) inconnue(s) : {}", inconnues.join(", ")));
            }
            return Err(format!("{} : {}. Les cles attendues sont celles de `paie essai monter --aide`. Rien n'est ecrit.", VARIABLE, p.join(" ; ")));
        }
        let id = Identite(racine);
        id.verifier()?;
        Ok(id)
    }

    /// Le texte d'une cle (`a.b` pour un objet), deja confrontee : un texte attendu.
    fn texte(&self, chemin: &str) -> Result<String, String> {
        let v = match chemin.split_once('.') {
            Some((o, k)) => self.0.get(o).and_then(|o| o.get(k)),
            None => self.0.get(chemin),
        };
        match v {
            Some(Value::String(s)) => Ok(s.trim().to_string()),
            _ => Err(format!("{} : `{}` doit etre un texte. Rien n'est ecrit.", VARIABLE, chemin)),
        }
    }

    /// Les formes que la norme et le moteur exigent, verifiees AVANT d'ecrire : un refus plus tard (a l'ecriture de la DSN,
    /// au depot) citerait la valeur fautive a l'ecran.
    fn verifier(&self) -> Result<(), String> {
        let refus = |chemin: &str, ce_qu_il_faut: &str| Err(format!("{} : `{}` -- {}. Rien n'est ecrit.", VARIABLE, chemin, ce_qu_il_faut));
        let mut vides = Vec::new();
        for (o, cles) in [("", &CLES[..8]), ("declarant", &CLES_DECLARANT[..]), ("declarant_dsn", &CLES_DECLARANT_DSN[..]), ("api_dpae", &CLES_API_DPAE[..])] {
            for k in cles {
                let chemin = if o.is_empty() { k.to_string() } else { format!("{}.{}", o, k) };
                if self.texte(&chemin)?.is_empty() && !PEUVENT_ETRE_VIDES.contains(&chemin.as_str()) {
                    vides.push(chemin);
                }
            }
        }
        if !vides.is_empty() {
            return Err(format!("{} : vide(s) : {}. Rien n'est ecrit.", VARIABLE, vides.join(", ")));
        }
        let siren = self.texte("siren")?;
        if !chiffres(&siren, 9) || !crate::champs::luhn(&siren) {
            return refus("siren", "9 chiffres attendus, cle de Luhn juste");
        }
        let nic = self.texte("nic")?;
        if !chiffres(&nic, 5) || !crate::champs::luhn(&format!("{}{}", siren, nic)) {
            return refus("nic", "5 chiffres attendus, et le SIRET (siren + nic) a cle de Luhn juste");
        }
        let siret_dpae = self.texte("api_dpae.siret")?;
        if !siret_dpae.is_empty() && (!chiffres(&siret_dpae, 14) || !crate::champs::luhn(&siret_dpae)) {
            return refus("api_dpae.siret", "14 chiffres attendus, cle de Luhn juste (ou vide : le SIRET de la compagnie)");
        }
        if !chiffres(&self.texte("code_postal")?, 5) {
            return refus("code_postal", "5 chiffres attendus");
        }
        if !matches!(self.texte("declarant_dsn.civilite")?.as_str(), "01" | "02") {
            return refus("declarant_dsn.civilite", "01 (Monsieur) ou 02 (Madame), cahier technique DSN, S10.G00.02.001");
        }
        // Chaque rubrique de la DSN que la cle remplit, a son format (longueur, expression : `formats-dsn-*.json`, relevees
        // du cahier technique) : la DSN refuserait plus tard EN CITANT la valeur. La qualite, par exemple, est un CODE de
        // deux caracteres (S20.G00.07.004, « Type » du contact : 04 pour les cotisations de Securite sociale ; cahier
        // technique 2026.1, p. 139, `norme-dsn/texte/bloc-S20.G00.07.txt`), pas un intitule.
        for (cle, rubriques) in RUBRIQUES {
            let t = self.texte(cle)?;
            for r in rubriques {
                let pourquoi = crate::formats::mauvaise(r, &t)?;
                if !pourquoi.is_empty() {
                    return refus(cle, &format!("ne tient pas dans {} de la DSN : {}", r, pourquoi));
                }
            }
        }
        // C_MT_DCL, dix caracteres dans le gabarit de la DPAE (`dpae::employeur`) : plus long, il serait tronque sans un mot.
        if self.texte("medecine_travail")?.chars().count() > 10 {
            return refus("medecine_travail", "10 caracteres au plus (C_MT_DCL de la DPAE)");
        }
        // La DUE MtoM refuse les accents (`depot::identifiants`, qui citerait la valeur : on refuse ici, avant).
        for k in ["api_dpae.nom", "api_dpae.prenom"] {
            let t = self.texte(k)?;
            if t != crate::dpae::ascii_nfkd(&t) {
                return refus(k, "sans accent, tel que le compte DPAE est ouvert sur urssaf.fr (la DUE MtoM refuse les accents)");
            }
        }
        Ok(())
    }

    /// La fiche de la fictive, sous cette identite : les cles de la variable posees, ce qui en derive recompose (le SIRET, le
    /// declarant net-entreprises, le compte DPAE), ce qui disait l'identite de la fictive retire.
    pub fn appliquer(&self, fiche: &Value) -> Result<Value, String> {
        let mut f = fiche.as_object().cloned().ok_or_else(|| "config/entreprise.json de la fictive n'est pas un objet.".to_string())?;
        for k in RETIREES {
            f.remove(k);
        }
        // `historique` : une valeur datee d'un champ d'identite redonnerait celle de la fictive a une date passee.
        if let Some(Value::Array(h)) = f.get_mut("historique") {
            h.retain(|e| e.get("champ").and_then(Value::as_str).is_none_or(|c| !CLES.contains(&c) && !RETIREES.contains(&c)));
        }
        let (siren, nic) = (self.texte("siren")?, self.texte("nic")?);
        let siret = format!("{}{}", siren, nic);
        for k in &CLES[..8] {
            f.insert(k.to_string(), json!(self.texte(k)?));
        }
        f.insert("siret".into(), json!(siret));
        f.insert("nom_affichage".into(), json!("Compagnie d'essai"));
        // Le declarant inscrit au service DSN, tel qu'inscrit : `transport::declarant` prend cette cle telle quelle (sans
        // couper `nom_prenom` au premier espace, faux pour un nom compose).
        f.insert("declarant".into(), json!({"siret": siret, "nom": self.texte("declarant.nom")?, "prenom": self.texte("declarant.prenom")?}));
        let mut contact = Map::new();
        for k in CLES_DECLARANT_DSN {
            contact.insert(k.into(), json!(self.texte(&format!("declarant_dsn.{}", k))?));
        }
        f.insert("declarant_dsn".into(), Value::Object(contact));
        let siret_dpae = self.texte("api_dpae.siret")?;
        let mut dpae = Map::new();
        dpae.insert("nom".into(), json!(self.texte("api_dpae.nom")?));
        dpae.insert("prenom".into(), json!(self.texte("api_dpae.prenom")?));
        dpae.insert("siret".into(), json!(if siret_dpae.is_empty() { siret.clone() } else { siret_dpae }));
        let service = self.texte("api_dpae.service")?;
        if !service.is_empty() {
            dpae.insert("service".into(), json!(service));
        }
        f.insert("api_dpae".into(), Value::Object(dpae));
        f.insert(
            "_source".into(),
            json!(format!("COMPAGNIE D'ESSAI (paie essai monter) : l'identite vient de {} ; le reste, de la compagnie fictive copiee. Ne se commite jamais.", VARIABLE)),
        );
        Ok(Value::Object(f))
    }
}

// ------------------------------------------------------------------ le montage

/// Ce que le montage a fait (des chemins et des noms ; aucune valeur de la variable).
#[derive(Debug)]
pub struct Montage {
    pub dossier: PathBuf,
    pub depuis: String,
    /// `false` : la copie existait deja, seule son identite a ete reecrite.
    pub neuf: bool,
}

/// Le dossier d'installation : celui qui porte `logiciel/` (`logiciel/donnees` -> `logiciel` -> l'installation).
fn installation() -> PathBuf {
    crate::fichiers::dossier_du_code().parent().and_then(Path::parent).map(Path::to_path_buf).unwrap_or_default()
}

/// La fictive a copier : un chemin (s'il designe un dossier), sinon un nom de `fictives/` de l'installation.
fn source(depuis: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(depuis);
    let d = if depuis.contains('/') || depuis.contains('\\') || p.is_dir() { p } else { installation().join(crate::interface::compagnies::DOSSIER_FICTIVES).join(depuis) };
    if !d.join("config").join("entreprise.json").is_file() {
        return Err(format!("{} : ce n'est pas une compagnie (pas de config/entreprise.json). Rien n'est ecrit.", d.display()));
    }
    // Seulement une FICTIVE : jamais `prive/` ni une autre compagnie reelle, dont les salaries partiraient sous un autre SIREN.
    if !crate::interface::compagnies::est_fictive_livree(&d) {
        return Err(format!("{} n'est pas une compagnie fictive (son marqueur ne le dit pas) : une compagnie d'essai ne se monte que depuis une fictive. Rien n'est ecrit.", d.display()));
    }
    Ok(d)
}

/// Le chemin absolu, sans `..`, meme s'il n'existe pas encore (on remonte au premier ancetre qui existe).
fn absolu(p: &Path) -> PathBuf {
    let p = if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir().unwrap_or_default().join(p) };
    let mut reste = Vec::new();
    let mut a = p.as_path();
    loop {
        if let Ok(c) = a.canonicalize() {
            let mut c = c;
            for r in reste.iter().rev() {
                c.push(r);
            }
            return c;
        }
        match (a.file_name(), a.parent()) {
            (Some(n), Some(parent)) => {
                reste.push(n.to_os_string());
                a = parent;
            }
            _ => return p,
        }
    }
}

/// Le depot Git ou l'installation qui contient `dossier`, s'il y en a un : la copie porterait l'identite d'une vraie compagnie,
/// elle ne va jamais la ou un `git add` la prendrait, ni a cote de ce que les gens recoivent.
fn depot_englobant(dossier: &Path) -> Option<PathBuf> {
    absolu(dossier).ancestors().find(|a| a.join(".git").exists() || a.join("logiciel").join("donnees").join("echeancier-base.json").is_file()).map(Path::to_path_buf)
}

fn copier(src: &Path, dst: &Path, racine: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("{} : {}", dst.display(), e))?;
    for e in std::fs::read_dir(src).map_err(|e| format!("{} : {}", src.display(), e))?.flatten() {
        let s = e.path();
        let rel = s.strip_prefix(racine).unwrap_or(&s).to_string_lossy().replace('\\', "/");
        if NON_COPIES.contains(&rel.as_str()) || s.extension().is_some_and(|x| x.eq_ignore_ascii_case("pdf")) {
            continue;
        }
        let d = dst.join(e.file_name());
        if s.is_dir() {
            copier(&s, &d, racine)?;
        } else {
            std::fs::copy(&s, &d).map_err(|x| format!("{} : {}", s.display(), x))?;
        }
    }
    Ok(())
}

/// Le journal de la copie sans les depots de DSN de la fictive (mensuelles et signalements, essais et reels) : voir l'en-tete.
fn retirer_les_depots_de_dsn(dossier: &Path) -> Result<(), String> {
    let p = dossier.join("journal").join("depots.json");
    if !p.is_file() {
        return Ok(());
    }
    let Value::Array(lignes) = crate::fichiers::lire_json(&p)? else {
        return Err(format!("{} : une liste est attendue.", p.display()));
    };
    let gardees: Vec<Value> = lignes.into_iter().filter(|l| l.get("quoi").and_then(Value::as_str) != Some("dsn")).collect();
    crate::ecriture::ecrire_json(&p, &Value::Array(gardees), 2)
}

/// Monte (ou remet a jour) la compagnie d'essai dans `dossier`, depuis la fictive `depuis`, sous l'identite `texte` (le
/// contenu de la variable, `None` si elle est absente). Tout se verifie AVANT d'ecrire : un refus ne laisse rien.
///
/// Une copie qui existe deja et qui est une compagnie d'essai garde tout (journal, DSN, retours releves) : seule sa fiche
/// reprend l'identite de la variable. Un dossier qui existe et qui n'en est pas une, s'il n'est pas vide, est refuse.
pub fn monter(dossier: &Path, depuis: &str, texte: Option<&str>) -> Result<Montage, String> {
    let texte = texte.filter(|t| !t.trim().is_empty()).ok_or_else(|| {
        format!("{} absente : elle porte l'identite de la compagnie (un objet JSON sur une ligne ; ses cles : `paie essai monter --aide`). La poser dans les reglages de l'environnement, puis ouvrir une nouvelle seance. Rien n'est ecrit.", VARIABLE)
    })?;
    let id = Identite::lire(texte)?;
    if let Some(d) = depot_englobant(dossier) {
        return Err(format!(
            "{} est dans {} (un depot Git ou une installation de PuppetPaie) : la compagnie d'essai porte l'identite d'une vraie compagnie, elle se monte hors de tout depot (par exemple dans le dossier temporaire, le defaut). Rien n'est ecrit.",
            dossier.display(),
            d.display()
        ));
    }
    let existe = dossier.exists();
    let deja = existe && est_essai(dossier);
    if existe && !deja && std::fs::read_dir(dossier).map(|mut d| d.next().is_some()).unwrap_or(true) {
        return Err(format!("{} existe et n'est pas une compagnie d'essai : choisir un dossier vide ou neuf. Rien n'est ecrit.", dossier.display()));
    }
    let (src, nom) = if deja {
        // La copie dit de quelle fictive elle vient ; `--depuis` ne la change pas (il faudrait une copie neuve).
        let fiche = crate::fichiers::lire_json(&dossier.join(MARQUEUR)).unwrap_or(Value::Null);
        (None, fiche.get("depuis").and_then(Value::as_str).unwrap_or("?").to_string())
    } else {
        let s = source(depuis)?;
        let nom = s.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        (Some(s), nom)
    };
    // La fiche sous la nouvelle identite, calculee AVANT toute ecriture.
    let base_fiche = match &src {
        Some(s) => s.join("config").join("entreprise.json"),
        None => dossier.join("config").join("entreprise.json"),
    };
    let fiche = crate::fichiers::lire_json(&base_fiche).map_err(|e| format!("{} : {}", base_fiche.display(), e))?;
    let fiche = id.appliquer(&fiche)?;

    if let Some(s) = &src {
        copier(s, dossier, s)?;
        retirer_les_depots_de_dsn(dossier)?;
        let quand = crate::paris::iso_secondes(crate::temps::secondes());
        let marqueur = json!({
            "copie_de": format!("{} (paie essai monter, identite de {}), depuis la compagnie fictive {}", MARQUE_ESSAI, VARIABLE, nom),
            "depuis": nom,
            "quand": quand,
            "_quoi": "Compagnie d'ESSAI : ses depots de TEST vont au reseau, sous l'identite d'une vraie compagnie ; tout acte reel est refuse (depot_acte::refuser_si_copie, et pas de coffre). Ne se commite jamais.",
        });
        crate::ecriture::ecrire_json(&dossier.join(MARQUEUR), &marqueur, 2)?;
        std::fs::write(dossier.join(".gitignore"), "# Compagnie d'essai : l'identite d'une vraie compagnie. Rien ici ne se commite.\n*\n")
            .map_err(|e| format!("{} : {}", dossier.display(), e))?;
    }
    crate::ecriture::ecrire_json(&dossier.join("config").join("entreprise.json"), &fiche, 2)?;
    Ok(Montage { dossier: absolu(dossier), depuis: nom, neuf: !deja })
}

// ------------------------------------------------------------------ la commande

const AIDE: &str = "usage : paie essai monter [DOSSIER] [--depuis FICTIVE]

Monte une COMPAGNIE D'ESSAI : la copie d'une compagnie fictive (defaut : fictive-3090-grande, dans fictives/ de
l'installation ; ou un chemin), sous l'identite d'une vraie compagnie lue dans la variable PUPPETPAIE_COMPAGNIE.
Ses depots de TEST vont au reseau (net-entreprises, Urssaf) ; tout depot reel y est refuse.
DOSSIER : hors de tout depot Git (defaut : <dossier temporaire>/puppetpaie-essai). Deja monte, seule l'identite
est reecrite (journal, DSN et retours gardes).

PUPPETPAIE_COMPAGNIE : un objet JSON sur une ligne, avec exactement ces cles (ses valeurs ne s'affichent jamais) :
  siren, nic, raison_sociale, adresse, code_postal, commune, naf, medecine_travail,
  declarant      {nom, prenom}                              (inscrit au service DSN de net-entreprises)
  declarant_dsn  {civilite, nom_prenom, tel, email, qualite} (la personne que les organismes contactent ;
                 civilite 01 ou 02, qualite : le code du type de contact, S20.G00.07.004, par exemple 04)
  api_dpae       {nom, prenom, siret, service}               (le compte DPAE d'urssaf.fr ; siret et service peuvent etre vides)
Les mots de passe restent dans NET_ENTREPRISES_MOTDEPASSE et URSSAF_MOTDEPASSE.

Puis, avec PUPPETPAIE_DONNEES=DOSSIER : paie mois, paie depot-dsn --test, paie retours, paie dpae, paie deposer-dpae --test
(la suite : atelier/docs/COMPAGNIE-D-ESSAI.md).";

/// `paie essai ...` : rend le code de sortie.
pub fn commande(args: &[String]) -> i32 {
    let mut it = args.iter();
    match it.next().map(String::as_str) {
        Some("monter") => {}
        Some("-h" | "--help" | "--aide" | "aide") => {
            println!("{}", AIDE);
            return 0;
        }
        _ => {
            eprintln!("{}", AIDE);
            return 2;
        }
    }
    let (mut dossier, mut depuis): (Option<PathBuf>, String) = (None, FICTIVE_PAR_DEFAUT.to_string());
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "--aide" => {
                println!("{}", AIDE);
                return 0;
            }
            "--depuis" => match it.next() {
                Some(v) => depuis = v.clone(),
                None => {
                    eprintln!("paie essai monter : --depuis attend une fictive.\n\n{}", AIDE);
                    return 2;
                }
            },
            x if x.starts_with("--") => {
                eprintln!("paie essai monter : option inconnue : {}\n\n{}", x, AIDE);
                return 2;
            }
            x if dossier.is_none() => dossier = Some(PathBuf::from(x)),
            x => {
                eprintln!("paie essai monter : un seul dossier ({} en trop).\n\n{}", x, AIDE);
                return 2;
            }
        }
    }
    crate::commandes::compagnie::fixer_le_code();
    let dossier = dossier.unwrap_or_else(|| std::env::temp_dir().join(DOSSIER_PAR_DEFAUT));
    let texte = std::env::var(VARIABLE).ok();
    match monter(&dossier, &depuis, texte.as_deref()) {
        Ok(m) => {
            let d = m.dossier.display();
            if m.neuf {
                println!("Compagnie d'essai montee dans {}, depuis la compagnie fictive {}.", d, m.depuis);
            } else {
                println!("Compagnie d'essai {} (depuis {}) : identite reecrite ; journal, DSN et retours gardes.", d, m.depuis);
            }
            println!("Identite : {} (posee ; ses valeurs ne s'affichent pas). Tout depot reel y est refuse.", VARIABLE);
            println!();
            println!("La suite, avec PUPPETPAIE_DONNEES={} :", d);
            println!("  paie mois paies/AAAA-MM.json       puis   paie depot-dsn sorties/AAAA-MM/<DSN>.dsn --test   puis   paie retours");
            println!("  paie dpae paies/AAAA-MM.json       puis   paie deposer-dpae sorties/AAAA-MM/<DPAE>.txt --test");
            println!("(atelier/docs/COMPAGNIE-D-ESSAI.md : l'ordre d'un essai deja fait d'ailleurs, les signalements)");
            0
        }
        Err(m) => {
            eprintln!("{}", m);
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une identite inventee (SIREN et SIRET a cle de Luhn juste).
    fn juste() -> Value {
        json!({
            "siren": "987654324", "nic": "00019", "raison_sociale": "CIE ESSAI", "adresse": "1 RUE", "code_postal": "75011",
            "commune": "PARIS", "naf": "9001Z", "medecine_travail": "MT1",
            "declarant": {"nom": "NOMDECL", "prenom": "PRENOMDECL"},
            "declarant_dsn": {"civilite": "01", "nom_prenom": "NOMCONTACT", "tel": "0100000000", "email": "a@b.invalid", "qualite": "04"},
            "api_dpae": {"nom": "NOMDPAE", "prenom": "PRENOMDPAE", "siret": "", "service": ""}
        })
    }

    fn refus(v: &Value) -> String {
        match Identite::lire(&v.to_string()) {
            Ok(_) => panic!("accepte : devait refuser"),
            Err(m) => m,
        }
    }

    #[test]
    fn l_identite_juste_se_lit() {
        assert!(Identite::lire(&juste().to_string()).is_ok());
    }

    /// Chaque forme exigee refuse en nommant la cle, sans citer la valeur fautive.
    #[test]
    fn chaque_forme_fausse_refuse_en_nommant_la_cle_sans_la_valeur() {
        let cas: [(&str, &str, Value); 11] = [
            ("siren", "siren", json!("987654321")),
            ("siren", "siren", json!("98765432")),
            ("nic", "nic", json!("00018")),
            ("code_postal", "code_postal", json!("7501A")),
            ("declarant_dsn.civilite", "declarant_dsn/civilite", json!("03")),
            ("medecine_travail", "medecine_travail", json!("CENTRE-ONZE-X")),
            ("api_dpae.nom", "api_dpae/nom", json!("NOMDPAÉ")),
            ("api_dpae.siret", "api_dpae/siret", json!("98765432400018")),
            ("raison_sociale", "raison_sociale", json!(42)),
            ("declarant_dsn.qualite", "declarant_dsn/qualite", json!("GERANTE")),
            ("commune", "commune", json!("SAINT-ETIENNE-DU-ROUVRAY")),
        ];
        for (cle, pointeur, fautive) in cas {
            let mut v = juste();
            *v.pointer_mut(&format!("/{}", pointeur)).unwrap() = fautive.clone();
            let m = refus(&v);
            assert!(m.contains(&format!("`{}`", cle)) && m.contains(VARIABLE), "{} : {}", cle, m);
            if let Some(t) = fautive.as_str() {
                assert!(!m.contains(t), "{} : la valeur est citee : {}", cle, m);
            }
        }
    }

    #[test]
    fn une_valeur_vide_refuse_sauf_le_siret_et_le_service_du_compte_dpae() {
        let mut v = juste();
        v["declarant"]["prenom"] = json!(" ");
        v["naf"] = json!("");
        let m = refus(&v);
        assert!(m.contains("declarant.prenom") && m.contains("naf"), "{}", m);
        assert!(!m.contains("api_dpae"), "{}", m);
    }

    #[test]
    fn un_objet_attendu_qui_n_en_est_pas_un_refuse() {
        let mut v = juste();
        v["declarant"] = json!("NOMDECL PRENOMDECL");
        assert!(refus(&v).contains("`declarant` doit etre un objet"));
        assert!(Identite::lire("[1, 2]").is_err());
    }

    /// La fiche de la fictive sous l'identite : les cles posees, le SIRET compose, le service DPAE pose s'il est dit, et ce que
    /// la fiche datait de son ancienne identite retire, le reste de son historique garde.
    #[test]
    fn appliquer_pose_l_identite_et_retire_celle_de_la_fictive() {
        let mut v = juste();
        v["api_dpae"]["service"] = json!("25");
        let id = Identite::lire(&v.to_string()).unwrap();
        let fiche = json!({
            "siren": "123456915", "siret": "12345691500010", "raison_sociale": "CIE FICTIVE", "nom_affichage": "Fictive",
            "complement_adresse": "BAT A", "tel": "0100000001", "idcc": "3090", "etablissements": null,
            "declarant_dsn": {"nom_prenom": "SAND GEORGE", "tel": "0100000092"},
            "historique": [
                {"champ": "raison_sociale", "valeur": "ANCIENNE FICTIVE", "jusqu_au": "2026-01-01"},
                {"champ": "idcc", "valeur": "1285", "jusqu_au": "2026-01-01"}
            ]
        });
        let f = id.appliquer(&fiche).unwrap();
        assert_eq!(f["siret"], json!("98765432400019"));
        assert_eq!(f["idcc"], json!("3090"));
        assert_eq!(f["api_dpae"], json!({"nom": "NOMDPAE", "prenom": "PRENOMDPAE", "siret": "98765432400019", "service": "25"}));
        assert_eq!(f["declarant"], json!({"siret": "98765432400019", "nom": "NOMDECL", "prenom": "PRENOMDECL"}));
        for k in ["complement_adresse", "tel", "etablissements"] {
            assert!(f.get(k).is_none(), "{}", k);
        }
        assert_eq!(f["historique"], json!([{"champ": "idcc", "valeur": "1285", "jusqu_au": "2026-01-01"}]));
        assert!(!f.to_string().contains("FICTIVE") && !f.to_string().contains("SAND"), "{}", f);
    }
}
