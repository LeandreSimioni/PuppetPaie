//! Un fichier de donnees JSON s'ecrit toujours aux memes octets, sur toute machine : c'est ce qui permet de comparer
//! les fichiers a l'octet dans le Drive et dans git, et ceux deja ecrits ne changent pas.
//!
//! La forme : une indentation de 2 (`", "`, `": "`, `[]` et `{}` vides), du texte non echappe hors ASCII, une fin de
//! ligne LF ; les flottants sous leur forme la plus courte qui revient au meme nombre, l'exposant signe sur deux
//! chiffres (`1e+16`, `1e-05`) et la decimale gardee (`100.0`). Un entier lu hors de 64 bits devient un flottant a la
//! lecture (pas d'`arbitrary_precision`) : il s'ecrit alors comme le flottant qu'il est devenu.

use std::path::Path;

use serde_json::Value;

use crate::num::Resultat;

/// Le JSON de `v`, indente de `indent`, sous la forme du module.
pub fn json_indente(v: &Value, indent: usize) -> String {
    let mut s = String::new();
    ecrire(v, indent, 0, &mut s);
    s
}

/// Le JSON de `v` sur une ligne : separateurs `, ` et
/// `: `, cles triees (par point de code), non-ASCII laisse tel quel.
pub fn json_trie(v: &Value) -> String {
    let mut s = String::new();
    ecrire_trie(v, &mut s);
    s
}

fn ecrire_trie(v: &Value, s: &mut String) {
    match v {
        Value::Array(a) => {
            s.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                ecrire_trie(x, s);
            }
            s.push(']');
        }
        Value::Object(o) => {
            let mut entrees: Vec<(&String, &Value)> = o.iter().collect();
            entrees.sort_by(|a, b| a.0.cmp(b.0));
            s.push('{');
            for (i, (k, x)) in entrees.into_iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                chaine(k, s);
                s.push_str(": ");
                ecrire_trie(x, s);
            }
            s.push('}');
        }
        autre => ecrire(autre, 0, 0, s),
    }
}

fn marge(s: &mut String, indent: usize, niveau: usize) {
    s.push('\n');
    for _ in 0..indent * niveau {
        s.push(' ');
    }
}

fn ecrire(v: &Value, indent: usize, niveau: usize, s: &mut String) {
    match v {
        Value::Null => s.push_str("null"),
        Value::Bool(true) => s.push_str("true"),
        Value::Bool(false) => s.push_str("false"),
        Value::Number(n) => match (n.as_i64(), n.as_u64()) {
            (Some(i), _) => s.push_str(&i.to_string()),
            (None, Some(u)) => s.push_str(&u.to_string()),
            _ => s.push_str(&repr_flottant(n.as_f64().unwrap_or(0.0))),
        },
        Value::String(t) => chaine(t, s),
        Value::Array(a) if a.is_empty() => s.push_str("[]"),
        Value::Object(o) if o.is_empty() => s.push_str("{}"),
        Value::Array(a) => {
            s.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                marge(s, indent, niveau + 1);
                ecrire(x, indent, niveau + 1, s);
            }
            marge(s, indent, niveau);
            s.push(']');
        }
        Value::Object(o) => {
            s.push('{');
            for (i, (k, x)) in o.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                marge(s, indent, niveau + 1);
                chaine(k, s);
                s.push_str(": ");
                ecrire(x, indent, niveau + 1, s);
            }
            marge(s, indent, niveau);
            s.push('}');
        }
    }
}

/// Une chaine JSON, non-ASCII laisse tel quel : `"` `\` `\b` `\f` `\n` `\r`
/// `\t` nommes, les autres controles en `\u00xx`.
fn chaine(t: &str, s: &mut String) {
    s.push('"');
    for c in t.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\u{8}' => s.push_str("\\b"),
            '\u{c}' => s.push_str("\\f"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            c if (c as u32) < 0x20 => s.push_str(&format!("\\u{:04x}", c as u32)),
            c => s.push(c),
        }
    }
    s.push('"');
}

/// Un flottant ecrit : les plus courts chiffres qui reviennent au meme
/// flottant, en notation positionnelle pour 1e-4 <= |x| < 1e16, sinon
/// `d.ddde+XX` (exposant sur deux chiffres au moins).
pub fn repr_flottant(x: f64) -> String {
    let negatif = x.is_sign_negative();
    let signe = if negatif { "-" } else { "" };
    if x == 0.0 {
        return format!("{}0.0", signe);
    }
    // `{:e}` donne les chiffres les plus courts : `1.2345e-5`.
    let e = format!("{:e}", x.abs());
    let (mantisse, exposant) = e.split_once('e').unwrap_or((&e, "0"));
    let chiffres: String = mantisse.chars().filter(|c| *c != '.').collect();
    let exp: i32 = exposant.parse().unwrap_or(0);
    let chiffres = pair_a_egalite(x.abs(), chiffres, exp);
    let n = chiffres.len() as i32;
    let decpt = exp + 1; // x = 0.CHIFFRES * 10^decpt
    let corps = if decpt <= -4 || decpt > 16 {
        let mut m = chiffres[..1].to_string();
        if n > 1 {
            m.push('.');
            m.push_str(&chiffres[1..]);
        }
        format!("{}e{}{:02}", m, if exp < 0 { '-' } else { '+' }, exp.abs())
    } else if decpt <= 0 {
        format!("0.{}{}", "0".repeat((-decpt) as usize), chiffres)
    } else if decpt >= n {
        format!("{}{}.0", chiffres, "0".repeat((decpt - n) as usize))
    } else {
        format!("{}.{}", &chiffres[..decpt as usize], &chiffres[decpt as usize..])
    };
    format!("{}{}", signe, corps)
}

/// Quand la valeur exacte tombe pile entre deux ecritures de meme longueur
/// (`...403.25` : `.2` et `.3`), la forme ecrite prend la paire (`.2`), la ou Rust prend la
/// superieure : on rend la paire, si elle revient au meme flottant.
fn pair_a_egalite(x: f64, chiffres: String, exp: i32) -> String {
    let n = chiffres.len();
    let dernier = chiffres.as_bytes()[n - 1] - b'0';
    if dernier % 2 == 0 {
        return chiffres;
    }
    let mut dessous = chiffres.clone().into_bytes();
    dessous[n - 1] -= 1;
    let dessous = String::from_utf8(dessous).unwrap_or_default();
    // La valeur exacte, chiffre par chiffre : `DESSOUS` puis `5` puis des zeros.
    let exact = format!("{:.800e}", x);
    let exact: String = exact.split_once('e').map(|(m, _)| m).unwrap_or("").chars().filter(|c| *c != '.').collect();
    let egalite = exact.len() > n && exact.starts_with(&dessous) && exact.as_bytes()[n] == b'5' && exact[n + 1..].bytes().all(|c| c == b'0');
    if !egalite {
        return chiffres;
    }
    let mut candidat = dessous[..1].to_string();
    if n > 1 {
        candidat.push('.');
        candidat.push_str(&dessous[1..]);
    }
    match format!("{}e{}", candidat, exp).parse::<f64>() {
        Ok(y) if y == x => dessous,
        _ => chiffres,
    }
}

/// Ecrit `donnees` dans `chemin` : cree le dossier, ecrit en
/// LF, le JSON suivi d'un saut de ligne.
pub fn ecrire_json(chemin: &Path, donnees: &Value, indent: usize) -> Resultat<()> {
    if let Some(dossier) = chemin.parent() {
        std::fs::create_dir_all(dossier).map_err(|e| format!("{} : {}", dossier.display(), e))?;
    }
    let mut texte = json_indente(donnees, indent);
    texte.push('\n');
    // ATOMIQUE : le fichier s'ecrit a cote puis prend sa place d'un coup. `std::fs::write` tronque d'abord,
    // puis ecrit : un lecteur arrive entre les deux (l'etat du mois se refait dans le fil de la fenetre, `etat_garde::
    // FileDesEtats`, pendant qu'une route relit `journal/depots.json`) lirait un fichier vide, « EOF while
    // parsing a value ». Un journal JSON ne se voit jamais a moitie ecrit.
    // Le fichier d'a cote est a cette ecriture seule (le processus, un numero) : la fenetre sert ses routes dans son
    // processus, et deux fils qui ecrivent le meme fichier ne s'ecrivent jamais l'un sur l'autre. En `.tmp` :
    // ni l'empreinte de l'etat d'un mois ni la sauvegarde ne le voient le temps qu'il existe.
    let provisoire = provisoire(chemin);
    std::fs::write(&provisoire, texte).map_err(|e| format!("{} : {}", provisoire.display(), e))?;
    std::fs::rename(&provisoire, chemin).map_err(|e| format!("{} : {}", chemin.display(), e))
}

/// Le fichier provisoire d'une ecriture de `chemin` : a cote, `<nom>.<processus>-<numero>.tmp`, que nulle autre ecriture
/// ne prend (ni un autre fil, ni un autre processus).
pub fn provisoire(chemin: &Path) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let nom = chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    chemin.with_file_name(format!("{}.{}-{}.tmp", nom, std::process::id(), n))
}

