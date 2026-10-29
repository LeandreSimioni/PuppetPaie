# Sortir ses documents de CoolPaie avant d'importer

Pour les années passées sous CoolPaie, les **DSN** disent ce qui a été
déclaré, pas le bulletin tel qu'il a été remis. L'import lit aussi les
**bulletins en PDF** : un bulletin fait foi sur la paie qu'il porte, et le
dossier d'un salarié (`moteur dossier`, onglet Exports) en reprend la page
**telle quelle**. Ce qui n'a jamais été sorti de CoolPaie n'y sera pas.

## Ce qu'il faut rassembler

Tout va dans la boîte `import/` de la compagnie, telle que CoolPaie l'a
laissé, sous-dossiers et zip compris :

- les **DSN** (le dossier `DSN/` de l'export de CoolPaie, ou les `.dsn` seuls) ;
- les **AEM** (le dossier `AEM_EDI/`) ;
- les **bulletins de paie** en PDF, un salarié et un mois par page ou
  plusieurs dans un même fichier.

Le nom d'un fichier ne compte pas : chaque fichier est reconnu par son
contenu, et chaque page d'un PDF de bulletins est rattachée à son salarié par
son NIR et à sa paie par sa période (`atelier/README.md`, « Reprendre
l'historique »). Une page qui ne se lit pas, ou qui se lit de deux façons,
n'est pas reprise, et le compte rendu de l'import le dit, page par page.

Un PDF vide ou sans texte (un scan) ne se lit pas : le compte rendu le
signale. Les contrats et les attestations en PDF n'entrent pas dans l'import :
les contrats se lisent dans les DSN, les attestations dans les AEM.

## Où les trouver dans CoolPaie

**Non relevé.** Les menus d'impression de CoolPaie (bulletins par période ou
par salarié) n'ont pas été relevés. Ce qui est sûr : CoolPaie écrit ses PDF là
où on lui dit, et ce sont ces PDF qu'il faut déposer dans `import/`.

## Puis importer

```bash
moteur importer            # l'aperçu : rien n'est écrit
moteur importer --ecrire   # le reprend
```

L'import copie ce qu'il reprend dans `historique/bulletins/`, sous un nom tiré
de l'empreinte du fichier ; c'est là que le dossier d'un salarié, le PAS amorcé
(`moteur pas --amorcer`) et l'arrivée lisent les pièces importées. Un import se
rejoue : ce qui est déjà copié reste tel quel, ce qui manque s'ajoute, et un
fichier du même nom au contenu différent est **signalé**, jamais écrasé.
