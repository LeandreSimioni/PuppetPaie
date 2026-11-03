# Ordonnance n° 96-50 du 24 janvier 1996 relative au remboursement de la dette sociale — la CRDS

Relevée par Léandre depuis son poste le 03/11/2026 (Légifrance refuse le conteneur), version « en vigueur au
03/11/2026 », « dernière mise à jour des données de ce texte : 31 décembre 2025 ». JORFTEXT000000190291.

| fichier | ce que c'est | URL | SHA-256 |
|---|---|---|---|
| `legifrance-ordonnance-96-50-crds-20251231.pdf` | le texte consolidé, imprimé de Légifrance | https://www.legifrance.gouv.fr/loda/id/JORFTEXT000000190291 | `16acbd4a…1bbd90ff` |
| `legifrance-ordonnance-96-50-crds-20251231.txt` | sa transcription (`pdftotext -layout`) | — | `59f68a45…0641a5d2` |

## Ce qu'on en lit

- **Art. 14, I** (en vigueur depuis le 01/09/2019, l. 525-532 de la transcription) : la contribution est « assise sur les
  revenus d'activité et de remplacement mentionnés à la section 1 du chapitre 4 du titre 3 du livre 1 du code de la
  sécurité sociale », « soumise aux conditions prévues aux articles L. 136-1-1 à L. 136-4 du même code ». Les indemnités
  journalières y sont : revenus de remplacement de L136-1-2 (`../code-de-la-securite-sociale/xml/L136-1-2.xml`).
- **Art. 19** (en vigueur depuis le 01/01/2020, l. 705-711) : « Le taux des contributions instituées par les articles 14 à
  17 est fixé à 0,5 % ».

Ces deux versions valent pour 2025 et 2026.

## Où le moteur s'en sert

Le net de l'indemnité journalière de principe (`engagement::maintien::ij_de_principe`, `net_de_csg_crds` ;
`logiciel/donnees/permanents-base.json`, `indemnites_journalieres.principe.crds` et `principe_accident.crds`).
