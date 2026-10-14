"""Lire un PDF par le moteur Rust (`paie pdf --texte` / `--inspecter`), pour les
extracteurs de l'atelier. Le moteur Python, qui portait cet appel, est retire
(lot K, 09/10/2026) ; les extracteurs n'en gardaient que ces deux modes.

Le programme : `PUPPETPAIE_FABRIQUE` s'il nomme un `paie`, sinon
`paie/target/release/paie`, sinon celui que le depot livre (`paie/bin/<systeme>/`).
Il doit repondre `fabrique-pdf 3` a `--version`.
"""
import json
import os
import platform
import subprocess
from pathlib import Path

FORMAT = 3


def _paie() -> Path:
    voulu = os.environ.get("PUPPETPAIE_FABRIQUE", "").strip()
    racine = Path(__file__).resolve().parent.parent / "sources" / "paie"
    systeme = {"Windows": "windows", "Darwin": "mac"}.get(platform.system(), "linux")
    candidats = [Path(voulu)] if voulu else []
    candidats += [racine / "target" / "release" / n for n in ("paie", "paie.exe")]
    candidats += [racine / "bin" / systeme / n for n in ("paie", "paie.exe")]
    for p in candidats:
        if p.is_file() and p.stem == "paie":
            r = subprocess.run([str(p), "pdf", "--version"], capture_output=True, text=True, timeout=30)
            if r.returncode == 0 and r.stdout.strip() == "fabrique-pdf %d" % FORMAT:
                return p
    raise SystemExit("le moteur `paie` est introuvable ou ne parle pas `fabrique-pdf %d` "
                     "(`cargo build --release` dans `paie/`)" % FORMAT)


def _lire(mode: str, chemin, delai: int):
    r = subprocess.run([str(_paie()), "pdf", mode, str(chemin)], capture_output=True, timeout=delai)
    if r.returncode != 0:
        raise SystemExit(r.stderr.decode("utf-8", "replace").strip() or "la fabrique a refuse (code %d)" % r.returncode)
    return json.loads(r.stdout.decode("utf-8"))


def texte(chemin, delai: int = 1800) -> list:
    """Le texte de chaque page, une chaine par page."""
    return _lire("--texte", chemin, delai)


def inspecter(chemin, delai: int = 1800) -> dict:
    """Les pages, leurs mots et leurs boites, leurs traits (`fabrique-pdf/FORMAT.md`)."""
    return _lire("--inspecter", chemin, delai)
