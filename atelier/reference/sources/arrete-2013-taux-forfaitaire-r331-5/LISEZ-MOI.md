# Arrêté du 28 mars 2013 fixant le taux forfaitaire mentionné à l'article R. 331-5 du code de la sécurité sociale

Relevé par Léandre depuis son poste le 03/11/2026 (Légifrance refuse le conteneur), version « en vigueur au 03/11/2026 »,
« dernière mise à jour des données de ce texte : 31 mars 2013 ». NOR BUDS1304319A, JORF n° 0076 du 30 mars 2013.

| fichier | ce que c'est | SHA-256 |
|---|---|---|
| `legifrance-arrete-20130328-taux-r331-5-20130331.pdf` | le texte, imprimé de Légifrance | `831b42da…908772eb` |
| `legifrance-arrete-20130328-taux-r331-5-20130331.txt` | sa transcription (`pdftotext -layout`) | `4e201313…e29bd1bd` |

## Ce qu'on en lit

**Article 1** (en vigueur depuis le 31/03/2013) : « Le taux forfaitaire mentionné au deuxième alinéa de l'article R. 331-5
du code de la sécurité sociale est fixé à 21 %. » R433-4, dernier alinéa (`../code-de-la-securite-sociale/xml/R433-4.xml`)
y renvoie : l'indemnité journalière d'un accident du travail « ne peut dépasser le montant du gain journalier net perçu
par la victime et déterminé par application au salaire de référence du taux forfaitaire mentionné au deuxième alinéa de
l'article R. 331-5 ».

## Où le moteur s'en sert

L'indemnité journalière de principe d'un accident du travail ou de trajet (`engagement::maintien::ij_de_principe`,
`montants_d_accident`) : `logiciel/donnees/permanents-base.json`, `indemnites_journalieres.principe_accident.taux_forfaitaire`.
