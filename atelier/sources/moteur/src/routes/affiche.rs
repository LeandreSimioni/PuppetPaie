//! CE QUE L'ECRAN AFFICHE, FAIT POUR L'ECRAN. Un texte que la page montre (le libelle d'une case, le bouton d'une
//! route sous « Avancé », la phrase qu'une route rend, l'aide d'une case de fiche) se lit par une personne : il est bref,
//! et ne porte ni nom de fichier, ni extension, ni chemin, ni terme de code. L'explication longue, qui peut nommer un
//! fichier ou une cle, est pour le LLM (le `sens` d'un champ, le `geste` d'une fiche) et ne s'affiche pas.
//!
//! Une seule fonction le verifie (`defaut`) : a la compilation pour les textes ecrits dans une fiche (un `Affiche` ne
//! nait que par `Affiche::new`, qu'une fiche evalue en constante : un texte fautif ne compile pas), au passage pour les
//! phrases qu'une route compose (`routes::phrase_a_sa_place`), et dans `tests/fichiers/structure.rs` pour tout ce que le
//! catalogue, les routes et `donnees/champs-base.json` rendent a l'ecran.

/// La place d'un texte court : le libelle d'une case, le bouton d'une route, en caracteres.
pub const COURT: usize = 40;

/// Un texte court que l'ecran affiche (un libelle, un bouton) : verifie a sa naissance, il ne se construit pas autrement.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Affiche(&'static str);

impl Affiche {
    /// Le texte, verifie (`defaut`, court) ; dans une constante, un texte fautif ne compile pas.
    pub const fn new(t: &'static str) -> Affiche {
        match defaut(t, COURT, true) {
            None => Affiche(t),
            Some(Defaut::Vide) => panic!("un texte de l'écran est vide"),
            Some(Defaut::TropLong) => panic!("un texte de l'écran passe sa place (40 caractères) : l'écrire plus bref"),
            Some(Defaut::Code) => panic!("un texte de l'écran porte un terme de code : le dire en mots"),
            Some(Defaut::Fichier) => panic!("un texte de l'écran nomme un fichier (une extension) : le dire en mots"),
            Some(Defaut::Chemin) => panic!("un texte de l'écran porte un chemin : le dire en mots"),
            Some(Defaut::Minuscule) => panic!("un libellé ou un bouton commence par une capitale"),
        }
    }

    pub const fn texte(self) -> &'static str {
        self.0
    }
}

/// Ce qui ne va pas dans un texte de l'ecran.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Defaut {
    Vide,
    TropLong,
    Code,
    Fichier,
    Chemin,
    Minuscule,
}

impl Defaut {
    /// Quoi corriger, dans le moteur.
    pub fn corriger(self) -> &'static str {
        match self {
            Defaut::Vide => "il est vide",
            Defaut::TropLong => "il passe sa place : l'écrire plus bref",
            Defaut::Code => "il porte un terme de code : le dire en mots",
            Defaut::Fichier => "il nomme un fichier : le dire en mots",
            Defaut::Chemin => "il porte un chemin : le dire en mots",
            Defaut::Minuscule => "un libellé ou un bouton commence par une capitale",
        }
    }
}

/// Les extensions qu'un texte de l'ecran ne nomme pas.
const EXTENSIONS: [&[u8]; 22] = [
    b"json", b"jsonl", b"pdf", b"csv", b"dsn", b"xml", b"txt", b"md", b"bin", b"zip", b"exe", b"html", b"js", b"css", b"rs",
    b"toml", b"cmd", b"bat", b"xlsx", b"docx", b"odt", b"log",
];

const fn alnum(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c >= 0x80
}

/// Une extension commence en `j` (apres un point) : une des `EXTENSIONS`, suivie d'autre chose qu'une lettre.
const fn extension(b: &[u8], j: usize) -> bool {
    let mut fin = j;
    while fin < b.len() && b[fin].is_ascii_lowercase() {
        fin += 1;
    }
    if fin < b.len() && alnum(b[fin]) {
        return false;
    }
    let mut e = 0;
    while e < EXTENSIONS.len() {
        let x = EXTENSIONS[e];
        if x.len() == fin - j {
            let mut k = 0;
            while k < x.len() && x[k] == b[j + k] {
                k += 1;
            }
            if k == x.len() {
                return true;
            }
        }
        e += 1;
    }
    false
}

/// Le texte commence par une capitale (ou un chiffre) : A-Z, ou une capitale accentuee (À à Þ, sauf ×).
const fn commence_par_une_capitale(b: &[u8]) -> bool {
    match b {
        [c, ..] if c.is_ascii_uppercase() || c.is_ascii_digit() => true,
        [0xC3, c, ..] => *c >= 0x80 && *c <= 0x9E && *c != 0x97,
        _ => false,
    }
}

/// CE QUI NE VA PAS dans `t`, un texte de l'ecran, en `place` caracteres au plus ; `court` : un libelle ou un bouton
/// (une capitale en tete). `None` : il est fait pour l'ecran.
pub const fn defaut(t: &str, place: usize, court: bool) -> Option<Defaut> {
    let b = t.as_bytes();
    if b.is_empty() {
        return Some(Defaut::Vide);
    }
    let mut n = 0;
    let mut i = 0;
    while i < b.len() {
        if b[i] & 0xC0 != 0x80 {
            n += 1;
        }
        i += 1;
    }
    if n > place {
        return Some(Defaut::TropLong);
    }
    i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'`' || c == b'_' || c == b'\\' || c == b'{' || c == b'}' {
            return Some(Defaut::Code);
        }
        // Une option de ligne de commande (`--racine`).
        if c == b'-' && i + 2 < b.len() && b[i + 1] == b'-' && b[i + 2].is_ascii_lowercase() {
            return Some(Defaut::Code);
        }
        // Un chemin : une barre contre une minuscule (`sorties/2026-11`), ou en tete d'un mot (`/home`) ; un sigle
        // (« AT/MP ») n'en est pas un.
        if c == b'/' {
            let avant = if i > 0 { b[i - 1] } else { b' ' };
            let apres = if i + 1 < b.len() { b[i + 1] } else { b' ' };
            if avant.is_ascii_lowercase() || (apres.is_ascii_lowercase() && (avant == b' ' || avant.is_ascii_lowercase())) {
                return Some(Defaut::Chemin);
            }
        }
        if c == b'.' && i > 0 && alnum(b[i - 1]) && extension(b, i + 1) {
            return Some(Defaut::Fichier);
        }
        i += 1;
    }
    if court && !commence_par_une_capitale(b) {
        return Some(Defaut::Minuscule);
    }
    None
}
