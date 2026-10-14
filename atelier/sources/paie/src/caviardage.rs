//! `caviardage` : enlever d'un texte tout ce qui identifie une personne, avant qu'il parte chez
//! l'editeur (`prive/journal/PLAN-IMPORT-BULLETINS.md` §2.c et §4, lot E1). Le texte est celui des
//! pages d'un bulletin qu'on n'a pas su lire, tel que la fabrique le rend : des lignes. L'editeur en
//! a besoin pour ecrire le gabarit manquant ; il n'a besoin d'AUCUNE valeur personnelle.
//!
//! Ce qui RESTE : les montants (« 1 234,56 »), les taux (« 4,830 % »), les dates de periode et de
//! reglement, les libelles de rubriques et de cotisations, le numero d'objet, le Siret, le NAF, et
//! l'entreprise. UNE RAISON SOCIALE ET UN SIRET NE SONT PAS UNE PERSONNE : le nom, l'adresse et le
//! Siret de la compagnie ne sont pas caviardes ; `Connus::entreprise` les PROTEGE meme (les etages
//! par la forme les prendraient sinon pour une adresse ou un nom). Un titre de spectacle peut etre
//! caviarde par exces (« Le Roi LEAR » a la forme d'un nom) : on l'accepte, jamais qu'un nom reste.
//!
//! Deux etages, et les deux s'appliquent, dans cet ordre :
//!
//! 1. PAR LES VALEURS CONNUES (`Connus`) : chaque valeur, et pour un nom ou un prenom chacune de
//!    ses parties, est remplacee par son etiquette, sans tenir compte de la casse ni des accents ni
//!    des separateurs (« Jean-Pierre » = « JEAN PIERRE », « Élodie » = « ELODIE »).
//! 2. PAR LA FORME, pour ce que le logiciel ne connait pas encore : NIR, date qui suit
//!    « naissance » ou « ne(e) le », numero Conges Spectacles, courriel, telephone, IBAN, code
//!    salarie, ligne d'adresse, code postal et commune, et « Prenom NOM » / « NOM Prenom ».
//!
//! REUTILISE `incidents::caviarder` (le caviardage du diagnostic a distance) : il passe au deuxieme
//! etage, ses `[NIR]`, `[IBAN]`, `[BIC]` deviennent `<NIR>`, `<IBAN>`, `<IBAN>`. Ses motifs sont
//! etroits (NIR au mois 01-12, IBAN francais) : ceux d'ici l'etendent (NIR corse, mois 20 et plus,
//! IBAN etranger) sans le doubler pour le reste.
//!
//! Le moteur ne leve jamais ici : un texte entre, un texte sort, avec le compte de ce qui a ete
//! enleve par etiquette (pour le dire a la personne : « 3 noms, 1 NIR, 2 adresses enleves »).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;

use crate::incidents;

// ------------------------------------------------------------------ les etiquettes

pub const NOM: &str = "NOM";
pub const PRENOM: &str = "PRENOM";
pub const NIR: &str = "NIR";
pub const NAISSANCE: &str = "NAISSANCE";
pub const ADRESSE: &str = "ADRESSE";
pub const CS: &str = "CS";
pub const COURRIEL: &str = "COURRIEL";
pub const TEL: &str = "TEL";
pub const IBAN: &str = "IBAN";
/// Le code salarie, le matricule : interne a la compagnie, mais souvent tire du nom (« DUPO01 »).
pub const CODE: &str = "CODE";

/// L'ordre des etiquettes, celui du resume.
const ETIQUETTES: [&str; 10] = [NOM, PRENOM, NIR, NAISSANCE, ADRESSE, CS, COURRIEL, TEL, IBAN, CODE];
const I_NOM: usize = 0;
const I_PRENOM: usize = 1;
const I_NIR: usize = 2;
const I_NAISSANCE: usize = 3;
const I_ADRESSE: usize = 4;
const I_CS: usize = 5;
const I_COURRIEL: usize = 6;
const I_TEL: usize = 7;
const I_IBAN: usize = 8;
const I_CODE: usize = 9;

/// (singulier, pluriel) de chaque etiquette, pour le resume.
const DITS: [(&str, &str); 10] = [
    ("nom", "noms"),
    ("prénom", "prénoms"),
    ("NIR", "NIR"),
    ("date de naissance", "dates de naissance"),
    ("adresse", "adresses"),
    ("numéro Congés Spectacles", "numéros Congés Spectacles"),
    ("courriel", "courriels"),
    ("téléphone", "téléphones"),
    ("IBAN", "IBAN"),
    ("code salarié", "codes salarié"),
];

// ----------------------------------------------------------------- l'entree, la sortie

/// Ce que le logiciel sait deja de la personne (toutes les listes sont optionnelles : vides, seul
/// le deuxieme etage travaille), et de la compagnie, qui n'est PAS caviardee.
#[derive(Debug, Clone, Default)]
pub struct Connus {
    pub noms: Vec<String>,
    pub prenoms: Vec<String>,
    pub nirs: Vec<String>,
    /// « 1985-07-23 », « 23/07/1985 » : la date se retrouve sous toutes ses ecritures.
    pub naissances: Vec<String>,
    /// Les lignes d'adresse (« 8 rue des Glycines »).
    pub adresses: Vec<String>,
    pub codes_postaux: Vec<String>,
    pub communes: Vec<String>,
    pub conges_spectacles: Vec<String>,
    pub courriels: Vec<String>,
    pub telephones: Vec<String>,
    pub ibans: Vec<String>,
    /// La compagnie : raison sociale, lignes d'adresse, code postal et commune. PROTEGES, pas
    /// caviardes : une raison sociale n'est pas une personne. Si une valeur de la personne est
    /// aussi celle de la compagnie (une compagnie domiciliee chez son salarie), le premier etage,
    /// qui passe avant, l'a deja enlevee.
    pub entreprise: Vec<String>,
}

/// Le texte caviarde, et ce qui a ete enleve, par etiquette (seules celles qui ont servi).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caviarde {
    pub texte: String,
    pub enleves: BTreeMap<&'static str, usize>,
}

impl Caviarde {
    /// Combien de valeurs ont ete enlevees, en tout.
    pub fn total(&self) -> usize {
        self.enleves.values().sum()
    }

    /// « 3 noms, 1 NIR, 2 adresses enlevés » ; « rien d'enlevé ».
    pub fn resume(&self) -> String {
        let parts: Vec<String> = ETIQUETTES
            .iter()
            .zip(DITS.iter())
            .filter_map(|(e, (un, plusieurs))| {
                self.enleves.get(e).filter(|n| **n > 0).map(|n| format!("{} {}", n, if *n > 1 { plusieurs } else { un }))
            })
            .collect();
        if parts.is_empty() {
            "rien d'enlevé".to_string()
        } else {
            format!("{} enlevé{}", parts.join(", "), if self.total() > 1 { "s" } else { "" })
        }
    }
}

/// Le caviardage : les deux etages, l'un apres l'autre.
pub fn caviarder(texte: &str, connus: &Connus) -> Caviarde {
    // Les marques de travail vivent dans une zone d'usage prive ; un caractere d'entree qui y
    // tomberait (rien ne s'y lit d'une page de bulletin) devient U+FFFD.
    let propre: String = texte.chars().map(|c| if est_marque(c) { '\u{FFFD}' } else { c }).collect();
    let mut t = Travail { s: propre, comptes: [0; 10], gardes: Vec::new() };
    par_les_connus(&mut t, connus);
    proteger(&mut t, connus);
    par_la_forme(&mut t);
    let texte = t.rendre();
    let enleves = ETIQUETTES.iter().zip(t.comptes.iter()).filter(|(_, n)| **n > 0).map(|(e, n)| (*e, *n)).collect();
    Caviarde { texte, enleves }
}

// ------------------------------------------------------------------- le travail

/// Une valeur enlevee devient une MARQUE (ouvre, etiquette, ferme), une valeur protegee une GARDE
/// (ouvre, numero, ferme), toutes en caracteres d'usage prive : ni lettre, ni chiffre, ni espace,
/// aucun motif ne les prend, et `\b` les traite comme une ponctuation. Elles redeviennent `<NOM>`
/// et la valeur protegee a la fin, pas avant : une etiquette en clair (« <NOM> ») serait prise
/// par un motif suivant (une personne qui s'appelle Nom).
const OUVRE_MARQUE: char = '\u{F0000}';
const FERME_MARQUE: char = '\u{F0001}';
const OUVRE_GARDE: char = '\u{F0002}';
const FERME_GARDE: char = '\u{F0003}';
const CHIFFRES: u32 = 0xF0100;

fn est_marque(c: char) -> bool {
    (0xF0000..=0xF0FFF).contains(&(c as u32))
}

fn chiffre_prive(d: u32) -> char {
    char::from_u32(CHIFFRES + d).unwrap_or('\u{FFFD}')
}

fn marque(etiquette: usize) -> String {
    [OUVRE_MARQUE, chiffre_prive(etiquette as u32), FERME_MARQUE].iter().collect()
}

fn garde(n: usize) -> String {
    let mut s = String::from(OUVRE_GARDE);
    s.extend(n.to_string().chars().map(|c| chiffre_prive(c.to_digit(10).unwrap_or(0))));
    s.push(FERME_GARDE);
    s
}

struct Travail {
    s: String,
    comptes: [usize; 10],
    gardes: Vec<String>,
}

impl Travail {
    /// Remplace chaque plage (octets de `s`) par la marque de `etiquette`. Une plage qui en
    /// chevauche une autre, ou qui toucherait une marque deja posee, est laissee : ce qu'elle
    /// couvre est deja enleve ou protege.
    fn poser(&mut self, plages: Vec<(usize, usize)>, etiquette: usize) {
        let n = self.remplacer(plages, |_, _| marque(etiquette));
        self.comptes[etiquette] += n;
    }

    /// Met les plages a l'abri des motifs qui suivent ; elles reviennent telles quelles a la fin.
    fn garder(&mut self, plages: Vec<(usize, usize)>) {
        let mut gardes = std::mem::take(&mut self.gardes);
        self.remplacer(plages, |_, v| {
            gardes.push(v.to_string());
            garde(gardes.len() - 1)
        });
        self.gardes = gardes;
    }

    fn remplacer(&mut self, mut plages: Vec<(usize, usize)>, mut par: impl FnMut(usize, &str) -> String) -> usize {
        plages.sort();
        let (mut out, mut dernier, mut n) = (String::with_capacity(self.s.len()), 0, 0);
        for (a, b) in plages {
            if a < dernier || a >= b || b > self.s.len() || self.s[a..b].chars().any(est_marque) {
                continue;
            }
            out.push_str(&self.s[dernier..a]);
            out.push_str(&par(n, &self.s[a..b]));
            dernier = b;
            n += 1;
        }
        out.push_str(&self.s[dernier..]);
        self.s = out;
        n
    }

    /// Les marques deviennent `<ETIQUETTE>`, les gardes leur valeur.
    fn rendre(&self) -> String {
        let mut out = String::with_capacity(self.s.len());
        let mut it = self.s.chars();
        while let Some(c) = it.next() {
            match c {
                OUVRE_MARQUE => {
                    let i = it.next().map_or(0, |d| (d as u32).saturating_sub(CHIFFRES) as usize);
                    it.next();
                    out.push('<');
                    out.push_str(ETIQUETTES.get(i).copied().unwrap_or("?"));
                    out.push('>');
                }
                OUVRE_GARDE => {
                    let mut n = 0usize;
                    for d in it.by_ref() {
                        if d == FERME_GARDE {
                            break;
                        }
                        n = n * 10 + (d as u32).saturating_sub(CHIFFRES) as usize;
                    }
                    out.push_str(self.gardes.get(n).map_or("", |g| g.as_str()));
                }
                autre => out.push(autre),
            }
        }
        out
    }
}

// ------------------------------------------------------------------- le pliage

/// Une lettre sans casse ni accent ; UN caractere pour UN caractere, pour que les positions du
/// texte plie soient celles du texte d'origine.
fn plier_car(c: char) -> char {
    let l = c.to_lowercase().next().unwrap_or(c);
    match l {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ą' => 'a',
        'ç' | 'ć' | 'č' => 'c',
        'ď' | 'đ' => 'd',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ę' | 'ě' => 'e',
        'ğ' => 'g',
        'ì' | 'í' | 'î' | 'ï' | 'ī' | 'ı' => 'i',
        'ł' => 'l',
        'ñ' | 'ń' | 'ň' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' => 'o',
        'ř' => 'r',
        'ś' | 'š' | 'ş' => 's',
        'ť' => 't',
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' => 'u',
        'ý' | 'ÿ' => 'y',
        'ź' | 'ż' | 'ž' => 'z',
        '’' | '‘' | 'ʼ' => '\'',
        '\u{a0}' | '\u{202f}' => ' ',
        '‐' | '‑' | '–' | '—' => '-',
        x => x,
    }
}

fn plier(s: &str) -> String {
    s.chars().map(plier_car).collect()
}

/// Pour les listes de mots : plie, en majuscules.
fn haut(s: &str) -> String {
    plier(s).to_uppercase()
}

/// Le texte plie, et pour chaque caractere son debut dans le plie et dans l'origine.
struct Plie {
    texte: String,
    debuts_plies: Vec<usize>,
    debuts_origine: Vec<usize>,
}

impl Plie {
    fn de(s: &str) -> Plie {
        let (mut texte, mut f, mut o) = (String::with_capacity(s.len()), Vec::new(), Vec::new());
        for (ob, c) in s.char_indices() {
            f.push(texte.len());
            o.push(ob);
            texte.push(plier_car(c));
        }
        f.push(texte.len());
        o.push(s.len());
        Plie { texte, debuts_plies: f, debuts_origine: o }
    }

    fn origine(&self, b: usize) -> usize {
        let i = self.debuts_plies.binary_search(&b).unwrap_or_else(|i| i);
        self.debuts_origine[i.min(self.debuts_origine.len() - 1)]
    }
}

// ------------------------------------------------------- premier etage : les connus

/// Les separateurs d'une valeur connue : un ou plusieurs, quels qu'ils soient.
const SEP: &str = r"[\s\-'.,/]+";
/// Entre deux caracteres d'un numero : rien, un espace, un point ou un tiret.
const SEP_NUM: &str = r"[\s.\-]?";

/// Une valeur en mots (nom, prenom, adresse, commune, entreprise), pliee, en motif.
fn motif_mots(v: &str) -> Option<String> {
    let p = plier(v);
    let p = p.trim_matches(|c: char| !c.is_alphanumeric());
    if p.chars().filter(|c| c.is_alphanumeric()).count() < 2 {
        return None;
    }
    let cs: Vec<char> = p.chars().collect();
    let (mut out, mut i, mut dans_sep) = (String::from(r"\b"), 0, false);
    while i < cs.len() {
        let c = cs[i];
        if !c.is_alphanumeric() {
            if !dans_sep {
                out.push_str(SEP);
                dans_sep = true;
            }
            i += 1;
            continue;
        }
        dans_sep = false;
        let suivant = cs.get(i + 1).copied();
        if c == 'œ' || (c == 'o' && suivant == Some('e')) {
            out.push_str("(?:oe|œ)");
            i += if c == 'œ' { 1 } else { 2 };
        } else if c == 'æ' || (c == 'a' && suivant == Some('e')) {
            out.push_str("(?:ae|æ)");
            i += if c == 'æ' { 1 } else { 2 };
        } else {
            out.push_str(&regex::escape(&c.to_string()));
            i += 1;
        }
    }
    out.push_str(r"\b");
    Some(out)
}

/// Un numero (NIR, IBAN, Conges Spectacles, code postal) : ses lettres et chiffres, dans l'ordre,
/// quels que soient les espaces.
fn motif_numero(v: &str, minimum: usize) -> Option<String> {
    let cs: Vec<String> = plier(v).chars().filter(|c| c.is_alphanumeric()).map(|c| regex::escape(&c.to_string())).collect();
    (cs.len() >= minimum).then(|| format!(r"\b{}\b", cs.join(SEP_NUM)))
}

/// Un telephone : « 06 12 34 56 78 », « 06.12.34.56.78 », « +33 6 12 34 56 78 », « +33 (0)6… ».
fn motif_telephone(v: &str) -> Option<String> {
    let d: String = v.chars().filter(|c| c.is_ascii_digit()).collect();
    let reste = if d.len() == 10 && d.starts_with('0') {
        &d[1..]
    } else if d.len() == 11 && d.starts_with("33") {
        &d[2..]
    } else if d.len() == 12 && d.starts_with("330") {
        &d[3..]
    } else {
        return motif_numero(v, 6);
    };
    let corps: Vec<String> = reste.chars().map(|c| c.to_string()).collect();
    Some(format!(r"(?:\+33[\s.\-]?(?:\(0\)[\s.\-]?)?|\b0033[\s.\-]?|\b0){}\b", corps.join(SEP_NUM)))
}

const MOIS: [(&str, &str); 12] = [
    ("janvier", "janv"),
    ("fevrier", "fevr|fev"),
    ("mars", "mars"),
    ("avril", "avr"),
    ("mai", "mai"),
    ("juin", "juin"),
    ("juillet", "juil"),
    ("aout", "aout"),
    ("septembre", "sept"),
    ("octobre", "oct"),
    ("novembre", "nov"),
    ("decembre", "dec"),
];

/// Une date de naissance connue, sous ses ecritures : 23/07/1985, 23.07.85, 1985-07-23,
/// 23 juillet 1985.
fn motif_date(v: &str) -> Option<String> {
    let groupes: Vec<&str> = v.split(|c: char| !c.is_ascii_digit()).filter(|g| !g.is_empty()).collect();
    if groupes.len() != 3 {
        return motif_numero(v, 4);
    }
    let (j, m, a) = if groupes[0].len() == 4 { (groupes[2], groupes[1], groupes[0]) } else { (groupes[0], groupes[1], groupes[2]) };
    let (Ok(j), Ok(m)) = (j.parse::<u32>(), m.parse::<u32>()) else { return motif_numero(v, 4) };
    if !(1..=31).contains(&j) || !(1..=12).contains(&m) || !(a.len() == 2 || a.len() == 4) {
        return motif_numero(v, 4);
    }
    let annee = if a.len() == 4 { format!("(?:{}|{})", a, &a[2..]) } else { a.to_string() };
    let (plein, court) = MOIS[(m - 1) as usize];
    let mut alternatives = vec![
        format!(r"\b0?{j}[\s/.\-]+0?{m}[\s/.\-]+{annee}\b"),
        format!(r"\b0?{j}(?:er)?\s+(?:{plein}|(?:{court})\.?)\s+{annee}\b"),
    ];
    if a.len() == 4 {
        alternatives.push(format!(r"\b{a}-0?{m}-0?{j}\b"));
    }
    Some(format!("(?:{})", alternatives.join("|")))
}

fn motif_courriel(v: &str) -> Option<String> {
    let p = plier(v.trim());
    if p.contains('@') && p.len() >= 5 {
        Some(regex::escape(&p))
    } else {
        motif_mots(v)
    }
}

/// Un nom ou un prenom : la valeur entiere, puis chacune de ses parties (un nom compose, un
/// prenom double), sauf les particules (« de », « Le ») qui seraient partout.
fn motifs_nom(v: &str, etiquette: usize, out: &mut Vec<(usize, String, usize)>) {
    let p = plier(v);
    if let Some(m) = motif_mots(v) {
        out.push((p.chars().count(), m, etiquette));
    }
    for partie in p.split(|c: char| !c.is_alphanumeric()) {
        if partie.chars().count() >= 2 && !est_particule(&partie.to_uppercase()) {
            if let Some(m) = motif_mots(partie) {
                out.push((partie.chars().count(), m, etiquette));
            }
        }
    }
}

fn par_les_connus(t: &mut Travail, c: &Connus) {
    let mut motifs: Vec<(usize, String, usize)> = Vec::new();
    let ajouter = |valeurs: &[String], faire: fn(&str) -> Option<String>, etiquette: usize, motifs: &mut Vec<(usize, String, usize)>| {
        for v in valeurs {
            if let Some(m) = faire(v) {
                motifs.push((plier(v).chars().count(), m, etiquette));
            }
        }
    };
    for v in &c.noms {
        motifs_nom(v, I_NOM, &mut motifs);
    }
    for v in &c.prenoms {
        motifs_nom(v, I_PRENOM, &mut motifs);
    }
    ajouter(&c.nirs, |v| motif_numero(v, 13), I_NIR, &mut motifs);
    ajouter(&c.naissances, motif_date, I_NAISSANCE, &mut motifs);
    ajouter(&c.adresses, motif_mots, I_ADRESSE, &mut motifs);
    ajouter(&c.codes_postaux, |v| motif_numero(v, 4), I_ADRESSE, &mut motifs);
    ajouter(&c.communes, motif_mots, I_ADRESSE, &mut motifs);
    ajouter(&c.conges_spectacles, |v| motif_numero(v, 5), I_CS, &mut motifs);
    ajouter(&c.courriels, motif_courriel, I_COURRIEL, &mut motifs);
    ajouter(&c.telephones, motif_telephone, I_TEL, &mut motifs);
    ajouter(&c.ibans, |v| motif_numero(v, 10), I_IBAN, &mut motifs);
    // Le plus long d'abord : « Martin-Durand » entier avant « Martin », un courriel avant le nom
    // qu'il contient, une adresse avant le nom de sa rue.
    motifs.sort_by(|a, b| b.0.cmp(&a.0));
    motifs.dedup_by(|a, b| a.1 == b.1);
    for (_, m, etiquette) in motifs {
        let Ok(re) = Regex::new(&m) else { continue };
        let plie = Plie::de(&t.s);
        let plages = re.find_iter(&plie.texte).map(|x| (plie.origine(x.start()), plie.origine(x.end()))).collect();
        t.poser(plages, etiquette);
    }
}

/// La compagnie, le Siret, le numero d'objet : mis a l'abri des motifs par la forme.
fn proteger(t: &mut Travail, c: &Connus) {
    let mut valeurs: Vec<&String> = c.entreprise.iter().collect();
    valeurs.sort_by_key(|v| std::cmp::Reverse(v.chars().count()));
    for v in valeurs {
        let Some(m) = motif_mots(v) else { continue };
        let Ok(re) = Regex::new(&m) else { continue };
        let plie = Plie::de(&t.s);
        let plages = re.find_iter(&plie.texte).map(|x| (plie.origine(x.start()), plie.origine(x.end()))).collect();
        t.garder(plages);
    }
    let m = motifs();
    let plages = groupes(&m.siret, &t.s);
    t.garder(plages);
    let plages = groupes(&m.objet, &t.s);
    t.garder(plages);
}

// ------------------------------------------------------ deuxieme etage : la forme

struct Motifs {
    siret: Regex,
    objet: Regex,
    courriel: Regex,
    iban: Regex,
    nir: Regex,
    nir_etiquette: Regex,
    naissance: Regex,
    cs: Regex,
    telephone: Regex,
    code: Regex,
    code_ligne_suivante: Regex,
    adresse: Regex,
    code_postal: Regex,
    mot: Regex,
    montant: Regex,
}

fn motif(source: &str) -> Regex {
    Regex::new(source).expect("motif du caviardage")
}

fn motifs() -> &'static Motifs {
    static M: OnceLock<Motifs> = OnceLock::new();
    M.get_or_init(|| Motifs {
        // Le Siret (14 chiffres, 3-3-3-5) ou le Siren (9) qui suit son mot : protege, pour qu'aucun
        // motif de numero ne le prenne. Le groupe s'arrete a 14 chiffres : un NIR qui suivrait
        // sur la ligne n'est pas protege avec lui.
        siret: motif(r"(?i)\bsir[eé][tn]\b[ \t:.°]*(\d{3}[ \u{a0}]?\d{3}[ \u{a0}]?\d{3}(?:[ \u{a0}]?\d{5})?)\b"),
        objet: motif(r"(?i)\bobjet\b[ \t:]*([0-9][0-9A-Z]{7,13})\b"),
        courriel: motif(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9\-]+(?:\.[A-Za-z0-9\-]+)+"),
        // Tout IBAN : deux lettres, deux chiffres, puis des groupes de quatre.
        iban: motif(r"\b[A-Z]{2}\d{2}(?:[ \u{a0}]?[A-Z0-9]{4}){2,7}(?:[ \u{a0}]?[A-Z0-9]{1,3})?\b"),
        // 13 chiffres + cle de 2, avec ou sans espaces ; departement corse (2A, 2B) ; mois 20 et
        // plus (naissance inconnue) ; outre-mer (departement a 3 chiffres) : 6 chiffres apres le
        // departement a 2 chiffres, quel que soit le decoupage. Un Siret colle (14 chiffres) n'y
        // entre pas : la cle est de deux chiffres ou absente.
        nir: motif(r"\b[12][ \u{a0}]?\d{2}[ \u{a0}]?\d{2}[ \u{a0}]?(?:\d{2}|2[ABab])(?:[ \u{a0}]?\d){6}(?:[ \u{a0}]?\d{2})?\b"),
        nir_etiquette: motif(r"(?i)(?:s[ée]curit[ée]\s+sociale|\bNIR\b|\bINSEE\b)[ \t:.°]*([12][0-9ABab \u{a0}]{12,22}[0-9])\b"),
        // La date qui suit « naissance » ou « ne(e) le », sur la ligne ou au debut de la suivante.
        naissance: motif(
            r"(?i)(?:naissance|\bn[ée]e?(?:\s?\(e\))?\s+le)[^0-9\n]{0,40}(?:\n[^0-9\n]{0,40})?(\d{1,2}[/.\-]\d{1,2}[/.\-]\d{2,4}|\d{4}-\d{2}-\d{2}|\d{1,2}(?:er)?\s+[a-zéû]+\.?\s+\d{4})",
        ),
        // Une lettre et sept chiffres : « X 012 3456 », « E0000001 ».
        cs: motif(r"\b[A-Z][ \u{a0}]?\d{3}[ \u{a0}]?\d{4}\b"),
        telephone: motif(r"(?:\+33[ .\-\u{a0}]?(?:\(0\)[ .\-\u{a0}]?)?[1-9]|\b0033[ .\-]?[1-9]|\b0[1-9])(?:[ .\-\u{a0}]?\d{2}){4}\b"),
        code: motif(r"(?i:code\s+salari[ée]|matricule)[ \t:.°]*([A-Z0-9][A-Z0-9\-]{1,15})\b"),
        // Le meme code, seul en tete de la ligne suivante quand l'etiquette finit la sienne (#149) : des chiffres
        // seulement (un mot en capitales y serait une autre etiquette), suivis d'un blanc ou de la fin de ligne (une
        // date, un montant ne s'y prennent pas).
        code_ligne_suivante: motif(r"(?i:code\s+salari[ée]|matricule)[ \t:.°]*\r?\n[ \t]*(\d{2,16})(?:[ \t]|\r?\n|$)"),
        // Le debut d'une ligne d'adresse : numero (« 14 », « 2-4 », « 7 bis ») et type de voie ;
        // la suite se lit mot a mot (`adresses`).
        adresse: motif(
            r"(?i)\b\d{1,4}(?:[ \u{a0}]?(?:bis|ter|quater|[a-d]))?(?:[ \u{a0}]?[-–/][ \u{a0}]?\d{1,4}(?:[ \u{a0}]?(?:bis|ter))?)?,?[ \u{a0}]+((?:rue|avenue|boulevard|bd|place|chemin|impasse|all[ée]e|route|quai|cours|square|passage|villa|cit[ée]|r[ée]sidence|lotissement|lieu[- ]dit|hameau|faubourg|voie|sentier|promenade|esplanade|parvis|rond[- ]point|mont[ée]e|traverse|clos|domaine|ruelle|venelle|chauss[ée]e)\b|av\.)",
        ),
        code_postal: motif(r"\b\d{5}\b"),
        mot: motif(r"[\p{L}][\p{L}'’\-]*"),
        montant: motif(r"\d,\d{2}\b"),
    })
}

/// Les plages du groupe 1 de chaque prise.
fn groupes(re: &Regex, s: &str) -> Vec<(usize, usize)> {
    re.captures_iter(s).filter_map(|c| c.get(1)).map(|g| (g.start(), g.end())).collect()
}

fn prises(re: &Regex, s: &str) -> Vec<(usize, usize)> {
    re.find_iter(s).map(|m| (m.start(), m.end())).collect()
}

fn par_la_forme(t: &mut Travail) {
    let m = motifs();
    // Le caviardage du diagnostic a distance d'abord, tel qu'il est.
    let (s, vus) = incidents::caviarder(&t.s);
    t.s = s;
    for (crochet, quoi, etiquette) in [("[NIR]", "NIR", I_NIR), ("[IBAN]", "IBAN", I_IBAN), ("[BIC]", "BIC", I_IBAN)] {
        let n = vus.iter().filter(|v| **v == quoi).count();
        if n > 0 {
            t.s = t.s.replace(crochet, &marque(etiquette));
            t.comptes[etiquette] += n;
        }
    }
    let p = prises(&m.courriel, &t.s);
    t.poser(p, I_COURRIEL);
    let p = prises(&m.iban, &t.s);
    t.poser(p, I_IBAN);
    let p = groupes(&m.nir_etiquette, &t.s);
    t.poser(p, I_NIR);
    let p = prises(&m.nir, &t.s);
    t.poser(p, I_NIR);
    let p = groupes(&m.naissance, &t.s);
    t.poser(p, I_NAISSANCE);
    let p = prises(&m.cs, &t.s);
    t.poser(p, I_CS);
    let p = prises(&m.telephone, &t.s);
    t.poser(p, I_TEL);
    let p = groupes(&m.code, &t.s);
    t.poser(p, I_CODE);
    let p = groupes(&m.code_ligne_suivante, &t.s);
    t.poser(p, I_CODE);
    let p = adresses(&t.s);
    t.poser(p, I_ADRESSE);
    let p = codes_postaux(&t.s);
    t.poser(p, I_ADRESSE);
    noms(t);
}

/// Le mot qui commence exactement a `pos`, apres au plus `blancs` espaces ; (debut, fin).
fn mot_suivant(s: &str, pos: usize, blancs: usize) -> Option<(usize, usize)> {
    let reste = &s[pos..];
    let n = reste.chars().take_while(|c| *c == ' ' || *c == '\u{a0}').count();
    if n == 0 || n > blancs {
        return None;
    }
    let debut = pos + reste.chars().take(n).map(char::len_utf8).sum::<usize>();
    let x = motifs().mot.find_at(s, debut)?;
    (x.start() == debut).then_some((x.start(), x.end()))
}

fn fin_de_ligne(s: &str, pos: usize) -> usize {
    s[pos..].find('\n').map_or(s.len(), |i| pos + i)
}

/// Les voies dont le mot est aussi un mot de bulletin (« 2 Cours de danse 50,00 100,00 ») : une
/// ligne qui porte un montant n'en est pas une adresse.
const VOIES_AMBIGUES: [&str; 8] = ["COURS", "PLACE", "PASSAGE", "DOMAINE", "CLOS", "VOIE", "VILLA", "SQUARE"];

/// Numero, type de voie, puis les mots qui suivent (six au plus), jusqu'a un mot de bulletin, un
/// chiffre, une ponctuation ou un grand blanc (une autre colonne).
fn adresses(s: &str) -> Vec<(usize, usize)> {
    let mut plages = Vec::new();
    for c in motifs().adresse.captures_iter(s) {
        let (Some(tout), Some(voie)) = (c.get(0), c.get(1)) else { continue };
        let mut fin = tout.end();
        let mut n = 0;
        while let Some((a, b)) = mot_suivant(s, fin, 1) {
            let mot = &s[a..b];
            if n >= 6 || (est_mot_de_bulletin(mot) && !est_particule(&haut(mot))) {
                break;
            }
            fin = b;
            n += 1;
        }
        let voie = haut(voie.as_str());
        if VOIES_AMBIGUES.contains(&voie.as_str()) && motifs().montant.is_match(&s[fin..fin_de_ligne(s, fin)]) {
            continue;
        }
        plages.push((tout.start(), fin));
    }
    plages
}

/// Un code postal suivi d'une commune : des mots en majuscules (ou capitalises) qui ne sont pas
/// des mots de bulletin, et « CEDEX 9 » s'il y est.
fn codes_postaux(s: &str) -> Vec<(usize, usize)> {
    let mut plages = Vec::new();
    for x in motifs().code_postal.find_iter(s) {
        let mut fin = x.end();
        let mut n = 0;
        while let Some((a, b)) = mot_suivant(s, fin, if n == 0 { 3 } else { 1 }) {
            let mot = &s[a..b];
            let cedex = haut(mot) == "CEDEX";
            if !cedex && !matches!(genre(mot), Genre::Majuscules | Genre::Capitale | Genre::Particule) {
                break;
            }
            fin = b;
            n += 1;
        }
        if n == 0 {
            continue;
        }
        // « CEDEX 9 »
        if s[x.end()..fin].to_uppercase().ends_with("CEDEX") {
            let reste = &s[fin..];
            let k = reste.chars().take_while(|c| *c == ' ').count();
            let chiffres = reste[k..].chars().take_while(|c| c.is_ascii_digit()).count();
            if k <= 1 && (1..=2).contains(&chiffres) {
                fin += k + chiffres;
            }
        }
        plages.push((x.start(), fin));
    }
    plages
}

// ----------------------------------------------------------------- Prenom NOM

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Genre {
    /// Tout en majuscules, deux lettres au moins : un NOM.
    Majuscules,
    /// Une capitale puis des minuscules (« Jean-Pierre ») : un Prenom.
    Capitale,
    /// « de », « LE », « van », une initiale : ne compte pas, ne coupe pas.
    Particule,
    /// Un mot de bulletin, un mot en minuscules : coupe.
    Autre,
}

/// Les particules et articles des noms : transparents dans « Jean de LA FONTAINE ».
const PARTICULES: [&str; 31] = [
    "DE", "DU", "DES", "LA", "LE", "LES", "D", "L", "VAN", "VON", "DER", "DEN", "DI", "DA", "DOS", "DAS", "DEL", "DELLA", "EL",
    "AL", "BEN", "BIN", "MAC", "MC", "TER", "TEN", "ST", "STE", "SAINT", "SAINTE", "Y",
];

fn est_particule(haut: &str) -> bool {
    PARTICULES.contains(&haut)
}

/// Les mots de bulletin qu'on ne prend pas pour des noms, plies en majuscules. Un nom qui serait
/// l'un d'eux (« Pierre BASE ») passe le deuxieme etage : le premier, par les connus, le prend.
const MOTS_DE_BULLETIN: &[&str] = &[
    // les familles, les totaux, l'en-tete
    "SALAIRE", "BRUT", "NET", "NETTE", "SANTE", "RETRAITE", "FAMILLE", "FAMILIALE", "FAMILIALES", "CHOMAGE", "CSG", "CRDS",
    "TOTAL", "TOTAUX", "PERIODE", "CUMUL", "LIBELLE", "BASE", "TAUX", "MONTANT", "SIRET", "SIREN", "NAF", "APE", "IDCC",
    "URSSAF", "STATUT", "EMPLOI", "ARTISTE", "MUSICIEN", "MUSICIENNE", "DRAMATIQUE", "TECHNICIEN", "TECHNICIENNE", "CACHET",
    "HEURE", "JOUR", "BULLETIN", "PAIE", "SPECTACLE", "NUMERO", "OBJET", "ANALYTIQUE", "REGLEMENT", "MODE", "VIREMENT",
    "CHEQUE", "ESPECE", "DATE", "TRAVAILLEE", "NAISSANCE", "SECURITE", "SOCIALE", "SOCIAL", "CODE", "SALARIE", "SALARIAL",
    "SALARIALE", "EMPLOYEUR", "PART", "PATRONAL", "PATRONALE", "CONVENTION", "COLLECTIVE", "CONV", "LICENCE", "ABATTEMENT",
    "CONGE", "COTISATION", "CONTRIBUTION", "CONTRIB", "OBLIGATOIRE", "STATUTAIRE", "CONVENTIONNELLE", "AUTRE", "ACCIDENT",
    "TRAVAIL", "MALADIE", "MATERNITE", "INVALIDITE", "DECES", "VIEILLESSE", "PLAFONNEE", "DEPLAFONNEE", "PLAFOND", "PLAFONNE",
    "COMPLEMENTAIRE", "ASSURANCE", "ALLOCATION", "PREVOYANCE", "MUTUELLE", "FRAIS", "EXONERATION", "REDUCTION", "GENERALE",
    "DEDUCTIBLE", "IMPOSABLE", "IMPOT", "REVENU", "SOURCE", "PRELEVE", "PRELEVEMENT", "PAYER", "AVANT", "APRES", "AU", "AUX",
    "VERSE", "VERSEMENT", "COUT", "DEFRAIEMENT", "INDEMNITE", "TRANSPORT", "REPAS", "PRIME", "SERVICE", "REPETITION", "REPET",
    "MASTERCLASS", "ACTIVITE", "PARTIELLE", "CADRE", "NON", "ASSIMILE", "INTERMITTENT", "PERMANENT", "COMEDIEN", "COMEDIENNE",
    "DANSEUR", "DANSEUSE", "CHANTEUR", "CHANTEUSE", "REGISSEUR", "REGISSEUSE", "MARIONNETTISTE", "ACTEUR", "ACTRICE",
    "TOTALE", "ISOLE", "ISOLEE", "GROUPE", "GROUPEE", "SOUMIS", "SOUMISE", "FORFAIT", "FORFAITAIRE", "TRANCHE", "ACOMPTE",
    "AVANCE", "RETENUE", "REGULARISATION", "RAPPEL", "HORAIRE", "MENSUEL", "MENSUELLE", "ANNUEL", "ANNUELLE", "MOIS", "ANNEE",
    "ECHELON", "NIVEAU", "COEFFICIENT", "CATEGORIE", "QUALIFICATION", "POSTE", "ANCIENNETE", "ENTREE", "SORTIE", "CONTRAT",
    "DEBUT", "FIN", "SP", "SPEC", "CEDEX",
    // les sigles
    "AGIRC", "ARRCO", "CEG", "CET", "APEC", "AGS", "FNAL", "FNAS", "FCAP", "CMB", "AFDAS", "AUDIENS", "CASC", "CNC", "PAS",
    "TA", "TB", "TC", "TU", "SS", "SMIC", "IJSS", "CDDU", "CDI", "CDD", "DSN", "DPAE", "NIR", "INSEE", "TVA", "RIB", "IBAN",
    "BIC", "HT", "TTC", "EUR", "EURO", "CSA", "CRL", "TSS", "PMSS", "JEP", "HEURES", "SUP", "COMP",
    // l'entreprise et les civilites
    "COMPAGNIE", "CIE", "ASSOCIATION", "ASSO", "SARL", "SAS", "SASU", "SA", "SCOP", "SCIC", "EURL", "THEATRE", "BUREAU",
    "PRODUCTION", "ENTREPRISE", "ARTISTIQUE", "CULTURELLE", "VIVANT", "PRIVE", "PUBLIC", "SUBVENTIONNE", "SECTEUR",
    "MONSIEUR", "MADAME", "MADEMOISELLE", "MME", "MLLE", "MR", "DR",
    // les mois
    "JANVIER", "FEVRIER", "MARS", "AVRIL", "MAI", "JUIN", "JUILLET", "AOUT", "SEPTEMBRE", "OCTOBRE", "NOVEMBRE", "DECEMBRE",
];

/// Un mot de bulletin, au singulier comme au pluriel ; un mot compose l'est si chacune de ses
/// parties l'est (« AGIRC-ARRCO »).
fn est_mot_de_bulletin(mot: &str) -> bool {
    let h = haut(mot);
    let un = |x: &str| {
        !x.is_empty() && (MOTS_DE_BULLETIN.contains(&x) || x.strip_suffix('S').is_some_and(|y| MOTS_DE_BULLETIN.contains(&y)))
    };
    un(&h) || (h.contains('-') && h.split('-').filter(|p| !p.is_empty()).all(|p| un(p) || est_particule(p)))
}

/// Le coeur d'un mot : sans « d' » ni « l' » devant.
fn coeur(mot: &str) -> &str {
    for p in ["d'", "D'", "l'", "L'", "d’", "D’", "l’", "L’"] {
        if let Some(r) = mot.strip_prefix(p) {
            if !r.is_empty() {
                return r;
            }
        }
    }
    mot
}

fn genre(mot: &str) -> Genre {
    let c = coeur(mot).trim_end_matches(['-', '\'', '’']);
    let h = haut(c);
    let lettres: Vec<char> = c.chars().filter(|x| x.is_alphabetic()).collect();
    if est_particule(&h) || (lettres.len() == 1 && lettres[0].is_uppercase()) {
        return Genre::Particule;
    }
    if est_mot_de_bulletin(c) {
        return Genre::Autre;
    }
    if lettres.len() >= 2 && lettres.iter().all(|x| x.is_uppercase()) {
        return Genre::Majuscules;
    }
    // Capitalise : une capitale en tete de chaque partie, des minuscules ensuite.
    let mut debut_de_partie = true;
    let mut minuscules = 0;
    for x in c.chars() {
        if x == '-' || x == '\'' || x == '’' {
            debut_de_partie = true;
            continue;
        }
        if debut_de_partie {
            if !x.is_uppercase() {
                return Genre::Autre;
            }
            debut_de_partie = false;
        } else if x.is_lowercase() {
            minuscules += 1;
        } else {
            return Genre::Autre;
        }
    }
    if minuscules > 0 {
        Genre::Capitale
    } else {
        Genre::Autre
    }
}

/// « Prenom NOM », « NOM Prenom », « Jean de LA FONTAINE » : une suite de mots voisins (separes
/// par des blancs seulement) ou se trouvent au moins un mot capitalise ET un mot en majuscules,
/// sans chiffre (un chiffre n'est pas une lettre : il coupe la suite) ni mot de bulletin. Les mots
/// capitalises deviennent `<PRENOM>`, ceux en majuscules `<NOM>`. Un grand blanc (deux espaces, une
/// tabulation) separe d'ordinaire deux colonnes : on prend d'abord les morceaux entre grands
/// blancs, et la suite entiere seulement si aucun morceau ne suffit.
fn noms(t: &mut Travail) {
    let s = t.s.clone();
    let mots: Vec<(usize, usize, Genre)> = motifs().mot.find_iter(&s).map(|x| (x.start(), x.end(), genre(x.as_str()))).collect();
    // Les suites de mots voisins qui ne sont pas « Autre », avec, pour chaque mot, si un grand
    // blanc le precede.
    let mut suites: Vec<Vec<(usize, usize, Genre, bool)>> = Vec::new();
    let mut courante: Vec<(usize, usize, Genre, bool)> = Vec::new();
    for (i, &(a, b, g)) in mots.iter().enumerate() {
        let entre = if i == 0 { "" } else { &s[mots[i - 1].1..a] };
        let voisin = i > 0 && !entre.is_empty() && entre.chars().all(|c| c == ' ' || c == '\t' || c == '\u{a0}');
        let grand = entre.contains('\t') || entre.chars().count() >= 2;
        if g == Genre::Autre || !voisin {
            if !courante.is_empty() {
                suites.push(std::mem::take(&mut courante));
            }
        }
        if g != Genre::Autre {
            courante.push((a, b, g, grand && voisin));
        }
    }
    if !courante.is_empty() {
        suites.push(courante);
    }
    let mut plages_nom = Vec::new();
    let mut plages_prenom = Vec::new();
    for suite in suites {
        let morceaux: Vec<&[(usize, usize, Genre, bool)]> = {
            let mut m = Vec::new();
            let mut debut = 0;
            for k in 1..suite.len() {
                if suite[k].3 {
                    m.push(&suite[debut..k]);
                    debut = k;
                }
            }
            m.push(&suite[debut..]);
            m
        };
        let bons: Vec<&[_]> = morceaux.iter().copied().filter(|m| est_un_nom(m)).collect();
        let a_prendre: Vec<&[_]> = if !bons.is_empty() {
            bons
        } else if est_un_nom(&suite) {
            vec![&suite[..]]
        } else {
            Vec::new()
        };
        for morceau in a_prendre {
            etiqueter(morceau, &mut plages_nom, &mut plages_prenom);
        }
    }
    // Les deux etiquettes ne se chevauchent pas : on pose les noms, puis les prenoms, sur les
    // positions d'origine (les noms poses decalent le texte : on repasse donc par une seule
    // pose, triee, avec l'etiquette de chaque plage).
    let mut toutes: Vec<(usize, usize, usize)> =
        plages_nom.into_iter().map(|(a, b)| (a, b, I_NOM)).chain(plages_prenom.into_iter().map(|(a, b)| (a, b, I_PRENOM))).collect();
    toutes.sort();
    let mut comptes = [0usize; 2];
    let etiquettes: Vec<usize> = toutes.iter().map(|x| x.2).collect();
    let plages: Vec<(usize, usize)> = toutes.iter().map(|x| (x.0, x.1)).collect();
    let mut k = 0;
    t.remplacer(plages, |_, _| {
        let e = etiquettes[k];
        k += 1;
        comptes[if e == I_NOM { 0 } else { 1 }] += 1;
        marque(e)
    });
    t.comptes[I_NOM] += comptes[0];
    t.comptes[I_PRENOM] += comptes[1];
}

/// Sans les particules des bords ; un nom s'il y a un mot capitalise ET un mot en majuscules.
fn sans_bords(m: &[(usize, usize, Genre, bool)]) -> &[(usize, usize, Genre, bool)] {
    let debut = m.iter().position(|x| x.2 != Genre::Particule).unwrap_or(m.len());
    let fin = m.iter().rposition(|x| x.2 != Genre::Particule).map_or(debut, |i| i + 1);
    &m[debut..fin.max(debut)]
}

fn est_un_nom(m: &[(usize, usize, Genre, bool)]) -> bool {
    let m = sans_bords(m);
    m.iter().any(|x| x.2 == Genre::Capitale) && m.iter().any(|x| x.2 == Genre::Majuscules)
}

/// Chaque groupe de mots du meme genre devient une plage ; une particule prend le genre du mot
/// qui la suit (« LE GOFF », « de LA FONTAINE » : le nom).
fn etiqueter(m: &[(usize, usize, Genre, bool)], noms: &mut Vec<(usize, usize)>, prenoms: &mut Vec<(usize, usize)>) {
    let m = sans_bords(m);
    let mut genres: Vec<Genre> = m.iter().map(|x| x.2).collect();
    for i in (0..genres.len()).rev() {
        if genres[i] == Genre::Particule {
            genres[i] = genres.get(i + 1).copied().unwrap_or(Genre::Majuscules);
        }
    }
    let mut i = 0;
    while i < m.len() {
        let mut j = i;
        while j + 1 < m.len() && genres[j + 1] == genres[i] {
            j += 1;
        }
        let plage = (m[i].0, m[j].1);
        if genres[i] == Genre::Capitale {
            prenoms.push(plage);
        } else {
            noms.push(plage);
        }
        i = j + 1;
    }
}
