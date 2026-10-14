"""Ecrit `paie/src/tables_nfkc.rs` : les tables de `unicodedata.normalize("NFKC", s)`
pour `paie/src/nfkc.rs` (le texte d'un PDF se compare apres NFKC : ligatures
defaites, espaces insecables ramenees a l'espace -- `moteur/avenant.py`).

Le moteur Rust n'embarque pas de crate de normalisation : les tables sont
produites ici, une fois, par l'Unicode de Python, et collees dans le depot.
Trois tables :
- la decomposition de compatibilite COMPLETE (NFKD) de chaque point de code
  qu'elle change, hors syllabes Hangul (algorithmiques) ;
- la classe de combinaison (`unicodedata.combining`), par intervalles ;
- les compositions canoniques (a, b) -> c, sans les exclusions de composition.

    python outils/generer_nfkc.py > paie/src/tables_nfkc.rs
"""
import sys
import unicodedata as u


def hangul(cp):
    return 0xAC00 <= cp <= 0xD7A3


def decompositions():
    out = []
    for cp in range(0x80, 0x110000):
        if 0xD800 <= cp < 0xE000 or hangul(cp):
            continue
        c = chr(cp)
        r = u.normalize("NFKD", c)
        if r != c:
            out.append((cp, r))
    return out


def classes():
    out = []
    for cp in range(0x110000):
        if 0xD800 <= cp < 0xE000:
            continue
        k = u.combining(chr(cp))
        if not k:
            continue
        if out and out[-1][1] + 1 == cp and out[-1][2] == k:
            out[-1][1] = cp
        else:
            out.append([cp, cp, k])
    return out


def compositions():
    out = []
    for cp in range(0x80, 0x110000):
        if 0xD800 <= cp < 0xE000 or hangul(cp):
            continue
        d = u.decomposition(chr(cp))
        if not d or d.startswith("<"):
            continue
        parties = d.split()
        if len(parties) != 2:
            continue
        # Un caractere exclu de la composition (NFC le laisse decompose) n'y entre pas.
        if u.normalize("NFC", chr(cp)) != chr(cp):
            continue
        out.append((int(parties[0], 16), int(parties[1], 16), cp))
    out.sort()
    return out


def main():
    dec = decompositions()
    print("//! GENERE par `outils/generer_nfkc.py` (Unicode %s de Python) : ne pas editer." % u.unidata_version)
    print("//!")
    print("//! Les tables de `nfkc.rs`. `DECOMP_POINTS[i]` se decompose en `DECOMP_TEXTE`")
    print("//! de `DECOMP_PLACE[i] >> 7` sur `DECOMP_PLACE[i] & 127` octets (NFKD complet).")
    print()
    texte = ""
    places = []
    for cp, r in dec:
        b = r.encode("utf-8")
        assert len(b) < 128
        places.append((len(texte.encode("utf-8")) << 7) | len(b))
        texte += r
    print("pub const DECOMP_POINTS: &[u32] = &[")
    for i in range(0, len(dec), 12):
        print("    " + ", ".join("0x%X" % cp for cp, _ in dec[i:i + 12]) + ",")
    print("];")
    print()
    print("pub const DECOMP_PLACE: &[u32] = &[")
    for i in range(0, len(places), 12):
        print("    " + ", ".join(str(p) for p in places[i:i + 12]) + ",")
    print("];")
    print()
    print("pub const DECOMP_TEXTE: &str = \"%s\";" % "".join(
        "\\u{%x}" % ord(c) for c in texte))
    print()
    print("/// (debut, fin, classe de combinaison), par intervalles.")
    print("pub const CLASSES: &[(u32, u32, u8)] = &[")
    for a, b, k in classes():
        print("    (0x%X, 0x%X, %d)," % (a, b, k))
    print("];")
    print()
    print("/// (premier, second, compose), triees par (premier, second).")
    print("pub const COMPOSITIONS: &[(u32, u32, u32)] = &[")
    for a, b, c in compositions():
        print("    (0x%X, 0x%X, 0x%X)," % (a, b, c))
    print("];")


if __name__ == "__main__":
    sys.exit(main())
