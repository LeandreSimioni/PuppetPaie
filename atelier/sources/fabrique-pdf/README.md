# La fabrique de PDF

Première brique Rust de PuppetPaie (décidée le 30/09/2026) : un programme qui
lit la description d'un document en JSON et écrit le PDF. Le contrat
d'interface est **`FORMAT.md`** ; la décision et les mesures, dans
`../docs/PORTAGE-RUST.md`, « La fabrique de PDF ».

## Construire

```
cargo test --release --locked
cargo build --release --locked      # target/release/fabrique-pdf[.exe]
```

Le moteur ne lance pas ce programme : `paie` lie la fabrique comme
bibliothèque (`paie/Cargo.toml`, `paie/src/fabrique.rs`), dans le même
processus, et sa ligne de commande est **`paie pdf …`** (les mêmes modes que
ci-dessous ; `paie pdf --aide`). Le programme `fabrique-pdf` reste pour
l'atelier.

## Appeler

```
fabrique-pdf sortie.pdf < description.json            # mettre en page
fabrique-pdf --assembler sortie.pdf < pieces.json     # fusionner des pages de PDF existants
fabrique-pdf --pages a.pdf [b.pdf …]                  # [3, 1] : le nombre de pages de chaque PDF
fabrique-pdf --texte a.pdf                            # ["texte page 1", …] : le texte de chaque page
fabrique-pdf --inspecter a.pdf                        # une ligne de JSON : taille des pages, mots et boîtes, images, traits, polices, métadonnées, chiffrement
fabrique-pdf --marquer entree.pdf sortie.pdf "texte"  # un texte en pied de chaque page
fabrique-pdf --verrouiller entree.pdf sortie.pdf      # chiffrement AES-256, ouvert sans mot de passe
fabrique-pdf --version                                # « fabrique-pdf 3 »
```

Les modes de lecture et de retouche (version 3, 01/10/2026) remplacent PyMuPDF
côté Python : le nombre de pages, `page.get_text()`, `_marquer` (`insert_text`
en pied de page) et `verrouiller` (`encryption=PDF_ENCRYPT_AES_256`) ; puis
`--champs` et `--remplir` (le formulaire AER : lecture des champs, remplissage
avec apparence régénérée, signature et cachet posés). Leur contrat
exact — arguments, sortie, codes de sortie, règles — est dans `FORMAT.md`.

Le moteur n'envoie que le chemin du `.modele` et les valeurs : la fabrique lit
le modèle elle-même (`src/modele.rs`, `src/style_contrat.rs`). Il ne reste plus
rien à PyMuPDF ni à PyPDF (01/10/2026) : l'AER passe par `--champs` et
`--remplir`, et l'aperçu d'une compagnie fictive montre le PDF dans le lecteur
du navigateur.

## Le texte d'un PDF, mesuré contre PyMuPDF

`--texte` est écrit sur lopdf (`src/texte.rs`) : un interpréteur du flux de contenu, les
polices, puis la règle de mise en lignes de MuPDF (`FORMAT.md`, « Lire un PDF »). Mesure faite
le 01/10/2026 avec un script jetable (hors du dépôt) qui compare, page par page, le texte de la
fabrique à `page.get_text()` de PyMuPDF 1.28 sur tous les PDF de `prive/`, après réduction de
toute suite de blancs à un espace :

| ensemble | pages | texte identique (blancs réduits) | identique à l'octet |
|---|---|---|---|
| exports de CoolPaie (`export-coolpaie/`) | 255 | **254** (99,6 %) | 248 |
| PDF de PuppetPaie (`sorties/` : contrats, avenants, bulletins, AER, certificats, y compris verrouillés) | 124 | **120** (96,8 %) | 120 |
| PDF fabriqués par la fabrique (`exemples/`) | 2 | 2 | 2 |
| documents publics des organismes (notices, circulaires, guides : 1 519 pages de mise en page libre) | 1 519 | 1 427 (93,9 %) | 951 |

Le nombre de pages est le même sur tous les fichiers lisibles par les deux (dix fichiers
vides de l'export ne s'ouvrent ni avec l'un ni avec l'autre). Les écarts typiques :

- un espace de plus ou de moins près d'un glyphe de symbole (case, puce) : MuPDF avance du
  glyphe du fichier de police, la fabrique de `/Widths` ; sans aucun blanc, 1 835 pages sur
  1 898 hors exports sont identiques ;
- le nombre de points d'un pointillé en bord de page ;
- des polices en petites capitales (MuPDF lit la capitale dans le nom du glyphe), et les flèches
  de Wingdings (zone privée, que MuPDF traduit) ;
- une page des formulaires d'impôts où MuPDF écarte un « 4 » au bord de la page.

Ce qui sert au Python — retrouver numéros, période, brut et net d'un bulletin
(`bulletin.numeros_archives`), rattacher une page CoolPaie à un salarié et une date
(`_categorie_page`, `_debut_de_document`, `_date_page`, les 13 chiffres d'un NIR, les mots
d'un nom), dédoublonner par empreinte du texte (`_cle_page`), comparer des textes de contrat
(`avenant._normaliser`) — a été vérifié en appliquant ces fonctions aux deux textes : mêmes
résultats sur les 139 pages CoolPaie du dossier (la seule page qui diffère est celle du
formulaire d'impôt ci-dessus), mêmes numéros sur les bulletins archivés, même partition de ces
139 pages en classes de doublons (124 classes de part et d'autre), même texte normalisé sur
46 des 50 PDF de `sorties/` (les 4 autres sont les AER ci-dessus).

Robustesse : 3 600 lancements des quatre modes sur des PDF abîmés (octets changés, coupés,
effacés, insérés) n'ont donné ni plantage ni boucle (codes 0 ou 1 seulement) ; un `>` isolé
dans une table `ToUnicode` bouclait, c'est corrigé (`tests/lecture.rs`).

## Poids

Programme : **4,4 Mo** (Linux, `opt-level = "z"`, LTO, `strip` : 4 401 600 octets) ; il
était de 4,1 Mo (4 060 616) avant les modes de lecture et de retouche (+0,34 Mo : la lecture
du texte, `aes`, `cbc`, `sha2` et `getrandom` pour `--verrouiller`) et de
3,6 Mo avant l'étape 8. Les huit fichiers de Liberation, comprimés, pèsent
1,4 Mo (romain, gras, italique et gras italique de Sans ; romain et gras de
Serif et de Mono), contre 1,0 Mo pour les cinq d'avant : `build.rs` en retire
crénage, substitutions et noms de glyphes, que rien ne lit. Le découpage des
polices est du code à nous (`genpdfi/src/pdf_subset.rs`) : la crate
`subsetter` et ses 330 Ko de dépendances sont retirées. Un contrat de la
démonstration : 32 Ko par page, dont 21 Ko de polices (les glyphes imprimés,
hinting compris), contre 6 Ko avant l'étape 8 et 13 Ko par PyMuPDF.

## Licences

- `genpdfi/` : copie modifiée de genpdfi 0.2.7 (Apache-2.0 ou MIT), textes
  dans `genpdfi/LICENSES/`, chaque modification dans
  `genpdfi/MODIFICATIONS.md`.
- `polices/` : Liberation Sans (SIL OFL, `LICENCE-Liberation-OFL.txt`) ;
  Liberation Serif et Mono (Times, Courier) pour les blocs à position fixe ;
  la licence permet de les embarquer : le PDF en porte un sous-ensemble (les
  glyphes imprimés), et leurs métriques mesurent le texte.
- Le chiffrement de `--verrouiller` (`src/chiffrement.rs`) s'appuie sur les crates `aes`, `cbc`,
  `cipher` et `sha2` de RustCrypto et sur `getrandom` (tirage au hasard du système) : toutes
  sous MIT ou Apache-2.0, comme lopdf et ttf-parser.
