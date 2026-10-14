# Sortir ses documents de CoolPaie avant d'importer

Pour les années passées sous CoolPaie, PuppetPaie n'a que les **DSN** : elles
disent ce qui a été versé, pas le bulletin tel qu'il a été remis. Le dossier
d'un salarié (`python -m moteur dossier`, onglet Exports) reprend donc **tels
quels** les PDF que CoolPaie a écrits — bulletins, contrats, attestations. Ce
qui n'a jamais été sorti de CoolPaie n'y sera pas.

## Ce qu'il faut rassembler

Tout va dans le dossier `export/` de CoolPaie, celui qui porte déjà `DSN/` et
`AEM_EDI/`, à sa racine :

| document | comment PuppetPaie le reconnaît |
|---|---|
| bulletins de paie | nom commençant par `BS` (ou `_…_Bulletin.pdf`, `paie…`) |
| attestations employeur (AEM) | nom commençant par `AEM` |
| contrats | nom commençant par `Contrat` |
| contrats signés | nom commençant par `Contrat` et contenant `sign` |
| bulletins et AEM mêlés | nom commençant par `BS&AEM` : chaque page est classée par son titre |

Un fichier peut réunir plusieurs salariés et plusieurs mois : **chaque page**
est rattachée à son salarié par son NIR, ou par son nom **et** son prénom, et
datée par la période qu'elle porte (« du … au … »), par la date d'embauche
d'un formulaire d'AEM, ou par la date écrite dans le nom du fichier
(`Contrats_16-12-2021.pdf`, `BS 06082021 FAVART.pdf`, `AEM 092021 X.pdf`).
Une page qui ne nomme personne suit la précédente du même fichier (la suite
d'un contrat) ; une page qu'on ne sait rattacher à personne n'entre dans
aucun dossier, et le dossier le dit.

Deux conseils qui évitent des pages perdues :

- **mettre une date dans le nom du fichier** quand le document n'a pas de
  période lisible — c'est le cas de contrats dont CoolPaie n'a pas rempli la
  période (« du & periode de travail & ») ;
- **vérifier qu'un PDF n'est pas vide** : plusieurs fichiers de notre propre
  export pesaient 0 octet. Le dossier les signale comme illisibles.

## Où les trouver dans CoolPaie

**Non relevé.** Les menus d'impression de CoolPaie (bulletins, contrats,
attestations par période ou par salarié) n'ont pas été relevés à ce jour ; ce
paragraphe se complétera le jour où quelqu'un les parcourt, écran par écran.
Ce qui est sûr : CoolPaie écrit ses PDF là où on lui dit, et ce sont ces PDF
qu'il faut déposer à la racine d'`export/`.

## Puis importer

```bash
python -m moteur importer            # l'aperçu
python -m moteur importer --ecrire   # copie l'export dans historique/coolpaie/, puis importe
```

L'import **copie** l'export dans les données de la compagnie
(`historique/coolpaie/`) : DSN, AEM, et ces PDF. Tout ce qui se reconstruit
se lit ensuite dans la copie — le dossier `export/` de CoolPaie peut
disparaître sans rien emporter. Un import se rejoue : ce qui est déjà copié
reste tel quel, ce qui manque s'ajoute, et un fichier du même nom au contenu
différent est **signalé**, jamais écrasé.
