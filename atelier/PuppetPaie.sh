#!/bin/sh
# ---------------------------------------------------------------
#  PuppetPaie (Linux, et Mac par PuppetPaie.command)
#  Ouvre la fenetre PuppetPaie : le programme Rust de ce systeme,
#  dans atelier/sources/paie/bin/ (rangement du 09/10/2026). Plus de Python (lot J5).
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

if [ -x "$BIN/puppetpaie" ] && "$BIN/puppetpaie" --peut-ouvrir >/dev/null 2>&1; then
    exec "$BIN/puppetpaie" "$@"
fi
if [ -x "$BIN/paie" ]; then
    exec "$BIN/paie" interface "$@"
fi

MESSAGE="Le programme PuppetPaie de ce systeme est absent ($BIN).
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
