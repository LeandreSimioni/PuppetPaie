//! Le JSON ASCII et compact qu'on scelle ou qu'on envoie a un service : `\uXXXX` hors ASCII, cles triees a la demande,
//! separateurs `, ` et `: ` (ou `,` et `:` serres) ; les flottants sous leur forme la plus courte qui se relit, en
//! notation scientifique hors de 1e-4 .. 1e16. Les octets qu'un coffre deja scelle ou qu'une signature couvre ne
//! changent pas d'une version a l'autre.

use serde_json::Value;


/// Un flottant en texte : le plus court qui se relit, notation scientifique hors de
/// 1e-4 .. 1e16.
pub fn repr_flottant(f: f64) -> String {
    if f.is_nan() {
        return "NaN".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    let signe = if f.is_sign_negative() { "-" } else { "" };
    let sci = format!("{:e}", f.abs());
    let (mant, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let chiffres: String = mant.chars().filter(|c| *c != '.').collect();
    let n = chiffres.len() as i32;
    let decpt = exp + 1;
    let corps = if f == 0.0 {
        "0.0".to_string()
    } else if decpt > 16 || decpt < -3 {
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

/// Le JSON en ASCII (tout autre caractere en `\uXXXX`), cles triees si `trie`. Sans
/// `indent`, les separateurs sont `, ` et `: ` (ou `,` et `:` si `serre`).
pub fn dumps(v: &Value, indent: Option<usize>, serre: bool, trie: bool) -> String {
    let mut s = String::new();
    ecrire(v, indent, serre, trie, 0, &mut s);
    s
}

fn chaine_json(t: &str, s: &mut String) {
    s.push('"');
    for c in t.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            '\u{8}' => s.push_str("\\b"),
            '\u{c}' => s.push_str("\\f"),
            ' '..='~' => s.push(c),
            c => {
                let mut tampon = [0u16; 2];
                for u in c.encode_utf16(&mut tampon) {
                    s.push_str(&format!("\\u{:04x}", u));
                }
            }
        }
    }
    s.push('"');
}

fn ecrire(v: &Value, indent: Option<usize>, serre: bool, trie: bool, niveau: usize, s: &mut String) {
    let (sep_item, sep_cle) = match (indent, serre) {
        (Some(_), _) => (",", ": "),
        (None, true) => (",", ":"),
        (None, false) => (", ", ": "),
    };
    let retrait = |n: usize| match indent {
        Some(i) => format!("\n{}", " ".repeat(i * n)),
        None => String::new(),
    };
    match v {
        Value::Null => s.push_str("null"),
        Value::Bool(b) => s.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => match n.as_f64().filter(|_| n.is_f64()) {
            Some(f) => s.push_str(&repr_flottant(f)),
            None => s.push_str(&n.to_string()),
        },
        Value::String(t) => chaine_json(t, s),
        Value::Array(a) => {
            if a.is_empty() {
                s.push_str("[]");
                return;
            }
            s.push('[');
            s.push_str(&retrait(niveau + 1));
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    s.push_str(sep_item);
                    s.push_str(&retrait(niveau + 1));
                }
                ecrire(x, indent, serre, trie, niveau + 1, s);
            }
            s.push_str(&retrait(niveau));
            s.push(']');
        }
        Value::Object(o) => {
            if o.is_empty() {
                s.push_str("{}");
                return;
            }
            let mut entrees: Vec<(&String, &Value)> = o.iter().collect();
            if trie {
                entrees.sort_by(|a, b| a.0.cmp(b.0));
            }
            s.push('{');
            s.push_str(&retrait(niveau + 1));
            for (i, (k, x)) in entrees.into_iter().enumerate() {
                if i > 0 {
                    s.push_str(sep_item);
                    s.push_str(&retrait(niveau + 1));
                }
                chaine_json(k, s);
                s.push_str(sep_cle);
                ecrire(x, indent, serre, trie, niveau + 1, s);
            }
            s.push_str(&retrait(niveau));
            s.push('}');
        }
    }
}
