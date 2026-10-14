"""Releve le FORMAT de chaque rubrique du cahier technique NEODeS.

Le tableau des usages (`extraire_usages.py`) dit si une rubrique a le droit
d'etre la. Il ne dit pas ce qu'elle doit CONTENIR. Or chaque rubrique du
cahier technique porte, en face de sa definition, sa longueur `[min,max]` et
une expression reguliere de controle nommee CSL -- par exemple, pour un
montant, `-?(([0-9]){1,12})\\.[0-9]{2}`, et pour une date `[0-9]{8}`.

POURQUOI CET OUTIL EXISTE. Le 19/09/2026, `dsn._primes_et_indemnites`
ecrivait le montant d'une indemnite de rupture dans `S21.G00.52.004`, qui est
une DATE, et les deux dates dans `.002` (le montant) et `.003`. Le rejeu de
cinq ans ne pouvait pas l'attraper -- aucune DSN de l'historique ne porte de
bloc 52 --, le tableau des usages non plus : les quatre rubriques ont le
droit d'etre la, c'est leur CONTENU qui etait faux. Une expression de
controle, elle, le dit tout de suite.

Outil de releve, pas du moteur : il ne tourne qu'a la main, quand une norme
sort. Le moteur lit le JSON produit (`donnees/formats-dsn-base.json`).

    python3 outils/extraire_formats.py <cahier.pdf> <cible.json>
    python3 outils/extraire_formats.py <categories-controles-datatypes.xlsx> <cible.json>

LE CLASSEUR (depuis la norme 2027.1.2). Net-entreprises sert les formats en
classeur (`dsn-tableaux-categories-controles-datatypes-usages-<version>-<AAAAMMJJ>.xlsx`,
fiche 3357) et plus le cahier en PDF. Le classeur dit, feuille « Fields », le
type de donnees de chaque rubrique (colonne « DataType Id ») et, feuille
« Data Types », pour chaque type son expression (« Regexp », la CSL 00 du
cahier) et sa longueur (« Lg Min », « Lg Max »). L'outil recompose le meme
JSON : longueur [Lg Min, Lg Max] quand le type en porte une, `csl` = [Regexp]
quand il en porte une. Une expression que Python ne compile pas est ecartee
comme sur le PDF. Une rubrique que la feuille « Fields » ne nomme pas n'a pas
de format (aucun controle) : l'outil la nomme, il ne la reprend pas d'ailleurs.
Preuve de la methode : sur la norme 2027, le classeur 2027.1.2 redonne le releve
du PDF 2027.1.1 rubrique pour rubrique, a l'exception des deux CSL que le journal
de maintenance 2027.1.2 change (evolutions 15 et 18 : S21.G00.40.070,
S21.G00.41.045) et de S89.G00.32.014, absente de « Fields ».

CE QUE LE RELEVE VAUT. Il est lu dans un PDF, donc il peut etre faux, et un
controle faux refuserait une declaration licite -- pire que pas de controle
du tout. D'ou la confrontation de `tests/test_formats.py` : toute valeur de
toute DSN REELLEMENT ACCEPTEE (cinq ans d'historique CoolPaie, la mensuelle
d'aout 2026 deposee en reel) doit passer. Une expression qui refuse une de
ces valeurs est retiree du releve, pas imposee au moteur.
"""
import datetime
import hashlib
import json
import re
import sys
from pathlib import Path

# Le texte du cahier est lu par la fabrique de PDF (`fabrique-pdf --texte`),
# comme tout PDF du logiciel.
sys.path.insert(0, str(Path(__file__).resolve().parent))
import lire_pdf as fabrique                                       # noqa: E402
import lire_xlsx                                                  # noqa: E402

RUBRIQUE = re.compile(r"(?m)^\s*(S\d\d\.G00\.\d\d\.\d\d\d)\s*$")
# UNE EXPRESSION DE CONTROLE VA JUSQU'A LA LIGNE VIDE SUIVANTE. Le PDF la
# coupe en plein milieu d'un token, sans espace : `(20)[0-` puis `9]{2}`. On
# recolle donc en retirant les sauts de ligne, et surtout pas en les
# remplacant par une espace -- ce qui ferait une expression fausse, qui
# refuserait une declaration licite.
CSL = re.compile(r"CSL\s*\d*\s*:\s*(.*?)(?=\n[ \t]*\n)", re.S)
LONGUEUR = re.compile(r"\[(\d+),(\d+)\]")


def relever(chemin: str, ecartees: list) -> dict:
    texte = "\n".join(fabrique.texte(chemin, delai=1800))
    morceaux = RUBRIQUE.split(texte)
    out = {}
    # Chaque identifiant apparait deux fois : dans la liste des rubriques du
    # bloc (sans format) et dans sa definition (avec). On garde celle qui
    # porte un format, et la derniere si plusieurs en portent.
    for i in range(1, len(morceaux) - 1, 2):
        rid, corps = morceaux[i], morceaux[i + 1]
        # On s'arrete au premier saut de bloc : le corps d'une rubrique va
        # jusqu'a la suivante, mais les entetes de page s'y glissent.
        # ET ELLE S'ARRETE A LA PREMIERE ESPACE. Aucune des 260 CSL du
        # cahier n'en contient ; en revanche la mise en page colle
        # parfois le libelle suivant derriere (S10.G00.01.002 recuperait
        # << Table OES - Organismes Emetteurs... >>, ce qui faisait
        # refuser un NIC pourtant accepte cinquante fois).
        csl = [c.replace("\n", "").strip().split(" ")[0]
               for c in CSL.findall(corps)]
        lon = LONGUEUR.search(corps)
        if not csl and not lon:
            continue
        fmt = {}
        if lon:
            fmt["longueur"] = [int(lon.group(1)), int(lon.group(2))]
        if csl:
            # Plusieurs CSL = plusieurs formes admises (CSL 00, CSL 01...).
            # Une expression que Python ne sait pas compiler est LAISSEE
            # TOMBER plutot que reparee a la main -- une expression
            # devinee refuserait des valeurs licites. Deux cas se
            # presentent : une coupure de page, et un renvoi en toutes
            # lettres (S21.G00.81.001 dit << [(table des caracteres
            # autorises pour les adresses)] >>, qui n'est pas une
            # expression). La rubrique garde alors sa seule longueur.
            bonnes = []
            for c in dict.fromkeys(csl):
                try:
                    re.compile(c)
                except re.error:
                    ecartees.append((rid, c))
                    continue
                bonnes.append(c)
            if bonnes:
                fmt["csl"] = bonnes
        out[rid] = fmt
    return out


def relever_xlsx(chemin: str, ecartees: list, sans_type: list) -> dict:
    """Les formats lus dans le classeur des types de donnees (fiche 3357)."""
    types = {}
    entete = None
    for r in lire_xlsx.lignes(chemin, "Data Types"):
        if entete is None:
            entete = r
            continue
        if r and r[0]:
            types[r[0]] = dict(zip(entete, r + [None] * (len(entete) - len(r))))
    out = {}
    entete = None
    for r in lire_xlsx.lignes(chemin, "Fields"):
        if entete is None:
            entete = r
            continue
        if not r or not r[0]:
            continue
        champ = dict(zip(entete, r + [None] * (len(entete) - len(r))))
        rid = "%s.%s" % (str(champ["Block Id"]).strip(), str(champ["Id"]).strip())
        genre = types.get(champ["DataType Id"])
        if genre is None:
            sans_type.append((rid, champ["DataType Id"]))
            continue
        fmt = {}
        if genre.get("Lg Min") is not None and genre.get("Lg Max") is not None:
            fmt["longueur"] = [int(genre["Lg Min"]), int(genre["Lg Max"])]
        expression = (genre.get("Regexp") or "").strip()
        if expression:
            try:
                re.compile(expression)
                fmt["csl"] = [expression]
            except re.error:
                ecartees.append((rid, expression))
        if fmt:
            out[rid] = fmt
    return out


def classeur(source: str) -> str:
    """La version de la norme, lue dans le nom du classeur depose
    (`dsn-tableaux-categories-controles-datatypes-usages-<version>-<AAAAMMJJ>.xlsx`)."""
    m = re.search(r"datatypes-usages-(\d[\d.]*\d)-\d{8}\.xlsx$", str(source))
    if not m:
        raise SystemExit("version illisible dans le nom du classeur : %s (attendu "
                         "dsn-tableaux-categories-controles-datatypes-usages-<version>-<AAAAMMJJ>.xlsx)"
                         % source)
    return m.group(1)


def cahier(source: str) -> str:
    """La version du cahier, lue dans le nom du PDF depose
    (`dsn-cahier-technique-<version>.pdf`) ; le releve ne la devine pas."""
    m = re.search(r"cahier-technique-(\d[\d.]*\d)\.pdf$", str(source))
    if not m:
        raise SystemExit("version du cahier illisible dans le nom du PDF : %s "
                         "(attendu dsn-cahier-technique-<version>.pdf)" % source)
    return m.group(1)


def main():
    source, cible = sys.argv[1], sys.argv[2]
    ecartees, sans_type = [], []
    jour = datetime.date.today().strftime("%d/%m/%Y")
    en_classeur = source.lower().endswith(".xlsx")
    if en_classeur:
        formats = relever_xlsx(source, ecartees, sans_type)
        provenance = ("Formats des rubriques de la norme DSN %s, classeur %s (feuilles "
                      "« Fields » et « Data Types »). Releve automatiquement par "
                      "outils/extraire_formats.py le %s." % (classeur(source), Path(source).name, jour))
    else:
        formats = relever(source, ecartees)
        provenance = ("Cahier technique NEODeS %s, definitions des "
                      "rubriques. Releve automatiquement par "
                      "outils/extraire_formats.py le %s." % (cahier(source), jour))
    sha = hashlib.sha256(open(source, "rb").read()).hexdigest()
    json.dump({
        "_quoi": ("Le format de chaque rubrique du cahier technique NEODeS : "
                  "sa longueur [min,max] et ses expressions de controle CSL. "
                  "Le tableau des usages dit si une rubrique a le droit "
                  "d'etre la ; ceci dit ce qu'elle doit contenir."),
        "_source": provenance,
        ("_sha256_du_classeur" if en_classeur else "_sha256_du_pdf"): sha,
        "_comment_le_verifier": (
            "tests/test_formats.py confronte ce releve a ce qui a ete "
            "REELLEMENT ACCEPTE : les DSN de l'historique CoolPaie et la "
            "mensuelle d'aout 2026 deposee en reel. Une expression qui "
            "refuserait une de ces valeurs serait fausse, et c'est elle "
            "qu'on retire -- un controle faux refuse une declaration licite, "
            "ce qui est pire que pas de controle."),
        "_ce_que_ce_controle_n_est_pas": (
            "Un validateur. Il verifie la FORME d'une valeur, pas sa "
            "justesse : un montant faux au bon format passe."),
        "_expressions_ecartees": [
            {"rubrique": r, "coupee": c} for r, c in ecartees],
        **({"_types_inconnus": [{"rubrique": r, "type": t} for r, t in sans_type]}
           if sans_type else {}),
        "formats": formats,
    }, open(cible, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    avec_csl = sum(1 for f in formats.values() if f.get("csl"))
    print("%d rubriques, dont %d avec expression de controle -> %s"
          % (len(formats), avec_csl, cible))
    for r, c in ecartees:
        print("  ecartee (non compilable) : %s -- %s" % (r, c[:40]))
    for r, t in sans_type:
        print("  type de donnees inconnu : %s -- %s" % (r, t))


main()
