#!/bin/sh
# ---------------------------------------------------------------
#  PuppetPaie (Mac). Double-cliquer ; la premiere fois, macOS
#  demande confirmation : clic droit > Ouvrir. Le travail est
#  celui de PuppetPaie.sh, a cote.
# ---------------------------------------------------------------
ICI=$(cd "$(dirname "$0")" && pwd)
exec /bin/sh "$ICI/PuppetPaie.sh" "$@"
