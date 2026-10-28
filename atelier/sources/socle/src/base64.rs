//! Le base64 standard (RFC 4648, alphabet `+/`, bourrage `=`) : les pieces qu'un service rend ou attend en texte
//! (le corps d'un depot, une archive de retour, une cle publique, un PDF que la page affiche).

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Les octets `o` en base64, bourres de `=`.
pub fn ecrire(o: &[u8]) -> String {
    let mut s = String::with_capacity(o.len().div_ceil(3) * 4);
    for bloc in o.chunks(3) {
        let n = (bloc[0] as u32) << 16 | (*bloc.get(1).unwrap_or(&0) as u32) << 8 | *bloc.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= bloc.len() {
                s.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

/// Le texte `t` en octets ; les blancs sont sautes, la lecture s'arrete au premier `=`.
pub fn lire(t: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(t.len() / 4 * 3);
    let (mut acc, mut n) = (0u32, 0);
    for c in t.bytes().filter(|c| !c.is_ascii_whitespace()) {
        if c == b'=' {
            break;
        }
        let v = ALPHABET.iter().position(|&a| a == c).ok_or("demande illisible : `base64` n'est pas du base64")?;
        acc = acc << 6 | v as u32;
        n += 6;
        if n >= 8 {
            n -= 8;
            out.push((acc >> n & 0xff) as u8);
        }
    }
    Ok(out)
}
