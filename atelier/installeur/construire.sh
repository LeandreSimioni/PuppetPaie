#!/usr/bin/env bash
# L'INSTALLEUR WINDOWS DE PUPPETPAIE, REFAIT D'UN TRAIT (lot I1, 14/10/2026).
#
# Sur un Linux (le conteneur), compile en croise pour Windows 64 bits (chaine GNU, MinGW) :
#   1. les outils, s'ils manquent : MinGW, NSIS, la cible Rust x86_64-pc-windows-gnu ;
#   2. le moteur `moteur.exe` (preuve que le moteur se compile ; il n'est PAS dans l'installeur : une compagnie
#      recoit `logiciel/`, ou la fenetre sert aussi les routes, `puppetpaie route ...`) ;
#   3. la fenetre `puppetpaie.exe` (Tauri 2) ;
#   4. l'installeur NSIS `PuppetPaie-<JJ.MM.AAAA>-installeur.exe` (la version lue, pas l'identifiant), dans `atelier/depot/installeur/`.
#
# Ce que l'installeur pose : exactement ce qu'une compagnie Windows recoit (CLAUDE.md, « Le rangement ») -- les fichiers
# suivis par Git de `logiciel/` et de `fictives/` (ni `logiciel/linux/` ni `logiciel/mac/`, qui ne sont pas suivis), et les
# marques de la livraison de fictives si le flux les a posees --,
# et la fenetre fraichement compilee a la place de `logiciel/puppetpaie.exe`. Rien sur le bridage : sans rien qui dise
# « debloque », le logiciel est bride (ticket #246, moteur/src/bridage.rs).
#
# Usage :
#   atelier/installeur/construire.sh                     tout
#   atelier/installeur/construire.sh --exe-du-logiciel   sans compiler la fenetre : l'installeur prend le
#                                                        `logiciel/puppetpaie.exe` du depot (celui du poste)
#
# CARGO_TARGET_DIR : la cible de compilation (par defaut ~/puppetpaie-cible-windows). Rien n'est ecrit dans
# `logiciel/` ni dans `atelier/sources/moteur/bin/` : le dossier de preparation est `<cible>/installeur/`.
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
        -h|--aide|--help) sed -n '2,23p' "$0"; exit 0 ;;
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
dire "Le moteur (moteur.exe)"
cargo build --release --locked --target "$CIBLE_RUST" --manifest-path "$SOURCES/moteur/Cargo.toml"

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
# Les fichiers suivis par Git, et eux seuls : ni les coffres d'une fictive ouverte (.gitignore), ni `fictives/.origine/`.
git ls-files -z logiciel fictives > "$PREPA/liste.z"
# L'installeur est celui de WINDOWS : les programmes de Linux et de Mac (`logiciel/linux/`, `logiciel/mac/`, que le flux
# « Publier » pose pour les archives de ces systemes et celle de tous ; moteur/src/maj.rs) ne sont jamais suivis
# (.gitignore), donc jamais poses. Si l'un l'etait, l'installeur s'arrete plutot que de les livrer a Windows.
if grep -zqE '^logiciel/(linux|mac)/' "$PREPA/liste.z"; then
    echo "Des programmes Linux ou Mac sont suivis par Git dans logiciel/ : l'installeur Windows ne les pose pas." >&2
    grep -zE '^logiciel/(linux|mac)/' "$PREPA/liste.z" | tr '\0' '\n' >&2
    exit 1
fi
while IFS= read -r -d '' f; do
    mkdir -p "$PREPA/pose/$(dirname "$f")"
    cp -p "$f" "$PREPA/pose/$f"
done < "$PREPA/liste.z"
# Les marques de la livraison de fictives (`fictives/.livraison.json`, `fictives/<nom>/.livree.json`), que le flux
# « Publier » pose avant de fabriquer l'installeur (`moteur maj --marquer-fictives`, moteur/src/maj.rs) : une installation de
# l'installeur retient sa livraison et ne retelecharge pas des fictives qui n'ont pas change. Pas suivies par Git ; hors
# du flux (un poste), il n'y en a pas, et la premiere livraison se pose alors par le marqueur des fictives livrees.
for f in fictives/.livraison.json fictives/*/.livree.json; do
    if [ -f "$f" ]; then cp -p "$f" "$PREPA/pose/$f"; fi
done
cp -p "$FENETRE" "$PREPA/pose/logiciel/puppetpaie.exe"
# L'application Google n'est pas suivie par le depot public : le flux « Publier » la pose depuis un secret GitHub.
[ -f "$DEPOT/logiciel/donnees/google-application.json" ] && cp -p "$DEPOT/logiciel/donnees/google-application.json" "$PREPA/pose/logiciel/donnees/"
VERSION="$(tr -d ' \r\n' < "$DEPOT/logiciel/VERSION")"
[[ "$VERSION" =~ ^[0-9]{4}\.[0-9]{2}\.[0-9]{2}(\.[0-9]+)?$ ]] || { echo "VERSION illisible : '$VERSION'" >&2; exit 1; }
# Ce qu'un humain lit (nom du fichier, pages, « Applications installees ») : JJ.MM.AAAA (Leandre, 14/10/2026 ; le pendant
# de `maj::version_lue`). L'identifiant AAAA.MM.JJ reste dans `logiciel/VERSION` et donne VERSION_WINDOWS.
VERSION_LUE="${VERSION:8:2}.${VERSION:5:2}.${VERSION:0:4}"
# Une deuxieme version du meme jour (AAAA.MM.JJ.N, flux « Publier ») : « JJ.MM.AAAA-N » dans le nom de l'installeur.
[ "${#VERSION}" -gt 10 ] && VERSION_LUE="$VERSION_LUE-${VERSION:11}"

# Les deux listes de l'installeur, tirees du meme dossier : ce qu'il pose, ce que le desinstalleur retire.
# Chemins Windows (`\`) ; un `$` se double pour NSIS (aucun aujourd'hui).
# L'ordre de pose : un dossier, ses fichiers par nom, puis ses sous-dossiers par nom, chacun a son tour (les noms se
# comparent octet par octet, LC_ALL=C). La cle de tri marque chaque dossier d'un `1` et le fichier d'un `0`, et les
# separe par \001, plus petit que tout caractere d'un nom : les fichiers d'un dossier passent avant ses sous-dossiers.
win() { W=${1//\//\\}; W=${W//\$/\$\$}; }
fichiers=()
while IFS= read -r -d '' ligne; do
    fichiers+=("${ligne#*$'\002'}")
done < <(
    cd "$PREPA/pose" && find . -type f -print0 | while IFS= read -r -d '' p; do
        rel=${p#./}; cle=; reste=$rel
        while [ "${reste#*/}" != "$reste" ]; do
            cle="${cle}1${reste%%/*}"$'\001'
            reste=${reste#*/}
        done
        printf '%s0%s\002%s\0' "$cle" "$reste" "$rel"
    done | LC_ALL=C sort -z
)
# Les dossiers du plus profond au moins profond, puis par nom ; RMDir sans /r : un dossier qui garde quelque chose reste.
dossiers=()
while IFS= read -r -d '' ligne; do
    dossiers+=("${ligne#*$'\001'}")
done < <(
    for rel in "${fichiers[@]}"; do
        d=$rel
        while [ "${d%/*}" != "$d" ]; do
            d=${d%/*}
            barres=${d//[!\/]/}
            printf '%04d\001%s\0' "$((9999 - ${#barres}))" "$d"
        done
    done | LC_ALL=C sort -z -u
)
{
    printf "; Genere par construire.sh : ce que l'installeur pose (%d fichiers).\n" "${#fichiers[@]}"
    courant=$'\001'
    for rel in "${fichiers[@]}"; do
        d=; [ "${rel%/*}" != "$rel" ] && d=${rel%/*}
        if [ "$d" != "$courant" ]; then
            win "$d"
            if [ -n "$d" ]; then printf 'SetOutPath "$INSTDIR\\%s"\n' "$W"; else printf 'SetOutPath "$INSTDIR"\n'; fi
            courant=$d
        fi
        p="$PREPA/pose/$rel"
        printf 'File "%s"\n' "${p//\$/\$\$}"
    done
} > "$PREPA/poser.nsh"
{
    printf "; Genere par construire.sh : ce que le desinstalleur retire, et rien d'autre.\n"
    for rel in "${fichiers[@]}"; do win "$rel"; printf 'Delete "$INSTDIR\\%s"\n' "$W"; done
    for d in "${dossiers[@]}"; do win "$d"; printf 'RMDir "$INSTDIR\\%s"\n' "$W"; done
} > "$PREPA/retirer.nsh"
if [ ${#fichiers[@]} -gt 0 ]; then printf '%s\n' "${fichiers[@]}"; else printf '\n'; fi > "$PREPA/liste-posee.txt"
echo "${#fichiers[@]} fichiers, ${#dossiers[@]} dossiers"

# --- 5. L'installeur ------------------------------------------------------------------------------------------
dire "L'installeur (NSIS)"
mkdir -p "$SORTIE"
NOM="PuppetPaie-$VERSION_LUE-installeur.exe"
makensis -V2 -INPUTCHARSET UTF8 \
    "-DVERSION=$VERSION_LUE" \
    "-DVERSION_WINDOWS=$(echo "$VERSION" | awk -F. '{printf "%d.%d.%d.%d", $1, $2, $3, $4}')" \
    "-DPREPA=$PREPA" \
    "-DICONE=$SOURCES/fenetre/icons/icon.ico" \
    "-DSORTIE=$SORTIE/$NOM" \
    "$ICI/puppetpaie.nsi"

dire "Fait"
ls -l "$SORTIE/$NOM"
sha256sum "$SORTIE/$NOM" "$PREPA/pose/logiciel/puppetpaie.exe" "$PREPA/pose/logiciel/WebView2Loader.dll" "$SORTIE_RUST/moteur.exe"
