"""Le guet de l'orchestrateur : combien de contexte le Sonnet a-t-il consommé ?

Lit toutes les 30 s le dernier `usage` du transcript du Sonnet (et de l'orchestrateur), jamais un
transcript entier : il ne coûte aucun jeton. Il SORT quand il faut réveiller l'orchestrateur, qui le
lance en arrière-plan avec le Sonnet (`ETAT.md`, « Les plafonds ») :

- le Sonnet passe `--seuil` (300 000 d'abord) ;
- avec `--commits`, un nouveau commit apparaît sur HEAD (le moment propre pour un arrêt) ;
- l'orchestrateur passe 300 000 ;
- le transcript du Sonnet n'a plus bougé depuis 20 minutes (fini ou bloqué).

    python3 atelier/guet.py                        # le dernier Sonnet lancé, palier 300 000
    python3 atelier/guet.py --seuil 330000 --commits

Ne l'arrêter jamais par `pkill -f guet` : le motif tue le shell qui le lance.
"""
import argparse
import glob
import json
import os
import subprocess
import time

DEPOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def dossier_projet():
    """`~/.claude/projects/<chemin du dépôt, / remplacés par ->`."""
    return os.path.join(os.path.expanduser("~/.claude/projects"), DEPOT.replace("/", "-"))


def plus_recent(motif):
    chemins = glob.glob(motif)
    return max(chemins, key=os.path.getmtime) if chemins else None


def dernier_usage(chemin):
    """Les jetons du dernier tour : entrée + cache créé + cache lu. Lit la fin du fichier seulement."""
    try:
        with open(chemin, "rb") as f:
            f.seek(0, 2)
            f.seek(max(0, f.tell() - 400_000))
            lignes = f.read().splitlines()
    except (OSError, TypeError):
        return 0
    for ligne in reversed(lignes):
        if b'"usage"' not in ligne:
            continue
        try:
            u = json.loads(ligne)["message"]["usage"]
        except (ValueError, KeyError, TypeError):
            continue
        return (u.get("input_tokens", 0) + u.get("cache_creation_input_tokens", 0)
                + u.get("cache_read_input_tokens", 0))
    return 0


def head():
    return subprocess.run(["git", "-C", DEPOT, "rev-parse", "HEAD"],
                          capture_output=True, text=True).stdout.strip()


def main():
    a = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    a.add_argument("--seuil", type=int, default=300_000)
    a.add_argument("--commits", action="store_true", help="sortir aussi au prochain commit")
    a.add_argument("--transcript", help="le transcript du Sonnet (par défaut : le plus récent)")
    o = a.parse_args()

    projet = dossier_projet()
    moi = plus_recent(os.path.join(projet, "*.jsonl"))
    sonnet = o.transcript or plus_recent(os.path.join(projet, "*", "subagents", "*.jsonl"))
    if not sonnet:
        raise SystemExit("aucun transcript de Sonnet sous %s" % projet)
    print("guet : %s, seuil %d%s" % (os.path.basename(sonnet), o.seuil,
                                     ", et au prochain commit" if o.commits else ""), flush=True)

    depart = head()
    while True:
        s, m = dernier_usage(sonnet), dernier_usage(moi)
        if s >= o.seuil:
            print("SEUIL : Sonnet à %d (>= %d)" % (s, o.seuil))
            return
        if m >= 300_000:
            print("SEUIL : orchestrateur à %d" % m)
            return
        if o.commits and head() != depart:
            print("COMMIT %s : Sonnet à %d" % (head()[:8], s))
            return
        try:
            if time.time() - os.path.getmtime(sonnet) > 1200:
                print("IMMOBILE depuis 20 minutes : Sonnet à %d" % s)
                return
        except OSError:
            pass
        time.sleep(30)


if __name__ == "__main__":
    main()
