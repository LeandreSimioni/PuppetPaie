"""Releve le tableau des usages du cahier technique NEODeS et l'ecrit en JSON.

Outil de relevé, pas du moteur : il ne tourne qu'a la main, quand une
nouvelle norme sort. Le moteur, lui, lit le JSON produit.

    python3 outils/extraire_usages.py <cahier.pdf> <cible.json>
    python3 outils/extraire_usages.py <tableau-des-usages.xlsx> <cible.json>

Depuis la norme 2027.1.2, net-entreprises sert le tableau des usages en
classeur (`dsn-tableau-des-usages-<version>-<AAAAMMJJ>.xlsx`, fiche 3358 de la
base de connaissances) et plus le cahier en PDF : l'outil lit l'un ou l'autre,
selon l'extension. Le classeur se lit tel quel (`lire_xlsx.py`) : la feuille
« 1 - Tableau des usages », l'entete des natures sur sa ligne (« 01 - DSN
mensuelle »...), une ligne par rubrique (colonne C). Les libelles sont ramenes
a ce que le releve sur PDF rend : blancs ramenes a un, sans blanc final, en
forme composee (NFC ; le classeur 2027.1.2 ecrit S21.G00.72.001 en accents
decomposes).

Le PDF est lu par la fabrique de PDF (`fabrique-pdf --texte` pour choisir les
pages, `--inspecter` pour les mots et leurs rectangles, et les FILETS du
tableau pour retrouver colonnes et lignes), comme tout PDF du logiciel.

COMMENT LE TABLEAU EST RETROUVE. Les pages du tableau sont tournees de 90
degres : l'inspection rend les rectangles dans le repere de la page NON
tournee (celui de PyMuPDF), on les remet donc d'abord a l'endroit. Puis :
  - les COLONNES sont les filets verticaux (traits fins, longs), regroupes
    par abscisse : une abscisse n'est une frontiere de colonne que si ses
    filets, mis bout a bout, couvrent une grande part du tableau ;
  - les LIGNES sont les filets horizontaux, regroupes par ordonnee de la meme
    facon ; une ligne de rubrique peut etre haute de plusieurs lignes de
    texte (un libelle long) ;
  - une CELLULE est le texte des mots dont le centre tombe dans la case, dans
    l'ordre du flux du PDF, separes par une espace.
Une page dont le tableau a moins de huit colonnes n'est pas un tableau d'usages.
"""
import datetime
import hashlib
import json
import re
import sys
import unicodedata
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import lire_pdf as fabrique                                       # noqa: E402
import lire_xlsx                                                  # noqa: E402

# La feuille du classeur des usages (fiche 3358).
FEUILLE_USAGES = "1 - Tableau des usages"

# Un filet est un trait de moins de 1,2 pt d'epaisseur et d'au moins 20 pt de long.
EPAISSEUR_MAX = 1.2
LONGUEUR_MIN = 20.0
# Deux filets dont les centres different de moins de 1 pt sont le meme.
TOLERANCE = 1.0
# Une frontiere de colonne : ses filets couvrent au moins 40 % de ceux de la
# colonne la mieux tracee ; une frontiere de ligne : au moins 150 pt de filet
# (les sept colonnes de natures, a elles seules, en font 170).
PART_COLONNE = 0.4
COUVERTURE_LIGNE = 150.0


def a_l_endroit(page: dict, boite: list) -> list:
    """Le rectangle d'une page tournee de /Rotate, dans le repere de la page
    telle qu'on la lit (origine en haut a gauche)."""
    largeur, hauteur = page["largeur"], page["hauteur"]
    tour = page["rotation"] % 360
    x0, y0, x1, y1 = boite
    if tour == 90:
        return [largeur - y1, x0, largeur - y0, x1]
    if tour == 180:
        return [largeur - x1, hauteur - y1, largeur - x0, hauteur - y0]
    if tour == 270:
        return [y0, hauteur - x1, y1, hauteur - x0]
    return [x0, y0, x1, y1]


def regrouper(filets: list) -> list:
    """[(centre, longueur)] -> [(centre moyen, longueur cumulee)] : les filets
    de meme position sont mis bout a bout."""
    groupes = []
    for centre, longueur in sorted(filets):
        if groupes and centre - groupes[-1][-1][0] <= TOLERANCE:
            groupes[-1].append((centre, longueur))
        else:
            groupes.append([(centre, longueur)])
    return [(sum(c for c, _ in g) / len(g), sum(l for _, l in g))
            for g in groupes]


def cases(page: dict):
    """(frontieres de colonnes, frontieres de lignes) du tableau de la page,
    a l'endroit ; deux listes vides si la page n'a pas de filets."""
    verticaux, horizontaux = [], []
    for trait in page["traits"]:
        x0, y0, x1, y1 = a_l_endroit(page, trait)
        if x1 - x0 <= EPAISSEUR_MAX and y1 - y0 >= LONGUEUR_MIN:
            verticaux.append(((x0 + x1) / 2, y1 - y0))
        elif y1 - y0 <= EPAISSEUR_MAX and x1 - x0 >= LONGUEUR_MIN:
            horizontaux.append(((y0 + y1) / 2, x1 - x0))
    colonnes = regrouper(verticaux)
    if not colonnes:
        return [], []
    seuil = PART_COLONNE * max(c for _, c in colonnes)
    return ([x for x, c in colonnes if c >= seuil],
            [y for y, c in regrouper(horizontaux) if c >= COUVERTURE_LIGNE])


def place(valeur: float, bornes: list):
    """L'indice de la case de `bornes` qui contient `valeur`, ou None."""
    for i in range(len(bornes) - 1):
        if bornes[i] <= valeur < bornes[i + 1]:
            return i
    return None


def lignes_du_tableau(page: dict) -> list:
    """Les lignes du tableau de la page : une liste de cellules (texte, blancs
    ramenes a un), de haut en bas ; [] s'il n'y a pas de tableau."""
    colonnes, lignes = cases(page)
    if len(colonnes) - 1 < 8 or len(lignes) < 2:
        return []
    cellules = {}
    for mot in page["mots"]:
        x0, y0, x1, y1 = a_l_endroit(page, mot["rect"])
        j = place((x0 + x1) / 2, colonnes)
        i = place((y0 + y1) / 2, lignes)
        if i is not None and j is not None:
            cellules.setdefault((i, j), []).append(mot["texte"])
    return [[" ".join(cellules.get((i, j), []))
             for j in range(len(colonnes) - 1)]
            for i in range(len(lignes) - 1)]


def releve(textes: list, inspection: dict):
    """(entete, lignes de rubriques) : l'entete du tableau (titres des
    colonnes) et chaque ligne dont la colonne 2 est un identifiant de
    rubrique, pour les pages du tableau des usages."""
    entete, lignes, pages = None, [], []
    for numero, (texte, page) in enumerate(zip(textes, inspection["pages"]), 1):
        if "Tableau" not in texte[:400] and "01 - DSN mensuelle" not in texte[:400]:
            continue
        for r in lignes_du_tableau(page):
            if r[4].startswith("01 - DSN"):
                entete = r
            elif re.fullmatch(r"S\d\d\.G00\.\d\d\.\d\d\d", r[2] or ""):
                lignes.append(r)
                if numero not in pages:
                    pages.append(numero)
    assert entete, "entete du tableau introuvable"
    return entete, lignes, pages


def cahier(source: str) -> str:
    """La version du cahier, lue dans le nom du PDF depose
    (`dsn-cahier-technique-<version>.pdf`) ; le releve ne la devine pas."""
    m = re.search(r"cahier-technique-(\d[\d.]*\d)\.pdf$", str(source))
    if not m:
        raise SystemExit("version du cahier illisible dans le nom du PDF : %s "
                         "(attendu dsn-cahier-technique-<version>.pdf)" % source)
    return m.group(1)


def propre(cellule) -> str:
    """Une cellule du classeur en texte comme le releve sur PDF le rend."""
    if cellule is None:
        return ""
    return unicodedata.normalize("NFC", " ".join(str(cellule).split()))


def releve_xlsx(source: str):
    """(entete, lignes de rubriques) lues dans le classeur des usages : l'entete
    est la ligne dont la colonne E est « 01 - DSN... », une ligne de rubrique
    celle dont la colonne C est un identifiant de rubrique."""
    entete, lignes = None, []
    for r in lire_xlsx.lignes(source, FEUILLE_USAGES):
        r = [propre(c) for c in r] + [""] * max(0, 11 - len(r))
        if r[4].startswith("01 - DSN"):
            entete = r
        elif re.fullmatch(r"S\d\d\.G00\.\d\d\.\d\d\d", r[2]):
            lignes.append(r)
    assert entete, "entete du tableau introuvable dans la feuille %s" % FEUILLE_USAGES
    return entete, lignes


def classeur(source: str) -> str:
    """La version de la norme, lue dans le nom du classeur depose
    (`dsn-tableau-des-usages-<version>-<AAAAMMJJ>.xlsx`)."""
    m = re.search(r"tableau-des-usages-(\d[\d.]*\d)-\d{8}\.xlsx$", str(source))
    if not m:
        raise SystemExit("version illisible dans le nom du classeur : %s (attendu "
                         "dsn-tableau-des-usages-<version>-<AAAAMMJJ>.xlsx)" % source)
    return m.group(1)


def main(source: str, cible: str):
    jour = datetime.date.today().strftime("%d/%m/%Y")
    if str(source).lower().endswith(".xlsx"):
        entete, lignes = releve_xlsx(source)
        provenance = ("Tableau des usages de la norme DSN %s, classeur %s (feuille "
                      "« %s »). Releve automatiquement par outils/extraire_usages.py le %s."
                      % (classeur(source), Path(source).name, FEUILLE_USAGES, jour))
    else:
        textes = fabrique.texte(source, delai=1800)
        inspection = fabrique.inspecter(source, delai=1800)
        entete, lignes, pages = releve(textes, inspection)
        provenance = ("Cahier technique NEODeS %s, tableau des usages "
                      "(p. %d-%d). Releve automatiquement par "
                      "outils/extraire_usages.py le %s."
                      % (cahier(source), min(pages), max(pages), jour))
    natures = {}
    for j, titre in enumerate(entete[4:], 4):
        m = re.match(r"(\d\d) - (.*)", titre)
        if m:
            natures[m.group(1)] = {"colonne": j, "libelle": m.group(2)}
    usages = {}
    for r in lignes:
        usages[r[2]] = {"libelle": r[3],
                        **{n: r[v["colonne"]] for n, v in natures.items()}}
    sha = hashlib.sha256(open(source, "rb").read()).hexdigest()
    cle_sha = "_sha256_du_classeur" if str(source).lower().endswith(".xlsx") else "_sha256_du_pdf"
    json.dump({
        "_quoi": ("Le tableau des usages du cahier technique NEODeS : pour chaque "
                  "rubrique et chaque nature de declaration, si elle est "
                  "obligatoire (O), conditionnelle (C), interdite (I) ou sans "
                  "objet (N). Ecrire une rubrique que la nature n'admet pas est "
                  "une anomalie BLOQUANTE (CST-04) et le fichier ne declare rien."),
        "_source": provenance,
        cle_sha: sha,
        "_comment_le_verifier": ("Le releve est confronte a ce que le moteur "
                                 "produit : `tests/test_usages.py` verifie que "
                                 "toute rubrique ecrite par `dsn.generer`, "
                                 "`signalement.generer` et `dpae` est admise par "
                                 "sa nature. Une DSN mensuelle reellement "
                                 "acceptee (aout 2026, 161 rubriques) et un "
                                 "signalement d'arret (49) y passent."),
        "natures": {n: v["libelle"] for n, v in natures.items()},
        "_valeurs": {"O": "obligatoire", "C": "conditionnelle",
                     "I": "interdite", "N": "sans objet / non prevue"},
        "usages": usages,
    }, open(cible, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    print("%d rubriques, %d natures -> %s" % (len(usages), len(natures), cible))


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
