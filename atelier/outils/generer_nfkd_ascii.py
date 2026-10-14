"""Ecrit `paie/src/nfkd_ascii.rs` : ce que `unicodedata.normalize("NFKD", c)`
laisse d'ASCII a chaque caractere non ASCII (la DPAE ecrit
`normalize("NFKD", x).encode("ascii", "ignore")`), et les caracteres que
`str.isdigit()` tient pour des chiffres (le SIRET, le telephone, le code
Urssaf de la DPAE le testent).

Le moteur Rust n'embarque pas de crate de normalisation : la table est
produite ici, une fois, par l'Unicode de Python, et collee dans le depot.
Un caractere absent de la table disparait (comme `encode("ascii", "ignore")`).

    python outils/generer_nfkd_ascii.py > paie/src/nfkd_ascii.rs
"""
import sys
import unicodedata


def table():
    ent = []
    for cp in range(0x80, 0x110000):
        r = unicodedata.normalize("NFKD", chr(cp)).encode("ascii", "ignore").decode()
        if r:
            ent.append((cp, r))
    # Les suites : meme texte d'un caractere sur plusieurs points de code
    # (« ÀÁÂ » -> A), ou texte d'un caractere qui avance d'une lettre a chaque
    # point de code (les alphabets pleine chasse, mathematiques...).
    suites = []
    for cp, r in ent:
        if suites and len(r) == 1 and len(suites[-1][2]) == 1 and suites[-1][1] + 1 == cp:
            debut, fin, t, avance = suites[-1]
            if avance is None:
                avance = ord(r) - ord(t)
                ok = avance in (0, 1) and ord(r) == ord(t) + avance * (cp - debut)
            else:
                ok = ord(r) == ord(t) + avance * (cp - debut)
            if ok:
                suites[-1][1], suites[-1][3] = cp, avance
                continue
        suites.append([cp, cp, r, None])
    return suites


def chiffres():
    """Les intervalles de points de code ou `chr(c).isdigit()` est vrai."""
    out = []
    for cp in range(0x110000):
        if 0xD800 <= cp < 0xE000 or not chr(cp).isdigit():
            continue
        if out and out[-1][1] + 1 == cp:
            out[-1][1] = cp
        else:
            out.append([cp, cp])
    return out


def main():
    print("//! GENERE par `outils/generer_nfkd_ascii.py` (Unicode %s de Python) : ne pas editer."
          % unicodedata.unidata_version)
    print("//!")
    print("//! Ce que `unicodedata.normalize(\"NFKD\", c).encode(\"ascii\", \"ignore\")` laisse")
    print("//! de chaque caractere non ASCII. `(debut, fin, texte, avance)` : de `debut` a `fin`,")
    print("//! `texte` ; si `avance`, la lettre de `texte` avance d'un cran a chaque point de code.")
    print()
    print("pub const TABLE: &[(u32, u32, &str, bool)] = &[")
    for debut, fin, t, avance in table():
        print("    (0x%X, 0x%X, %s, %s)," % (debut, fin, '"%s"' % t.replace("\\", "\\\\").replace('"', '\\"'),
                                             "true" if avance == 1 and fin > debut else "false"))
    print("];")
    print()
    print("/// Les intervalles de points de code ou `str.isdigit()` est vrai (Unicode %s)."
          % unicodedata.unidata_version)
    print("pub const CHIFFRES: &[(u32, u32)] = &[")
    for a, b in chiffres():
        print("    (0x%X, 0x%X)," % (a, b))
    print("];")


if __name__ == "__main__":
    sys.exit(main())
