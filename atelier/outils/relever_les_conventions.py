"""Le texte complet des conventions collectives suivies, tenu a jour en local.

    python3 atelier/outils/relever_les_conventions.py mettre-a-jour atelier/reference/sources [--reconstruire]
    python3 atelier/outils/relever_les_conventions.py article atelier/reference/sources <IDCC> <numero> [...]
    python3 atelier/outils/relever_les_conventions.py chercher atelier/reference/sources <IDCC> <mot>

POURQUOI (24/09/2026, journal §122). La veille des conventions
(`moteur/kali.py`) est incrementale, mais elle ne garde que la LISTE des
textes de chaque convention -- pas leurs articles. Le 24/09/2026, relever les
clauses de rupture de la 3090 puis de la 1285 a demande de retelecharger
deux fois la base KALI entiere (180 Mo) et ses 246 livraisons, et de filtrer
a la main. Cet outil garde, dans `kali-entrepot/`, TOUS les articles des
conventions citees par `donnees/sources.json` -- clauses communes, annexes,
avenants, toutes versions --, et la liste des livraisons deja appliquees :
un article se lit en une seconde, et une mise a jour ne telecharge que les
livraisons nouvelles.

Un article se rend avec son identifiant KALI, son etat (VIGUEUR_ETEN,
VIGUEUR_NON_ETEN, ABROGE...), ses dates et le texte auquel il appartient :
c'est ce qu'une transcription dans `atelier/reference/sources/` recopie.

La base entiere ne se relit que pour construire l'entrepot, ou avec
`--reconstruire` (une convention nouvelle a suivre, par exemple).
"""
from __future__ import annotations

import html
import json
import os
import re
import sys
import time
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from entrepot_dila import Entrepot, archive, en_ordre, flux   # noqa: E402

BASE = "https://echanges.dila.gouv.fr/OPENDATA/KALI/"
MANIFESTE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "logiciel",
                         "donnees", "sources.json")
DOSSIER = "kali-entrepot"

RE = {k: re.compile(r"<%s>(.*?)</%s>" % (k, k), re.S)
      for k in ("ID", "NUM", "ETAT", "DATE_DEBUT", "DATE_FIN", "CONTENU")}
RE_TXT = re.compile(r'<TITRE_TXT[^>]*id_txt="([^"]*)"[^>]*>(.*?)</TITRE_TXT>', re.S)
RE_TM = re.compile(r"<TITRE_TM[^>]*>(.*?)</TITRE_TM>", re.S)


def suivies() -> dict:
    """{conteneur KALICONT: IDCC} -- les conventions que `sources.json` cite,
    et celles qu'il met de cote a relever."""
    m = json.load(open(MANIFESTE, encoding="utf-8"))
    out = {}
    sources = m.get("sources") or ()
    for s in (sources.values() if isinstance(sources, dict) else sources):
        k = s.get("kali") if isinstance(s, dict) else None
        if k and k.get("conteneur"):
            out[k["conteneur"]] = str(k.get("idcc") or "")
    # Les conventions que le moteur ne porte pas encore, gardees pour etre
    # relues avant d'ecrire leur couche (25/09/2026, journal §130).
    for idcc, c in (m.get("conventions_a_relever") or {}).items():
        if not idcc.startswith("_") and isinstance(c, dict) and c.get("conteneur"):
            out[c["conteneur"]] = idcc
    return out


def _nettoyer(x: str) -> str:
    x = re.sub(r"<br\s*/?>|</p>", "\n", x)
    x = html.unescape(re.sub(r"<[^>]+>", "", x))
    return re.sub(r"\n\s*\n+", "\n\n", x).strip()


def _lire(xml: str, conteneurs: dict):
    """(cle, article) d'un fichier d'article, ou None s'il n'est d'aucune
    convention suivie. La cle est (IDCC, identifiant KALIARTI)."""
    cont = next((c for c in conteneurs if c in xml), None)
    if cont is None:
        return None
    g = lambda k: (RE[k].search(xml).group(1).strip() if RE[k].search(xml) else "")  # noqa: E731
    txt = RE_TXT.search(xml)
    ident = g("ID")
    if not ident:
        return None
    return (conteneurs[cont], ident), {
        "id": ident, "num": g("NUM"), "etat": g("ETAT"),
        "debut": g("DATE_DEBUT"), "fin": g("DATE_FIN"),
        "conteneur": cont,
        "texte": txt.group(1) if txt else "",
        "titre_texte": _nettoyer(txt.group(2)) if txt else "",
        "chemin": " > ".join(_nettoyer(t) for t in RE_TM.findall(xml)),
        "contenu": _nettoyer(g("CONTENU")),
    }


def _passer(tar, ent: Entrepot, conteneurs: dict, livraison: str = "") -> int:
    n = 0
    for m in tar:
        if not m.isfile():
            continue
        if m.name.endswith(".dat") and "suppression" in m.name:
            retires = {l.strip().rsplit("/", 1)[-1] for l in tar.extractfile(m)
                       .read().decode("utf-8", "replace").splitlines() if l.strip()}
            for cle in [k for k in ent.articles if k[1] in retires]:
                del ent.articles[cle]
            continue
        if "/article/" not in m.name:
            continue
        xml = tar.extractfile(m).read().decode("utf-8", "replace")
        lu = _lire(xml, conteneurs)
        if lu is None:
            continue
        cle, a = lu
        ancien = ent.articles.get(cle)
        if livraison and ancien and (ancien["etat"], ancien["contenu"]) != (a["etat"], a["contenu"]):
            ent.etat.setdefault("changes", []).append(
                {"idcc": cle[0], "id": cle[1], "num": a["num"], "livraison": livraison,
                 "avant": ancien["etat"], "apres": a["etat"],
                 "texte_change": ancien["contenu"] != a["contenu"]})
        ent.articles[cle] = a
        n += 1
    return n


def _livraisons() -> list:
    page = urllib.request.urlopen(BASE, timeout=120).read().decode("utf-8", "replace")
    return sorted(set(re.findall(r"KALI_(\d{8}-\d{6})\.tar\.gz", page)))


def _globale() -> str:
    page = urllib.request.urlopen(BASE, timeout=120).read().decode("utf-8", "replace")
    noms = sorted(set(re.findall(r"(Freemium_kali_global_\d{8}-\d{6}\.tar\.gz)", page)))
    if not noms:
        raise RuntimeError("aucune archive globale KALI a %s" % BASE)
    return noms[-1]


def mettre_a_jour(racine: str, reconstruire: bool = False) -> Entrepot:
    conteneurs = suivies()
    ent = Entrepot(os.path.join(racine, DOSSIER))
    manquantes = set(conteneurs) - set((ent.etat.get("conteneurs") or {}))
    if ent.existe() and not reconstruire:
        ent.charger()
        manquantes = set(conteneurs) - set(ent.etat.get("conteneurs") or {})
    if not ent.existe() or reconstruire or manquantes:
        # UNE CONVENTION NOUVELLE A SUIVRE, ou la premiere fois : la base
        # entiere defile une fois (180 Mo), puis les livraisons depuis sa
        # date.
        glob = _globale()
        print("== base KALI entiere : %s ==" % glob, flush=True)
        ent.articles, t0 = {}, time.time()
        n = _passer(flux(BASE + glob), ent, conteneurs)
        ent.etat = {"global": glob, "lues": [], "changes": [],
                    "conteneurs": conteneurs, "construit_le": time.strftime("%Y-%m-%d")}
        print("  %d articles gardes en %.0fs" % (n, time.time() - t0), flush=True)
        ent.sauver()
    date_globale = re.search(r"(\d{8})", ent.etat["global"]).group(1)
    toutes = [l for l in _livraisons() if l[:8] >= date_globale]
    neuves = [l for l in toutes if l not in ent.lues]
    print("== %d livraison(s) nouvelle(s) sur %d ==" % (len(neuves), len(toutes)), flush=True)
    for i, (url, octets) in enumerate(en_ordre(["%sKALI_%s.tar.gz" % (BASE, l)
                                                 for l in neuves]), 1):
        nom = neuves[i - 1]
        if isinstance(octets, Exception):
            print("  ! livraison %s : %s -- arret, reprise au prochain passage"
                  % (nom, octets))
            break
        _passer(archive(octets), ent, conteneurs, nom)
        ent.etat["lues"].append(nom)
        if i % 50 == 0:
            ent.sauver()
            print("  [%d/%d] %s" % (i, len(neuves), nom), flush=True)
    ent.sauver()
    print("== %d articles, %d livraisons appliquees ==" % (len(ent.articles), len(ent.lues)))
    return ent


def charger(racine: str) -> Entrepot:
    ent = Entrepot(os.path.join(racine, DOSSIER))
    if not ent.existe():
        raise SystemExit("Pas d'entrepot KALI dans %s : lancer d'abord `mettre-a-jour`."
                         % racine)
    return ent.charger()


def articles(ent: Entrepot, idcc: str, num: str = None, en_vigueur: bool = True) -> list:
    """Les articles d'une convention, d'un numero donne s'il y en a un ; les
    versions en vigueur seulement, par defaut."""
    out = [a for (i, _), a in ent.articles.items() if i == str(idcc)
           and (num is None or a["num"] == num)
           and (not en_vigueur or a["etat"].startswith("VIGUEUR"))]
    return sorted(out, key=lambda a: (a["texte"], a["num"], a["debut"]))


def transcrire(a: dict) -> str:
    """Un article comme les transcriptions de `atelier/reference/sources/` le
    recopient."""
    return ("=" * 78 + "\n%s -- article %s\n%s -- %s du %s au %s\ntexte : %s (%s)\n%s\n%s\n"
            % (a["chemin"][:120], a["num"] or "(chapeau)", a["id"], a["etat"], a["debut"],
               a["fin"], a["titre_texte"], a["texte"], "-" * 78, a["contenu"]))


def main() -> int:
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    verbe, racine = sys.argv[1], sys.argv[2]
    if verbe == "mettre-a-jour":
        mettre_a_jour(racine, "--reconstruire" in sys.argv)
    elif verbe == "article" and len(sys.argv) >= 5:
        ent = charger(racine)
        for num in sys.argv[4:]:
            trouves = articles(ent, sys.argv[3], num)
            if not trouves:
                print("(aucun article %s en vigueur pour l'IDCC %s)" % (num, sys.argv[3]))
            for a in trouves:
                print(transcrire(a))
    elif verbe == "chercher" and len(sys.argv) >= 5:
        ent = charger(racine)
        mot = sys.argv[4].lower()
        for a in articles(ent, sys.argv[3]):
            if mot in a["contenu"].lower():
                print("%s  %-8s %-22s %s" % (a["id"], a["num"], a["etat"], a["chemin"][:90]))
    else:
        print(__doc__)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
