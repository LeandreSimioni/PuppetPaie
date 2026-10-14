"""Lire une feuille d'un classeur .xlsx, pour les extracteurs de l'atelier.

Depuis la norme 2027.1.2, net-entreprises ne sert plus le tableau des usages et
les formats des rubriques que dans des classeurs (fiches 3357 et 3358 de la base
de connaissances) : le cahier technique 2027.1.2 en PDF n'est pas dans la
categorie. Un .xlsx est une archive zip de XML ; la bibliotheque standard suffit
(aucune dependance), et ce qui est lu est la valeur de la cellule telle que le
classeur la porte : texte partage, texte en ligne ou nombre. Rien n'est
interprete ici (pas de formule recalculee, pas de format applique).

    lignes(chemin, "Fields")  ->  [[cellule, ...], ...]   (None pour une case vide)
"""
import re
import zipfile
import xml.etree.ElementTree as ET

NS = {"m": "http://schemas.openxmlformats.org/spreadsheetml/2006/main",
      "r": "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
      "p": "http://schemas.openxmlformats.org/package/2006/relationships"}


def _colonne(reference: str) -> int:
    """`C12` -> 2 (colonnes comptees depuis 0)."""
    n = 0
    for c in re.match(r"[A-Z]+", reference).group(0):
        n = n * 26 + ord(c) - 64
    return n - 1


def _texte(noeud) -> str:
    """Le texte d'un <si> ou d'un <is> : ses <t>, bout a bout (texte riche compris)."""
    return "".join(t.text or "" for t in noeud.iter("{%s}t" % NS["m"]))


def feuilles(chemin) -> list:
    """Les noms des feuilles du classeur, dans l'ordre."""
    with zipfile.ZipFile(chemin) as z:
        classeur = ET.fromstring(z.read("xl/workbook.xml"))
    return [f.get("name") for f in classeur.find("m:sheets", NS)]


def lignes(chemin, feuille: str) -> list:
    """Les lignes de la feuille nommee, de haut en bas, chacune la liste de ses
    cellules de la premiere colonne a la derniere renseignee ; une ligne absente
    du classeur est rendue vide. Une feuille inconnue arrete tout."""
    with zipfile.ZipFile(chemin) as z:
        classeur = ET.fromstring(z.read("xl/workbook.xml"))
        liens = ET.fromstring(z.read("xl/_rels/workbook.xml.rels"))
        cible = {l.get("Id"): l.get("Target") for l in liens.findall("p:Relationship", NS)}
        rid = None
        for f in classeur.find("m:sheets", NS):
            if f.get("name") == feuille:
                rid = f.get("{%s}id" % NS["r"])
        if rid is None:
            raise SystemExit("feuille « %s » absente de %s (feuilles : %s)"
                             % (feuille, chemin, ", ".join(feuilles(chemin))))
        chemin_feuille = cible[rid].lstrip("/")
        if not chemin_feuille.startswith("xl/"):
            chemin_feuille = "xl/" + chemin_feuille
        partages = []
        if "xl/sharedStrings.xml" in z.namelist():
            partages = [_texte(si) for si in
                        ET.fromstring(z.read("xl/sharedStrings.xml")).findall("m:si", NS)]
        donnees = ET.fromstring(z.read(chemin_feuille)).find("m:sheetData", NS)
    rangs = []
    for rang in donnees.findall("m:row", NS):
        numero = int(rang.get("r"))
        cellules = {}
        for c in rang.findall("m:c", NS):
            genre, v = c.get("t"), c.find("m:v", NS)
            if genre == "inlineStr":
                valeur = _texte(c.find("m:is", NS))
            elif v is None or v.text is None:
                continue
            elif genre == "s":
                valeur = partages[int(v.text)]
            elif genre in ("str", "e"):
                valeur = v.text
            elif genre == "b":
                valeur = v.text == "1"
            else:
                valeur = float(v.text) if re.search(r"[.eE]", v.text) else int(v.text)
            cellules[_colonne(c.get("r"))] = valeur
        while len(rangs) < numero - 1:
            rangs.append([])
        largeur = max(cellules) + 1 if cellules else 0
        rangs.append([cellules.get(j) for j in range(largeur)])
    return rangs
