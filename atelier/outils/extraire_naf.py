"""Releve la NAF 2025 et sa table de correspondance depuis la NAF rev. 2 (INSEE)
et l'ecrit en JSON pour le moteur (`logiciel/donnees/naf-base.json`).

Outil de releve, pas du moteur : il ne tourne qu'a la main, quand l'INSEE
republie ses fichiers. Le moteur lit le JSON produit (`paie/src/naf.rs`).

    python3 outils/extraire_naf.py <structure.xlsx> <correspondances.xlsx> <cible.json>

Les deux classeurs sont ceux de la page INSEE « NAF 2025 », deposes dans
`atelier/reference/sources/insee-naf-2025/` :
  - la structure (`Structure_NAF_2025_Maj_2024-10-04.xlsx`), feuille
    « Sous-classes » : le code (`90.20Y`) et l'intitule de chaque sous-classe ;
  - la correspondance (`Correspondances_NAFrev2-NAF2025.xlsx`), feuille
    « NAF rév. 2 vers NAF 2025 » : pour chaque sous-classe NAF rev. 2, une ligne
    par sous-classe NAF 2025 ou passe une partie de son contenu, avec ce contenu
    et le type (« Unique », « Multiple »).
Les codes s'ecrivent dans le JSON comme la fiche entreprise les porte : quatre
chiffres et une lettre, sans le point (`9020Y`) ; le code INSEE, avec son point,
est garde a cote. Rien n'est interprete : l'outil recopie.
"""
import datetime
import hashlib
import json
import re
import sys
import unicodedata
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import lire_xlsx                                                  # noqa: E402

CODE = re.compile(r"\d\d\.\d\d[A-Z]")


def propre(cellule) -> str:
    if cellule is None:
        return ""
    return unicodedata.normalize("NFC", " ".join(str(cellule).split()))


def sans_point(code: str) -> str:
    return code.replace(".", "")


def sha(chemin) -> str:
    return hashlib.sha256(open(chemin, "rb").read()).hexdigest()


def main(structure: str, correspondances: str, cible: str):
    naf_2025 = {}
    for r in lire_xlsx.lignes(structure, "Sous-classes"):
        r = [propre(c) for c in r] + ["", ""]
        if CODE.fullmatch(r[0]):
            naf_2025[sans_point(r[0])] = {"code": r[0], "intitule": r[1]}
    naf_rev2 = {}
    for r in lire_xlsx.lignes(correspondances, "NAF rév. 2 vers NAF 2025"):
        r = [propre(c) for c in r] + [""] * 6
        if not CODE.fullmatch(r[0]):
            continue
        if not CODE.fullmatch(r[3]) or sans_point(r[3]) not in naf_2025:
            raise SystemExit("correspondance de %s vers un code absent de la structure : %r" % (r[0], r[3]))
        entree = naf_rev2.setdefault(sans_point(r[0]), {"code": r[0], "intitule": r[1], "type": r[5], "vers": []})
        if entree["type"] != r[5]:
            raise SystemExit("type de correspondance incoherent pour %s : %r et %r" % (r[0], entree["type"], r[5]))
        entree["vers"].append({"code": sans_point(r[3]), "code_insee": r[3], "intitule": r[4], "contenu": r[2]})
    communs = sorted(set(naf_2025) & set(naf_rev2))
    if communs:
        raise SystemExit("codes communs aux deux nomenclatures, le moteur ne saurait pas les dater : %s" % communs)
    jour = datetime.date.today().strftime("%d/%m/%Y")
    json.dump({
        "_quoi": ("La NAF 2025 (sous-classes) et la correspondance de chaque sous-classe NAF rev. 2 vers la NAF 2025. "
                  "La fiche entreprise porte le code APE sans point (`9020Y`). Le moteur s'en sert pour reconnaitre un "
                  "code saisi et pour refuser, dans un fichier DSN P27V01, un code NAF rev. 2 qui n'a plus cours, en "
                  "disant vers quels codes NAF 2025 il passe : il ne choisit jamais entre eux, c'est l'INSEE qui "
                  "attribue le code (`paie/src/naf.rs`)."),
        "_source": ("INSEE, page « NAF 2025 » (https://www.insee.fr/fr/information/8181066) : %s (structure, feuille "
                    "« Sous-classes ») et %s (feuille « NAF rév. 2 vers NAF 2025 »), deposes dans "
                    "atelier/reference/sources/insee-naf-2025/. Releve automatiquement par outils/extraire_naf.py le %s."
                    % (Path(structure).name, Path(correspondances).name, jour)),
        "_sha256": {Path(structure).name: sha(structure), Path(correspondances).name: sha(correspondances)},
        "en_vigueur_naf_2025": "2027-01-01",
        "_source_en_vigueur": ("Decret n° 2025-736 du 31/07/2025 (atelier/reference/sources/insee-naf-2025/"
                               "legifrance-decret-2025-736-naf-2025-20250731.txt) : art. 2, la NAF 2025 « entre en "
                               "vigueur le 1er janvier 2027 » ; art. 6, le decret de 2007 (NAF rev. 2) est abroge a la "
                               "meme date pour la nomenclature d'activites."),
        "naf_2025": naf_2025,
        "naf_rev2": naf_rev2,
    }, open(cible, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    print("%d sous-classes NAF 2025, %d sous-classes NAF rev. 2 -> %s" % (len(naf_2025), len(naf_rev2), cible))


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], sys.argv[3])
