---
name: ouverture
description: Ouvrir PuppetPaie au début d'une session — sauvegarder le dossier privé dans le Drive et relever les retours des organismes —, consulter l'état du coffre, et savoir quelles actions sont permises. Déclencher au début de toute session de travail sur les paies, quand l'utilisateur demande « où on en est », s'il parle des retours, d'une sauvegarde, d'un mot de passe, ou avant de tenter un dépôt DSN ou DPAE.
---

# Ouvrir PuppetPaie

## Au début d'une session

```bash
cd logiciel && python -m moteur ouverture     # depuis la racine du dépôt
```

Si le coffre ne s'ouvre pas avec la clé du poste, ouvrir d'abord **la
séance** (voir « Le coffre ») : sinon la première commande qui en a besoin
la demandera elle-même.

Elle sauvegarde le dossier privé dans le Drive (par différences) et relève les
retours des organismes. Elle **n'échoue jamais** : sans réseau, sans compte
Google ou sans secret posé, elle le dit et rend la main. Compter une dizaine de
secondes quand rien n'a bougé.

`--json` pour une sortie exploitable. `--pas-avant 20` pour ne rien refaire si
l'ouverture a moins de vingt minutes.

Puis lire ce qui est arrivé :

```bash
python -m moteur retours          # le détail, et les taux de PAS
python -m moteur pas              # l'état courant des taux
```

Reprendre l'historique (une compagnie qui arrive avec ce que son ancien logiciel a laissé) : tout va dans le dossier
`import/` de la compagnie, sans nommer de logiciel ; `paie importer` en rend l'aperçu en JSON (rien n'est écrit ; `tri`
dit le sort de chaque fichier), `paie importer --ecrire` le reprend ; à l'écran, Configuration ›
Reprendre l'historique. Le message à l'éditeur sur les fichiers non repris ne part que de l'écran, au clic de la
personne : jamais d'un agent.

## Ce qui est permis, et ce qui ne l'est pas

| | condition |
|---|---|
| lire, calculer, produire un mois, sauvegarder, relever les retours | aucune |
| **déposer une DSN ou une DPAE en réel, envoyer, faire signer** | **le mot de passe du logiciel, tapé par un humain au moment de l'acte** |

**Deux mots de passe, deux chemins** (l'utilisateur, 23/09/2026) :

- **Ouvrir le coffre = la séance** : `python -m moteur coffre seance --fenetre`,
  une boîte, une fois, pour 12 h. Elle sert aux dépôts de **test** et aux
  relevés. Elle n'est **jamais** une preuve pour un acte réel.
- **Un acte réel = le mot de passe retapé, à chaque fois**, par un humain.

Un agent **peut déposer en test** (`depot-dsn --test`, `deposer-dpae --test`)
sans demander d'aval (décision du 29/09/2026, qui lève le « oui » du
23/09 ; voir `CLAUDE.md`). Il ne dépose jamais en réel.

Dépôt réel demandé : **dire que c'est à l'utilisateur de le lancer** (la fenêtre
PuppetPaie, `Depot DSN.cmd`, ou la commande dans son terminal), ne pas
contourner — ni en posant `PUPPETPAIE_MOTDEPASSE` ou
`NET_ENTREPRISES_MOTDEPASSE` à sa place, ni autrement. Un dépôt engage la
compagnie, ordres de télépaiement compris.

## Le coffre

```bash
python -m moteur coffre                          # ce qui est posé, jamais les valeurs
python -m moteur coffre poser net-entreprises    # (à l'utilisateur de le taper)
python -m moteur coffre poser urssaf
python -m moteur coffre ouverture                # le mot de passe du logiciel
python -m moteur coffre seance --fenetre         # ouvrir le coffre pour 12 h
python -m moteur coffre seance fermer            # le refermer
```

**Ne jamais lire `prive/config/coffre.bin`, `google-jeton.bin` ni
`maj-cle-privee.bin`.**
`python -m moteur coffre` répond à toutes les questions légitimes sans afficher
un seul secret.

## Deux pièges à ne pas rejouer

1. **Ne jamais régénérer une déclaration déjà déposée.** Les numéros de contrat
   se renumérotent dès que les compteurs avancent : un annule-et-remplace aux
   numéros changés ne remplace plus rien de reconnaissable. Vérifier un fichier
   se fait avec `python -m moteur depot-dsn <fichier> --apercu`, qui ne touche
   ni au réseau ni au disque.
2. **La base de test a une mémoire**, séparée de la base réelle : une seconde
   DSN « normale » du même mois y est refusée (`B1-208-10`), et un
   annule-et-remplace doit y porter un ordre supérieur au dernier accepté
   (`B1-208-11`). Depuis le 23/09/2026 le moteur **force** le type et l'ordre
   d'un essai en mémoire (`dsn_api`), sans toucher au fichier : on dépose
   donc toujours en test **avant** le réel, et les vraies DSN ne gênent pas
   un test.
