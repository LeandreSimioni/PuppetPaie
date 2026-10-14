"""Ecrit `paie/src/tables_retours.rs` : les trois tables que la lecture des
retours (`paie/src/retours.rs`) tient de Python.

- les chiffres de `\\d` (les expressions regulieres de Python lisent les
  chiffres decimaux de tout l'Unicode : `str.isdecimal()`) ;
- cp437, l'encodage des noms d'un .zip dont le drapeau UTF-8 n'est pas leve ;
- windows-1252, qu'un XML peut declarer ;
- les noms d'encodage que Python connait (`codecs.lookup`), pour qu'un nom qu'il
  ne connait pas reste un `LookupError`, comme avec `pyexpat` ;
- les caracteres que `expat` (donc ElementTree) admet dans un nom d'element,
  d'attribut ou d'instruction : ceux de XML 1.0 quatrieme edition, bien moins
  nombreux que ceux de la cinquieme que `roxmltree` accepte. Ils sont releves
  en faisant lire `<X/>` et `<aX/>` a ElementTree, un point de code a la fois.

    python outils/generer_tables_retours.py > paie/src/tables_retours.rs
"""
import sys
import unicodedata
import warnings
import xml.etree.ElementTree as ET


def decimaux():
    out = []
    for cp in range(0x110000):
        if 0xD800 <= cp < 0xE000 or not chr(cp).isdecimal():
            continue
        if out and out[-1][1] + 1 == cp:
            out[-1][1] = cp
        else:
            out.append([cp, cp])
    return out


def accepte(texte):
    try:
        ET.fromstring(texte)
        return True
    except ET.ParseError:
        return False


def noms(gabarit):
    """Les intervalles de points de code que `gabarit` (avec X) laisse lire. Le
    deux-points est traite a part."""
    out = []
    for cp in range(0x110000):
        if 0xD800 <= cp < 0xE000 or cp == 0x3A or not accepte(gabarit.replace("X", chr(cp))):
            continue
        if out and out[-1][1] + 1 == cp:
            out[-1][1] = cp
        else:
            out.append([cp, cp])
    return out


def normaliser(nom):
    """`encodings.normalize_encoding`, en minuscules."""
    out, punct = [], False
    for c in nom.lower():
        if c.isalnum() or c == ".":
            if punct and out:
                out.append("_")
            if c.isascii():
                out.append(c)
            punct = False
        else:
            punct = True
    return "".join(out)


def codecs_connus():
    """Les noms d'encodage que Python connait, par ce que `pyexpat` en fait : il
    decode les 256 octets avec le codec (`replace`) et s'en sert comme table si
    cela rend 256 caracteres. `latin1` : la table identite ; `ascii` : l'ASCII,
    le reste est refuse (donc aussi `utf8` ou `utf-8-sig`, qui n'en rendent pas
    plus) ; `cp1252` ; `autres` : une autre table de 256 caracteres (que le
    moteur ne porte pas) ; `refuses` : pas 256 caracteres, expat n'en veut pas."""
    import codecs
    import encodings
    import encodings.aliases
    import pkgutil
    noms = {m.name for m in pkgutil.iter_modules(encodings.__path__)}
    for k, v in encodings.aliases.aliases.items():
        noms |= {k, v}
    toutes = {"latin1": set(), "ascii": set(), "cp1252": set(), "autres": set(), "refuses": set()}
    brut = bytes(range(256))
    ident = brut.decode("latin-1")
    ascii_ = "".join(chr(b) if b < 128 else "\ufffd" for b in range(256))
    cp1252 = brut.decode("cp1252", "replace")
    for n in noms:
        try:
            if not codecs.lookup(n)._is_text_encoding:
                continue
        except LookupError:
            continue
        try:
            t = brut.decode(n, "replace")
        except Exception:
            toutes["refuses"].add(normaliser(n))
            continue
        classe = ("refuses" if len(t) != 256 else "latin1" if t == ident else "ascii" if t == ascii_
                  else "cp1252" if t == cp1252 else "autres")
        toutes[classe].add(normaliser(n))
    return {k: sorted(v) for k, v in toutes.items()}


def octets_vers_texte(codec):
    return [ord(bytes([b]).decode(codec, "replace")) for b in range(128, 256)]


CONNUS = None


def main():
    global CONNUS
    warnings.simplefilter("ignore")
    CONNUS = codecs_connus()
    print("//! GENERE par `outils/generer_tables_retours.py` (Unicode %s de Python) : ne pas editer."
          % unicodedata.unidata_version)
    print()
    print("/// Les intervalles de points de code ou `str.isdecimal()` est vrai (le `\\d` de `re`).")
    print("pub const DECIMAUX: &[(u32, u32)] = &[")
    for a, b in decimaux():
        print("    (0x%X, 0x%X)," % (a, b))
    print("];")
    for nom, codec, doc in (("CP437", "cp437", "cp437 des octets 0x80 a 0xFF"),
                            ("WINDOWS_1252", "cp1252", "windows-1252 des octets 0x80 a 0xFF (U+FFFD : non defini)")):
        print()
        print("/// %s." % doc)
        print("pub const %s: [u32; 128] = [" % nom)
        v = octets_vers_texte(codec)
        for i in range(0, 128, 8):
            print("    " + ", ".join("0x%X" % x for x in v[i:i + 8]) + ",")
        print("];")
    for classe, doc in (("latin1", "la table identite"), ("ascii", "l'ASCII, le reste refuse"), ("cp1252", "windows-1252"),
                        ("autres", "une autre table de 256 caracteres (non portee)"), ("refuses", "pas de table : expat refuse")):
        print()
        print("/// Noms d'encodage normalises que `pyexpat` lit comme %s." % doc)
        print("pub const CODECS_%s: &[&str] = &[" % classe.upper())
        for n in CONNUS[classe]:
            print('    "%s",' % n)
        print("];")
    for nom, gabarit, doc in (("NOM_DEBUT", "<X/>", "peuvent commencer un nom"),
                              ("NOM_SUITE", "<aX/>", "peuvent suivre la premiere lettre d'un nom")):
        print()
        print("/// Les intervalles de points de code qui %s (hors deux-points)." % doc)
        print("pub const %s: &[(u32, u32)] = &[" % nom)
        for a, b in noms(gabarit):
            print("    (0x%X, 0x%X)," % (a, b))
        print("];")


if __name__ == "__main__":
    sys.exit(main())
