# Le code civil

Le code civil n'est pas parmi les codes que `moteur releve-codes` suit (`moteur/src/atelier/releve/codes/textes.rs`) : ses
articles se relèvent sur Légifrance, depuis le poste de Léandre (ticket #307 du privé).

| fichier | ce que c'est | relevé |
|---|---|---|
| `legifrance-code-civil-art1218-force-majeure-20261030.pdf` (et sa transcription `.md`) | article 1218, version en vigueur depuis le 01/10/2016 (ordonnance n° 2016-131), https://www.legifrance.gouv.fr/codes/article_lc/LEGIARTI000032041431 | 30/10/2026, par Léandre |

## Ce qu'on en lit

- **La force majeure a trois conditions, toutes à la fois** : un événement « échappant au contrôle du débiteur », qui « ne
  pouvait être raisonnablement prévu lors de la conclusion du contrat » et dont « les effets ne peuvent être évités par des
  mesures appropriées ». Empêchement temporaire : l'exécution est suspendue ; définitif : le contrat est résolu de plein
  droit.
- C'est la « force majeure » que nomment L. 1243-1 et L. 1243-4 du code du travail (`../code-du-travail/`) pour la rupture
  anticipée d'un CDD, et le motif 083 de S21.G00.62.002 (« Rupture pour force majeure ou fait du prince »). Une
  représentation que la compagnie annule n'en est pas une d'elle-même : c'est la compagnie, débitrice du contrat, qui
  décide, et le salaire se doit (décision de Léandre du 30/10/2026, que ce texte appuie : inférence, la jurisprudence n'est
  pas déposée).

## Où le moteur s'en sert

- `logiciel/donnees/contrats-base.json` : les motifs de rupture 036 (sa phrase « hors faute grave, force majeure ou
  inaptitude », L. 1243-4) et 083 ; la phrase « rupture » du contrat d'usage.
- `moteur/src/routes/ouverture.rs` (« LES DATES ET LA TRANSACTION SELON LE MOTIF ») : la date de notification que la norme
  exige pour le motif 083.
