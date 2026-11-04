# Afdas : les contributions conventionnelles 2026

L'Afdas est un organisme privé (opérateur de compétences) : ses originaux ne se republient pas. Ce dossier n'en garde
que la transcription ; les originaux, aux mêmes noms, sont dans le dépôt privé (`reference-privee/afdas-contributions-2026/`).
Relevé le 04/11/2026 (HTTP 200).

| transcription | original et URL | ce qu'on en lit |
|---|---|---|
| `afdas-contributions-2026-audiovisuel-20261104.txt` (`pdftotext -layout`) | `afdas-contributions-2026-audiovisuel-20261104.pdf`, `https://www.afdas.com/fileadmin/user_upload/collecte/Collecte_2026/Contributions_2026_Audiovisuel.pdf` | plaquette « Entreprises de l'audiovisuel, vos contributions formation professionnelle et apprentissage en 2026 » : contribution conventionnelle de la branche de l'audiovisuel, « - de 11 » 0,45 %, « 11 à 49 » 0,30 %, « 50 à 299 » 0,20 % ; intermittents du spectacle « 0.10% + forfait » ; dialogue social « 0.004% avec un minimum du de 15€ HT et un maximum du de 2 000€ HT », « 0.006% dus sans minimum ni maximum du par les Radios privées », « 0,05% dus par les entreprises de Radio-diffusion » ; déclaration à l'Afdas « avant le 28/02/26 » sur la masse salariale 2025. |
| `afdas-taux-contribution-formation-continue-20261104.txt` (`pandoc -t plain`) | `afdas-taux-contribution-formation-continue-20261104.html`, `https://www.afdas.com/entreprise/vous-informer-sur-vos-contributions/taux-de-contribution-formation-continue.html` | page « Taux de contribution formation continue », un tableau par branche. « Entreprises de l'audiovisuel et production cinématographique » : taux conventionnel 0,45 % (moins de 11), 0,30 % (11 à 49), 0,20 (50 à 299), « - » (300 et plus). « Spectacle vivant » : 0,75 % (moins de 11), 0,30 % (11 et plus). « Intermittents du spectacle » : 0,10 % « quel que soit l'effectif », « + un forfait de 50€ HT soit 60€ TTC ». |

**Inférence**, dite telle : la page range sous un même tableau l'audiovisuel et la production cinématographique sans
nommer d'IDCC ; le rattachement de ce tableau aux conventions 2642 (production audiovisuelle) et 3097 (production
cinématographique) est une lecture du titre.

**Où le moteur s'en sert** : `logiciel/donnees/baremes/2026.json`, `par_convention.2642.hors_bulletin` et
`par_convention.3097.hors_bulletin` (`contribution_conventionnelle_permanents`, `cpnef_dialogue_social`) ; la
contribution conventionnelle des intermittents (0,10 %) est dans `autres_1` du corps du barème.
