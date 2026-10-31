# La fabrique de PDF

Première brique Rust de PuppetPaie (décidée le 30/09/2026) : un programme qui
lit la description d'un document en JSON et écrit le PDF. Le contrat
d'interface est **`FORMAT.md`**.

## Construire

```
cargo test --release --locked
cargo build --release --locked      # target/release/fabrique-pdf[.exe]
```

Le moteur ne lance pas ce programme : `moteur` lie la fabrique comme
bibliothèque (`moteur/Cargo.toml`, `documents/src/fabrique.rs`), dans le même
processus, et sa ligne de commande est **`moteur pdf …`** (les mêmes modes que
ci-dessous ; `moteur pdf --aide`). Le programme `fabrique-pdf` reste pour
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

Les modes de lecture et de retouche : le nombre de pages, le texte d'une page, le pied de page (`--marquer`), le verrouillage
(AES-256), puis `--champs` et `--remplir` (le formulaire AER : lecture des champs, remplissage
avec apparence régénérée, signature et cachet posés). Leur contrat
exact — arguments, sortie, codes de sortie, règles — est dans `FORMAT.md`.

Le moteur n'envoie que le chemin du `.modele` et les valeurs : la fabrique lit
le modèle elle-même (`src/modele.rs`, `src/style_contrat.rs`). L'AER passe par `--champs` et
`--remplir`, et l'aperçu d'une compagnie fictive montre le PDF dans le lecteur
du navigateur.

## Le texte d'un PDF

`--texte` est écrit sur lopdf (`src/texte.rs`) : un interpréteur du flux de contenu, les
polices, puis la règle de mise en lignes de MuPDF (`FORMAT.md`, « Lire un PDF »). Le texte
rendu est celui de `page.get_text()` de PyMuPDF, à ces écarts près :

- un espace de plus ou de moins près d'un glyphe de symbole (case, puce) : MuPDF avance du
  glyphe du fichier de police, la fabrique de `/Widths` ;
- le nombre de points d'un pointillé en bord de page ;
- des polices en petites capitales (MuPDF lit la capitale dans le nom du glyphe), et les flèches
  de Wingdings (zone privée, que MuPDF traduit) ;
- une page des formulaires d'impôts où MuPDF écarte un « 4 » au bord de la page.

Un fichier abîmé (octets effacés, insérés) ne fait ni planter ni boucler la lecture (codes 0 ou
1 seulement) ; une table `ToUnicode` qui porte un `>` isolé est lue sans boucle (`tests/lecture.rs`).

## Poids

Programme : **4,4 Mo** (Linux, `opt-level = "z"`, LTO, `strip` : 4 401 600 octets), dont la lecture du
texte, `aes`, `cbc`, `sha2` et `getrandom` pour `--verrouiller`. Les huit fichiers de Liberation, comprimés,
pèsent 1,4 Mo (romain, gras, italique et gras italique de Sans ; romain et gras de Serif et de Mono) :
`build.rs` en retire crénage, substitutions et noms de glyphes, que rien ne lit. Le découpage des
polices est du code à nous (`genpdfi/src/pdf_subset.rs`), sans dépendance. Un contrat de la
démonstration : 32 Ko par page, dont 21 Ko de polices (les glyphes imprimés, hinting compris).

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
