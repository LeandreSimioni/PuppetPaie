"""Cherche les donnees personnelles dans ce qui partirait au depot public.

    python atelier/outils/verifier_avant_commit.py            # tout ce qui part
    python atelier/outils/verifier_avant_commit.py --indexes  # ce que git s'apprete a commiter

**REVEILLE LE 14/10/2026 (lot N1), POUR LE JOUR DU PREMIER COMMIT.** Le depot
public part de la racine de ce depot-ci, hors de `prive/` : `logiciel/` (ce
qu'une compagnie recoit), `demo/` (les compagnies fictives), `atelier/` (les
sources, les outils, les docs), `.claude/` et les fichiers de la racine
(`atelier/PLAN-DEPOT-PUBLIC.md`, « La forme du depot public » ;
`atelier/AVANT-LE-PREMIER-COMMIT.md`). Ce qui part, c'est ce que Git
emporterait : les fichiers suivis et ceux qu'un `git add -A` prendrait, les
`.gitignore` appliques par Git lui-meme. `prive/` n'y va jamais.

Le verrou de la suite (`atelier/sources/paie/tests/fichiers/donnees_reelles.rs`)
tient deja, a chaque passage de tests, le nom et le SIREN de la compagnie du
poste, les chemins du poste et les NIR sans la marque d'invention. Ce script-ci
fait ce que le verrou ne peut pas faire sans porter lui-meme ce qu'il cherche :
il LIT les vraies valeurs dans `prive/config/` (SIRET, IBAN, BIC, NIR, adresses,
courriels, identites des salaries) et les cherche dans ce qui part. Il ne
tourne pas dans la suite, et son hook (`atelier/.githooks/pre-commit`) ne se
pose que dans le depot public : dans le depot de travail, prive, il refuserait
chaque commit.

Ce qu'il ne faut surtout pas en faire : nettoyer un test parce qu'il le
signale. Une trouvaille se regarde : une valeur reelle sort du depot public,
une valeur inventee s'inscrit dans `FICTIVES` (ou porte la marque d'invention
d'un NIR, `999`, `123456` ou `000`, la convention du verrou).

**Il ne contient aucune donnee sensible.** Il LIT ce qu'il faut proteger dans
`prive/config/`, qui ne sera pas dans le depot public. Un garde-fou qui
porterait la liste des NIR a proteger serait exactement ce qu'il pretend
empecher.

Pose en hook git, dans le depot public seulement :

    git config core.hooksPath atelier/.githooks

Sortie : 0 si rien n'a ete trouve, 1 sinon, avec la liste fichier par ligne.
"""
from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

# La racine du depot (`atelier/outils/` -> `atelier/` -> la racine) : `logiciel/`, `demo/`, `atelier/`, `.claude/`.
CODE = Path(__file__).resolve().parent.parent.parent

# Ce qui ne part jamais : les donnees de la compagnie du poste.
PRIVE = "prive/"
# Les fichiers ecrits en texte. Les formats de declaration en sont (`.dsn`,
# les gabarits `.txt`, les retours `.xml`) : ceux des demonstrations portent
# des NIR, inventes, qu'il faut voir passer.
EXTENSIONS = {".py", ".md", ".html", ".css", ".js", ".json", ".txt", ".cmd",
              ".pyw", ".modele", ".yml", ".yaml", ".toml", ".rs", ".dsn",
              ".xml", ".csv", ".sh", ".command", ".log"}
# La marque d'un NIR invente, la meme que le verrou de la suite
# (`tests/fichiers/donnees_reelles.rs`, INVENTE) : une commune ou un rang que
# personne n'a, dans les treize premiers chiffres.
INVENTE = ("999", "123456", "000")

# Les valeurs FABRIQUEES qui remplacent les vraies dans les tests et les
# exemples. Elles ne designent personne, mais elles ont la forme du vrai --
# c'est meme leur interet : leurs cles se verifient, donc un test ne peut pas
# passer sur un numero impossible. Sans cette liste, le motif generique les
# signalerait a chaque fois et le garde-fou finirait ignore.
#
# Y ajouter une valeur est un acte visible, relu comme le reste du code. N'y
# mettre QUE des valeurs inventees : cette liste est publiee avec le depot.
FICTIVES = {
    "2930899999999": "NIR feminin fabrique (cle 47)",
    "1850375999999": "NIR masculin fabrique (cle 87)",
    # Les deux ci-dessus finissent par six « 9 », ce que la norme n'admet
    # pas (CCH-16) : ils servent d'exemples dans des comptes rendus, jamais
    # dans une fiche. Celui-ci est fabrique ET declarable -- c'est celui
    # qu'il faut quand un test construit une fiche de salarie.
    "1850375999001": "NIR masculin fabrique et declarable (cle 18)",
    "1234567890123": "NIR de forme invalide, pour les tests de rejet",
    "12345678900017": "SIRET fabrique (cle de Luhn juste)",
    # La compagnie fictive de `outils/compagnies_fictives.py` a besoin d'un
    # compte : la DSN porte un ordre de telepaiement, et un IBAN de forme
    # fausse ferait refuser le fichier par la plate-forme avant qu'on ait
    # rien eprouve. Celui-ci est fabrique, sa cle de controle (85) est juste,
    # et il ne designe aucun compte. Les SIREN, SIRET et NIR de ces
    # compagnies, eux, se calculent a l'execution et ne sont ecrits nulle
    # part dans le code : il n'y a rien a lister pour eux.
    "FR7630001007941234567890185": "IBAN fabrique (cle 85 juste)",
    # (14/10/2026) Les deux suivants se disent inventes dans leur fichier :
    # la vie de demonstration `atelier/outils/demos/les-faisceaux.json`
    # (« Aucune valeur ne designe une entreprise ou une personne reelle »), et
    # `tests/fichiers/caviardage.rs` (« les IBAN sont faits de zeros »).
    "FR7630001007943252044000039": "IBAN de la demonstration Les Faisceaux",
    "FR7630001000000000000000185": "IBAN fait de zeros, test du caviardage",
}


def _secrets_a_proteger() -> list:
    """Les valeurs a ne jamais publier, lues hors du depot.

    Rend une liste de (motif, quoi, casse). `casse` dit si la recherche
    respecte les majuscules : un BIC et un IBAN s'ecrivent en capitales, et
    chercher « CHIFFRE » sans tenir compte de la casse trouve le mot
    « chiffres » dans une phrase francaise -- premiere version de ce script,
    56 fausses alertes.

    Si la configuration est absente -- un clone frais, une autre machine --
    on se rabat sur les motifs generiques, qui suffisent a attraper un NIR ou
    un IBAN quelconque.
    """
    import json

    motifs = [
        # Un NIR : sexe, annee, mois, puis 8 chiffres. Le motif generique
        # attrape aussi ceux qu'on ne connait pas.
        # Sexe, annee, MOIS 01-12, puis 8 chiffres. Le mois est restreint
        # aux valeurs courantes : sans cela, un horodatage « 20260906080000 »
        # se lit comme un NIR. On y perd les immatriculations a mois 20-42,
        # rares ; ce motif n'est qu'un filet, les vraies valeurs a proteger
        # sont lues dans la configuration, plus bas.
        # Treize chiffres, ou quinze avec la cle (14/10/2026 : le motif
        # d'avant, 8 a 9 chiffres apres le mois, laissait passer un NIR ecrit
        # avec sa cle).
        (r"\b[12][0-9]{2}(?:0[1-9]|1[0-2])[0-9]{8}(?:[0-9]{2})?\b", "NIR", True),
        (r"\bFR\d{2} ?(?:\d{4} ?){5}\d{3}\b", "IBAN", True),
        # Un BIC est en capitales et porte au moins un chiffre chez nous.
        # Sans ces deux contraintes, le motif trouvait le mot « chiffres »
        # dans une phrase francaise : 56 fausses alertes, premiere version.
        (r"\b[A-Z]{4}FR[A-Z0-9]{2}[0-9][A-Z0-9]{0,2}\b", "BIC", True),
    ]
    cfg = CODE / "prive" / "config"
    if not cfg.is_dir():
        return motifs

    valeurs = {}
    try:
        ent = json.loads((cfg / "entreprise.json").read_text(encoding="utf-8"))
        for cle in ("siret", "siren", "urssaf_compte",
                    "congesspectacles_affiliation", "affiliation_france_travail"):
            if ent.get(cle):
                valeurs[str(ent[cle])] = cle
        banque = ent.get("banque") or {}
        for cle in ("iban", "bic"):
            if banque.get(cle):
                valeurs[str(banque[cle])] = "banque." + cle
        sal = json.loads((cfg / "salaries.json").read_text(encoding="utf-8"))
        for k, s in sal.items():
            if k.startswith("_"):
                continue
            for cle in ("nir", "num_conges_spectacles", "email", "adresse"):
                if s.get(cle):
                    valeurs[str(s[cle])] = "%s.%s" % (k, cle)
            # Le nom seul est trop courant pour etre un motif ; le nom ET le
            # prenom colles le sont beaucoup moins.
            if s.get("nom") and s.get("prenom"):
                valeurs["%s %s" % (s["nom"], s["prenom"])] = "%s (identite)" % k
    except (OSError, ValueError, KeyError):
        return motifs

    for v, quoi in valeurs.items():
        if len(str(v)) >= 6:              # sous 6 caracteres, trop de faux positifs
            motifs.append((re.escape(str(v)), quoi, False))
    return motifs


def _fichiers(indexes: bool) -> list:
    """Ce qui part : ce que Git commiterait (`--indexes`), ou ce qu'il
    emporterait du depot entier -- hors de `prive/` dans les deux cas."""
    if indexes:
        commande = ["git", "diff", "--cached", "--name-only", "-z",
                    "--diff-filter=ACM"]
    else:
        commande = ["git", "ls-files", "-z", "--cached", "--others",
                    "--exclude-standard"]
    sortie = subprocess.run(commande, cwd=CODE, capture_output=True, check=True)
    noms = sorted({n.decode("utf-8", "replace") for n in sortie.stdout.split(b"\0") if n})
    return [CODE / n for n in noms
            if not n.startswith(PRIVE)
            and Path(n).suffix.lower() in EXTENSIONS and (CODE / n).is_file()]


def _invente(trouve: str, quoi: str) -> bool:
    """Une valeur fabriquee : listee dans `FICTIVES`, ou un NIR qui porte la
    marque d'invention dans ses treize premiers chiffres. Un IBAN se compare
    sans ses espaces (il s'ecrit par groupes de quatre)."""
    if trouve.replace(" ", "") in FICTIVES:
        return True
    return quoi == "NIR" and any(m in trouve[:13] for m in INVENTE)


def verifier(indexes: bool = False) -> list:
    motifs = [(re.compile(m, 0 if casse else re.IGNORECASE), quoi)
              for m, quoi, casse in _secrets_a_proteger()]
    moi = Path(__file__).resolve()
    trouvailles = []
    for f in _fichiers(indexes):
        if f.resolve() == moi:
            continue
        try:
            texte = f.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        for n, ligne in enumerate(texte.splitlines(), 1):
            for motif, quoi in motifs:
                m = next((m for m in motif.finditer(ligne)
                          if not _invente(m.group(0), quoi)), None)
                if m:
                    trouvailles.append({
                        "fichier": str(f.relative_to(CODE)), "ligne": n,
                        "quoi": quoi, "extrait": ligne.strip()[:110]})
                    break
    return trouvailles


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--indexes", action="store_true",
                    help="ne verifier que les fichiers indexes par git")
    args = ap.parse_args(argv)

    trouvailles = verifier(args.indexes)
    if not trouvailles:
        print("Rien de personnel dans le code. C'est publiable.")
        return 0

    print("ARRET - %d occurrence(s) de donnees personnelles :\n" % len(trouvailles))
    par_fichier = {}
    for t in trouvailles:
        par_fichier.setdefault(t["fichier"], []).append(t)
    for fichier, lignes in sorted(par_fichier.items()):
        print("  %s" % fichier)
        for t in lignes:
            print("    ligne %-5d %-22s %s" % (t["ligne"], t["quoi"], t["extrait"]))
        print()
    print("Ces valeurs identifient des personnes reelles. L'historique d'un")
    print("depot est definitif : une fois pousse, efface ne veut plus rien dire.")
    print("Retirer les identites, garder les chiffres -- ce sont eux qui prouvent")
    print("que le moteur est cale. Voir AVANT-LE-PREMIER-COMMIT.md.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
