# Modifications apportées à genpdfi

Cette copie vient de **genpdfi 0.2.7** (crates.io, dépôt
<https://github.com/theiskaa/genpdfi>, commit `fe9569cc52eec0719a2a5eb3074cd25b79b8da5d`),
écrit par Robin Krahl et Ismael Sh, publié sous **Apache-2.0 OU MIT**. Les textes des licences
sont dans `LICENSES/` (`Apache-2.0.txt`, `MIT.txt`, `CC0-1.0.txt` — ce dernier couvre les
fichiers de documentation, comme dans la source). Le dossier `LICENSES/` est celui de la source,
recopié sans changement ; la mention de droits d'auteur de la source est reprise ci-dessus.

L'Apache-2.0 (§ 4 b) demande que les fichiers modifiés portent une mention visible : chacun des
fichiers ci-dessous commence par un commentaire qui renvoie ici. Ce fichier liste **chaque
modification**, par fichier. Il se met à jour à chaque changement dans `genpdfi/`.

Version de cette copie : `0.2.7-puppetpaie.1`.

## Retraits

- **Fonctionnalité `hyphenation`** (césure) et la dépendance `hyphenation` : retirées
  (`Cargo.toml`, `Context::hyphenator`, `Document::set_hyphenator`, la fonction `split` de
  `wrap.rs`).
- **Exemples** (`examples/`) et le fichier `subset_test.ttf` : retirés.
- **Dépendance `lopdf` 0.26** : retirée (elle ne servait qu'à l'ancien encodage Windows-1252, remplacé
  ci-dessous ; printpdf apporte sa propre version de lopdf, dont on réutilise la ré-exportation).
- **Dépendance de développement `float-cmp`** et les tests de la bibliothèque qui en dépendaient
  (`lib.rs`, `elements/images.rs` : calculs de rotation et de boîte englobante).
- **`Rotation` et `Scale`** (`lib.rs`) : retirés avec l'élément `Image` qui seul les utilisait.
- **`src/subsetting.rs`** (découpage de police par la crate `subsetter`) et la dépendance
  **`subsetter`** : retirés. `subsetter` 0.2.6 traîne `skrifa`, `read-fonts` et `write-fonts`
  (330 Ko dans le programme, pour un module qu'aucun appelant n'employait) ; le découpage dont la
  fabrique a besoin (polices TrueType à contours `glyf`) est écrit dans `src/pdf_subset.rs`.
- **`CHANGELOG.md` et `README.md`** de la source : conservés tels quels, pour mémoire.

## `Cargo.toml`

- Réécrit : nom `genpdfi`, version `0.2.7-puppetpaie.1`, `publish = false`.
- Fonctionnalités : `images` (PNG et JPEG, crate `image` 0.24 sans fonctionnalité par défaut, avec
  `png` et `jpeg`) et `svg` (printpdf/svg et `usvg` 0.35) ; les deux sont actives par défaut.
- Dépendance `printpdf` 0.7.0 sans fonctionnalité par défaut (retire `js-sys`).
- Dépendance `subsetter` retirée (voir Retraits).

## `src/lib.rs`

- `Alignment::Justified` (alignement justifié) ; pour une image, il vaut « à gauche ».
- La taille de police passe de `u8` à **`f32`** (`Document::set_font_size`), ici et dans `style.rs`,
  `fonts.rs` et `render.rs` (la fabrique écrit du 7,5 pt).
- `pub mod wrap` (le résultat de la mise en lignes est public), `pub use printpdf` (la fabrique
  y prend `BuiltinFont`), `pub fn pt(f32) -> Mm` (conversion des points en millimètres).
- `Document::context()` : donne le contexte de rendu, pour mesurer un élément avant de le poser.
- `pub mod pdf_subset` (voir plus bas) ; `Document::render` donne au `Renderer` les polices en
  sous-ensemble et leurs caractères (`Renderer::set_subsets`) avant d'écrire le fichier.

## `src/wrap.rs` — réécrit

Le `Wrapper` d'origine est remplacé par `wrap::layout`, qui rend des `Line` :

- un mot peut s'étendre sur plusieurs morceaux de styles différents (« FIL TENDU » en gras suivi de
  « , » ne se coupe pas entre les deux) ;
- les espaces de fin de ligne ne comptent pas dans la largeur (l'original y comptait l'espace
  qui suit le dernier mot : mot renvoyé à la ligne trop tôt, lignes décalées à droite ou au
  centre) ;
- `\n` force un retour à la ligne ; une ligne vide est possible ; un `\n` final n'en ajoute pas ;
- un mot plus large que la ligne est coupé lettre à lettre (l'original abandonnait le rendu avec
  une erreur) ;
- chaque `Line` porte sa largeur naturelle, son nombre d'espaces ordinaires et un indicateur
  « dernière ligne du paragraphe ou ligne finie par `\n` » — ce qu'il faut à la justification.

## `src/elements.rs`

- `Paragraph` réécrit autour de `wrap::layout` : la mise en lignes se fait une fois, à la première
  pose, puis les lignes sont consommées page après page.
- **Alignement justifié** : pour chaque ligne qui n'est ni la dernière du paragraphe ni finie par
  `\n`, l'espace restant est réparti **entre les mots** par l'opérateur PDF `Tw` (espacement des
  mots), posé ligne par ligne et remis à zéro à la fin de la ligne. Il ne s'applique qu'à l'espace
  ordinaire (U+0020) : l'espace insécable (U+00A0) ne s'étire pas et ne coupe pas la ligne.
- `Paragraph::set_line_height_factor` : hauteur de ligne exprimée en **facteur de la taille de
  police** (1,25 = 1,25 × la taille, comme `line-height: 1.25` en CSS), la ligne de base étant
  centrée dans l'interligne. Sans ce réglage, le comportement d'origine (métriques de la police
  × interligne du style) est conservé.
- `Paragraph::layout` et `Paragraph::height` : mise en lignes et hauteur **sans dessiner**, pour
  qu'un bloc qui ne se coupe pas (les colonnes de la fabrique) sache sa hauteur avant de se poser.
- `Element::image` : l'élément `Image` est remplacé par `Picture` (voir `elements/picture.rs`).

## `src/elements/picture.rs` — nouveau (remplace `elements/images.rs`)

- `Picture` : PNG, JPEG ou SVG, posé à une **largeur** donnée, hauteur selon les proportions ;
  passe à la page suivante s'il ne tient pas dans la place restante ; `draw_at` le pose en position
  absolue (le logo de chaque page).
- Le format se reconnaît à l'**octet de tête** (signature PNG, signature JPEG, balise `<svg`), jamais
  à l'extension ; `svg_hint` permet à l'appelant de dire « l'extension est `.svg` ».
- **PNG avec canal alpha accepté** (l'original le refusait). printpdf 0.7.0 écrit mal le masque
  de transparence (flux en ligne, hauteur fausse) : l'image est donnée à printpdf en RVB opaque,
  marquée dans son `BBox`, et le vrai masque (un flux indirect) est attaché à l'écriture du fichier
  (`render::finish`).
- **JPEG en niveaux de gris ou en couleurs recopié tel quel** (filtre `DCTDecode`), sans
  décompression ; les autres (CMJN, précision autre que 8 bits) sont décodés. Dans tous les cas le
  fichier est **décodé une fois pour le vérifier** : un fichier tronqué est un refus.
- **Image ramenée à la résolution d'impression** (ticket #228, 20/10/2026) : une image PNG ou JPEG
  plus fine que 300 pixels par pouce à sa largeur posée (de plus d'un quart) est réduite à
  300 pixels par pouce (Lanczos 3, proportions gardées ; en couleurs prémultipliées quand elle a
  une transparence). Un JPEG réduit est réencodé en JPEG (qualité 90). Un logo de 1 200 pixels
  posé sur 26 mm (1 168 pixels par pouce) pesait l'essentiel du bulletin.
- **SVG vectoriel** (`printpdf::svg`). Proportions exactes lues par usvg (printpdf arrondit la
  taille à l'entier). Un SVG qui utilise du texte, un dégradé, un motif, une transparence ou une
  image intégrée est **refusé** : printpdf 0.7.0 recopie l'image sans les objets auxquels elle
  renvoie, ce qui ferait un PDF faux (vérifié sur un dégradé : MuPDF signale une erreur et le
  dégradé disparaît ; une transparence est perdue).

## `src/fonts.rs`

- **Les polices standard utilisent les métriques du TTF fourni** (Liberation Sans, identique à
  Helvetica) au lieu d'une table de largeurs codée en dur (Helvetica droite seule : le gras,
  l'italique et les lettres accentuées avaient des largeurs fausses, `œ`, `—`, `’` la largeur par
  défaut). Sans ce changement, ni la césure des lignes ni la justification ne tombent juste.
- Approche gauche du premier caractère : nulle pour les polices standard.
- Tailles en `f32`.
- **Polices embarquées en sous-ensemble** : `FontData::new_subset` (le fichier TTF, partagé), un
  `RawFontData::Subset` et `Font::is_subset`. Une telle police se **pose comme une police
  standard** (`is_builtin` vrai : texte écrit d'un trait, sans approche gauche ni crénage, largeurs
  de la police) mais ses glyphes sont embarqués. `FontCache` note les caractères réellement
  imprimés avec chacune (`subset_codes`, appelé à l'écriture du texte) et en donne la liste au
  rendu (`subset_faces`) ; `load_pdf_fonts` ne donne à printpdf qu'une **doublure** de la police
  (voir `pdf_subset.rs`).

## `src/pdf_subset.rs` — nouveau

Le sous-ensemble de police, fait une fois le fichier écrit par printpdf (appelé par
`render::finish`) :

- printpdf embarque la police entière et sa table `ToUnicode` entière, en parcourant tout
  l'espace Unicode (70 ms par police) : on lui donne à la place une **doublure** (la police
  réduite à son `.notdef`, marquée d'un numéro), qu'on retrouve dans le fichier écrit ;
- **découpage TrueType** écrit ici (`decouper_ttf`) : les glyphes voulus, le `.notdef` en tête,
  les composants des glyphes composés ; `glyf`, `loca` (format long), `hmtx`, `head`, `hhea`,
  `maxp` refaits, somme de contrôle de `head` recalculée. Ni `cmap`, ni `name`, ni `post`, ni
  crénage : un PDF n'en demande pas. Les contours sont recopiés tels quels ; **le hinting ne l'est
  plus** (ticket #228, 20/10/2026) : ni `cvt `, ni `fpgm`, ni `prep`, ni les instructions de chaque
  glyphe (longueur remise à zéro pour un glyphe simple, drapeau `WE_HAVE_INSTRUCTIONS` retiré d'un
  glyphe composé, `maxSizeOfInstructions` à zéro). Elles faisaient 36 % des octets des glyphes de
  Liberation Sans ; elles ajustent les contours à la grille d'un écran, ce que ni l'impression ni
  un lecteur qui lisse les contours ne demandent (inférence, constatée au rendu de poppler : même
  texte, mêmes positions). Les polices CFF sont refusées ;
- le texte s'écrit avec des **codes de deux octets qui sont le point de code Unicode**
  (`Identity-H`, caractères du plan de base) ; `CIDToGIDMap` les traduit vers les glyphes du
  sous-ensemble et la table **`ToUnicode`** rend le code lui-même : le texte extrait est exactement
  le texte imprimé (l'espace insécable reste insécable). `W` donne la largeur de chaque glyphe
  employé ;
- **déterminisme** : glyphes numérotés dans l'ordre des caractères ; l'étiquette `ABCDEF+` du nom
  est tirée d'une empreinte (FNV-1a) du nom PostScript et des caractères, pas du hasard ;
- une police qu'aucun texte n'a employée sort du fichier (ses références sont retirées des
  ressources, ses objets élagués) ;
- un caractère absent de la police ou hors du plan de base est une erreur qui le nomme ;
- **`embarquer`** (ajouté pour `fabrique-pdf --marquer`) : le même sous-ensemble, posé dans un PDF
  **existant** (lopdf) au lieu de remplacer une doublure de printpdf — la police `Type0`, son
  descripteur, le fichier, la table `ToUnicode` et `CIDToGIDMap` sont créés puis comprimés, et
  l'objet de la police est rendu.

## `src/style.rs`

- Taille de police en `f32` (défaut 12.0).

## `src/render.rs`

- **Écriture du texte des polices standard** : encodage Windows-1252 à nous (`encode_win1252`,
  `win1252_byte`), écrit en octets par l'opérateur `Tj`, à la place de `printpdf::write_text` qui
  **abandonnait en silence** les caractères inconnus. Un caractère absent de l'encodage est une
  erreur qui le nomme. Couvre `é à ç ô € œ « » — ’` (vérifié à l'extraction par PyMuPDF).
- `Area::fill_rect` : un rectangle plein (fond des lignes d'un tableau).
- `TextSection::set_word_spacing` (opérateur `Tw`, écrit au premier texte et remis à zéro à la
  fin de la section) pour les polices standard. **Polices en sous-ensemble : `Tw` ne s'applique
  pas aux codes de deux octets** ; la justification est alors écrite en `TJ`, avec après chaque
  espace ordinaire (U+0020) le décalage qui l'étire (`Layer::write_subset_text`) ; l'espace
  insécable ne s'étire pas.
- `TextSection::write_text` : un texte s'écrit en Windows-1252 (police standard) ou avec les
  codes de la police en sous-ensemble.
- `Area::add_raster` et `Area::add_svg` : pose d'une image à une position et une taille données
  (remplacent `Area::add_image` et ses échelle, rotation, résolution).
- **Sortie déterministe** :
  - dates de création, de modification et des métadonnées fixées à `1970-01-01T00:00:00Z` ;
  - les deux identifiants du fichier (`/ID` de la fin du fichier), tirés au hasard par printpdf,
    remplacés par un identifiant fixe ;
  - les tables de hachage de printpdf donnaient, d'une exécution à l'autre, un fichier numéroté
    autrement : les clés de tous les dictionnaires sont triées et les objets renumérotés dans l'ordre
    d'un parcours depuis la racine (`canonicalize`).
- **`Renderer::write` passe par `finish`** : relit le fichier écrit par printpdf (lopdf), **coupe
  les polices en sous-ensemble** (`pdf_subset::embed_subsets`), attache
  les masques de transparence, retire les `BBox` et `SMask` nuls que printpdf met dans chaque image
  (clés invalides là), **n'écrit qu'une fois une image posée sur plusieurs pages** (le logo en haut
  de chaque page : un masque par plan alpha, puis les objets image identiques fusionnés,
  `merge_identical_images`, ticket #228), **comprime les pixels avec les prédicteurs PNG**
  (`predict_images` : par ligne, le filtre PNG le plus court, `DecodeParms` `Predictor 15`, gardé
  seulement s'il fait mieux que le Flate seul ; dépendance `flate2`, déjà là par lopdf), refuse les SVG dont des ressources manquent, fixe les identifiants, élague
  les objets inutiles, retire les flux vides, **comprime** les flux et renumérote. Le même
  traitement s'applique en `debug` et en `release` (printpdf ne comprime qu'en `release`) : le fichier
  ne dépend pas du profil de compilation.

## `src/error.rs`

Non modifié.
