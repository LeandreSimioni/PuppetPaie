"""L'entrepot local des textes de la DILA : lu une fois, tenu a jour ensuite.

POURQUOI (24/09/2026, journal §122). La moisson des codes relisait, a CHAQUE
passage, l'archive globale de la base LEGI (1,1 Go) puis toutes les
livraisons quotidiennes parues depuis (438 le 24/09/2026), et ne gardait que
les articles qu'on lui demandait. Un article de plus -- le 231 du code
general des impots pour la taxe sur les salaires -- coutait donc une heure.
Et les conventions collectives, dont la veille ne garde que la liste des
textes, se relisaient a la main dans la base KALI entiere.

Ce module tient les deux choses qui manquaient :

  1. LE TEXTE COMPLET, garde en local et compresse : tous les articles des
     codes et des conventions suivis, pas seulement ceux qu'on cite. Ajouter
     un article, c'est le lire dans l'entrepot -- quelques secondes, rien a
     telecharger.
  2. LA LISTE DES LIVRAISONS DEJA APPLIQUEES : a chaque passage, seules les
     nouvelles se telechargent -- une par soir ouvre, quelques centaines de
     Ko --, plusieurs a la fois, et s'appliquent dans l'ordre.

L'archive globale ne se relit que pour construire l'entrepot la premiere
fois, ou si on le demande (`--reconstruire`).

Ce module ne sait rien des codes ni des conventions : il range des articles
sous une cle, et il sait faire defiler des archives. Les deux outils qui s'en
servent (`relever_les_codes.py`, `relever_les_conventions.py`) disent quoi
garder.
"""
from __future__ import annotations

import concurrent.futures as cf
import gzip
import io
import json
import os
import tarfile
import time
import urllib.request

ARTICLES = "articles.jsonl.gz"
ETAT = "etat.json"


class Entrepot:
    """Des articles sous une cle (un tuple de chaines), et l'etat de lecture.

    `articles` : {cle: {"id", "debut", "fin", "etat", "xml", ...}} ;
    `etat` : {"global", "lues", "changes", "construit_le", "mis_a_jour_le"}.
    """

    def __init__(self, dossier: str):
        self.dossier = dossier
        self.articles: dict = {}
        self.etat: dict = {"global": "", "lues": [], "changes": []}

    def existe(self) -> bool:
        return (os.path.exists(os.path.join(self.dossier, ARTICLES))
                and os.path.exists(os.path.join(self.dossier, ETAT)))

    def charger(self) -> "Entrepot":
        with gzip.open(os.path.join(self.dossier, ARTICLES), "rt", encoding="utf-8") as f:
            for ligne in f:
                a = json.loads(ligne)
                self.articles[tuple(a.pop("cle"))] = a
        with open(os.path.join(self.dossier, ETAT), encoding="utf-8") as f:
            self.etat = json.load(f)
        return self

    def sauver(self) -> None:
        os.makedirs(self.dossier, exist_ok=True)
        tmp = os.path.join(self.dossier, ARTICLES + ".tmp")
        # Trie : deux sauvegardes du meme contenu donnent le meme fichier, et
        # Git ne voit changer que ce qui a change.
        with gzip.GzipFile(tmp, "wb", mtime=0) as g:
            for cle in sorted(self.articles):
                g.write((json.dumps(dict(self.articles[cle], cle=list(cle)),
                                    ensure_ascii=False, sort_keys=True)
                         + "\n").encode("utf-8"))
        os.replace(tmp, os.path.join(self.dossier, ARTICLES))
        self.etat["mis_a_jour_le"] = time.strftime("%Y-%m-%d")
        with open(os.path.join(self.dossier, ETAT), "w", encoding="utf-8") as f:
            json.dump(self.etat, f, ensure_ascii=False, indent=1, sort_keys=True)

    @property
    def lues(self) -> set:
        return set(self.etat.get("lues") or ())


def telecharger(url: str, delai: int = 300, essais: int = 4) -> bytes:
    """Une archive entiere en memoire, avec trois nouvelles tentatives : une
    coupure de transport n'est pas une reponse."""
    derniere = None
    for n in range(essais):
        try:
            return urllib.request.urlopen(url, timeout=delai).read()
        except Exception as e:                                 # noqa: BLE001
            derniere = e
            time.sleep(2 * (n + 1))
    raise derniere


def en_ordre(urls: list, fils: int = 8):
    """Telecharge plusieurs archives a la fois, et les rend DANS L'ORDRE de
    la liste : une livraison s'applique apres celle de la veille, jamais
    avant. Rend (url, octets ou exception)."""
    with cf.ThreadPoolExecutor(fils) as ex:
        futurs = [ex.submit(telecharger, u) for u in urls]
        for u, f in zip(urls, futurs):
            try:
                yield u, f.result()
            except Exception as e:                             # noqa: BLE001
                yield u, e


def flux(url: str):
    """Une archive lue en flux, sans jamais l'ecrire sur le disque (l'archive
    globale de LEGI fait 1,1 Go)."""
    return tarfile.open(fileobj=urllib.request.urlopen(url, timeout=600), mode="r|gz")


def archive(octets: bytes):
    return tarfile.open(fileobj=io.BytesIO(octets), mode="r:gz")

