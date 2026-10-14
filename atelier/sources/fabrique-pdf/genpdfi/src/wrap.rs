// Fichier modifié pour PuppetPaie (copie locale de genpdfi 0.2.7, Apache-2.0 OU MIT) :
// voir MODIFICATIONS.md à la racine de cette copie.
//! Mise en lignes d'un paragraphe.
//!
//! Modification PuppetPaie : ce module remplace entièrement le `Wrapper` de genpdfi 0.2.7 (voir
//! MODIFICATIONS.md).  Différences avec l'original :
//!
//! - un mot peut s'étendre sur plusieurs morceaux de styles différents (« FIL TENDU » en gras suivi
//!   de « , » en normal ne se coupe pas entre les deux) ;
//! - les espaces de fin de ligne ne comptent pas dans la largeur (l'original y comptait l'espace
//!   qui suit le dernier mot, ce qui faisait passer un mot à la ligne trop tôt et décalait les
//!   paragraphes alignés à droite, au centre ou justifiés) ;
//! - `\n` force un retour à la ligne ;
//! - un mot plus large que la ligne est coupé lettre à lettre au lieu de faire échouer le rendu ;
//! - la césure (feature `hyphenation`) est supprimée ;
//! - le résultat porte, pour chaque ligne, sa largeur naturelle et son nombre d'espaces, ce qu'il
//!   faut à l'alignement justifié.

use crate::fonts::FontCache;
use crate::style::StyledString;
use crate::Mm;

/// Une ligne de texte mise en page.
#[derive(Clone, Debug)]
pub struct Line {
    /// Les morceaux de la ligne, sans espace final.
    pub runs: Vec<StyledString>,
    /// La largeur naturelle de la ligne (sans espace final).
    pub width: Mm,
    /// Le nombre d'espaces ordinaires (U+0020) dans la ligne, sans l'espace final.
    pub spaces: usize,
    /// Vrai si la ligne est la dernière de son paragraphe ou finit par un `\n` : elle n'est pas
    /// justifiée.
    pub ends_paragraph: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Word,
    Space,
}

#[derive(Clone, Debug)]
struct Token {
    kind: Kind,
    pieces: Vec<StyledString>,
    width: Mm,
}

enum Item {
    Token(Token),
    Newline,
}

fn piece_width(font_cache: &FontCache, s: &StyledString) -> Mm {
    s.style.str_width(font_cache, &s.s)
}

fn push_piece(pieces: &mut Vec<StyledString>, c: char, template: &StyledString) {
    if let Some(last) = pieces.last_mut() {
        if last.style == template.style && last.link == template.link {
            last.s.push(c);
            return;
        }
    }
    pieces.push(StyledString::new(
        c.to_string(),
        template.style,
        template.link.clone(),
    ));
}

/// Découpe les morceaux en mots, espaces et retours à la ligne.
fn tokenize(font_cache: &FontCache, strings: &[StyledString]) -> Vec<Item> {
    let mut items = Vec::new();
    let mut current: Option<(Kind, Vec<StyledString>)> = None;

    fn flush(
        font_cache: &FontCache,
        items: &mut Vec<Item>,
        current: &mut Option<(Kind, Vec<StyledString>)>,
    ) {
        if let Some((kind, pieces)) = current.take() {
            let width = pieces.iter().map(|p| piece_width(font_cache, p)).sum();
            items.push(Item::Token(Token {
                kind,
                pieces,
                width,
            }));
        }
    }

    for s in strings {
        for c in s.s.chars() {
            if c == '\n' {
                flush(font_cache, &mut items, &mut current);
                items.push(Item::Newline);
                continue;
            }
            let kind = if c == ' ' { Kind::Space } else { Kind::Word };
            if current.as_ref().map(|(k, _)| *k) != Some(kind) {
                flush(font_cache, &mut items, &mut current);
                current = Some((kind, Vec::new()));
            }
            push_piece(&mut current.as_mut().unwrap().1, c, s);
        }
    }
    flush(font_cache, &mut items, &mut current);
    items
}

/// Coupe un mot trop large, lettre à lettre, en morceaux qui tiennent dans `width`.
fn split_long_word(font_cache: &FontCache, token: Token, width: Mm) -> Vec<Token> {
    let mut out = Vec::new();
    let mut cur = Token {
        kind: Kind::Word,
        pieces: Vec::new(),
        width: Mm(0.0),
    };
    for piece in &token.pieces {
        for c in piece.s.chars() {
            let cw = piece.style.str_width(font_cache, &c.to_string());
            if cur.width + cw > width && !cur.pieces.is_empty() {
                out.push(std::mem::replace(
                    &mut cur,
                    Token {
                        kind: Kind::Word,
                        pieces: Vec::new(),
                        width: Mm(0.0),
                    },
                ));
            }
            push_piece(&mut cur.pieces, c, piece);
            cur.width += cw;
        }
    }
    if !cur.pieces.is_empty() {
        out.push(cur);
    }
    out
}

const EPS: f32 = 0.001;

struct Builder {
    tokens: Vec<Token>,
    x: Mm,
}

impl Builder {
    fn new() -> Self {
        Builder {
            tokens: Vec::new(),
            x: Mm(0.0),
        }
    }

    fn has_word(&self) -> bool {
        self.tokens.iter().any(|t| t.kind == Kind::Word)
    }

    fn push(&mut self, t: Token) {
        self.x += t.width;
        self.tokens.push(t);
    }

    fn finish(&mut self, ends_paragraph: bool) -> Line {
        while matches!(self.tokens.last(), Some(t) if t.kind == Kind::Space) {
            self.tokens.pop();
        }
        let mut runs: Vec<StyledString> = Vec::new();
        let mut width = Mm(0.0);
        let mut spaces = 0;
        for t in self.tokens.drain(..) {
            width += t.width;
            if t.kind == Kind::Space {
                spaces += t.pieces.iter().map(|p| p.s.chars().count()).sum::<usize>();
            }
            for p in t.pieces {
                match runs.last_mut() {
                    Some(last) if last.style == p.style && last.link == p.link => {
                        last.s.push_str(&p.s)
                    }
                    _ => runs.push(p),
                }
            }
        }
        self.x = Mm(0.0);
        Line {
            runs,
            width,
            spaces,
            ends_paragraph,
        }
    }
}

/// Met les morceaux en lignes de largeur au plus `width`.
///
/// Les styles des morceaux doivent déjà avoir reçu les valeurs par défaut du paragraphe.
pub fn layout(font_cache: &FontCache, strings: &[StyledString], width: Mm) -> Vec<Line> {
    let items = tokenize(font_cache, strings);
    let mut lines = Vec::new();
    let mut b = Builder::new();
    // Vrai si la ligne en cours a commencé par un retour automatique : les espaces de début de
    // ligne sont alors ignorés.
    let mut soft_start = false;

    for item in items {
        match item {
            Item::Newline => {
                lines.push(b.finish(true));
                soft_start = false;
            }
            Item::Token(t) => {
                match t.kind {
                    Kind::Space => {
                        if !(soft_start && b.tokens.is_empty()) {
                            b.push(t);
                        }
                    }
                    Kind::Word => {
                        if b.x + t.width <= width + Mm(EPS) {
                            b.push(t);
                        } else {
                            if b.has_word() {
                                lines.push(b.finish(false));
                                soft_start = true;
                            } else {
                                // Seulement des espaces de début : on les abandonne si le mot
                                // doit être coupé.
                                if t.width > width + Mm(EPS) {
                                    b.tokens.clear();
                                    b.x = Mm(0.0);
                                }
                            }
                            if t.width <= width + Mm(EPS) {
                                b.push(t);
                            } else {
                                let parts = split_long_word(font_cache, t, width);
                                let n = parts.len();
                                for (i, part) in parts.into_iter().enumerate() {
                                    b.push(part);
                                    if i + 1 < n {
                                        lines.push(b.finish(false));
                                        soft_start = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // Une dernière ligne vide après un `\n` final n'est pas émise ; un paragraphe entièrement
    // vide non plus.
    if !b.tokens.is_empty() {
        lines.push(b.finish(true));
    }
    lines
}
