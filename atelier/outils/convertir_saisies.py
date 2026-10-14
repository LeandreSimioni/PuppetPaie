"""Convertit une saisie de paie de l'ancienne ecriture a l'ecriture par lignes.

    python outils/convertir_saisies.py paies/2026-08.json          # montre
    python outils/convertir_saisies.py paies/2026-08.json --ecrire  # ecrit

L'ancienne ecriture posait `dates`, `cachets`, `heures`, `taux_horaire` sur
l'engagement ; la nouvelle porte tout sur des LIGNES DE PAIE, chacune avec sa
rubrique, sa quantite, son taux, ses dates et son lieu :

    {"cachets": 1, "heures": 12, "taux_horaire": 20, "dates": [...]}
        -> {"lignes": [{"rubrique": "cachet", "quantite": 1, "taux": 130, "dates": [...]},
                       {"rubrique": "heure",  "quantite": 12, "taux": 20}]}

Le moteur ne lit plus l'ancienne (depuis le 13/09/2026) : un fichier qui la
porte encore est refuse avec un renvoi vers cet outil. La conversion est
STRICTEMENT EQUIVALENTE -- `tests/test_non_regression.py` produit un mois
des deux facons et compare les fichiers octet pour octet -- et ne touche a
rien d'autre : `_note`, `valide`, `numero`, `jours`, `date_paiement` restent.

Les heures d'un contrat mixte deviennent une ligne `heure`, pas
`enseignement`, meme quand c'etait de la masterclass : c'est ce que
l'ancienne ecriture declarait, et on ne reecrit pas le passe. Une note le
dit dans le fichier.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

ANCIENS = ("dates", "cachets", "heures", "taux_horaire")


def est_ancienne(e: dict) -> bool:
    return any(k in e for k in ANCIENS)


def convertir_engagement(e: dict, taux_cachet_mois) -> dict:
    """Un engagement, converti. Ne modifie pas l'original."""
    if not est_ancienne(e):
        return dict(e)
    e = dict(e)
    dates = list(e.pop("dates", None) or [])
    cachets = e.pop("cachets", None)
    heures = e.pop("heures", None)
    taux_horaire = e.pop("taux_horaire", None)
    taux_cachet = e.pop("taux_cachet", taux_cachet_mois)
    lignes = list(e.get("lignes") or [])

    if lignes:
        # Deja des lignes, mais les dates etaient restees sur l'engagement :
        # elles descendent sur la premiere ligne qui n'en a pas.
        if dates and not any(l.get("dates") for l in lignes):
            lignes[0] = dict(lignes[0], dates=dates)
    else:
        if cachets is None:
            cachets = len(dates)             # l'ancien defaut : un cachet par date
        if cachets:
            lignes.append({"rubrique": "cachet", "quantite": cachets,
                           "taux": taux_cachet, "dates": dates})
        if heures:
            # Sans cachet, le taux du cachet tenait lieu de taux horaire.
            taux = taux_horaire if taux_horaire is not None else taux_cachet
            l = {"rubrique": "heure", "quantite": heures, "taux": taux}
            if not cachets:
                l["dates"] = dates
            lignes.append(l)
        if not lignes and dates:
            lignes.append({"rubrique": "cachet", "quantite": 0,
                           "taux": taux_cachet, "dates": dates})
    if e.get("lieu"):
        # Le lieu de l'engagement vaut pour chaque ligne qui n'en dit pas.
        for l in lignes:
            l.setdefault("lieu", e["lieu"])
        e.pop("lieu")
    e["lignes"] = lignes
    return e


def convertir(data: dict) -> tuple:
    """La saisie entiere. Rend (donnees, nombre d'engagements convertis)."""
    data = dict(data)
    taux = data.get("taux_cachet", 130.0)
    n = 0
    out = []
    for e in data.get("engagements") or []:
        if est_ancienne(e):
            n += 1
            out.append(convertir_engagement(e, taux))
        else:
            out.append(dict(e))
    data["engagements"] = out
    return data, n


def main(argv=None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    ecrire = "--ecrire" in argv
    chemins = [a for a in argv if not a.startswith("--")]
    if not chemins:
        print(__doc__)
        return 2
    for c in chemins:
        p = Path(c)
        data = json.loads(p.read_text(encoding="utf-8"))
        neuf, n = convertir(data)
        if not n:
            print("%s : rien a convertir." % p)
            continue
        if ecrire:
            avant = p.parent / "avant"
            avant.mkdir(exist_ok=True)
            (avant / (p.name + ".avant-conversion")).write_bytes(p.read_bytes())
            p.write_text(json.dumps(neuf, ensure_ascii=False, indent=2) + "\n",
                         encoding="utf-8")
            print("%s : %d engagement(s) converti(s), ancienne copie dans %s"
                  % (p, n, avant))
        else:
            print("%s : %d engagement(s) a convertir (--ecrire pour le faire)" % (p, n))
            print(json.dumps(neuf, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
