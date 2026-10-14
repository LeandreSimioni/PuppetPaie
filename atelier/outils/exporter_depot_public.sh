#!/usr/bin/env bash
# LE DEPOT PUBLIC, REFAIT DEPUIS LE DEPOT DE TRAVAIL (14/10/2026 ; Leandre : « on met tout sur le depot public »,
# le depot prive garde `prive/` et recoit les tickets).
#
# Usage :
#   atelier/outils/exporter_depot_public.sh <dossier du depot public>
#
# Copie dans <dossier> tout ce que Git suit ici, SAUF ce qui ne part jamais (EXCLUS, ci-dessous, avec sa raison) ;
# efface dans <dossier> ce qui n'est plus suivi ici (son `.git/` reste) ; ajoute au `.gitignore` du depot public le
# bloc de ce qui n'y entre pas. Il ne commite pas et ne pousse pas : apres lui, dans <dossier>, on relit `git status`
# en entier, on lance le garde-fou (`python3 atelier/outils/verifier_avant_commit.py`) et le verrou de la suite
# (`cargo nextest run --cargo-profile essais -E 'binary(fichiers) & test(/donnees_reelles/)'`), puis on commite.
# Voir `atelier/AVANT-LE-PREMIER-COMMIT.md` §4.
set -euo pipefail

DEPOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CIBLE="${1:?le dossier du depot public}"
mkdir -p "$CIBLE"
CIBLE="$(cd "$CIBLE" && pwd)"
[ "$CIBLE" != "$DEPOT" ] || { echo "Le depot public n'est pas le depot de travail." >&2; exit 2; }

# Ce qui ne part JAMAIS au depot public (un prefixe de chemin, relatif a la racine). La raison est a cote.
EXCLUS=(
    "prive/"                                            # les donnees de la compagnie du poste (regle de fond 9)
    "atelier/prive-dev/"                                # la copie de developpement : les memes donnees (lot N1)
    "import/"                                           # des bulletins reels d'autres compagnies (lot N1)
    "atelier/bac-a-sable/prive/config/coffre.bin"       # un fichier scelle suivi par Git (lot N1)
    "atelier/bac-a-sable/prive/config/.jeton-interface" # idem
    "logiciel/donnees/google-application.json"          # l'application Google : un secret GitHub, pose par le flux « Publier » (14/10/2026)
    "atelier/PUPPETPAIE-TEL-QU-IL-EST.md"               # le recit de la compagnie du banc d'essai (lot N1, point 3)
)

cd "$DEPOT"
git ls-files -z > "$CIBLE/.export-liste.z"
python3 -I - "$DEPOT" "$CIBLE" "${EXCLUS[@]}" <<'PY'
import os, shutil, sys
depot, cible, exclus = sys.argv[1], sys.argv[2], sys.argv[3:]
with open(os.path.join(cible, ".export-liste.z"), "rb") as f:
    suivis = [p.decode("utf-8") for p in f.read().split(b"\0") if p]
os.remove(os.path.join(cible, ".export-liste.z"))
part = [p for p in suivis if not any(p.startswith(e) for e in exclus)]
voulus = set(part)
# 1. Ce qui n'est plus suivi ici s'efface la-bas (jamais `.git/`).
effaces = 0
for racine, ds, fs in os.walk(cible):
    ds[:] = [d for d in ds if not (racine == cible and d == ".git")]
    for f in fs:
        rel = os.path.relpath(os.path.join(racine, f), cible).replace(os.sep, "/")
        if rel not in voulus:
            os.remove(os.path.join(racine, f))
            effaces += 1
for racine, ds, fs in os.walk(cible, topdown=False):
    if racine != cible and not racine.startswith(os.path.join(cible, ".git")) and not os.listdir(racine):
        os.rmdir(racine)
# 2. La copie, octet pour octet, dates comprises.
for rel in part:
    src, dst = os.path.join(depot, rel), os.path.join(cible, rel)
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    shutil.copy2(src, dst)
# 3. Le .gitignore du depot public : le meme, plus ce qui n'y entre jamais.
bloc = """
# ---------------------------------------------------------------------------
# LE DEPOT PUBLIC (bloc ajoute par atelier/outils/exporter_depot_public.sh) :
# les donnees d'une compagnie vivent a cote du code et n'y entrent jamais
# (a la racine seulement : le bac a sable de l'atelier garde son `prive/`,
# qui est une compagnie fictive copiee).
# ---------------------------------------------------------------------------
/prive/
/compagnies/
atelier/prive-dev/
import/
atelier/bac-a-sable/prive/config/coffre.bin
atelier/bac-a-sable/prive/config/.jeton-interface
logiciel/donnees/google-application.json
"""
with open(os.path.join(cible, ".gitignore"), "a", encoding="utf-8", newline="\n") as o:
    o.write(bloc)
print("%d fichiers copies, %d exclus, %d effaces la-bas" % (len(part), len(suivis) - len(part), effaces))
PY
echo "Fait : $CIBLE. Relire 'git status' en entier, lancer le garde-fou et le verrou, puis commiter (AVANT-LE-PREMIER-COMMIT.md §4)."
