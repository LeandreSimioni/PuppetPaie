---
name: contrat
description: Rédige les contrats d'engagement à durée déterminée d'usage (CDDU) des artistes et techniciens, avec PuppetPaie. Déclencher quand l'utilisateur demande un contrat, une lettre d'engagement, un CDDU, ou veut formaliser l'engagement d'un artiste sur des dates - sans forcément faire la paie tout de suite.
---

# Contrat d'engagement (CDDU)

Le contrat est produit par `moteur/contrat.py` à partir des mêmes données que la
paie : un contrat cohérent avec le bulletin, par construction.

## Le faire seul, sans la paie

Créer le fichier de saisie du mois (voir la compétence `paie`) puis :

```bash
python -m moteur mois paies/AAAA-MM.json
```

Le PDF `Contrats_<Mois><Année>.pdf` contient un contrat par page, avec la DPAE
correspondante. Si la paie a déjà été faite et qu'il ne faut régénérer que les
bulletins, ajouter `--sans-contrat` pour ne pas renuméroter contrats et DPAE.

## Ce que le contrat dit

CDD d'usage au titre de l'article L.1242-2 3° du code du travail et de la
convention collective de la compagnie (`idcc` de la fiche entreprise : 3090,
spectacle vivant privé, ou 1285, entreprises artistiques et culturelles). Il porte : les
parties, l'emploi et le spectacle avec son numéro d'objet, les dates et les
dates jouées, le lieu, le nombre de cachets et le brut, les congés payés par la
Caisse des Congés Spectacles, le rattachement aux annexes VIII et X, et une
clause de droits voisins qui **exclut** toute captation de la rémunération.

## Points de vigilance

- **Le lieu** est vide par défaut et le contrat écrit alors « le lieu de la
  représentation ». Renseigner `lieu` dans le fichier de saisie quand il est
  connu : c'est une mention utile en cas de litige.
- **Une captation** (vidéo, disque, diffusion) ne se règle pas dans ce contrat :
  il faut une autorisation écrite séparée, et le plus souvent une rémunération
  distincte. Le signaler si l'utilisateur parle de filmer.
- **La numérotation** des contrats (`FA1.26.13`) est tenue dans
  `config/compteurs.json` et reprise telle quelle dans la DSN. Ne pas la
  bricoler à la main.
- Faire signer **avant** le début du travail, comme la DPAE.
