//! La preparation des depots, sans reseau.
//!
//! Ce que le fichier declare (`lire`), le passage en test en memoire
//! (`basculer_dsn`, `renumeroter`, `basculer_dpae`), les garde-fous du depot
//! (`verifier`) et le controle des identifiants du declarant DPAE. Un depot de
//! test ne part jamais avec la nature `02` ni l'indicateur `120` : la bascule
//! les remplace avant tout envoi, et `verifier` refuse un test demande sur un
//! fichier reel comme un reel demande sur un fichier de test. Le transport
//! est ailleurs (`transport`).

use serde_json::Value;

use socle::texte::ascii_nfkd;
use socle::fichiers::vrai;
use crate::retours::{est_chiffre};
use socle::echec::Echec;
use socle::texte::{lignes_py as lignes, strip};

type Resultat<T> = Result<T, Echec>;

pub const NATURE_TEST: &str = "01";
pub const NATURE_REEL: &str = "02";
pub const RUB_NATURE: &str = "S10.G00.00.005";
pub const MENSUELLE: &str = "01";
pub use declarations::norme::SIGNALEMENTS;

/// Les rubriques relues dans le fichier, dans l'ordre des champs de l'apercu.
/// `dernier_jour`, `fin_contrat` et `debut_contrat` datent un signalement
/// (arret, fin de contrat, amorcage), que le mois principal ne date pas ;
/// `etablissement` suit les declarations d'une compagnie a plusieurs.
pub const LECTURE: [(&str, &str); 14] = [
    ("nature_envoi", RUB_NATURE),
    ("logiciel", "S10.G00.00.001"),
    ("siren", "S10.G00.01.001"),
    ("nature", "S20.G00.05.001"),
    ("type_declaration", "S20.G00.05.002"),
    ("fraction", "S20.G00.05.003"),
    ("ordre", "S20.G00.05.004"),
    ("mois", "S20.G00.05.005"),
    ("remplace", "S20.G00.05.006"),
    ("constitution", "S20.G00.05.007"),
    ("dernier_jour", "S21.G00.60.002"),
    ("fin_contrat", "S21.G00.62.001"),
    ("debut_contrat", "S21.G00.40.001"),
    ("etablissement", "S21.G00.11.001"),
];

pub const NATURES: [(&str, &str); 7] = [
    ("01", "DSN mensuelle"),
    ("04", "signalement arret de travail"),
    ("05", "signalement reprise suite a arret de travail"),
    ("07", "signalement fin de contrat de travail unique"),
    ("08", "signalement amorcage des donnees variables"),
    ("09", "DSN de substitution"),
    ("10", "signalement declaration prealable a l'embauche"),
];

pub const TYPES: [(&str, &str); 5] = [
    ("01", "declaration normale"),
    ("02", "declaration normale sans individu"),
    ("03", "annule et remplace integral"),
    ("04", "declaration annule"),
    ("05", "annule et remplace sans individu"),
];

/// Le champ C_VERSION de l'enregistrement DPAE a plat : positions 72 a 75.
pub const INDICATEUR: (usize, usize) = (72, 75);
pub const INDICATEUR_REEL: &str = "120";
pub const INDICATEUR_TEST: &str = "TST";
pub const LONGUEUR_DPAE: i64 = 990;

#[track_caller]
fn refus(classe: &'static str, message: impl Into<String>) -> Echec {
    socle::echec::noter_et(Some(classe), message)
}

#[track_caller]
fn depot_refuse(message: impl Into<String>) -> Echec {
    refus("DepotRefuse", message)
}

// ------------------------------------------------------------- l'apercu

/// Ce qu'un fichier DSN declare : `dsn_api.Apercu` sans son chemin.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Apercu {
    pub champs: Vec<(&'static str, String)>,
    pub salaries: usize,
    pub versements: Vec<String>,
    pub lignes: usize,
    pub pas_versements: usize,
    pub pas_bareme: usize,
}

impl Apercu {
    pub fn champ(&self, nom: &str) -> &str {
        self.champs.iter().find(|(n, _)| *n == nom).map_or("", |(_, v)| v.as_str())
    }

    pub fn reel(&self) -> bool {
        self.champ("nature_envoi") == NATURE_REEL
    }

    fn nature_ou_mensuelle(&self) -> &str {
        let n = self.champ("nature");
        if n.is_empty() {
            MENSUELLE
        } else {
            n
        }
    }

    pub fn est_signalement(&self) -> bool {
        SIGNALEMENTS.contains(&self.nature_ou_mensuelle())
    }

    pub fn nature_lisible(&self) -> String {
        let n = self.nature_ou_mensuelle();
        match NATURES.iter().find(|(c, _)| *c == n) {
            Some((_, l)) => l.to_string(),
            None => format!("nature {}", self.champ("nature")),
        }
    }

    /// `01082026` -> `2026-08` ; un signalement se date par son evenement.
    pub fn mois_lisible(&self) -> String {
        let mois = self.champ("mois");
        let c: Vec<char> = mois.chars().collect();
        if c.len() == 8 && c[0] == '0' && c[1] == '1' && c[2..].iter().all(|&x| est_chiffre(x)) {
            return format!("{}-{}", c[4..].iter().collect::<String>(), c[2..4].iter().collect::<String>());
        }
        if self.est_signalement() {
            return self.evenement_lisible();
        }
        if mois.is_empty() {
            "?".to_string()
        } else {
            mois.to_string()
        }
    }

    /// `05102026` -> `2026-10-05` : l'arret par son dernier jour travaille, la fin
    /// de contrat par sa date de fin, l'amorcage par le debut du contrat, un
    /// amorcage de changement de logiciel par sa constitution.
    pub fn evenement_lisible(&self) -> String {
        let mut jour = "";
        for s in [self.champ("dernier_jour"), self.champ("fin_contrat"), self.champ("debut_contrat")] {
            if !s.is_empty() {
                jour = s;
                break;
            }
        }
        if jour.is_empty() && self.champ("nature") == "08" {
            jour = self.champ("constitution");
        }
        let c: Vec<char> = jour.chars().collect();
        if c.len() == 8 && c.iter().all(|&x| est_chiffre(x)) {
            let t = |a: usize, b: usize| c[a..b].iter().collect::<String>();
            format!("{}-{}-{}", t(4, 8), t(2, 4), t(0, 2))
        } else {
            "?".to_string()
        }
    }

    /// Ce qu'on fait recopier pour prouver qu'on a regarde.
    pub fn confirmation_attendue(&self) -> String {
        if self.est_signalement() {
            self.evenement_lisible()
        } else {
            self.mois_lisible()
        }
    }
}

/// `Path.read_text(encoding="latin-1")` : un octet, un caractere, et les fins de
/// ligne `\r\n` et `\r` lues comme `\n`.
fn texte_latin1(o: &[u8]) -> String {
    let mut s = String::with_capacity(o.len());
    let mut i = 0;
    while i < o.len() {
        if o[i] == b'\r' {
            s.push('\n');
            if o.get(i + 1) == Some(&b'\n') {
                i += 1;
            }
        } else {
            s.push(o[i] as char);
        }
        i += 1;
    }
    s
}

/// `re.match(r"(S\d\d\.G\d\d\.\d\d\.\d\d\d),'(.*)'$", ligne)` : la rubrique et sa valeur.
fn rubrique_de(ligne: &str) -> Option<(&str, &str)> {
    let c: Vec<char> = ligne.chars().take(16).collect();
    let chiffres = [1, 2, 5, 6, 8, 9, 11, 12, 13];
    if c.len() < 16
        || c[0] != 'S'
        || c[3] != '.'
        || c[4] != 'G'
        || c[7] != '.'
        || c[10] != '.'
        || c[14] != ','
        || c[15] != '\''
        || !chiffres.iter().all(|&i| c[i].is_ascii_digit())
    {
        return None;
    }
    let reste = &ligne[16..];
    let valeur = reste.strip_suffix('\'')?;
    Some((&ligne[..14], valeur))
}

/// `_rubriques` : la premiere valeur de chaque rubrique.
fn rubriques(texte: &str) -> Vec<(&str, &str)> {
    let mut out: Vec<(&str, &str)> = Vec::new();
    for l in lignes(texte) {
        if let Some((r, v)) = rubrique_de(l) {
            if !out.iter().any(|(x, _)| *x == r) {
                out.push((r, v));
            }
        }
    }
    out
}

/// `re.findall(r"^<rubrique>,'(.*)'$", texte, re.M)` : une ligne (coupee sur
/// `\n` seul) qui commence par `<rubrique>,'` et finit par `'`.
fn valeurs_de(texte: &str, rubrique: &str) -> Vec<String> {
    let debut = format!("{},'", rubrique);
    texte
        .split('\n')
        .filter_map(|l| l.strip_prefix(debut.as_str())?.strip_suffix('\'').map(str::to_string))
        .collect()
}

/// `dsn_api.lire`, sur les octets du fichier.
pub fn lire(octets: &[u8]) -> Apercu {
    let texte = texte_latin1(octets);
    let r = rubriques(&texte);
    let champs = LECTURE
        .iter()
        .map(|(nom, rub)| (*nom, r.iter().find(|(x, _)| x == rub).map_or(String::new(), |(_, v)| v.to_string())))
        .collect();
    let types = valeurs_de(&texte, "S21.G00.50.007");
    Apercu {
        champs,
        salaries: declarations::ntt::individus(&texte).len(),
        versements: valeurs_de(&texte, "S21.G00.20.005"),
        lignes: lignes(&texte).len(),
        pas_versements: types.len(),
        pas_bareme: types.iter().filter(|x| *x != "01").count(),
    }
}

// ------------------------------------------------ la bascule en test, en memoire

/// Position du premier `prefixe` que `ok` accepte, avec la longueur du motif.
fn premier(octets: &[u8], prefixe: &[u8], ok: impl Fn(&[u8]) -> Option<usize>) -> Option<(usize, usize)> {
    let mut i = 0;
    while i + prefixe.len() <= octets.len() {
        if &octets[i..i + prefixe.len()] == prefixe {
            if let Some(n) = ok(&octets[i + prefixe.len()..]) {
                return Some((i, prefixe.len() + n));
            }
        }
        i += 1;
    }
    None
}

fn remplacer(octets: &[u8], debut: usize, longueur: usize, par: &[u8]) -> Vec<u8> {
    let mut s = Vec::with_capacity(octets.len() + par.len());
    s.extend_from_slice(&octets[..debut]);
    s.extend_from_slice(par);
    s.extend_from_slice(&octets[debut + longueur..]);
    s
}

/// `<rubrique>,'\d\d'` (chiffres ASCII, comme un motif d'octets) : la longueur de `\d\d'`.
fn deux_chiffres(reste: &[u8]) -> Option<usize> {
    (reste.len() >= 3 && reste[0].is_ascii_digit() && reste[1].is_ascii_digit() && reste[2] == b'\'').then_some(3)
}

/// `basculer_en_test` : S10.G00.00.005 repasse a `01`, jamais sur le disque.
pub fn basculer_dsn(octets: &[u8]) -> Resultat<Vec<u8>> {
    let prefixe = format!("{},'", RUB_NATURE);
    match premier(octets, prefixe.as_bytes(), deux_chiffres) {
        Some((i, n)) => Ok(remplacer(octets, i, n, format!("{},'{}'", RUB_NATURE, NATURE_TEST).as_bytes())),
        None => Err(depot_refuse(format!("Rubrique {} introuvable : ce fichier ne ressemble pas a une DSN.", RUB_NATURE))),
    }
}

/// `poser` : `<rubrique>,'[^']*'` devient `<rubrique>,'<valeur>'`, la premiere fois.
fn poser(octets: &[u8], rubrique: &str, valeur: &str) -> Resultat<Vec<u8>> {
    let prefixe = format!("{},'", rubrique);
    let fin = |reste: &[u8]| reste.iter().position(|&b| b == b'\'').map(|p| p + 1);
    match premier(octets, prefixe.as_bytes(), fin) {
        Some((i, n)) => Ok(remplacer(octets, i, n, format!("{},'{}'", rubrique, valeur).as_bytes())),
        None => Err(depot_refuse(format!(
            "Rubrique {} introuvable : ce fichier ne ressemble pas a une DSN mensuelle.",
            rubrique
        ))),
    }
}

/// `renumeroter_pour_test` : un annule-et-remplace d'ordre `ordre` (type 01 -> 03,
/// 02 -> 05), en memoire.
pub fn renumeroter(octets: &[u8], ordre: &str) -> Resultat<Vec<u8>> {
    let prefixe = b"S20.G00.05.002,'";
    let mut contenu = octets.to_vec();
    if let Some((i, _)) = premier(octets, prefixe, deux_chiffres) {
        let type_ar = match &octets[i + prefixe.len()..i + prefixe.len() + 2] {
            b"01" => Some("03"),
            b"02" => Some("05"),
            _ => None,
        };
        if let Some(t) = type_ar {
            contenu = poser(&contenu, "S20.G00.05.002", t)?;
        }
    }
    poser(&contenu, "S20.G00.05.004", ordre)
}

// ----------------------------------------------------------------- verifier

/// La confirmation recopiee, sans les blancs de part et d'autre.
fn nettoye(s: &str) -> &str {
    strip(s)
}

/// `dsn_api.verifier` : le mode demande doit correspondre au fichier, et un reel
/// exige une preuve.
pub fn verifier(a: &Apercu, reel: bool, mois_confirme: Option<&str>, presence_prouvee: bool) -> Resultat<()> {
    let nature_envoi = a.champ("nature_envoi");
    if reel && nature_envoi != NATURE_REEL {
        return Err(depot_refuse(format!(
            "Le fichier porte {}='{}' (envoi de test) et vous demandez un depot REEL. Un fichier de test depose reellement ne declare RIEN : les donnees ne sont pas conservees. Produire le fichier sans --test.",
            RUB_NATURE, nature_envoi
        )));
    }
    if !reel && nature_envoi != NATURE_REEL {
        return Ok(());
    }
    if reel {
        let attendu = a.confirmation_attendue();
        if let Some(m) = mois_confirme.filter(|m| !m.is_empty()) {
            if nettoye(m) != attendu {
                return Err(depot_refuse(format!(
                    "{} confirme '{}' alors que le fichier declare '{}'. Rien n'est parti.",
                    if a.est_signalement() { "Jour de l'evenement" } else { "Mois" },
                    nettoye(m),
                    attendu
                )));
            }
            return Ok(());
        }
        if !presence_prouvee {
            return Err(depot_refuse(format!(
                "Un depot REEL demande une preuve : reecrire {} ({}), ou retaper le mot de passe du logiciel.",
                if a.est_signalement() { "le jour de l'evenement signale" } else { "le mois declare" },
                attendu
            )));
        }
    }
    Ok(())
}

// --------------------------------------------------------------------- DPAE

/// `dpae_api.indicateur` : C_VERSION du premier enregistrement.
pub fn indicateur(octets: &[u8]) -> String {
    let (a, b) = INDICATEUR;
    let fin = b.min(octets.len());
    if a >= fin {
        return String::new();
    }
    octets[a..fin].iter().map(|&c| c as char).collect()
}

pub fn est_reel(octets: &[u8]) -> bool {
    indicateur(octets) == INDICATEUR_REEL
}

/// `dpae_api.basculer_en_test` : chaque enregistrement repasse en C_VERSION = TST,
/// avec la meme affectation de tranche qu'un `bytearray` (une tranche hors du
/// flux s'ajoute a la fin).
pub fn basculer_dpae(octets: &[u8], longueur: i64) -> Resultat<Vec<u8>> {
    if longueur == 0 {
        return Err(refus("ZeroDivisionError", "integer modulo by zero"));
    }
    let n = octets.len() as i64;
    if n % longueur != 0 {
        return Err(refus("ValueError", "flux DPAE de longueur inattendue"));
    }
    let mut sortie = octets.to_vec();
    let (a, b) = (INDICATEUR.0 as i64, INDICATEUR.1 as i64);
    for i in 0..(n / longueur).max(0) {
        let long = sortie.len() as i64;
        let debut = (i * longueur + a).min(long);
        let fin = (i * longueur + b).clamp(debut, long);
        sortie.splice(debut as usize..fin as usize, INDICATEUR_TEST.bytes());
    }
    Ok(sortie)
}

/// Un caractere ecrit en echappement dans la forme citee d'un texte : les controles, les
/// espaces insecables et les caracteres de format les plus courants.
fn non_imprimable(c: char) -> bool {
    matches!(c as u32, 0..=0x1f | 0x7f..=0xa0 | 0xad | 0x61c | 0x1680 | 0x180e | 0x2000..=0x200f | 0x2028..=0x202f | 0x205f..=0x206f | 0x3000 | 0xe000..=0xf8ff | 0xfeff | 0xfff9..=0xfffb)
}

/// Un texte cite entre guillemets, ses caracteres non imprimables echappes.
fn repr_py(s: &str) -> String {
    let guillemet = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::from(guillemet);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == guillemet => {
                out.push('\\');
                out.push(c);
            }
            c if non_imprimable(c) => {
                let n = c as u32;
                out.push_str(&if n < 0x100 { format!("\\x{:02x}", n) } else if n < 0x10000 { format!("\\u{:04x}", n) } else { format!("\\U{:08x}", n) });
            }
            c => out.push(c),
        }
    }
    out.push(guillemet);
    out
}

fn type_python(v: &Value) -> &'static str {
    match v {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(n) if n.is_f64() => "float",
        Value::Number(_) => "int",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

/// Ce que le controle des identifiants DPAE verifie sans rien lire d'ailleurs :
/// l'identite du declarant et ses accents. `arobase` : le mot de passe porte un
/// `@`.
pub fn identifiants(declarant: &Value, arobase: bool) -> Resultat<Vec<String>> {
    let mut soucis = Vec::new();
    let champ = |c: &str| declarant.get(c).filter(|v| vrai(v));
    let manquants: Vec<&str> = ["siret", "nom", "prenom"].into_iter().filter(|c| champ(c).is_none()).collect();
    if !manquants.is_empty() {
        soucis.push(format!(
            "Identite du declarant DPAE incomplete (manque : {}). Elle se saisit dans Configuration > Mots de passe, ligne Urssaf (DPAE) : le nom et le prenom du compte ouvert sur urssaf.fr pour la DPAE (cle `api_dpae` de config/entreprise.json).",
            manquants.join(", ")
        ));
    }
    for c in ["nom", "prenom"] {
        let v = match champ(c) {
            None => "",
            Some(Value::String(s)) => s.as_str(),
            Some(x) => {
                return Err(refus("TypeError", socle::fichiers::attendu("un texte", type_python(x))));
            }
        };
        if v != ascii_nfkd(v) {
            soucis.push(format!(
                "Le {} du declarant ({}) porte un accent. La DUE MtoM les refuse : corriger config/entreprise.json.",
                c,
                repr_py(v)
            ));
        }
    }
    if arobase {
        soucis.push(
            "Le mot de passe contient un caractere @, que la connexion MtoM refuse. Il doit etre change sur net-entreprises.fr."
                .to_string(),
        );
    }
    Ok(soucis)
}

