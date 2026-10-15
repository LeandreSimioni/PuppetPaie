# La description d'un document — format d'échange, version 3

Le Python décrit, la fabrique met en page. Le Python ne connaît ni HTML ni
CSS pour ce chemin ; la fabrique ne connaît pas la paie. Ce fichier est le
seul contrat entre les deux.

Deux façons de décrire un document, qui se mêlent dans une même description :
**des blocs** (le Python dit la mise en page, ci-dessous) ou **un modèle** (le
Python dit quel fichier `.modele` et avec quelles valeurs ; la fabrique le lit
et le met en page elle-même, voir « Le document modèle »).

**Version 2 (01/10/2026)** : ajoute `style` et `modele`, puis le bloc `tableau`
et la couleur d'un morceau, puis les blocs à position fixe `texte_fixe`, `trait`
et `image_fixe`, puis `entete_page` et `pied_page` (même jour, sans changer le numéro : un programme du format 2
plus ancien refuse `tableau` ou `texte_fixe` par un « type inconnu », et le
bulletin comme le certificat Congés Spectacles s'arrêtent alors, sans PDF fait autrement). Une description du
format 1 reste lue telle quelle ; le numéro monte parce qu'un programme du
format 1 refuserait un document modèle (champ inconnu), et que `fabrique.py`
doit donc ne pas l'employer : il ne retient qu'un programme qui répond
`fabrique-pdf 2` (puis 3, voir ci-dessous), et s'arrête sinon : plus aucun PDF ne se fait
par Python.

**Version 3 (01/10/2026)** : la description ne change pas, à une clé facultative près
(`largeur_max` de `texte_fixe`) ; ce qui s'ajoute, ce sont **quatre modes** qui lisent
ou retouchent un PDF existant, de quoi se passer de PyMuPDF : `--pages` (nombre de
pages), `--texte` (texte de chaque page), `--marquer` (un texte en pied de chaque page)
et `--verrouiller` (chiffrement AES-256). Les descriptions portant `"format": 1`, `2`
ou `3` se lisent de la même façon (le 3 n'est que le numéro du programme) ; un
programme du format 2 refuse `largeur_max` (champ inconnu) et ne connaît aucun des
quatre modes : `fabrique.py` n'emploie les nouveautés qu'avec un programme qui répond
`fabrique-pdf 3`. Un cinquième mode, `--inspecter` (« Inspecter un PDF »), s'est ajouté
au format 3 le même jour, sans changer le numéro.

Deux modes encore, **même numéro** (01/10/2026), pour l'attestation employeur de France
Travail : `--champs` (les champs d'un formulaire) et `--remplir` (champs texte et images).
Un programme du format 3 plus ancien ne les connaît pas : il répond par son usage, code 2,
et l'opération s'arrête sur ce refus — aucun formulaire ne se remplit autrement.

## L'appel

```
fabrique-pdf <sortie.pdf>                      # la description arrive en JSON sur l'entrée standard
fabrique-pdf --assembler <sortie.pdf>          # fusion de PDF existants (« L'assemblage »)
fabrique-pdf --pages a.pdf [b.pdf …]           # le nombre de pages de chaque PDF (« Lire un PDF »)
fabrique-pdf --texte a.pdf                     # le texte de chaque page (« Lire un PDF »)
fabrique-pdf --inspecter a.pdf                 # taille, mots et boîtes, images, traits, polices, métadonnées (« Inspecter un PDF »)
fabrique-pdf --marquer entree.pdf sortie.pdf "texte"   # un texte en pied de chaque page
fabrique-pdf --verrouiller entree.pdf sortie.pdf       # chiffrement AES-256, sans mot de passe
fabrique-pdf --champs a.pdf                    # les champs d'un formulaire (« Remplir un formulaire »)
fabrique-pdf --remplir entree.pdf sortie.pdf   # champs texte et images, JSON sur l'entrée standard
fabrique-pdf --version                         # « fabrique-pdf 3 » : le numéro du format
```

Code de sortie 0 : le PDF est écrit. Sinon : rien n'est écrit (ou le fichier
partiel est effacé), et **une ligne en français** sur la sortie d'erreur dit
ce qui ne va pas (« bloc 12 : type inconnu "tablo" »). Le Python la relaie
telle quelle.

## Unités et conventions

- Longueurs en **points** (1/72 de pouce), nombres décimaux.
- Couleurs en `"#RRGGBB"`.
- Chemins de fichiers absolus, en UTF-8.
- Un champ absent prend sa valeur par défaut ; **un champ inconnu est un
  refus** (faute de frappe attrapée, pas ignorée).

## Le document

```json
{
  "format": 2,
  "style": "contrat",
  "page": {"largeur": 595.28, "hauteur": 841.89,
           "marges": {"haut": 106, "droite": 52, "bas": 46, "gauche": 52}},
  "police": {"taille": 7.5, "interligne": 1.25, "couleur": "#2C3E50"},
  "logo": {"chemin": "/…/logo.svg", "largeur": 74,
           "marge_haut": 28, "marge_droite": 34},
  "titre": "Contrat CDDU — DUPONT",
  "documents": [ {"blocs": [ … ]}, {"blocs": [ … ]} ]
}
```

- `style` (facultatif) : un jeu de mise en page nommé : `contrat` (CDDU),
  `avenant` ou `attestation` (certificat de travail, reçu pour solde de tout
  compte), qui ne diffèrent que par ce qui suit le préambule (voir « Le
  document modèle »). Il donne la **page** (A4, marges 106 / 59,5 / 46 / 59,5) et la
  **police** (7,5 pt, interligne 1,25, `#2C3E50`) quand la description ne les
  dit pas, et il règle la mise en page des documents modèle. Un style inconnu
  est un refus.
- `titre` : sans lui, le titre du fichier PDF est celui du premier document
  modèle, marqueurs substitués et sans gras.
- `police` : Liberation Sans (les métriques d'Helvetica), embarquée **en
  sous-ensemble** : le PDF ne porte que les glyphes des caractères imprimés.
  `interligne` est un facteur de la taille.
- `logo` (facultatif) : posé en haut à droite de **chaque page**, largeur
  fixée, hauteur selon les proportions. SVG (vectoriel), PNG ou JPEG.
- `documents` : chacun **commence sur une nouvelle page** (un contrat par
  salarié) ; un document long continue sur les pages suivantes. Un document
  porte `blocs` **ou** `modele`, pas les deux.

## Le document modèle

```json
{"modele": {"chemin": "/…/contrat-cddu.modele",
            "valeurs": {"employeur.siret": "123 456 789 00012",
                        "salarie.nom_complet": "Paul Dupont", "…": "…"},
            "remplace": {"numero": "C-17", "date": "02/01/2025"},
            "signature": "/…/signature.svg"}}
```

- `chemin` : le fichier `.modele`, lu par la fabrique (UTF-8). Illisible : refus.
- `texte` (à la place de `chemin`) : le modèle lui-même, en texte (même
  grammaire). C'est ce que le Python garde avec la description d'un contrat
  envoyé : si une clause change ensuite dans le fichier, le contrat qu'on refait
  à la signature reste celui qui est parti. `chemin` et `texte` s'excluent, et
  l'un des deux est obligatoire : sinon refus. Un texte sans article est refusé
  comme un fichier (« le modèle donné en texte ne contient aucun article »).
- `valeurs` : **toutes** les valeurs que le modèle peut demander, clé
  « famille.nom » → texte. Ce que le modèle n'emploie pas est ignoré.
- `remplace` (facultatif) : le contrat annule et remplace un autre. Pose la
  phrase de la clé `ANNULE:` (ou la phrase par défaut du contrat) en tête, avec
  les valeurs `{{remplace.numero}}` et `{{remplace.date}}`.
- `signature` (facultatif) : l'image de la signature de l'employeur, posée
  sous sa fonction (voir `image`, flottante). Sans elle, la case reste vide.
- `modifications` (style `avenant`, obligatoire et non vide) : les articles du
  contrat que l'avenant modifie, `[{"numero": 4, "article": "Rémunération",
  "avant": ["…"], "apres": ["…"]}]` ; `avant` et `apres` ont le même nombre de
  paragraphes. Le bloc `MODIFICATION:` du modèle s'y répète pour chaque article
  (intitulé « Article n — Modification de l'article … ») ; ses marqueurs
  `{{modification.numero}}` et `{{modification.article}}` ne valent que là, et
  un paragraphe qui porte `{{modification.avant}}` ou `{{modification.apres}}`
  se répète pour chaque paragraphe cité, en italique et en retrait de 12 pt.
  Les articles du modèle suivent, numérotés à la suite. Un modèle sans bloc
  `MODIFICATION:` est refusé.
- `signature_employeur` (style `attestation`, facultatif) : l'image de signature
  de l'employeur, posée en fin de document dans la moitié droite — nom et
  fonction du signataire (`employeur.signataire`, `employeur.signataire_fonction`),
  puis l'image, qui occupe sa hauteur ; le bloc ne se coupe pas entre deux
  pages. Le certificat de travail l'envoie, pas le reçu (le salarié y signe).
- `mention_artiste` (styles `contrat` et `avenant`, facultatif) : ce que
  l'artiste a signé d'un clic (qui, quand, adresse, code), posé dans SA case,
  sous « signature » : un texte à lignes séparées par `\n`, dans un cadre gris
  de 46 pt de haut et de la largeur de la colonne, en 6,6 pt. La case ne prend
  aucune hauteur (comme une image flottante) : rien d'autre ne bouge, et sans
  cette clé le PDF est exactement celui d'avant. C'est ce qui fait de la
  contresignature « le contrat, plus une mention » : même description, même
  modèle (`texte`), plus ce champ. Au plus 4 lignes, chacune assez courte pour
  tenir dans la case, en caractères que la police possède : sinon refus. Pas
  avec `attestation`.
- `remplace` ne va qu'avec le style `contrat`, `modifications` qu'avec
  `avenant`, et `signature` pas avec `attestation` (pas de case) : sinon refus.
- Pour les styles `contrat` et `avenant`, les valeurs `employeur.signataire`,
  `employeur.signataire_fonction` et `salarie.nom_complet` sont celles du
  bloc de signatures : absentes, c'est un refus.

### La grammaire du `.modele`

Lue comme `lire_modele` du Python (`moteur/contrat.py`) l'a toujours fait ;
c'est `src/modele.rs` (le chemin PyMuPDF de repli n'existe plus).

- Une **ligne-clé** (au début de la ligne, sans blanc devant) ouvre une
  section dont le texte court jusqu'à la clé suivante : `TITRE:`,
  `SOUSTITRE:`, `PREAMBULE:`, `ANNULE:`, `MODIFICATION:`, `ARTICLE:` (le titre
  de l'article suit sur la même ligne ; il peut y en avoir autant qu'on veut,
  dans l'ordre du fichier), `SIGNATURES:`. Pour `TITRE:`, `SOUSTITRE:` et
  `ARTICLE:`, ce qui suit la clé sur sa ligne est le texte ; pour les autres,
  ce sont les lignes suivantes. Un texte avant toute clé est ignoré.
- Une ligne dont le premier caractère non blanc est `#` est un commentaire.
- Dans un corps, une **ligne vide** sépare deux paragraphes ; les autres
  lignes se recollent par une espace (blancs de bord rognés).
- `{{famille.nom}}` (minuscules et `_`, un point, blancs tolérés dedans) est
  remplacé par la valeur. **Un marqueur absent des `valeurs` est un refus**
  qui les nomme tous et liste les marqueurs disponibles — jamais un trou. La
  valeur substituée n'est pas relue.
- `**texte**` met en gras (au moins un caractère, premier `**` fermant) ; un
  `**` sans fermeture reste du texte. Les blancs se comptent comme en HTML
  (une suite = une espace), sauf l'espace insécable.
- Sans article, le modèle est refusé (« … ne contient aucun article »).
- La mise en page du `contrat` : titre 11 pt rouge en gras, sous-titre
  6,8 pt gris justifié, phrase d'annulation en gras, préambule et articles
  justifiés, intitulés « Article n — titre » 7,7 pt rouge en gras, mention
  finale (`SIGNATURES:`), puis deux cases à filet — employeur (nom, fonction,
  image) et artiste (nom, « signature »). Les nombres sont dans
  `src/style_contrat.rs`.

- `avenant` : le contrat, sans phrase d'annulation, avec les articles modifiés
  avant ceux du modèle (ci-dessus).
- `attestation` : mêmes titres, mais les intitulés d'article n'ont pas de
  numéro, une valeur de plusieurs lignes vaut autant de paragraphes (l'inventaire
  du reçu de solde de tout compte, une somme par ligne), et il n'y a pas de
  cases de signature.

## Les blocs

### `paragraphe`

```json
{"type": "paragraphe",
 "morceaux": [{"texte": "Entre la société "}, {"texte": "FIL TENDU", "gras": true}],
 "taille": 7.5, "couleur": "#2C3E50", "alignement": "justifie",
 "majuscules": false, "espace_avant": 0, "espace_apres": 2.6}
```

- `alignement` : `gauche` (défaut), `justifie`, `centre`, `droite`. Justifié :
  la dernière ligne reste à gauche.
- `majuscules` : le texte s'imprime en capitales (le texte transmis garde sa
  casse ; c'est l'équivalent de `text-transform: uppercase`).
- Taille, couleur : celles de `police` par défaut. Un morceau peut avoir sa propre
  `couleur` (`{"texte": "N° :", "couleur": "#777777"}`) ; sans elle, celle du bloc.
- `italique` (d'un morceau, faux par défaut) : le morceau s'imprime en oblique ;
  `retrait` (du bloc, 0 par défaut) : marge à gauche en points, le texte se
  met en page dans une largeur d'autant réduite (une citation).
- Le texte ne porte **aucun balisage** : pas d'échappement HTML, un « & » est
  un « & ». `\n` force un retour à la ligne.

### `espace`

```json
{"type": "espace", "hauteur": 6}
```

### `colonnes`

```json
{"type": "colonnes",
 "filet_haut": {"couleur": "#CCCCCC", "epaisseur": 0.5},
 "espace_avant": 4, "marge_interne_haut": 5,
 "colonnes": [
   {"largeur": 0.5, "blocs": [ … ]},
   {"largeur": 0.5, "blocs": [ … ]}
 ]}
```

- `largeur` : fraction de la largeur utile ; leur somme vaut 1.
- Les marges verticales voisines **se fondent**, comme en CSS : entre un bloc
  qui laisse `a` après lui et un bloc qui demande `b` avant lui, l'écart vaut
  max(a, b). Un bloc `espace` s'ajoute toujours.
- `filet_haut` (facultatif) : un trait au-dessus de chaque colonne.
- Un bloc `colonnes` **ne se coupe pas** entre deux pages : s'il ne tient
  pas, il passe entier à la page suivante.

### `image`

```json
{"type": "image", "chemin": "/…/signature.png", "largeur": 92, "flottante": false}
```

`flottante` (défaut `false`) : l'image se pose à l'endroit courant **sans
occuper de hauteur** ; ce qui suit passe dessous, et elle peut descendre dans
la marge du bas. C'est ainsi que la signature de l'employeur recouvre les
lignes vides de sa case, comme le faisait l'ancienne passe PyMuPDF qui la posait après coup.

Hauteur selon les proportions. SVG, PNG ou JPEG (`.svg`, `.png`, `.jpg`,
`.jpeg`, sans égard à la casse ; le PNG et le JPEG se reconnaissent à leur
contenu, pas à leur extension). Un fichier illisible est un refus, pas un
blanc. Même règle pour le `logo`.

### `tableau`

```json
{"type": "tableau",
 "colonnes": [{"largeur": 0.55},
              {"largeur": 0.15, "alignement": "droite"},
              {"largeur": 0.30, "alignement": "droite"}],
 "taille": 7, "couleur": "#2C3E50", "interligne": 1.27,
 "marge_cellule": {"haut": 1.4, "droite": 3, "bas": 1.4, "gauche": 3},
 "filet_lignes": {"couleur": "#E4E4E4", "epaisseur": 0.3},
 "filets_cellules": {"couleur": "#DDDDDD", "epaisseur": 0.5},
 "entete": [{"cellules": ["LIBELLE", "Base", "Montant"],
             "fond": "#B22222", "couleur": "#FFFFFF", "gras": true, "taille": 6.6}],
 "lignes": [
   {"cellules": ["Salaire", "1 234,56", {"texte": "12,00", "gras": true}]},
   {"cellules": [{"texte": "Total", "etendue": 2}, "1 246,56"],
    "gras": true, "filet_haut": {"couleur": "#B22222", "epaisseur": 0.6}}
 ],
 "espace_avant": 4, "espace_apres": 0}
```

Un tableau « de données » : le Python donne des **cellules de texte**, la fabrique
calcule les largeurs, les retours à la ligne, les hauteurs et les coupures de page.
C'est le bloc du bulletin de paie (`moteur/bulletin.py`) et des tableaux des exports.

- `colonnes` : au moins une ; `largeur` = fraction de la largeur utile (leur somme
  vaut 1) ; `alignement` : `gauche` (défaut), `droite` (les nombres), `centre`,
  `justifie`.
- `taille`, `couleur`, `interligne` : ceux de `police` par défaut. `marge_cellule` :
  l'espace entre le bord d'une cellule et son texte (0 par défaut) ; une colonne plus
  étroite que ses marges est refusée.
- `entete` : lignes **répétées en haut de chaque page** où le tableau continue
  (aucune : pas d'en-tête). `lignes` : le corps (aucune : l'en-tête seul s'écrit).
- Une **ligne** : `cellules` (liste), et, tous facultatifs, `gras` (toutes ses cellules),
  `couleur`, `taille` (celles du tableau par défaut), `fond` (rectangle plein sur toute
  la largeur), `filet_haut` et `filet_bas` (un trait sur toute la largeur, dans la
  hauteur de la ligne). `filet_lignes` pose un `filet_bas` à chaque ligne du corps
  (pas de l'en-tête) qui n'en a pas ; `filets_cellules` cadre **chaque cellule** (un
  cadre autour d'un tableau d'une ligne). Les filets sont dessinés après les textes,
  les fonds avant.
- Une **cellule** : une chaîne (le texte) ou un objet `{"texte" | "morceaux", "gras",
  "couleur", "alignement", "etendue"}`. `morceaux` : comme ceux d'un `paragraphe`
  (gras, couleur par morceau) ; `\n` force un retour. `etendue` : le nombre de colonnes
  qu'elle occupe (défaut 1) ; sans `alignement`, celui de la colonne où elle commence.
  Une ligne qui a moins de cellules que de colonnes laisse les dernières vides ; une
  cellule vide (`""`) n'écrit rien dans le PDF.
- Le texte passe à la ligne dans la cellule ; la hauteur d'une ligne est celle de sa
  cellule la plus haute, marges comprises. **Une ligne ne se coupe jamais entre deux
  pages** : le tableau se coupe entre les lignes, et l'en-tête se redessine en haut de
  la page suivante. L'en-tête ne reste jamais seul en bas de page (il suit sa première
  ligne), et l'en-tête plus la ligne la plus haute doivent tenir sur une page : sinon
  c'est un refus qui le dit.
- Un montant n'est pas protégé du retour à la ligne : une colonne trop étroite le
  coupe aux espaces (« 1 781,98 » sur deux lignes). C'est à la description de donner
  des colonnes assez larges.
- Les marges verticales voisines se fondent comme pour un paragraphe. Un tableau ne se
  place **pas dans des colonnes** (il se coupe entre deux pages, elles non) : refus.
- Le texte extrait est celui des cellules, **ligne après ligne, de gauche à droite**
  (l'en-tête redessiné y revient à chaque page).

### Les blocs à position fixe : `texte_fixe`, `trait`, `image_fixe`

Pour un formulaire dessiné point par point (le certificat d'emploi Congés
Spectacles, `moteur/conges_spectacles.py`) : chaque bloc se pose à des
**coordonnées absolues de la page**, en points depuis son **coin haut-gauche**
(y vers le bas, comme dans PyMuPDF), quelles que soient les marges de la page.
Ces blocs **ne coulent pas** : aucun n'occupe de hauteur, aucun ne déplace le
suivant, et ils ne se coupent pas entre deux pages.

```json
{"type": "trait", "x1": 40.8, "y1": 419.28, "x2": 63.84, "y2": 419.28,
 "epaisseur": 3.12, "couleur": "#000000"}
{"type": "texte_fixe", "texte": "N° SIRET", "x": 48.0, "y": 579.84,
 "taille": 13.92, "police": "times", "gras": false, "couleur": "#000000",
 "alignement": "gauche"}
{"type": "image_fixe", "chemin": "/…/signature.png",
 "x": 441.84, "y": 775.5, "largeur": 80, "hauteur": 33.6, "centree": true}
```

- `trait` : un segment de (`x1`, `y1`) à (`x2`, `y2`), bouts droits.
  `epaisseur` (0,5 par défaut, strictement positive) ; `couleur` (`#000000`
  par défaut).
- `texte_fixe` : `texte`, `x`, `y` obligatoires. **`y` est la ligne de base**
  (le bas des lettres sans jambage, comme `insert_text` de PyMuPDF), pas le haut
  du texte. `taille` (celle de `police` par défaut), `couleur` (idem), `gras`
  (faux). `police` : `helvetica` (défaut, celle du reste du document), `times`
  ou `courier` : Liberation Sans, Serif (Times) ou Mono (Courier), embarquées
  en sous-ensemble comme le reste du document. Le gras de `courier` est
  Liberation Mono Bold. `alignement` : `gauche`
  (défaut : `x` est le bord gauche), `droite` (`x` est le bord droit du texte)
  ou `centre` (`x` est son milieu) ; `justifie` est refusé. Le texte tient sur
  **une ligne** (`\n` refusé), ne retourne jamais à la ligne, garde ses espaces
  (même en tête), et ne s'écrit qu'avec des caractères que la police possède :
  sinon c'est un refus d'une ligne qui nomme le caractère. Un texte vide
  n'écrit rien.
- `largeur_max` (facultatif, version 3, en points, strictement positive ; 0 ou
  négative est un refus) : la largeur que le texte ne doit pas dépasser. Le texte
  est mesuré **au corps demandé**, avec les métriques de la police embarquée
  (Liberation : celles d'Helvetica, de Times et de Courier ; le gras compte).
  Tant qu'il y tient (`largeur <= largeur_max`), le corps ne change pas ; sinon
  le corps devient `max(6, corps × largeur_max ÷ largeur)` — exactement
  l'ancien `corps_qui_tient` de `moteur/conges_spectacles.py` (retiré : le Python passe
  maintenant `largeur_max` = `limite − RESPIRATION − x`). Pas de palier : une règle de trois, d'un coup. Le
  plancher est de **6 pt, absolu** : un corps demandé plus petit que 6 et qui ne
  tient pas monte donc à 6, comme dans le Python. L'alignement (`droite`,
  `centre`) se fait sur la largeur du texte une fois réduit. *Écart connu avec le
  Python* : le `get_text_length` de PyMuPDF compte les lettres accentuées, « € » ou « ° »
  pour une largeur de repli (environ 0,35 em), pas la leur ; la fabrique mesure
  les vrais glyphes, donc réduit un peu plus un texte accentué qui déborde.
- `image_fixe` : `chemin`, `x`, `y` (coin haut-gauche de la **boîte**), `largeur`
  et `hauteur` de la boîte. L'image tient dans la boîte, **proportions
  gardées** : à la pleine largeur, ou, si elle déborde en hauteur, à la largeur
  qui remplit la hauteur. Par défaut elle est calée en haut à gauche de la
  boîte ; `centree` (faux par défaut) la centre. SVG, PNG, JPEG ; illisible ou
  introuvable : refus.

**Une page par certificat.** Une page fixe est **un document** (`documents`) :
chaque document commence sur une page neuve, et ses blocs fixes y sont
posés. Un document **ne mêle pas** blocs fixes et blocs qui coulent (refus qui
nomme le bloc fautif : une coordonnée absolue n'aurait pas d'origine claire
après un paragraphe), et ces blocs ne se placent pas dans des `colonnes` ni
dans un `tableau`. Les marges de la page ne changent pas leurs coordonnées (le
certificat les met à 0) ; le `logo` de la description, s'il y en a un, se
pose comme sur toute page. Les blocs se dessinent **dans l'ordre de la
liste** : ce qui vient après recouvre ce qui vient avant.

Le texte extrait (`get_text()`) est celui des blocs, dans l'ordre de la liste.
Sans `texte_fixe`, rien de la page n'est du texte : les traits et les images
ne s'extraient pas.

## En-tête et pied de page répétés

Deux clés facultatives de la description, `entete_page` et `pied_page`, posent
un texte **dans la marge** de chaque page (le haut pour l'en-tête, le bas pour le
pied) : le titre d'un état, « page n sur N ». Elles ne prennent aucune place dans
le corps, donc ne changent pas les sauts de page.

```json
"entete_page": {"gauche": "Livre de paie — 2026 (suite)", "taille": 8.5, "gras": true,
                "couleur": "#2C3E50", "distance": 24, "premiere_page": false},
"pied_page":   {"gauche": "Livre de paie — 2026", "droite": "page {page} sur {pages}",
                "taille": 6.5, "couleur": "#777777", "distance": 20}
```

- `gauche`, `centre`, `droite` (vides par défaut) : trois textes d'**une ligne**
  (`\n` refusé), avec les caractères de la police comme `texte_fixe`. Gauche et droite s'alignent
  sur les marges gauche et droite de la page ; le centre est centré entre elles.
- `{page}` y vaut le numéro de la page (à partir de 1, tous documents confondus),
  `{pages}` le nombre total de pages. Tout autre texte entre accolades reste tel quel.
- `taille` et `couleur` : celles de `police` par défaut ; `gras` (faux).
- `distance` (20 par défaut) : de la **ligne de base** du texte au bord de la page
  (haut pour l'en-tête, bas pour le pied). Le texte doit tenir dans la marge : c'est à
  la description d'y veiller.
- `premiere_page` (vrai par défaut) : faux, et la première page n'a pas ce repère (un
  état y porte déjà son grand titre, l'en-tête n'est que le rappel des suivantes).
- **Le total des pages** demande deux passes : un `{pages}` fait mettre le document en
  page une fois pour compter, puis une seconde avec le vrai nombre. L'en-tête et le
  pied sont dans les marges, la largeur d'un chiffre ne change donc aucun saut de page,
  et le résultat est le même à l'octet près d'un appel à l'autre. Sans `{pages}`, une
  seule passe.
- Le texte extrait les contient, page par page. Un programme plus ancien refuse ces
  deux clés (champ inconnu) : `fabrique.py` ne retient pas un programme qui ne répond pas
  `fabrique-pdf 3`, et l'opération s'arrête (aucun repli).

## Ce que la fabrique garantit

- Le **texte extrait** du PDF (PyMuPDF `page.get_text()`, ou `fabrique-pdf --texte`, qui
  donne le même sur les PDF de la fabrique) est celui des
  morceaux, dans l'ordre : l'avenant relit le contrat parti, et
  `contrat.contresigner` cherche le mot « signature » dans la colonne de
  droite.
- Les accents et « € » s'impriment et s'extraient justes ; **tout caractère que
  Liberation possède** aussi (« Ł », « ő », « č », « Ş », cyrillique, grec…) :
  les polices sont embarquées en sous-ensemble, avec la table `ToUnicode` qui
  rend le texte imprimé tel quel à l'extraction (l'espace insécable reste une
  espace insécable). Un caractère que la police n'a pas (un idéogramme, un
  emoji) est un refus d'une ligne : `bloc 3 : le caractère « 漢 » (U+6F22)
  n'existe pas dans la police Liberation`.
- Un texte à position fixe tombe à ses coordonnées à 0,1 pt près (tests).
- Même description, même PDF à l'octet près (aucune date variable dans le
  fichier, aucun nom de sous-ensemble tiré au hasard : l'étiquette
  `ABCDEF+` est une empreinte du nom de la police et des caractères) : les
  tests comparent.

## L'assemblage : fusionner des PDF existants

```
fabrique-pdf --assembler <sortie.pdf>   # la description arrive en JSON sur l'entrée standard
```

Même contrat que l'appel de mise en page (code 0 : le PDF est écrit ; sinon rien
n'est écrit et une ligne de français dit pourquoi), mais la description ne met rien
en page : elle liste, dans l'ordre, les pièces dont on prend les pages.

```json
{
  "format": 1,
  "pieces": [
    {"fichier": "/…/Bulletins_Octobre2026.pdf", "pages": [2, 1]},
    {"fichier": "/…/R_dupont.pdf"}
  ]
}
```

- `format` : facultatif ; s'il est là, c'est 1.
- `pieces` : au moins une. `fichier` : chemin d'un PDF. `pages` : numéros **à partir de 1**,
  dans l'ordre voulu ; une page peut revenir ; absent = toutes les pages du fichier.
  Un champ inconnu est un refus (comme partout).
- Un fichier illisible, chiffré, ou une page qui n'existe pas est un refus qui nomme la
  pièce (« pièce 2 : la page 9 n'existe pas dans … (4 page(s)) »).

Ce que le résultat garantit : mêmes pages, même texte extrait, même taille de page et
même rotation que les pages d'origine (les attributs hérités de l'arbre des pages sont
posés sur chaque page) ; seuls les objets que les pages choisies atteignent sont
recopiés ; les objets identiques (logo, polices, flux) ne sont écrits qu'une fois ;
même description, mêmes octets (aucune date, aucun identifiant).

Ce qu'il ne fait pas : signets / table des matières, métadonnées du document, formulaires
(les champs deviennent des annotations inertes), liens internes vers des pages non
retenues ; ni insertion d'images en page, ni rotation. Lire le texte d'un PDF, compter ses
pages, y poser un pied de page et le verrouiller ont leurs propres modes (ci-dessous).
Le dédoublonnage des pages par leur texte est fait par l'appelant (qui lit le texte avec
`--texte`) et qui n'envoie à `--assembler` que les pages retenues.

## Lire un PDF : `--pages` et `--texte`

```
fabrique-pdf --pages a.pdf [b.pdf …]     # [3, 1]
fabrique-pdf --texte a.pdf               # ["Page 1\n…\n", "Page 2\n…\n"]
```

Ils remplacent, de PyMuPDF, le nombre de pages d'un document (`len(doc)`) et le texte de
chaque page (`[page.get_text() for page in doc]`). La
sortie standard porte **une seule ligne de JSON** (UTF-8, les accents tels quels, pas
d'échappement `\uXXXX` hors des caractères de contrôle) et rien d'autre ; code 0.
Sinon rien n'est écrit sur la sortie standard, une ligne de français sur la sortie
d'erreur **nomme le fichier**, et le code est **1** (fichier absent, illisible, pas un PDF ;
pour `--texte`, aussi : chiffré avec un autre mot de passe que le vide) ; **2** si les arguments manquent
(`--pages` sans fichier, `--texte` sans ou avec plus d'un fichier).

- `--pages` : un tableau d'entiers, **dans l'ordre des fichiers donnés** ; un seul fichier
  refusé refuse l'ensemble (l'appelant qui veut tolérer un fichier abîmé les passe un à un).
  Un PDF chiffré se compte sans être déchiffré.
- `--texte` : un tableau de chaînes, **une par page, dans l'ordre des pages** ; une page sans
  texte (un scan) donne `""`. Chaque chaîne est faite comme celle de `page.get_text()` :
  **une ligne par `\n`, un `\n` après la dernière**, pas de ligne vide entre les blocs. Une page
  dont le contenu est illisible donne `""` et **une ligne d'avertissement** sur la sortie
  d'erreur (code 0) : le reste du document se lit.
- Un PDF chiffré en AES-256 par `--verrouiller` ou par MuPDF (mot de passe utilisateur vide)
  se lit sans rien demander ; les anciens chiffrements RC4 (`/V` 1 ou 2) aussi, quand lopdf les
  lit ; un autre mot de passe que le vide, ou un chiffrement qu'on ne sait pas lire
  (AES-128 de `/V` 4), est un refus (« … ne s'ouvre pas sans mot de passe »).
- Un fichier dont la table des références est fausse (« Microsoft Print To PDF » décale tout
  d'un octet) est réparé à la lecture : on retrouve les objets dans le fichier, comme les
  lecteurs de PDF.

**Comment le texte est fait.** Pas de tri géométrique : le texte sort **dans l'ordre où le PDF
l'écrit**, comme MuPDF. Ce que MuPDF fait en plus, la fabrique le fait :

- *Les lignes.* Un caractère prolonge la ligne courante s'il part du bout du précédent à moins
  de 0,15 corps (collé), ou entre 0,15 et 0,8 corps (un espace est ajouté s'il n'y en a pas
  déjà), ou en retrait de 0,8 corps au plus (deux glyphes qui se chevauchent), **et** si sa ligne
  de base est à moins de 0,8 corps de celle du précédent (un exposant, « 1er », reste dans sa
  ligne). Sinon il ouvre une ligne. Ces seuils ont été retrouvés sur 370 000 couples de
  caractères de nos PDF : aucun n'est mal classé. Un texte en tableau (un libellé puis, plus à
  droite, sa valeur) donne donc deux lignes, comme avec PyMuPDF.
- *Les polices.* La table `ToUnicode`, quand la police en a une, donne le texte (elle l'emporte,
  comme dans MuPDF) ; sinon l'encodage de la police simple (WinAnsi, MacRoman, Standard,
  `/Differences` avec les noms de glyphes d'Adobe, `uniXXXX`) ; une police composée sans
  `ToUnicode` donne U+FFFD. Les largeurs viennent de `/Widths` ou `/W` ; Liberation tient lieu
  de métrique aux polices standard (Helvetica, Times, Courier) citées sans largeurs. Le 0xAD de
  WinAnsi est le trait d'union conditionnel (U+00AD), comme chez MuPDF.
- *Les annotations.* MuPDF dessine les champs de formulaire : leur apparence (`/AP /N`, état
  `/AS`, calée sur `/Rect`) est lue **après** le contenu de la page, sauf les annotations
  cachées ou `Popup`. Les AEM de CoolPaie sont des formulaires : leurs champs sortent dans le texte.
- *Le reste.* `/ActualText` remplace les glyphes qu'il enveloppe (Word écrit ainsi ses espaces) ;
  les objets formulaire (`Do`) sont suivis ; un caractère entièrement hors de la page
  (`CropBox` ∩ `MediaBox`) est ignoré ; `/Contents` peut être un flux, un tableau ou la
  référence d'un tableau.

**Ce qui diffère encore de PyMuPDF**, mesuré (README.md) : l'**espacement** près de certains
glyphes de symboles (MuPDF prend l'avance du glyphe dans le fichier de police, la fabrique celle
de `/Widths` : un espace de plus ou de moins après une case ou une puce) ; les polices en
petites capitales dont les codes sont des minuscules (MuPDF lit la capitale dans le nom du
glyphe) ; les caractères de la zone privée de Wingdings (MuPDF les traduit en flèches, la
fabrique les rend tels quels) ; les polices CFF et Type 3 sans `ToUnicode` ; le texte vertical et
les pages dont `/Rotate` change le sens de lecture ne sont pas retraités. Mesuré sur nos PDF
(README.md) : 254 pages sur 255 des exports de CoolPaie et 120 sur 124 de celles de PuppetPaie
(contrats, avenants, bulletins, certificats, y compris verrouillés) ont **le même texte que
PyMuPDF, espaces ramenés à un** ; les quatre autres (des attestations France Travail, avec
leurs cases et leurs pointillés) diffèrent par quelques espaces et le nombre de points d'un
pointillé. Les expressions que lit le Python (`numeros_archives`, `_a_qui`,
`_date_page`, `_categorie_page`, `_cle_page`, la normalisation de l'avenant) donnent les mêmes
résultats avec l'un et l'autre texte.

## Inspecter un PDF : `--inspecter`

```
fabrique-pdf --inspecter a.pdf
```

Remplace ce que les tests Python demandaient encore à PyMuPDF : `page.rect`, `search_for`,
`get_drawings`, `get_images` / `get_image_bbox`, `get_fonts`, `doc.metadata`,
`doc.permissions`, `metadata["encryption"]` ; et sert à reconstruire des tableaux (le relevé
du cahier technique DSN, à la place de `page.find_tables()`) à partir des mots et des traits.
Même contrat que `--texte` : **une seule ligne de JSON** sur la sortie standard, code 0 ; un
refus (fichier absent, illisible, pas un PDF, chiffré avec un autre mot de passe que le vide) est
une ligne de français qui **nomme le fichier** sur la sortie d'erreur, code **1** ; **2** si
l'argument manque ou s'il y en a plus d'un. Un PDF chiffré avec le mot de passe utilisateur vide
(AES-256 de `--verrouiller`, ou RC4 que lopdf lit) se lit comme pour `--texte`. Une page dont le
contenu est illisible donne une page sans mots, sans images ni traits, et une ligne d'avertissement
sur la sortie d'erreur (code 0).

```json
{"chiffrement": null,
 "metadonnees": {"Producer": "…", "Title": "…"},
 "pages": [
   {"largeur": 595.28, "hauteur": 841.89, "rotation": 0,
    "polices": ["QTVKDB+LiberationSans", "DTPUXL+LiberationSans-Bold"],
    "images": [[487.28, 28.0, 561.28, 52.67]],
    "traits": [[41.4, 108.44, 285.1, 108.94]],
    "mots": [{"texte": "Bonjour", "rect": [50.0, 52.09, 83.35, 64.38], "ligne": 0}]}]}
```

Chiffré : `"chiffrement": {"filtre": "Standard", "V": 5, "R": 6, "longueur": 256, "P": -3372,
"methode": "AESV3"}` (celui de `--verrouiller`). `P` est lu en entier **signé** sur 32 bits ;
`methode` est le `/CFM` du filtre des flux (`AESV3`, `AESV2`, `V2`), et `RC4` pour `/V` 1 et 2 ;
`longueur` est `/Length` (40 par défaut), 128 pour `/V` 4, 256 pour `/V` 5.

- **Repère.** Points, **origine en haut à gauche** de la page visible (`CropBox` ∩ `MediaBox`),
  y vers le bas : celui de PyMuPDF. Tous les nombres sont arrondis à 2 décimales. Une boîte est
  `[x0, y0, x1, y1]` avec `x0 ≤ x1` et `y0 ≤ y1`.
- **`largeur`, `hauteur`, `rotation`.** Ceux de `page.rect` : la page **tournée** de `/Rotate`
  (une page 200 × 100 de `/Rotate 90` donne 100 × 200 ; `rotation` vaut 0, 90, 180 ou 270). Comme
  dans PyMuPDF, **les boîtes (mots, images, traits) ne sont pas tournées** : elles restent dans le
  repère de la page non tournée (c'est ce que rendent `search_for` et `get_drawings`, mesuré sur
  une page à `/Rotate 90`, 180 et 270). Pour un PDF paysage fabriqué par la fabrique (largeur >
  hauteur dans la description, pas de `/Rotate`) il n'y a rien à corriger. Sans `MediaBox`, la
  page est au format Lettre (612 × 792), comme PyMuPDF.
- **`mots`.** Les lignes de `--texte` coupées aux blancs (espace, espace insécable, toute espace
  Unicode), **dans l'ordre de `--texte`**, avec la boîte de leurs glyphes. `ligne` est le numéro
  (0, 1, …) de la ligne de texte dans la page, **le même découpage en lignes que `--texte`** : en
  recollant les mots d'une même `ligne` avec un espace, on retrouve la ligne de `--texte`, blancs
  ramenés à un. Les annotations dessinées (`/AP /N`) sont lues comme `--texte` le fait, **après**
  le contenu de la page : leurs mots, leurs images et leurs traits viennent à la fin des listes
  (leurs polices, non : voir `polices`).
- **Boîte d'un glyphe.** De la ligne de base moins l'ascendant à la ligne de base plus le
  descendant, sur l'avance du glyphe, sous le corps, la matrice du texte et la CTM. Ascendant et
  descendant sont, comme dans MuPDF, ceux **du fichier de police incorporé** (`hhea` de la table
  TrueType / OpenType, lue avec `ttf-parser`) : exacts pour Liberation Sans (0,905 / 0,212). Les
  polices standard non incorporées (Helvetica, Arial, Times, Courier) prennent les valeurs mesurées
  de MuPDF (Helvetica 1,075 / 0,299 ; Times 1,053 / 0,281 ; Courier 0,932 / 0,317, un peu moins
  en gras) ; sinon le `/FontDescriptor` (`Ascent`, `Descent`) ; sinon 0,8 et 0,2 du corps. Une
  séquence `/ActualText` répartit sa boîte à parts égales entre ses caractères.
- **`images`.** La boîte de chaque image dessinée, dans l'ordre du flux : le `Do` d'un XObject
  `/Image` et les images en ligne (`BI … ID … EI`), c'est-à-dire le carré unité transformé par la
  CTM (un objet formulaire suivi par `Do` transmet la sienne). Les images en ligne ne sont lues
  que par `--inspecter` (lopdf ne sait pas lire leurs données binaires : on les remplace par un
  opérateur neutre avant l'analyse ; `--texte` d'une page qui en porte garde son comportement).
- **`traits`.** La boîte de chaque chemin **peint** (`S s f F f* B B* b b*`, pas `n`), dans l'ordre
  du flux, **sans compter l'épaisseur du trait**. Une boîte est celle des points de construction,
  courbes comprises (points de contrôle) : un rectangle à coins arrondis est plus large, d'au plus
  2 à 3 pt, que la boîte exacte de PyMuPDF. **Comme PyMuPDF**, un chemin tracé (`S`) juste après
  avoir été rempli (`f`), mêmes points, ne compte que pour un dessin (le « fs » de
  `get_drawings`) : un cadre rempli et bordé est un trait, pas deux. Les chemins qui ne servent
  qu'à découper (`W n`) et les chemins à moins de deux points ne comptent pas.
- **`polices`.** Les `/BaseFont` des polices **déclarées** dans les ressources de la page et
  dans celles des objets formulaire que le contenu appelle (`Do`), sans doublon, dans l'ordre de
  première rencontre : ce que rend `page.get_fonts()` (qui compte aussi les polices déclarées et
  jamais employées). Le préfixe de sous-ensemble (`QTVKDB+`) est conservé ; les polices de
  l'apparence des annotations et celles sans `/BaseFont` (Type 3) n'en font pas partie.
- **`metadonnees`.** Le dictionnaire `/Info`, tel quel (clés de l'auteur du PDF : `Title`,
  `Producer`, `CreationDate`…), les chaînes **décodées** : UTF-16BE ou UTF-16LE avec marque, UTF-8
  (avec marque, ou sans quand les octets en sont de l'UTF-8 valide : printpdf écrit ainsi le titre
  de nos PDF), sinon PDFDocEncoding ; les noms et les nombres sont rendus en texte. `{}` s'il n'y a
  pas de `/Info`. (`doc.metadata` de PyMuPDF renomme les clés en minuscules et remplit les absentes
  de `""` : le Python qui s'y appuyait lit ici `metadonnees.get("Title", "")`.)

**Mesuré contre PyMuPDF 1.28** (script jetable hors du dépôt, les quatre premières pages de
chacun de 37 PDF — 72 pages — de `exemples/`, `demo/`, `prive/sorties/`,
`prive/export-coolpaie/` et `atelier/reference/` : contrats, bulletins, AER, certificats, exports de
CoolPaie, DSN, plaquettes d'organismes) :

| ce qu'on compare | résultat |
|---|---|
| `largeur` et `hauteur` contre `page.rect` | identiques sur 100 % des pages (écart < 0,02) |
| `polices` contre les noms de `get_fonts()` | mêmes ensembles sur 100 % des pages |
| `images` contre `get_image_info()["bbox"]` | même nombre partout ; écart maximal 0,005 pt |
| nombre de `traits` contre `len(get_drawings())` | identique sur 100 % des pages (après la fusion « fs ») ; boîtes : médiane 0, p90 0, max 2,77 pt (courbes) |
| texte des `mots` contre `get_text("words")` | même suite de mots sur 63 pages sur 72 (les autres : voir ci-dessous) ; boîtes : médiane 0,00 pt, p90 0,04 pt |
| boîte d'un mot contre `search_for(mot)` | écart médian 0,01 pt, p90 0,75 pt (253 mots tirés au hasard) ; sur les PDF de la fabrique **exactement** les mêmes boîtes (< 0,01 pt) |

Les écarts qui restent : sur les polices dont MuPDF tire l'ascendant d'un fichier que la fabrique
ne lit pas (CFF nu, Type 1, Type 3) la boîte est trop haute ou trop basse de 1 à 3 pt (jusqu'à
6,6 pt sur le glyphe « ⚠ » d'une plaquette) : **les `x` sont toujours justes, les `y` sont bons à
1 ou 2 pt près**, ce qui suffit à ranger des mots en lignes et en colonnes, pas à comparer deux
bords au demi-point ; **MuPDF coupe aussi les mots à certains changements d'écriture** (un glyphe
hébreu de symbole `ן`, un émoji collé à un mot), la fabrique seulement aux blancs : un mot de
PyMuPDF peut y être deux, ou l'inverse ; `search_for` d'un pointillé long donne plusieurs
rectangles là où la fabrique a un seul mot. Le texte et les lignes sont ceux de `--texte`, avec
ses propres écarts (« Lire un PDF »).

## Marquer un PDF : `--marquer`

```
fabrique-pdf --marquer entree.pdf sortie.pdf "texte"
```

Pose `texte` en pied de **chaque page** de `entree.pdf` et écrit `sortie.pdf` (qui peut être
le même fichier : l'entrée est lue en entier avant). C'est `_marquer` de
`moteur/operations_dossier.py` (`page.insert_text((34, hauteur − 12), texte, fontname="helv",
fontsize=6, color=(0.45, 0.45, 0.45))`) : **à 34 pt du bord gauche, ligne de base à 12 pt du
bas de la page** (la `CropBox`, dans la `MediaBox`), **corps 6**, gris `#737373`
(0,45 / 0,45 / 0,45), aligné à gauche, **sur une ligne** (les sauts de ligne du texte deviennent
des espaces) et sans retour à la ligne : un texte trop long sort de la page. Une page tournée
(`/Rotate` 90, 180, 270) reçoit la marque **dans son sens de lecture**, à 34 pt du bord gauche
et 12 pt du bas *tels qu'on les voit* (PyMuPDF, lui, la posait dans le repère non tourné).

- *La police* est Liberation Sans (les métriques d'Helvetica) **en sous-ensemble** (les glyphes du
  texte seulement : quelques kilo-octets), avec sa table `ToUnicode` : le texte extrait de la
  page est celui d'avant, puis la marque sur sa ligne. Un caractère que Liberation n'a pas est un
  refus qui le nomme. L'em-tiret « — » du pied de page s'imprime (PyMuPDF, avec Helvetica
  standard, le remplaçait par « · »).
- *Les flux existants ne sont pas touchés.* Le contenu de la page devient `q`, les anciens flux (les
  mêmes objets, les mêmes octets), autant de `Q` qu'il restait de `q` ouverts **plus un**, puis le
  flux de la marque, qui part donc toujours d'un état graphique neuf (matrice, couleur, découpe
  de l'ancien contenu sans effet sur elle). Les ressources de la page sont **recopiées dans la
  page** avec la police de la marque (`/PPMarque`) en plus : celles d'un nœud `Pages`, partagées
  par plusieurs pages, ne sont pas modifiées.
- Le fichier est réécrit en entier (pas de mise à jour incrémentale) ; même entrée, même texte :
  mêmes octets. Un texte vide laisse le fichier tel quel. Un PDF chiffré est refusé (« le PDF est
  chiffré : on ne le marque pas »).
- Code 0 : le PDF est écrit ; 1 : refus d'une ligne qui nomme le fichier, rien n'est écrit (et
  la sortie, si ce n'est pas l'entrée, n'existe pas) ; 2 : il manque un argument.

## Verrouiller un PDF : `--verrouiller`

```
fabrique-pdf --verrouiller entree.pdf sortie.pdf
```

Remplace `verrouiller` de `moteur/contrat.py` (PyMuPDF, `PDF_ENCRYPT_AES_256`). Le PDF est
chiffré avec le **chiffrement standard de PDF 2.0** (ISO 32000-2) : `/V 5`, `/R 6`, AES-256 en
CBC (`/CFM /AESV3`), **chaînes et flux** chiffrés un à un (IV de 16 octets tiré au hasard,
bourrage PKCS#7), `/EncryptMetadata true` (les métadonnées aussi). L'en-tête du fichier
annonce `%PDF-2.0`.

- *Mot de passe utilisateur : vide.* Le fichier s'ouvre dans tout lecteur sans rien demander.
- *Mot de passe propriétaire : tiré au hasard et jeté.* Personne ne peut lever les restrictions.
- *Permissions : impression, copie et accessibilité, rien d'autre* : `/P` = **−3372**
  (`0xFFFFF2D4`), la valeur que MuPDF écrivait pour `PDF_PERM_PRINT | PDF_PERM_COPY |
  PDF_PERM_ACCESSIBILITY` : modification, annotation, remplissage de formulaire et assemblage
  interdits. *À savoir* : le bit 12 (impression en haute qualité) n'y est pas, comme avant ; les
  lecteurs qui le respectent (qpdf : `print_highres False`) impriment en basse résolution. Ajouter
  `0x800` à `PERMISSIONS` (`src/chiffrement.rs`) le lèverait.
- Le dictionnaire `/Encrypt` porte `/O`, `/U` (48 octets), `/OE`, `/UE` (32), `/Perms` (16,
  chiffré, avec la marque `adb` et l'indicateur `EncryptMetadata`), `/CF`, `/StmF`, `/StrF`, `/Length 256`.
  Un `/ID` neuf est écrit. Les objets de structure du fichier d'origine (flux d'objets, table de
  références en flux) sont réécrits en table classique.
- Un fichier déjà verrouillé (mot de passe vide) est d'abord ouvert, puis verrouillé de neuf. La clé de
  fichier, les sels et les IV sont tirés au hasard : **deux verrous du même fichier ne donnent pas
  les mêmes octets** (la seule sortie de la fabrique qui ne soit pas déterministe).
- Vérifié avec PyMuPDF (`needs_pass` faux, `permissions` −3372, `encryption` « Standard V5 R6
  256-bit AES », texte identique avant et après) et avec qpdf 12 (`--check`, `--show-encryption`,
  contenus de pages identiques une fois déchiffrés, un autre mot de passe est refusé).
- Code 0 : le PDF est écrit ; 1 : refus d'une ligne qui nomme le fichier (illisible, pas un PDF,
  chiffré avec un mot de passe qu'on n'a pas), rien n'est écrit ; 2 : il manque un argument.

## Remplir un formulaire : `--champs` et `--remplir`

```
fabrique-pdf --champs a.pdf                    # {"NOM": {"valeur": "…", …}, …}
fabrique-pdf --remplir entree.pdf sortie.pdf   # la description arrive en JSON sur l'entrée standard
```

Ils remplacent ce que PyPDF (`PdfReader(...).get_fields()`) et PyMuPDF (`page.widgets()`,
`widget.update()`, `insert_image`, `show_pdf_page`) faisaient de l'attestation employeur
rematérialisée de France Travail (`moteur/aer.py`, `moteur/cachet.py`) : lire ce que France
Travail a imprimé, remplir le cadre 5, poser la signature et le cachet.

**Les rectangles** sont en points, **origine en haut à gauche** de la page visible (`CropBox`
dans `MediaBox`), y vers le bas : le repère de `Widget.rect` dans PyMuPDF. **Les pages** se
comptent à partir de 1.

### `--champs`

Une ligne de JSON sur la sortie standard, code 0 : un objet dont les clés sont les **noms
complets** des champs terminaux (`parent.enfant` pour un champ dans une hiérarchie), dans
l'ordre de `/AcroForm /Fields`.

```json
{"EMPLOYEUR_NOM": {"valeur": null, "type": "texte", "lecture_seule": false,
                   "page": 3, "rect": [131.1, 541.82, 569.27, 557.84]},
 "SAL_NB_CACHET_01": {"valeur": "2", "type": "texte", "lecture_seule": true, "page": 2, "rect": […]}}
```

- `valeur` : `/V`, hérité des parents s'il le faut ; une chaîne de texte (PDFDocEncoding,
  UTF-16BE ou UTF-8 à marque, décodés), un nom pour une case à cocher (`"Off"`, `"Oui"`), une
  liste pour un choix multiple ; **`null`** si le champ n'a pas de valeur.
- `type` : `texte` (`/FT /Tx`), `bouton`, `choix`, `signature`, `inconnu`.
- `lecture_seule` : le bit 1 de `/Ff` (hérité). Les champs que France Travail remplit le sont.
- `page`, `rect` : ceux du **premier** widget du champ (celui qui est dans le `/Annots` d'une
  page) ; `null` si aucun widget n'est sur une page. Arrondis à deux décimales.
- Un nom porté par deux champs : le premier (comme PyPDF). Un PDF sans formulaire rend `{}`.
  Un PDF chiffré avec un mot de passe utilisateur vide se lit ; sinon refus.
- Refus (code 1) : une ligne qui nomme le fichier ; code 2 : il manque l'argument.

### `--remplir`

```json
{"format": 3,
 "champs": {"EMPLOYEUR_NOM": "DUPONT", "EMPLOYEUR_DATE_SIGN_J": "01"},
 "images": [{"page": 3, "rect": [35.1, 687.4, 135.8, 735.6], "marge": 3, "fichier": "/…/signature.svg"},
            {"page": 3, "rect": [401.4, 685.4, 562.1, 764.7], "marge": 3, "svg": "<svg …>…</svg>"}]}
```

`format` est obligatoire (3) ; `champs` et `images` sont facultatifs ; une clé inconnue est un
refus. **Tout est vérifié avant d'écrire** : un seul refus, et rien n'est écrit.

*Les champs.* Chaque nom doit être celui d'un champ **texte** du formulaire (refus sinon :
« le formulaire n'a pas de champ « X » », « … n'est pas un champ texte »). Un champ **en lecture
seule** est un refus (« … est en lecture seule : on n'y écrit pas ») : c'est ce qui garde les
chiffres de France Travail, tous en lecture seule, de toute retouche. Un champ de mot de passe
aussi. Une valeur plus longue que `/MaxLen` est un refus. Pour chaque champ :

- `/V` reçoit la valeur (PDFDocEncoding quand elle s'y écrit, UTF-16BE sinon) ;
- **chaque widget reçoit une apparence neuve** (`/AP /N`, un objet formulaire de la taille de
  `/Rect`) : le texte s'affiche pareil dans tous les lecteurs, y compris ceux qui ne refont pas
  les apparences (Poppler, les aperçus des messageries). Elle reprend celle qu'écrivait MuPDF,
  mesurée sur les AER : `/Tx BMC`, découpe à 1 pt du bord, texte à 2 pt du bord gauche (ou
  centré, ou à 2 pt du bord droit, selon `/Q`), ligne de base à `(h − corps) / 2 + 0,2 × corps`
  du bas ; **peigne** (`Comb` avec `/MaxLen`) : un caractère centré dans chacune des `MaxLen`
  cases ; **multiligne** : lignes de `1,116 × corps` à partir du haut, coupées aux espaces pour
  tenir à 2 pt des bords (et aux sauts de ligne de la valeur). Le corps et la couleur sont ceux
  de `/DA` (un corps 0 : ce qui tient en hauteur, 12 au plus, et sur une ligne en largeur) ;
- **la police est Liberation Sans** (les métriques d'Helvetica, que nomme le `/DA` des AER)
  embarquée en sous-ensemble avec sa `ToUnicode` : tout caractère qu'elle a s'écrit
  (« Łukasz »), un caractère qu'elle n'a pas est un refus qui nomme le champ. `/DA` et `/DR`
  ne sont pas touchés : un lecteur qui refait l'apparence garde la police du formulaire.

*Les images* se posent **par-dessus** le contenu de la page, dans l'ordre de la liste :

- `page`, `rect` (`[x0, y0, x1, y1]`, x1 > x0, y1 > y0) ; `marge` (0 par défaut) rentre la case
  de chaque côté ; `fichier` (un chemin) **ou** `svg` (le texte d'un SVG), l'un des deux.
- *Proportions gardées* : l'image occupe le plus grand rectangle à ses proportions (pixels d'un
  PNG ou d'un JPEG, taille d'un SVG) qui tient dans la case, **centré**. (PyMuPDF étirait un
  JPEG dont la résolution n'était pas celle qu'il attendait : un tampon rond sortait ovale.)
- *Ce qu'est le fichier* se lit dans ses premiers octets, pas dans son extension : PNG, JPEG ou
  SVG ; autre chose est un refus.
- *SVG* : **vectoriel** (usvg, puis svg2pdf) ; son texte est mis en tracés avec **Liberation**
  (Sans, Serif, Mono ; `Helvetica`, `Arial` et `sans-serif` sont Liberation Sans) et aucune
  police du système : le même SVG donne le même dessin sur tous les postes. Une image matricielle
  dans le SVG (`data:`) est reprise ; un fichier externe qu'il citerait ne l'est pas.
- *PNG* : décodé, réécrit en Flate (RVB ou gris), sa **transparence** en `/SMask`. *JPEG* en
  niveaux de gris ou RVB : **tel quel** (`DCTDecode`, aucune recompression) ; CMJN : décodé et
  réécrit en RVB.
- Le contenu de la page devient `q`, les anciens flux (les mêmes objets), les `Q` qui les
  referment, puis le flux des images (`q a 0 0 d e f cm /PPImageN Do Q`), comme `--marquer` ;
  les ressources de la page sont recopiées dans la page avec les images en plus. Une page tournée
  (`/Rotate`) est un refus ; une page qui n'existe pas aussi.

Le fichier est réécrit en entier (pas de mise à jour incrémentale) ; un PDF chiffré est refusé
(« le PDF est chiffré : on ne le remplit pas »). Même entrée, même description : mêmes octets.
Code 0 : le PDF est écrit ; 1 : refus d'une ligne qui nomme le fichier d'entrée, rien n'est
écrit (et la sortie, si ce n'est pas l'entrée, n'existe pas) ; 2 : il manque un argument.

**Vérifié sur les huit AER réelles** (septembre et octobre 2026, deux salariés, jusqu'à deux par
salarié et par mois) : pages 1 et 2 **identiques au pixel** à celles que signait PyMuPDF (rien
n'est touché hors du cadre 5) ; page 3, le même cadre 5 à l'œil, les écarts tenant au dessin des
lettres (Liberation embarquée contre l'Helvetica de MuPDF) ; rendu identique par MuPDF et par
Poppler.
