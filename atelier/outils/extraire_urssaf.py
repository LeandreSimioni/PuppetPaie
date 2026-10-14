"""Fabrique `donnees/urssaf-dpae-base.json` depuis la Table des Urssaf.

LA SOURCE : « Table des Urssaf », publiee par l'Urssaf Caisse nationale sur
https://fichierdirect.declaration.urssaf.fr/TablesReference.htm parmi les
tables de reference « proposees pour le parametrage urssaf des logiciels de
paye ». La copie relevee est decrite dans `donnees/sources.json` sous la clef
`urssaf-table-des-urssaf`, avec sa date et son condensat.

CE QU'ON EN TIRE : la colonne `Codification` (U117, U200, U917...). Les trois
chiffres qui suivent le U sont le `C_URSSAF` de la DPAE -- positions 133-136
du format, annexe II.1 de la norme. C'est la liste des codes VALIDES, celle
que le point de depot oppose au rejet « Code URSSAF invalide -- n'appartenant
pas a la liste des Codes Urssaf ».

LE TERRITOIRE, LUI, SE LIT AILLEURS -- ET IL SE LIT (22/09/2026). La Table
des Urssaf ne porte que le code postal du SIEGE de chaque organisme, pas de
sa circonscription : ce module a d'abord refuse d'en deduire quoi que ce
soit. L'editeur a insiste, et il avait raison. Deux sources publiees ferment
la chaine, sans rien inventer :

  * la **Liste des sites et des activites** du reseau (open data Urssaf)
    porte `code_organisme_ur_regionale` A COTE de `code_ancienne_region` :
    c'est le pont region -> code Urssaf, publie. 11 -> 117, 82 -> 827,
    94 -> 200 ;
  * le **COG 2015 de l'Insee** donne les 101 departements avec leur region.
    Le millesime 2015 expres : c'est le dernier AVANT la reforme, donc
    celui des ANCIENNES regions -- et les codes Urssaf sont restes sur ce
    decoupage-la (827 Rhone-Alpes et 837 Auvergne coexistent alors que la
    region est une seule depuis 2016). Un COG recent ne saurait pas les
    distinguer.

La chaine est donc departement -> ancienne region -> code Urssaf, deux
tables relevees et datees, et elle couvre les 101 departements sans une
seule ambiguite. Trois precisions :

  - la NORMANDIE porte 237 et 257 dans la liste des sites, heritage de la
    Haute et de la Basse ; seul 287 figure dans la Table des Urssaf, et
    c'est lui qu'on retient. Le filtre est la : un code qui n'est pas dans
    la table officielle n'entre pas ;
  - l'OUTRE-MER n'a pas de code d'ancienne region utilisable ici : le code
    de la CGSS EST celui du departement (971 Guadeloupe, 974 Reunion...),
    comme la Table des Urssaf le montre ;
  - CE N'EST QU'UNE SUGGESTION. Une compagnie en versement en lieu unique,
    une grande entreprise rattachee a un pole TGE, un employeur qui passe
    par le GUSO ne suivent pas la geographie. Le rattachement est un fait
    administratif, ecrit sur les bordereaux : l'ecran presente le code
    deduit, la compagnie le corrige si le sien differe.

    python outils/extraire_urssaf.py
"""
from __future__ import annotations

import csv
import io
import zipfile
import json
import sys
from pathlib import Path

RACINE = Path(__file__).resolve().parent.parent
# La copie de la source vit avec les donnees (hors depot public), comme les
# autres : `donnees/sources.json` dit ou et sous quel nom.
DONNEES = RACINE.parent / "prive" / "reference" / "sources"
CIBLE = RACINE.parent / "logiciel" / "donnees" / "urssaf-dpae-base.json"


def extraire(csv_source: Path) -> dict:
    # Le fichier est en windows-1252 : « Numéro », « Dénomination ».
    lignes = list(csv.DictReader(
        io.StringIO(csv_source.read_bytes().decode("cp1252")), delimiter=";"))
    organismes = []
    for l in lignes:
        cod = (l.get("Codification") or "").strip()
        if not cod.startswith("U"):
            continue
        organismes.append({
            "code": cod[1:],
            "codification": cod,
            "nom": (l.get("Dénomination") or "").strip(),
            # Les valeurs du CSV sont entourees d'apostrophes pour qu'un
            # tableur ne mange pas les zeros de tete.
            "siret": (l.get("SIRET") or "").strip().strip("'"),
            "emetteur": (l.get("Numéro National Emetteur") or "").strip().strip("'"),
        })
    organismes.sort(key=lambda o: o["code"])
    return organismes


def par_region(sites_csv: Path, codes_valides: set) -> dict:
    """Ancienne region -> code Urssaf, depuis la liste des sites du reseau.

    Le filtre `codes_valides` est ce qui tranche le cas de la Normandie :
    la liste des sites y porte encore 237 et 257 (Haute et Basse), mais la
    Table des Urssaf ne connait que 287.
    """
    lignes = list(csv.DictReader(
        io.StringIO(sites_csv.read_bytes().decode("utf-8-sig")), delimiter=";"))
    trouve = {}
    for l in lignes:
        ur = (l.get("code_organisme_ur_regionale") or "").strip()
        reg = (l.get("code_ancienne_region") or "").strip().zfill(2)
        if ur and reg and ur in codes_valides:
            trouve.setdefault(reg, set()).add(ur)
    ambigus = {r: sorted(v) for r, v in trouve.items() if len(v) != 1}
    if ambigus:
        raise ValueError(
            "Plusieurs codes Urssaf pour une meme ancienne region : %s.\n"
            "  La deduction ne peut pas trancher toute seule ; relire la "
            "source avant d'aller plus loin." % ambigus)
    return {r: next(iter(v)) for r, v in trouve.items()}


def par_departement(cog_zip: Path, regions: dict, codes_valides: set) -> dict:
    """Departement -> code Urssaf, par l'ancienne region du COG 2015."""
    with zipfile.ZipFile(cog_zip) as z:
        nom = next(n for n in z.namelist() if n.endswith(".txt"))
        lignes = list(csv.DictReader(
            io.StringIO(z.read(nom).decode("latin-1")), delimiter="\t"))
    sortie, sans = {}, []
    for l in lignes:
        dep = l["DEP"].strip()
        if dep.startswith("97"):
            # Outre-mer : le code de la CGSS EST celui du departement.
            code = dep if dep in codes_valides else None
        else:
            code = regions.get(l["REGION"].strip().zfill(2))
        if code:
            sortie[dep] = code
        else:
            sans.append(dep)
    if sans:
        raise ValueError(
            "Departements sans code Urssaf : %s.\n"
            "  Une source a bouge ; ne pas combler a la main." % ", ".join(sans))
    # La Corse se saisit par son code postal, qui commence par 20 : 2A et 2B
    # n'y apparaissent jamais. Les deux vont au meme organisme.
    if sortie.get("2A") and sortie["2A"] == sortie.get("2B"):
        sortie["20"] = sortie["2A"]
    return dict(sorted(sortie.items()))


def main() -> int:
    tables = sorted(DONNEES.glob("urssaf-table-des-urssaf-*.csv"))
    sites = DONNEES / "urssaf-liste-des-sites-et-des-activites.csv"
    cog = DONNEES / "insee-cog-2015-departements.zip"
    manque = [n for n, p in (("Table des Urssaf", tables[-1] if tables else None),
                             ("Liste des sites", sites), ("COG 2015", cog))
              if p is None or not p.exists()]
    if manque:
        print("Source(s) absente(s) de %s : %s.\n"
              "  `python -m moteur sources` dit d'ou chacune se releve."
              % (DONNEES, ", ".join(manque)), file=sys.stderr)
        return 1

    organismes = extraire(tables[-1])
    codes = {o["code"] for o in organismes}
    regions = par_region(sites, codes)
    departements = par_departement(cog, regions, codes)

    sortie = {
        "_quoi": "Les organismes de recouvrement et leur code a trois "
                 "chiffres, tel que la DPAE l'attend en C_URSSAF (positions "
                 "133-136, annexe II.1 de la norme), et le departement dont "
                 "chacun releve.",
        "_source": "Trois sources relevees et datees -- `python -m moteur "
                   "sources` : urssaf-table-des-urssaf (les codes), "
                   "urssaf-liste-des-sites (region -> code), "
                   "insee-cog-2015-departements (departement -> ancienne "
                   "region). Extrait par outils/extraire_urssaf.py ; ne pas "
                   "editer a la main.",
        "_par_departement_est_une_suggestion":
            "Le rattachement d'une entreprise a une Urssaf est un fait "
            "ADMINISTRATIF, pas une consequence de son adresse : versement en "
            "lieu unique, pole grandes entreprises, GUSO. Cette table dit "
            "l'organisme du territoire, ce qui est le cas ordinaire ; l'ecran "
            "la propose, il ne l'impose pas, et la valeur de la fiche gagne "
            "toujours.",
        "_defaut": "117 -- Urssaf Ile-de-France, la valeur du gabarit quand "
                   "ni la fiche ni le departement ne disent rien.",
        "organismes": organismes,
        "par_ancienne_region": regions,
        "par_departement": departements,
    }
    CIBLE.write_bytes(
        (json.dumps(sortie, ensure_ascii=False, indent=2) + "\n").encode("utf-8"))
    print("%s : %d organismes, %d departements."
          % (CIBLE.name, len(organismes), len(departements)))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
