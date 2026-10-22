#!/bin/bash
# Seances Claude dans le nuage seulement : la suite de tests se compile d'avance, en arriere-plan, des l'ouverture
# du conteneur (6 min a froid) ; la suite livree seulement : celle de l'atelier a sa propre cible et ne se lance que
# quand on touche a l'atelier (`atelier/CLAUDE.md`, « Les tests »). A la fin de la seance, `cargo nextest run` ne
# recompile que ce qui a change (`cargo test` si nextest manque).
# Le poste de Leandre, une installation ordinaire, n'est pas concerne.
set -euo pipefail

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

# On ne developpe que dans le nuage (22/10/2026) : le garde des donnees reelles (`atelier/.githooks/pre-commit`) y juge
# chaque commit, avec le `paie` que la compilation d'avance produit (profil `essais`).
git -C "$CLAUDE_PROJECT_DIR" config core.hooksPath atelier/.githooks || true

PAIE="$CLAUDE_PROJECT_DIR/atelier/sources/paie"
JOURNAL="${TMPDIR:-/tmp}/puppetpaie-compilation-davance.log"
if [ -d "$PAIE" ] && command -v cargo >/dev/null 2>&1; then
  # cargo-nextest manque dans l'image du conteneur (13/10/2026) : le binaire precompile se telecharge en quelques
  # secondes ; si le reseau le refuse, la suite retombe sur `cargo test`.
  if ! cargo nextest --version >/dev/null 2>&1; then
    (curl -LsSf --max-time 120 https://get.nexte.st/latest/linux | tar zxf - -C "${CARGO_HOME:-$HOME/.cargo}/bin") 2>/dev/null || true
  fi
  if cargo nextest --version >/dev/null 2>&1; then
    (cd "$PAIE" && nohup cargo nextest run --cargo-profile essais --no-run >"$JOURNAL" 2>&1 &) || true
  else
    (cd "$PAIE" && nohup cargo test --profile essais --no-run >"$JOURNAL" 2>&1 &) || true
  fi
fi
exit 0
