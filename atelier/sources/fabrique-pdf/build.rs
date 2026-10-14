//! Comprime les polices (zlib, niveau maximal) dans le dossier de sortie : le programme les
//! embarque comprimées (`include_bytes!`), les décomprime au démarrage, mesure le texte avec leurs
//! métriques et en écrit un sous-ensemble dans chaque PDF. Les fichiers de `polices/` restent
//! intacts, tels que Liberation les publie.
//!
//! Avant de comprimer, on retire de la copie du programme les tables dont il ne se sert pas
//! (crénage et substitutions `GPOS`, `GSUB`, `GDEF`, `kern`, quelques autres) et les noms de
//! glyphes de `post` (l'en-tête, qui porte l'angle d'italique, reste) : ni la mise en page, ni le
//! sous-ensemble qu'écrit le PDF ne les lisent. Les glyphes, les métriques et la table `cmap` sont
//! intacts. Ce qui est dans le PDF est le sous-ensemble.

use std::io::Write;

/// Les tables qu'on ne garde pas.
const RETIREES: [&[u8; 4]; 10] = [
    b"GPOS", b"GSUB", b"GDEF", b"kern", b"FFTM", b"gasp", b"DSIG", b"LTSH", b"hdmx", b"VDMX",
];

/// La somme de contrôle d'une table (somme des mots de 32 bits, la fin complétée de zéros).
fn somme(table: &[u8]) -> u32 {
    let mut somme: u32 = 0;
    for bloc in table.chunks(4) {
        let mut mot = [0u8; 4];
        mot[..bloc.len()].copy_from_slice(bloc);
        somme = somme.wrapping_add(u32::from_be_bytes(mot));
    }
    somme
}

/// Le fichier TrueType sans les tables de `RETIREES` (le répertoire refait, les tables alignées).
fn alleger(ttf: &[u8]) -> Vec<u8> {
    let u16_en = |i: usize| u16::from_be_bytes([ttf[i], ttf[i + 1]]) as usize;
    let u32_en = |i: usize| u32::from_be_bytes([ttf[i], ttf[i + 1], ttf[i + 2], ttf[i + 3]]);
    let mut gardees = Vec::new();
    for i in 0..u16_en(4) {
        let e = 12 + 16 * i;
        let nom = [ttf[e], ttf[e + 1], ttf[e + 2], ttf[e + 3]];
        if !RETIREES.iter().any(|r| **r == nom) {
            let (debut, longueur) = (u32_en(e + 8) as usize, u32_en(e + 12) as usize);
            let mut table = ttf[debut..debut + longueur].to_vec();
            if &nom == b"post" && table.len() > 32 {
                // Format 3 : l'en-tête seul, sans les noms de glyphes.
                table.truncate(32);
                table[0..4].copy_from_slice(&0x0003_0000u32.to_be_bytes());
            }
            gardees.push((nom, somme(&table), table));
        }
    }
    let n = gardees.len();
    let mut puissance = 1;
    let mut log = 0;
    while puissance * 2 <= n {
        puissance *= 2;
        log += 1;
    }
    let mut sortie = Vec::new();
    sortie.extend_from_slice(&ttf[0..4]);
    sortie.extend_from_slice(&(n as u16).to_be_bytes());
    sortie.extend_from_slice(&((puissance * 16) as u16).to_be_bytes());
    sortie.extend_from_slice(&(log as u16).to_be_bytes());
    sortie.extend_from_slice(&((n * 16 - puissance * 16) as u16).to_be_bytes());
    let mut corps = Vec::new();
    for (nom, somme, table) in &gardees {
        sortie.extend_from_slice(nom);
        sortie.extend_from_slice(&somme.to_be_bytes());
        sortie.extend_from_slice(&((12 + 16 * n + corps.len()) as u32).to_be_bytes());
        sortie.extend_from_slice(&(table.len() as u32).to_be_bytes());
        corps.extend_from_slice(table);
        while corps.len() % 4 != 0 {
            corps.push(0);
        }
    }
    sortie.extend_from_slice(&corps);
    sortie
}

fn main() {
    let sortie = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    // Sans (le texte courant), puis Serif (Times) et Mono (Courier) pour les blocs à position fixe.
    for nom in [
        "LiberationSans-Regular.ttf",
        "LiberationSans-Bold.ttf",
        "LiberationSans-Italic.ttf",
        "LiberationSans-BoldItalic.ttf",
        "LiberationSerif-Regular.ttf",
        "LiberationSerif-Bold.ttf",
        "LiberationMono-Regular.ttf",
        "LiberationMono-Bold.ttf",
    ] {
        let source = format!("polices/{}", nom);
        println!("cargo:rerun-if-changed={}", source);
        let donnees = std::fs::read(&source).unwrap_or_else(|e| panic!("{} : {}", source, e));
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
        z.write_all(&alleger(&donnees)).expect("compression");
        let comprime = z.finish().expect("compression");
        std::fs::write(sortie.join(format!("{}.z", nom)), comprime).expect("écriture");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
