"""Ecrit `paie/src/tables_journal.rs` : les caracteres que `str.isalnum()` de
Python admet (le rapprochement des noms de `journal._meme_nom`).

    python outils/generer_tables_journal.py > paie/src/tables_journal.rs
"""
import sys
import unicodedata


def alnum():
    out = []
    for cp in range(0x110000):
        if 0xD800 <= cp < 0xE000 or not chr(cp).isalnum():
            continue
        if out and out[-1][1] + 1 == cp:
            out[-1][1] = cp
        else:
            out.append([cp, cp])
    return out


def main():
    w = sys.stdout.write
    w("//! GENERE par `outils/generer_tables_journal.py` (Unicode %s de Python) : ne pas editer.\n\n" % unicodedata.unidata_version)
    w("/// Les intervalles de points de code ou `str.isalnum()` est vrai.\n")
    w("pub const ALNUM: &[(u32, u32)] = &[\n")
    for a, b in alnum():
        w("    (0x%X, 0x%X),\n" % (a, b))
    w("];\n")


main()
