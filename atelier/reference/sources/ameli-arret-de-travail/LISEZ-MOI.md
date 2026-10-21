# ameli — l'arrêt de travail d'un salarié, côté employeur

| fichier | ce que c'est | URL | empreinte |
|---|---|---|---|
| `ameli-formalites-employeur-arret-travail-20240906.md` | « Démarches de l'employeur en cas d'arrêt de travail ou d'hospitalisation » (06/09/2024). **Transcription** copiée par Léandre depuis son navigateur le 21/10/2026 : ameli refuse le conteneur (403), le brut n'est pas déposé | https://www.ameli.fr/entreprise/vos-salaries/arret-de-travail/formalites-employeur | `e28e3773…e2c828af` |

## Ce qu'on en lit

- **Début de l'arrêt** : l'attestation de salaire dès réception du volet 3 ; en DSN, c'est le signalement d'arrêt.
- **Arrêt prolongé au-delà de 6 mois**, en DSN : « modifier la date de fin prévisionnelle de l'arrêt de travail dans votre
  logiciel de paie ».
- **Reprise à la date prévue** : signalée dans la DSN mensuelle. **Reprise plus tôt que prévu** : à déclarer « dans un délai de
  5 jours suivant son retour » (la page dit « 5 jours » ; l'arrêté du 2 juillet 2013 dit cinq jours **ouvrés** :
  `../arrete-2013-delai-evenements-dsn/`).
- Le pied de page mène à une « Documentation technique » (https://www.ameli.fr/entreprise/documentation-technique), elle aussi
  refusée au conteneur : c'est peut-être là qu'est la documentation XML de la DAT que le CNDA dit publiée « sur son site
  ameli.fr » (`../accident-du-travail/LISEZ-MOI.md`). Inférence, à vérifier depuis le poste.

## Où le moteur s'en sert

`logiciel/donnees/obligations-base.json` (`signalement_arret`, `signalement_reprise`) et `atelier/sources/paie/src/obligations.rs`
(`de_l_arret`) : les dates butoirs des signalements. La prolongation (21/10/2026) : route `arret-prolonger`
(`atelier/sources/paie/src/routes_imprevus.rs`, `arret_prolonger`), qui ne change que la fin prévisionnelle (la DSN mensuelle
la porte, S21.G00.60.003 ; aucun nouveau signalement : fiche net-entreprises 1983, `../net-entreprises-base-connaissances/`) ;
l'arrêt « en cours » de la page (`routes_imprevus::en_cours`) cesse quand la DSN du mois de sa fin est partie — la reprise à la
date prévue y est alors déclarée. La reprise plus tôt que prévu : `arret-reprise`, qui fait naître le signalement 05.
