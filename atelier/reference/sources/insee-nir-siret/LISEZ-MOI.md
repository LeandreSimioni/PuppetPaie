# La clé du NIR et le contrôle de Luhn du SIRET (INSEE)

## D'où ça vient

Deux pages du schéma XML des identifiants de l'INSEE, relevées le 28/10/2026 par `curl`, déposées telles que servies :

| fichier | URL | transcription |
|---|---|---|
| `insee-schema-nir-20261028.html` | `https://xml.insee.fr/schema/nir.html` (« Identifiant d'individus : NIR ») | `insee-schema-nir-20261028.txt` (`pandoc -t plain`) |
| `insee-schema-siret-20261028.html` | `https://xml.insee.fr/schema/siret.html` (« Identifiants économiques : SIREN, NIC et SIRET ») | `insee-schema-siret-20261028.txt` |

## Ce qu'on en lit

- **Clé du NIR** (`nir.html`, § 4.1, « Clé de contrôle du NIR ») : la clé est le complément à 97 du reste de la division
  par 97 du nombre formé par les 13 chiffres du NIR. Pour la Corse, les lettres de département sont remplacées par `0`,
  puis on soustrait du nombre à 13 chiffres 1 000 000 pour `2A` ou 2 000 000 pour `2B`. **Inférence** : remplacer `2A`
  par `19` et `2B` par `18` aux rangs 6-7 donne le même nombre (20 000 000 − 1 000 000 = 19 000 000 ; 20 000 000 −
  2 000 000 = 18 000 000), c'est ce que fait le moteur.
- **Contrôle du SIRET** (`siret.html`, § 8.1, « Contrôle de parité ») : les SIREN, NIC et SIRET portent la clé de Luhn ;
  on parcourt les chiffres, on multiplie par 2 ceux de rang impair, on additionne **les chiffres** des résultats (12
  compte pour 1 + 2), et le contrôle est bon si la somme est un multiple de 10. Sur 14 chiffres, le rang impair compté
  depuis la gauche est le rang pair compté depuis la droite, ce que le moteur parcourt.
- Non repris : l'exception que d'autres sources signalent pour les SIRET de La Poste (`356000000XXXXX`) ; le moteur n'en
  fait aucune.

## Où le moteur s'en sert

`atelier/sources/declarations/src/champs.rs` : `cle_nir` (la clé attendue d'un NIR, refus d'un NIR dont la clé diffère) et
`luhn` (le SIRET d'un établissement). Tests : `la_cle_du_nir_et_celle_de_la_corse`, `le_siret_se_controle_par_luhn`.
