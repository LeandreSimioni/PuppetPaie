#!/bin/sh
# ---------------------------------------------------------------
#  PuppetPaie (Linux, et Mac par PuppetPaie.command)
#  Ouvre la fenetre PuppetPaie : le programme Rust de ce systeme,
#  dans logiciel/ (ce que le flux « Publier » livre depuis le 15/10/2026 :
#  `puppetpaie` et `paie` a cote de `puppetpaie.exe`), sinon dans
#  atelier/sources/paie/bin/ (un depot de travail). Plus de Python (lot J5).
#  Sans la fenetre native (un Linux sans WebKitGTK), la meme page
#  s'ouvre dans le navigateur : `paie interface`.
#  La premiere fois : clic droit > « Lancer comme un programme »,
#  ou ./PuppetPaie.sh dans un terminal.
# ---------------------------------------------------------------
ICI=$(cd "$(dirname "$0")" && pwd)
case "$(uname)" in
    Darwin) BIN="$ICI/sources/paie/bin/mac" ;;
    *)      BIN="$ICI/sources/paie/bin/linux" ;;
esac

LOGICIEL="$ICI/../logiciel"
for D in "$LOGICIEL" "$BIN"; do
    if [ -x "$D/puppetpaie" ] && "$D/puppetpaie" --peut-ouvrir >/dev/null 2>&1; then
        exec "$D/puppetpaie" "$@"
    fi
done
for D in "$LOGICIEL" "$BIN"; do
    if [ -x "$D/paie" ]; then
        exec "$D/paie" interface "$@"
    fi
done

MESSAGE="Le programme PuppetPaie de ce systeme est absent ($LOGICIEL, $BIN).
Reprendre la derniere version du logiciel, puis relancer."
echo "$MESSAGE"
if [ "$(uname)" = "Darwin" ]; then
    osascript -e "display dialog \"$MESSAGE\" buttons {\"OK\"}" >/dev/null 2>&1
elif command -v zenity >/dev/null 2>&1; then
    zenity --error --no-wrap --title PuppetPaie --text "$MESSAGE"
elif command -v xmessage >/dev/null 2>&1; then
    xmessage -center "$MESSAGE"
fi
exit 1
