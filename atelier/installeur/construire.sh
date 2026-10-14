#!/usr/bin/env bash
# L'INSTALLEUR WINDOWS DE PUPPETPAIE, REFAIT D'UN TRAIT (lot I1, 14/10/2026).
#
# Sur un Linux (le conteneur), compile en croise pour Windows 64 bits (chaine GNU, MinGW) :
#   1. les outils, s'ils manquent : MinGW, NSIS, la cible Rust x86_64-pc-windows-gnu ;
#   2. le moteur `paie.exe` (preuve que le moteur se compile ; il n'est PAS dans l'installeur : une compagnie
#      recoit `logiciel/`, ou la fenetre sert aussi les routes, `puppetpaie route ...`) ;
#   3. la fenetre `puppetpaie.exe` (Tauri 2) ;
#   4. l'installeur NSIS `PuppetPaie-<VERSION>-installeur.exe`, dans `atelier/depot/installeur/`.
#
# Ce que l'installeur pose : exactement ce qu'une compagnie recoit (CLAUDE.md, « Le rangement ») -- les fichiers
# suivis par Git de `logiciel/` et de `demo/` --, la fenetre fraichement compilee a la place de
# `logiciel/puppetpaie.exe`, et `logiciel/EDITION.json` (toujours demo, a cote de ce script).
#
# Usage :
#   atelier/installeur/construire.sh                     tout
#   atelier/installeur/construire.sh --exe-du-logiciel   sans compiler la fenetre : l'installeur prend le
#                                                        `logiciel/puppetpaie.exe` du depot (celui du poste)
#
# CARGO_TARGET_DIR : la cible de compilation (par defaut ~/puppetpaie-cible-windows). Rien n'est ecrit dans
# `logiciel/` ni dans `atelier/sources/paie/bin/` : le dossier de preparation est `<cible>/installeur/`.
# Rien ne s'execute de ce qui est fabrique (pas de Windows ici) : l'essai se fait sur un poste Windows.

set -euo pipefail

ICI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEPOT="$(cd "$ICI/../.." && pwd)"
SOURCES="$DEPOT/atelier/sources"
CIBLE_RUST="x86_64-pc-windows-gnu"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/puppetpaie-cible-windows}"
PREPA="$CARGO_TARGET_DIR/installeur"
SORTIE="$DEPOT/atelier/depot/installeur"

EXE_DU_LOGICIEL=0
for a in "$@"; do
    case "$a" in
        --exe-du-logiciel) EXE_DU_LOGICIEL=1 ;;
        -h|--aide|--help) sed -n '2,25p' "$0"; exit 0 ;;
        *) echo "Option inconnue : $a" >&2; exit 2 ;;
    esac
done

dire() { printf '\n== %s\n' "$*"; }

# --- 1. Les outils --------------------------------------------------------------------------------------------
dire "Les outils"
manque=()
command -v x86_64-w64-mingw32-gcc >/dev/null || manque+=(gcc-mingw-w64-x86-64)
command -v makensis >/dev/null || manque+=(nsis)
if [ ${#manque[@]} -gt 0 ]; then
    if command -v apt-get >/dev/null && [ "$(id -u)" = 0 ]; then
        apt-get update -qq
        apt-get install -y -qq "${manque[@]}"
    else
        echo "Il manque : ${manque[*]} (apt-get install ${manque[*]}, en administrateur)." >&2
        exit 1
    fi
fi
rustup target list --installed | grep -qx "$CIBLE_RUST" || rustup target add "$CIBLE_RUST"
echo "MinGW   : $(x86_64-w64-mingw32-gcc --version | head -1)"
echo "NSIS    : $(makensis -VERSION)"
echo "Rust    : $(rustc --version)"
echo "Cible   : $CARGO_TARGET_DIR"

# --- 2. et 3. Le moteur, puis la fenetre -------------------------------------------------------------------------
SORTIE_RUST="$CARGO_TARGET_DIR/$CIBLE_RUST/release"
dire "Le moteur (paie.exe)"
cargo build --release --locked --target "$CIBLE_RUST" --manifest-path "$SOURCES/paie/Cargo.toml"

if [ "$EXE_DU_LOGICIEL" = 1 ]; then
    dire "La fenetre : celle du depot (logiciel/puppetpaie.exe), sans compiler"
    FENETRE="$DEPOT/logiciel/puppetpaie.exe"
else
    dire "La fenetre (puppetpaie.exe)"
    cargo build --release --locked --target "$CIBLE_RUST" --manifest-path "$SOURCES/fenetre/Cargo.toml"
    FENETRE="$SORTIE_RUST/puppetpaie.exe"
fi
test -f "$FENETRE" || { echo "La fenetre est absente : $FENETRE" >&2; exit 1; }

# --- 4. Le dossier de preparation : ce que l'installeur pose, a l'identique -------------------------------------
dire "Ce que l'installeur pose"
rm -rf "$PREPA"
mkdir -p "$PREPA/pose"
cd "$DEPOT"
# Les fichiers suivis par Git, et eux seuls : ni les coffres d'une demo ouverte (.gitignore), ni `demo/.origine/`.
git ls-files -z logiciel demo > "$PREPA/liste.z"
while IFS= read -r -d '' f; do
    mkdir -p "$PREPA/pose/$(dirname "$f")"
    cp -p "$f" "$PREPA/pose/$f"
done < "$PREPA/liste.z"
cp -p "$FENETRE" "$PREPA/pose/logiciel/puppetpaie.exe"
cp -p "$ICI/EDITION.json" "$PREPA/pose/logiciel/EDITION.json"
VERSION="$(tr -d ' \r\n' < "$DEPOT/logiciel/VERSION")"
[[ "$VERSION" =~ ^[0-9]{4}\.[0-9]{2}\.[0-9]{2}$ ]] || { echo "VERSION illisible : '$VERSION'" >&2; exit 1; }

# Les deux listes de l'installeur, tirees du meme dossier : ce qu'il pose, ce que le desinstalleur retire.
# Chemins Windows (`\`) ; un `$` se double pour NSIS (aucun aujourd'hui).
python3 -I - "$PREPA" <<'PY'
import os, sys
prepa = sys.argv[1]
pose = os.path.join(prepa, "pose")
fichiers, dossiers = [], set()
for racine, ds, fs in os.walk(pose):
    ds.sort()
    for f in sorted(fs):
        rel = os.path.relpath(os.path.join(racine, f), pose)
        fichiers.append(rel)
        d = os.path.dirname(rel)
        while d:
            dossiers.add(d)
            d = os.path.dirname(d)
win = lambda p: p.replace("/", "\\").replace("$", "$$")
with open(os.path.join(prepa, "poser.nsh"), "w", encoding="utf-8", newline="\n") as o:
    o.write("; Genere par construire.sh : ce que l'installeur pose (%d fichiers).\n" % len(fichiers))
    courant = None
    for rel in fichiers:
        d = os.path.dirname(rel)
        if d != courant:
            o.write('SetOutPath "$INSTDIR\\%s"\n' % win(d) if d else 'SetOutPath "$INSTDIR"\n')
            courant = d
        o.write('File "%s"\n' % os.path.join(pose, rel).replace("$", "$$"))
with open(os.path.join(prepa, "retirer.nsh"), "w", encoding="utf-8", newline="\n") as o:
    o.write("; Genere par construire.sh : ce que le desinstalleur retire, et rien d'autre.\n")
    for rel in fichiers:
        o.write('Delete "$INSTDIR\\%s"\n' % win(rel))
    # Les dossiers du plus profond au moins profond ; RMDir sans /r : un dossier qui garde quelque chose reste.
    for d in sorted(dossiers, key=lambda x: (-x.count("/"), x)):
        o.write('RMDir "$INSTDIR\\%s"\n' % win(d))
with open(os.path.join(prepa, "liste-posee.txt"), "w", encoding="utf-8", newline="\n") as o:
    o.write("\n".join(fichiers) + "\n")
print("%d fichiers, %d dossiers" % (len(fichiers), len(dossiers)))
PY

# --- 5. L'installeur ------------------------------------------------------------------------------------------
dire "L'installeur (NSIS)"
mkdir -p "$SORTIE"
NOM="PuppetPaie-$VERSION-installeur.exe"
makensis -V2 -INPUTCHARSET UTF8 \
    "-DVERSION=$VERSION" \
    "-DVERSION_WINDOWS=$(echo "$VERSION" | awk -F. '{printf "%d.%d.%d.0", $1, $2, $3}')" \
    "-DPREPA=$PREPA" \
    "-DICONE=$SOURCES/fenetre/icons/icon.ico" \
    "-DSORTIE=$SORTIE/$NOM" \
    "$ICI/puppetpaie.nsi"

dire "Fait"
ls -l "$SORTIE/$NOM"
sha256sum "$SORTIE/$NOM" "$PREPA/pose/logiciel/puppetpaie.exe" "$PREPA/pose/logiciel/WebView2Loader.dll" "$SORTIE_RUST/paie.exe"
