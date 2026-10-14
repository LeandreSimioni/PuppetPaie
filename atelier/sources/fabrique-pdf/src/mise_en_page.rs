//! La mise en page : de la description validée aux éléments de genpdfi.

use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;

use genpdfi::error::Error as ErreurGenpdfi;
use genpdfi::elements::{PageBreak, Paragraph, Picture};
use genpdfi::fonts::{Font, FontFamily};
use genpdfi::render::Area;
use genpdfi::style::{Color, LineStyle, Style, StyledString};
use genpdfi::{
    pt, Alignment, Context, Document, Element, Margins, PageDecorator,
    Position, RenderResult, Size,
};

use crate::description::{
    lire_description, refus, Alignement, Bloc, BlocEtiquete, CelluleTableau, Description, Filet,
    LigneTableau, Morceau, Police, PoliceFixe, Refus, Repere, Rvb, CORPS_PLANCHER,
};
use crate::polices;

/// Un peu de marge pour les comparaisons de hauteurs en millimètres (erreurs d'arrondi).
const EPS_MM: f32 = 0.01;

type Mm = genpdfi::Mm;

fn en_points(mm: Mm) -> f32 {
    genpdfi::printpdf::Pt::from(mm).0
}

fn couleur_genpdfi((r, v, b): Rvb) -> Color {
    Color::Rgb(r, v, b)
}

// ---------------------------------------------------------------------------
// Les éléments.
// ---------------------------------------------------------------------------

/// Un bloc prêt à être posé.
pub enum Noeud {
    Paragraphe {
        avant: Mm,
        apres: Mm,
        para: Paragraph,
        /// La marge à gauche : le texte se pose dans une largeur d'autant réduite.
        retrait: Mm,
        commence: bool,
    },
    Espace {
        hauteur: Mm,
        fait: bool,
    },
    Image(Picture),
    /// La case de la mention de signature : posée là où l'on est, sans hauteur.
    Mention(Mention),
    /// Une image posée là où l'on est, qui n'occupe aucune hauteur : le texte qui suit passe
    /// dessous, et elle peut descendre dans la marge du bas (FORMAT.md, `flottante`).
    ImageFlottante(Picture),
    Colonnes(Colonnes),
    Tableau(Tableau),
    /// Un bloc à position absolue sur la page (FORMAT.md) : il ne consomme aucune hauteur.
    Fixe(Fixe),
}

/// Une case encadrée de quelques lignes de texte (`Bloc::Mention`) ; elle recouvre ce qui est
/// dessous sans rien pousser, comme l'image flottante.
pub struct Mention {
    lignes: Vec<String>,
    style: Style,
    filet: Color,
    largeur: Mm,
    hauteur: Mm,
    marge: (Mm, Mm),
}

/// Un texte, un trait ou une image posés à des coordonnées de la page (points → millimètres), et
/// non à l'endroit courant. Les coordonnées partent du coin haut-gauche de la PAGE : `origine` est
/// la marge (gauche, haut) à retrancher, car la zone qu'on nous donne commence dans la marge.
pub struct Fixe {
    origine: (Mm, Mm),
    page: Size,
    objet: ObjetFixe,
}

pub enum ObjetFixe {
    Texte {
        texte: String,
        x: Mm,
        /// La ligne de base.
        y: Mm,
        style: Style,
        alignement: Alignement,
    },
    Trait {
        de: (Mm, Mm),
        a: (Mm, Mm),
        epaisseur: Mm,
        couleur: Color,
    },
    Image {
        image: Picture,
        x: Mm,
        y: Mm,
    },
}

impl Fixe {
    fn poser(&self, ctx: &Context, area: &Area<'_>) -> Result<(), ErreurGenpdfi> {
        // La zone de la page entière, à l'origine de la page.
        let mut zone = area.clone();
        let zero = Mm::from(0.0f32);
        zone.add_offset(Position::new(zero - self.origine.0, zero - self.origine.1));
        zone.set_size(self.page);
        match &self.objet {
            ObjetFixe::Texte { texte, x, y, style, alignement } => {
                if texte.is_empty() {
                    return Ok(());
                }
                let largeur = style.text_width(&ctx.font_cache, texte);
                let x = match alignement {
                    Alignement::Droite => *x - largeur,
                    Alignement::Centre => *x - largeur / 2.0,
                    _ => *x,
                };
                // La section de texte pose la ligne de base à `position.y + ascent`.
                let metrics = style.metrics(&ctx.font_cache);
                zone.print_str(&ctx.font_cache, Position::new(x, *y - metrics.ascent), *style, texte)?;
            }
            ObjetFixe::Trait { de, a, epaisseur, couleur } => {
                zone.draw_line(
                    vec![Position::new(de.0, de.1), Position::new(a.0, a.1)],
                    LineStyle::new().with_thickness(*epaisseur).with_color(*couleur),
                );
            }
            ObjetFixe::Image { image, x, y } => image.draw_at(&zone, Position::new(*x, *y)),
        }
        Ok(())
    }
}

/// Un tableau prêt à être posé : les hauteurs de toutes ses lignes sont calculées une fois, dans la
/// largeur où il sera posé. Il se coupe entre les lignes (jamais dans une ligne), et son en-tête se
/// redessine en haut de chaque page où il continue.
pub struct Tableau {
    avant: Mm,
    apres: Mm,
    largeur: Mm,
    /// Abscisse et largeur de chaque colonne.
    colonnes: Vec<(Mm, Mm)>,
    interligne: f32,
    /// haut, droite, bas, gauche.
    marge: [Mm; 4],
    filet_lignes: Option<Filet>,
    filets_cellules: Option<Filet>,
    entete: Vec<LigneTableau>,
    lignes: Vec<LigneTableau>,
    h_entete: Vec<Mm>,
    h_lignes: Vec<Mm>,
    /// La prochaine ligne du corps à poser.
    suivante: usize,
    commence: bool,
}

pub struct Colonnes {
    filet: Option<(Color, Mm)>,
    avant: Mm,
    marge_haut: Mm,
    colonnes: Vec<(f32, Vec<Noeud>)>,
    fait: bool,
}

impl Noeud {
    /// La hauteur que le bloc occupe dans une largeur donnée (sans rien dessiner).
    pub fn hauteur(&self, ctx: &Context, largeur: Mm, style: Style) -> Mm {
        match self {
            Noeud::Paragraphe {
                avant, apres, para, retrait, ..
            } => *avant + para.height(ctx, largeur - *retrait, style) + *apres,
            Noeud::Espace { hauteur, .. } => *hauteur,
            Noeud::Image(p) => p.height(),
            Noeud::ImageFlottante(_) | Noeud::Fixe(_) | Noeud::Mention(_) => Mm::from(0.0f32),
            Noeud::Colonnes(c) => c.hauteur(ctx, largeur, style),
            // Le tableau entier, comme s'il tenait sur une page : pour qui l'additionne.
            Noeud::Tableau(t) => {
                t.avant
                    + t.h_entete.iter().copied().sum::<Mm>()
                    + t.h_lignes.iter().copied().sum::<Mm>()
                    + t.apres
            }
        }
    }
}

impl Colonnes {
    fn epaisseur_filet(&self) -> Mm {
        self.filet.map(|(_, e)| e).unwrap_or(Mm::from(0.0f32))
    }

    fn hauteur_contenu(&self, ctx: &Context, largeur: Mm, style: Style) -> Mm {
        self.colonnes
            .iter()
            .map(|(frac, blocs)| {
                let l = largeur * *frac;
                blocs.iter().map(|b| b.hauteur(ctx, l, style)).sum::<Mm>()
            })
            .fold(Mm::from(0.0f32), |a, b| a.max(b))
    }

    fn hauteur(&self, ctx: &Context, largeur: Mm, style: Style) -> Mm {
        self.avant
            + self.epaisseur_filet()
            + self.marge_haut
            + self.hauteur_contenu(ctx, largeur, style)
    }
}

impl Element for Noeud {
    fn render(
        &mut self,
        ctx: &Context,
        area: Area<'_>,
        style: Style,
    ) -> Result<RenderResult, ErreurGenpdfi> {
        match self {
            Noeud::Image(p) => p.render(ctx, area, style),
            Noeud::Fixe(f) => {
                f.poser(ctx, &area)?;
                Ok(RenderResult::default())
            }
            Noeud::Mention(m) => {
                let mut zone = area.clone();
                zone.set_height(m.hauteur);
                zone.set_width(m.largeur);
                let (l, h) = (m.largeur, m.hauteur);
                zone.draw_line(
                    vec![Position::new(0, 0), Position::new(l, 0), Position::new(l, h), Position::new(0, h), Position::new(0, 0)],
                    LineStyle::new().with_thickness(pt(0.5)).with_color(m.filet),
                );
                let hauteur_ligne = m.style.line_height(&ctx.font_cache);
                let mut y = m.marge.1;
                for ligne in &m.lignes {
                    zone.print_str(&ctx.font_cache, Position::new(m.marge.0, y), m.style, ligne)?;
                    y += hauteur_ligne;
                }
                Ok(RenderResult::default())
            }
            Noeud::ImageFlottante(p) => {
                // La zone s'agrandit à la taille de l'image : elle se pose même au ras du bas.
                let mut zone = area.clone();
                zone.set_height(p.height());
                p.render(ctx, zone, style)?;
                Ok(RenderResult::default())
            }
            Noeud::Espace { hauteur, fait } => {
                if *fait {
                    return Ok(RenderResult::default());
                }
                *fait = true;
                let mut r = RenderResult::default();
                // Un espace qui déborde de la page est absorbé par le saut de page.
                r.size.height = if *hauteur > area.size().height {
                    area.size().height
                } else {
                    *hauteur
                };
                Ok(r)
            }
            Noeud::Paragraphe {
                avant,
                apres,
                para,
                retrait,
                commence,
            } => {
                let premier = !*commence;
                let decalage = if premier { *avant } else { Mm::from(0.0f32) };
                if decalage > area.size().height {
                    return Ok(RenderResult {
                        size: Size::new(0, 0),
                        has_more: true,
                    });
                }
                let mut sous = area.clone();
                sous.add_offset(Position::new(*retrait, decalage));
                sous.set_width(area.size().width - *retrait);
                let r = para.render(ctx, sous, style)?;
                if r.has_more && r.size.height == Mm::from(0.0f32) {
                    // Pas même une ligne : tout à la page suivante, espace avant compris.
                    return Ok(RenderResult {
                        size: Size::new(0, 0),
                        has_more: true,
                    });
                }
                *commence = true;
                let mut hauteur = decalage + r.size.height;
                if !r.has_more {
                    let reste = area.size().height - hauteur;
                    hauteur += if *apres > reste { reste.max(Mm::from(0.0f32)) } else { *apres };
                }
                Ok(RenderResult {
                    size: Size::new(r.size.width, hauteur),
                    has_more: r.has_more,
                })
            }
            Noeud::Colonnes(c) => c.render(ctx, area, style),
            Noeud::Tableau(t) => t.render(ctx, area, style),
        }
    }
}

impl Colonnes {
    fn render(
        &mut self,
        ctx: &Context,
        area: Area<'_>,
        style: Style,
    ) -> Result<RenderResult, ErreurGenpdfi> {
        if self.fait {
            return Ok(RenderResult::default());
        }
        let largeur = area.size().width;
        let besoin = self.hauteur(ctx, largeur, style);
        if besoin > area.size().height + Mm::from(EPS_MM) {
            // Les colonnes ne se coupent pas : tout à la page suivante.
            return Ok(RenderResult {
                size: Size::new(0, 0),
                has_more: true,
            });
        }
        let epaisseur = self.epaisseur_filet();
        let contenu = self.hauteur_contenu(ctx, largeur, style);
        let mut x = Mm::from(0.0f32);
        for (frac, blocs) in self.colonnes.iter_mut() {
            let lc = largeur * *frac;
            let mut zone = area.clone();
            zone.add_offset(Position::new(x, self.avant));
            zone.set_width(lc);
            if let Some((couleur, e)) = self.filet {
                zone.draw_line(
                    vec![
                        Position::new(0, e / 2.0),
                        Position::new(lc, e / 2.0),
                    ],
                    LineStyle::new().with_thickness(e).with_color(couleur),
                );
            }
            let mut dedans = zone.clone();
            dedans.add_offset(Position::new(0, epaisseur + self.marge_haut));
            dedans.set_height(contenu + Mm::from(EPS_MM));
            for b in blocs.iter_mut() {
                let r = b.render(ctx, dedans.clone(), style)?;
                dedans.add_offset(Position::new(0, r.size.height));
            }
            x += lc;
        }
        self.fait = true;
        Ok(RenderResult {
            size: Size::new(largeur, besoin),
            has_more: false,
        })
    }
}

fn aligner(a: Alignement) -> Alignment {
    match a {
        Alignement::Gauche => Alignment::Left,
        Alignement::Justifie => Alignment::Justified,
        Alignement::Centre => Alignment::Center,
        Alignement::Droite => Alignment::Right,
    }
}

impl Tableau {
    /// Le texte d'une cellule, à refaire à chaque pose (un paragraphe se consomme en s'imprimant,
    /// et l'en-tête se pose sur chaque page).
    fn paragraphe(&self, c: &CelluleTableau, taille: f32, defaut: Rvb) -> Paragraph {
        let mut para = Paragraph::default()
            .aligned(aligner(c.alignement))
            .with_line_height_factor(self.interligne);
        for m in &c.morceaux {
            let base = Style::new()
                .with_font_size(taille)
                .with_color(couleur_genpdfi(m.couleur.unwrap_or(defaut)));
            let st = if m.gras { base.bold() } else { base };
            let st = if m.italique { st.italic() } else { st };
            let texte = m.texte.replace("\r\n", "\n").replace('\r', "\n");
            para.push(StyledString::new(texte, st, None));
        }
        para
    }

    fn vide(c: &CelluleTableau) -> bool {
        c.morceaux.iter().all(|m| m.texte.is_empty())
    }

    /// Abscisse et largeur d'une cellule qui commence à la colonne `debut`.
    fn emprise(&self, debut: usize, etendue: usize) -> (Mm, Mm) {
        let x = self.colonnes[debut].0;
        let l: Mm = self.colonnes[debut..debut + etendue].iter().map(|c| c.1).sum();
        (x, l)
    }

    fn largeur_texte(&self, l: Mm) -> Mm {
        l - self.marge[1] - self.marge[3]
    }

    /// La hauteur d'une ligne : celle de sa cellule la plus haute, marges comprises.
    fn hauteur_ligne(&self, ctx: &Context, l: &LigneTableau, style: Style) -> Mm {
        let mut debut = 0;
        let mut h = Mm::from(0.0f32);
        for c in &l.cellules {
            let (_, lc) = self.emprise(debut, c.etendue);
            debut += c.etendue;
            if Self::vide(c) {
                continue;
            }
            let p = self.paragraphe(c, l.taille, (0, 0, 0));
            h = h.max(p.height(ctx, self.largeur_texte(lc), style));
        }
        h + self.marge[0] + self.marge[2]
    }

    /// Fond, texte puis traits d'une ligne dont le haut est à `y` : dans cet ordre, pour que le
    /// fond de la ligne suivante ne recouvre pas le filet de celle-ci.
    fn poser_lignes(
        &self,
        ctx: &Context,
        area: &Area<'_>,
        style: Style,
        lignes: &[(&LigneTableau, Mm, Mm, bool)],
    ) -> Result<(), ErreurGenpdfi> {
        for (l, y, h, _) in lignes {
            if let Some(f) = l.fond {
                area.fill_rect(Position::new(0, *y), Size::new(self.largeur, *h), couleur_genpdfi(f));
            }
        }
        for (l, y, h, _) in lignes {
            let mut debut = 0;
            for c in &l.cellules {
                let (x, lc) = self.emprise(debut, c.etendue);
                debut += c.etendue;
                if Self::vide(c) {
                    continue;
                }
                let mut zone = area.clone();
                zone.add_offset(Position::new(x + self.marge[3], *y + self.marge[0]));
                zone.set_width(self.largeur_texte(lc));
                zone.set_height(*h - self.marge[0] + Mm::from(0.05f32));
                let mut p = self.paragraphe(c, l.taille, (0, 0, 0));
                p.render(ctx, zone, style)?;
            }
        }
        for (l, y, h, corps) in lignes {
            if let Some((couleur, e)) = self.filets_cellules {
                let (e, couleur) = (pt(e), couleur_genpdfi(couleur));
                let mut debut = 0;
                for c in &l.cellules {
                    let (x, lc) = self.emprise(debut, c.etendue);
                    debut += c.etendue;
                    area.draw_line(
                        vec![
                            Position::new(x, *y),
                            Position::new(x + lc, *y),
                            Position::new(x + lc, *y + *h),
                            Position::new(x, *y + *h),
                            Position::new(x, *y),
                        ],
                        LineStyle::new().with_thickness(e).with_color(couleur),
                    );
                }
            }
            let haut: Option<Filet> = l.filet_haut;
            let bas: Option<Filet> = l.filet_bas.or(if *corps { self.filet_lignes } else { None });
            if let Some((c, e)) = haut {
                let (e, c) = (pt(e), couleur_genpdfi(c));
                area.draw_line(
                    vec![Position::new(0, *y + e / 2.0), Position::new(self.largeur, *y + e / 2.0)],
                    LineStyle::new().with_thickness(e).with_color(c),
                );
            }
            if let Some((c, e)) = bas {
                let (e, c) = (pt(e), couleur_genpdfi(c));
                area.draw_line(
                    vec![
                        Position::new(0, *y + *h - e / 2.0),
                        Position::new(self.largeur, *y + *h - e / 2.0),
                    ],
                    LineStyle::new().with_thickness(e).with_color(c),
                );
            }
        }
        Ok(())
    }

    fn render(
        &mut self,
        ctx: &Context,
        area: Area<'_>,
        style: Style,
    ) -> Result<RenderResult, ErreurGenpdfi> {
        let rien = || RenderResult { size: Size::new(0, 0), has_more: true };
        let premier = !self.commence;
        let decalage = if premier { self.avant } else { Mm::from(0.0f32) };
        let dispo = area.size().height - decalage + Mm::from(EPS_MM);
        let h_entete: Mm = self.h_entete.iter().copied().sum();
        // Les lignes du corps qui tiennent sous l'en-tête ; sans une seule (et sans corps du
        // tout, l'en-tête seul), tout passe à la page suivante.
        let mut fin = self.suivante;
        let mut bas = h_entete;
        while fin < self.lignes.len() && bas + self.h_lignes[fin] <= dispo {
            bas += self.h_lignes[fin];
            fin += 1;
        }
        let rien_tient = if self.lignes.is_empty() { h_entete > dispo } else { fin == self.suivante };
        if rien_tient {
            return Ok(rien());
        }
        let mut a_poser: Vec<(&LigneTableau, Mm, Mm, bool)> = Vec::new();
        let mut y = decalage;
        for (l, h) in self.entete.iter().zip(&self.h_entete) {
            a_poser.push((l, y, *h, false));
            y += *h;
        }
        for k in self.suivante..fin {
            a_poser.push((&self.lignes[k], y, self.h_lignes[k], true));
            y += self.h_lignes[k];
        }
        self.poser_lignes(ctx, &area, style, &a_poser)?;
        self.suivante = fin;
        self.commence = true;
        let a_suivre = fin < self.lignes.len();
        let mut hauteur = y;
        if !a_suivre {
            let reste = area.size().height - hauteur;
            hauteur += if self.apres > reste { reste.max(Mm::from(0.0f32)) } else { self.apres };
        }
        Ok(RenderResult { size: Size::new(self.largeur, hauteur), has_more: a_suivre })
    }

}

/// Un en-tête ou un pied de page prêt à être posé : le style est choisi une fois.
struct ReperePose {
    repere: Repere,
    style: Style,
    /// Le pied se mesure depuis le bas de la page, l'en-tête depuis le haut.
    en_bas: bool,
}

/// Pose le logo en haut à droite de chaque page, l'en-tête et le pied répétés (FORMAT.md), puis
/// les marges.
struct Decorateur {
    marges: Margins,
    /// Les marges gauche et droite, d'où partent les textes de l'en-tête et du pied.
    gauche: Mm,
    droite: Mm,
    logo: Option<(Picture, Mm, Mm)>,
    reperes: Vec<ReperePose>,
    /// Le total de pages, quand on le connaît (deuxième passe) ; 0 sinon.
    total: usize,
    /// Le numéro de la page en cours, et, à la fin, le nombre de pages écrites.
    pages: Rc<Cell<usize>>,
}

impl PageDecorator for Decorateur {
    fn decorate_page<'a>(
        &mut self,
        context: &Context,
        mut area: Area<'a>,
        _style: Style,
    ) -> Result<Area<'a>, ErreurGenpdfi> {
        self.pages.set(self.pages.get() + 1);
        let n = self.pages.get();
        if let Some((logo, haut, droite)) = &self.logo {
            let x = area.size().width - *droite - logo.width();
            logo.draw_at(&area, Position::new(x, *haut));
        }
        for r in &self.reperes {
            if n == 1 && !r.repere.premiere_page {
                continue;
            }
            let hauteur = area.size().height;
            let metrics = r.style.metrics(&context.font_cache);
            let base = if r.en_bas { hauteur - pt(r.repere.distance) } else { pt(r.repere.distance) };
            let y = base - metrics.ascent;
            let (gauche, droite) = (self.gauche, area.size().width - self.droite);
            let remplacer = |t: &str| {
                t.replace("{page}", &n.to_string()).replace("{pages}", &self.total.to_string())
            };
            for (texte, ou) in [
                (&r.repere.gauche, 0),
                (&r.repere.centre, 1),
                (&r.repere.droite, 2),
            ] {
                let texte = remplacer(texte);
                if texte.is_empty() {
                    continue;
                }
                let largeur = r.style.text_width(&context.font_cache, &texte);
                let x = match ou {
                    0 => gauche,
                    1 => (gauche + droite) / 2.0 - largeur / 2.0,
                    _ => droite - largeur,
                };
                area.print_str(&context.font_cache, Position::new(x, y), r.style, texte)?;
            }
        }
        area.add_margins(self.marges);
        Ok(area)
    }
}

// ---------------------------------------------------------------------------
// La construction.
// ---------------------------------------------------------------------------

fn charger_image(
    chemin: &str,
    largeur_pt: f32,
    ou: &str,
    quoi: &str,
) -> Result<Picture, Refus> {
    let donnees = match std::fs::read(chemin) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return refus(format!("{} : {} introuvable : {}", ou, quoi, chemin))
        }
        Err(e) => return refus(format!("{} : {} illisible ({}) : {}", ou, quoi, e, chemin)),
    };
    let svg = Path::new(chemin)
        .extension()
        .map(|e| e.eq_ignore_ascii_case("svg"))
        .unwrap_or(false);
    Picture::from_bytes(&donnees, pt(largeur_pt), svg)
        .map_err(|e| Refus(format!("{} : {} {} : {}", ou, quoi, chemin, e)))
}

/// Refuse un texte dont un caractère manque à la police elle-même (un idéogramme, un emoji) :
/// tout ce que Liberation possède — latin étendu, grec, cyrillique — s'imprime.
fn verifier_texte(ctx: &Context, style: Style, texte: &str, ou: &str) -> Result<(), Refus> {
    let police = ctx.font_cache.get_rt_font(style.font(&ctx.font_cache));
    for c in texte.chars() {
        if c != '\n' && (c as u32 > 0xFFFF || police.glyph(c).id().0 == 0) {
            return refus(format!(
                "{} : le caractère « {} » (U+{:04X}) n'existe pas dans la police Liberation",
                ou, c, c as u32
            ));
        }
    }
    Ok(())
}

/// Le style d'un morceau de texte, pour la vérification de ses caractères.
fn style_du_morceau(m: &Morceau) -> Style {
    let st = Style::new();
    let st = if m.gras { st.bold() } else { st };
    if m.italique { st.italic() } else { st }
}

/// Ce que les blocs à position fixe demandent à la page : d'où partent leurs coordonnées, et les
/// familles Times et Courier (ajoutées au document seulement si un texte les emploie).
pub struct Cadre {
    origine: (Mm, Mm),
    page: Size,
    times: Option<FontFamily<Font>>,
    courier: Option<FontFamily<Font>>,
}

fn construire(
    b: &BlocEtiquete,
    ctx: &Context,
    largeur: Mm,
    police: &Police,
    hauteur_page: Mm,
    style: Style,
    cadre: &Cadre,
) -> Result<Noeud, Refus> {
    let ou = b.etiquette.as_str();
    let noeud = match &b.bloc {
        Bloc::Paragraphe {
            morceaux,
            taille,
            couleur,
            alignement,
            espace_avant,
            espace_apres,
            retrait,
        } => {
            let base = Style::new()
                .with_font_size(*taille)
                .with_color(couleur_genpdfi(*couleur));
            let mut para = Paragraph::default()
                .aligned(aligner(*alignement))
                .with_line_height_factor(police.interligne);
            for m in morceaux {
                let texte = m.texte.replace("\r\n", "\n").replace('\r', "\n");
                verifier_texte(ctx, style_du_morceau(m), &texte, ou)?;
                let base = match m.couleur {
                    Some(c) => base.with_color(couleur_genpdfi(c)),
                    None => base,
                };
                let st = if m.gras { base.bold() } else { base };
                let st = if m.italique { st.italic() } else { st };
                para.push(StyledString::new(texte, st, None));
            }
            Noeud::Paragraphe {
                avant: pt(*espace_avant),
                apres: pt(*espace_apres),
                para,
                retrait: pt(*retrait),
                commence: false,
            }
        }
        Bloc::Mention { lignes, taille, interligne, couleur, filet, hauteur, marge } => {
            let st = Style::new()
                .with_font_size(*taille)
                .with_line_spacing(*interligne)
                .with_color(couleur_genpdfi(*couleur));
            for l in lignes {
                verifier_texte(ctx, st, l, ou)?;
                if st.text_width(&ctx.font_cache, l) + pt(2.0 * marge.0) > largeur {
                    return refus(format!("{} : la mention de signature ne tient pas dans la case (la ligne « {} » est trop longue)", ou, l));
                }
            }
            let besoin = st.line_height(&ctx.font_cache) * lignes.len() as f32 + pt(marge.1);
            if besoin > pt(*hauteur) + Mm::from(EPS_MM) {
                return refus(format!("{} : la mention de signature ne tient pas dans la case ({} lignes)", ou, lignes.len()));
            }
            Noeud::Mention(Mention {
                lignes: lignes.clone(),
                style: st,
                filet: couleur_genpdfi(*filet),
                largeur,
                hauteur: pt(*hauteur),
                marge: (pt(marge.0), pt(marge.1)),
            })
        }
        Bloc::Espace(h) => Noeud::Espace {
            hauteur: pt(*h),
            fait: false,
        },
        Bloc::Image {
            chemin,
            largeur: l,
            flottante,
        } => {
            let p = charger_image(chemin, *l, ou, "image")?;
            if p.width() > largeur + Mm::from(EPS_MM) {
                return refus(format!(
                    "{} : l'image ({} pt de large) est plus large que la place disponible ({:.1} pt)",
                    ou,
                    l,
                    en_points(largeur)
                ));
            }
            if *flottante {
                Noeud::ImageFlottante(p)
            } else {
                Noeud::Image(p)
            }
        }
        Bloc::TexteFixe {
            texte,
            x,
            y,
            taille,
            police: nom,
            gras,
            couleur,
            alignement,
            largeur_max,
        } => {
            let texte = texte.replace("\r\n", "\n").replace('\r', "\n");
            if texte.contains('\n') {
                return refus(format!("{} : un texte à position fixe tient sur une ligne (pas de \\n)", ou));
            }
            let base = Style::new().with_font_size(*taille).with_color(couleur_genpdfi(*couleur));
            let base = match nom {
                PoliceFixe::Helvetica => base,
                PoliceFixe::Times => base.with_font_family(cadre.times.expect("famille Times")),
                PoliceFixe::Courier => base.with_font_family(cadre.courier.expect("famille Courier")),
            };
            verifier_texte(ctx, if *gras { base.bold() } else { base }, &texte, ou)?;
            // `largeur_max` : le corps demandé tant que le texte y tient, sinon réduit au prorata
            // (corps × place ÷ largeur), sans descendre sous le plancher — l'algorithme de
            // `corps_qui_tient` de moteur/conges_spectacles.py, mesuré sur les métriques de la
            // police embarquée.
            let base = match largeur_max {
                Some(place) => {
                    let largeur = en_points(if *gras { base.bold() } else { base }.text_width(&ctx.font_cache, &texte));
                    if largeur <= *place {
                        base
                    } else {
                        base.with_font_size((*taille * *place / largeur).max(CORPS_PLANCHER))
                    }
                }
                None => base,
            };
            Noeud::Fixe(Fixe {
                origine: cadre.origine,
                page: cadre.page,
                objet: ObjetFixe::Texte {
                    texte,
                    x: pt(*x),
                    y: pt(*y),
                    style: if *gras { base.bold() } else { base },
                    alignement: *alignement,
                },
            })
        }
        Bloc::Trait { x1, y1, x2, y2, epaisseur, couleur } => Noeud::Fixe(Fixe {
            origine: cadre.origine,
            page: cadre.page,
            objet: ObjetFixe::Trait {
                de: (pt(*x1), pt(*y1)),
                a: (pt(*x2), pt(*y2)),
                epaisseur: pt(*epaisseur),
                couleur: couleur_genpdfi(*couleur),
            },
        }),
        Bloc::ImageFixe { chemin, x, y, largeur: l, hauteur: h, centree } => {
            // L'image tient dans la boîte (l × h), proportions gardées : d'abord à la pleine
            // largeur, puis, si elle déborde en hauteur, à la largeur qui remplit la hauteur.
            let mut p = charger_image(chemin, *l, ou, "image")?;
            let mut largeur_image = *l;
            if p.height() > pt(*h) + Mm::from(EPS_MM) {
                largeur_image = *l * *h / en_points(p.height());
                p = charger_image(chemin, largeur_image, ou, "image")?;
            }
            let (dx, dy) = if *centree {
                (
                    (*l - largeur_image) / 2.0,
                    (*h - en_points(p.height())) / 2.0,
                )
            } else {
                (0.0, 0.0)
            };
            Noeud::Fixe(Fixe {
                origine: cadre.origine,
                page: cadre.page,
                objet: ObjetFixe::Image { image: p, x: pt(*x + dx), y: pt(*y + dy) },
            })
        }
        Bloc::Tableau {
            colonnes,
            interligne,
            marge_cellule,
            filet_lignes,
            filets_cellules,
            entete,
            lignes,
            espace_avant,
            espace_apres,
        } => {
            for l in entete.iter().chain(lignes.iter()) {
                for c in &l.cellules {
                    for m in &c.morceaux {
                        verifier_texte(ctx, style_du_morceau(m), &m.texte, ou)?;
                    }
                }
            }
            let mut x = Mm::from(0.0f32);
            let mut cols = Vec::new();
            for (frac, _) in colonnes {
                let lc = largeur * *frac;
                cols.push((x, lc));
                x += lc;
            }
            let [mh, md, mb, mg] = *marge_cellule;
            if let Some((k, (_, lc))) = cols
                .iter()
                .enumerate()
                .find(|(_, (_, lc))| *lc <= pt(md + mg) + Mm::from(EPS_MM))
            {
                return refus(format!(
                    "{} : la colonne {} ({:.1} pt) est plus étroite que les marges de ses cellules",
                    ou,
                    k + 1,
                    en_points(*lc)
                ));
            }
            let mut t = Tableau {
                avant: pt(*espace_avant),
                apres: pt(*espace_apres),
                largeur,
                colonnes: cols,
                interligne: *interligne,
                marge: [pt(mh), pt(md), pt(mb), pt(mg)],
                filet_lignes: *filet_lignes,
                filets_cellules: *filets_cellules,
                entete: entete.clone(),
                lignes: lignes.clone(),
                h_entete: Vec::new(),
                h_lignes: Vec::new(),
                suivante: 0,
                commence: false,
            };
            t.h_entete = t.entete.iter().map(|l| t.hauteur_ligne(ctx, l, style)).collect();
            t.h_lignes = t.lignes.iter().map(|l| t.hauteur_ligne(ctx, l, style)).collect();
            // L'en-tête et la plus haute ligne doivent tenir ensemble sur une page : sinon le
            // tableau ne finirait jamais de passer à la page suivante.
            let plus_haute = t.h_lignes.iter().copied().fold(Mm::from(0.0f32), |a, b| a.max(b));
            let besoin = t.avant + t.h_entete.iter().copied().sum::<Mm>() + plus_haute;
            if besoin > hauteur_page + Mm::from(EPS_MM) {
                return refus(format!(
                    "{} : l'en-tête et la plus haute ligne du tableau occupent {:.1} pt, plus que la hauteur utile d'une page ({:.1} pt) ; une ligne ne se coupe pas entre deux pages",
                    ou,
                    en_points(besoin),
                    en_points(hauteur_page)
                ));
            }
            Noeud::Tableau(t)
        }
        Bloc::Colonnes {
            filet,
            espace_avant,
            marge_interne_haut,
            colonnes,
        } => {
            let mut cols = Vec::new();
            for (frac, blocs) in colonnes {
                let lc = largeur * *frac;
                let mut noeuds = Vec::new();
                for sb in blocs {
                    noeuds.push(construire(sb, ctx, lc, police, hauteur_page, style, cadre)?);
                }
                fondre_marges(&mut noeuds);
                cols.push((*frac, noeuds));
            }
            Noeud::Colonnes(Colonnes {
                filet: filet.map(|(c, e)| (couleur_genpdfi(c), pt(e))),
                avant: pt(*espace_avant),
                marge_haut: pt(*marge_interne_haut),
                colonnes: cols,
                fait: false,
            })
        }
    };
    // Ce qui ne se coupe pas doit tenir sur une page.
    if matches!(noeud, Noeud::Colonnes(_) | Noeud::Image(_)) {
        let h = noeud.hauteur(ctx, largeur, style);
        if h > hauteur_page + Mm::from(EPS_MM) {
            let quoi = if matches!(noeud, Noeud::Colonnes(_)) {
                "les colonnes ne se coupent pas entre deux pages, et elles"
            } else {
                "l'image"
            };
            return refus(format!(
                "{} : {} occupent {:.1} pt, plus que la hauteur utile d'une page ({:.1} pt)",
                ou,
                quoi,
                en_points(h),
                en_points(hauteur_page)
            ));
        }
    }
    Ok(noeud)
}

/// Les marges verticales voisines se fondent, comme en CSS : entre un bloc qui laisse `a` après
/// lui et un bloc qui demande `b` avant lui, l'écart vaut max(a, b), pas a + b. Sans cela, un
/// contrat de douze articles prenait deux pages là où le rendu HTML d'avant en prenait une.
/// Un bloc `espace` interrompt la fonte (il s'ajoute toujours).
fn fondre_marges(noeuds: &mut [Noeud]) {
    let mut apres_prec: Option<Mm> = None;
    for n in noeuds.iter_mut() {
        let avant = match n {
            Noeud::Paragraphe { avant, .. } => Some(avant),
            Noeud::Colonnes(c) => Some(&mut c.avant),
            Noeud::Tableau(t) => Some(&mut t.avant),
            _ => None,
        };
        if let (Some(avant), Some(a)) = (avant, apres_prec) {
            let b = *avant;
            *avant = if b > a { b - a } else { Mm::from(0.0) };
        }
        apres_prec = match n {
            Noeud::Paragraphe { apres, .. } => Some(*apres),
            Noeud::Colonnes(_) => Some(Mm::from(0.0)),
            Noeud::Tableau(t) => Some(t.apres),
            _ => None,
        };
    }
}

fn traduire(e: ErreurGenpdfi) -> Refus {
    Refus(format!("écriture du PDF impossible : {}", e))
}

/// Fabrique le PDF (ses octets) d'après une description JSON.
///
/// Un `{pages}` dans l'en-tête ou le pied de page demande le nombre total de pages, qu'on ne
/// connaît qu'une fois le document mis en page : on le met donc en page DEUX fois, la première
/// pour compter (le total y reste à 0), la seconde avec le vrai nombre. La largeur d'un chiffre
/// ne change rien aux sauts de page (l'en-tête et le pied sont dans les marges), et le texte est
/// aligné sur le bord, pas sur la largeur du total : le résultat est stable. Sans `{pages}`, une
/// seule passe.
pub fn fabriquer(json: &str) -> Result<Vec<u8>, Refus> {
    let (octets, pages, veut_total) = fabriquer_passe(json, 0)?;
    if !veut_total {
        return Ok(octets);
    }
    Ok(fabriquer_passe(json, pages)?.0)
}

/// Une passe de mise en page : les octets, le nombre de pages écrites, et si l'en-tête ou le pied
/// emploie `{pages}`.
fn fabriquer_passe(json: &str, total: usize) -> Result<(Vec<u8>, usize, bool), Refus> {
    let d: Description = lire_description(json)?;
    let mut doc = Document::new(polices::famille()?);
    // Times et Courier : seulement si un texte à position fixe les emploie.
    let emploie = |voulue: PoliceFixe| {
        d.documents.iter().flatten().any(|b| {
            matches!(&b.bloc, Bloc::TexteFixe { police, .. } if *police == voulue)
        })
    };
    let times = if emploie(PoliceFixe::Times) {
        Some(doc.add_font_family(polices::famille_times()?))
    } else {
        None
    };
    let courier = if emploie(PoliceFixe::Courier) {
        Some(doc.add_font_family(polices::famille_courier()?))
    } else {
        None
    };
    let cadre = Cadre {
        origine: (pt(d.page.gauche), pt(d.page.haut)),
        page: Size::new(pt(d.page.largeur), pt(d.page.hauteur)),
        times,
        courier,
    };
    doc.set_title(d.titre.clone());
    doc.set_paper_size(Size::new(pt(d.page.largeur), pt(d.page.hauteur)));
    doc.set_minimal_conformance();
    doc.set_font_size(d.police.taille);

    let logo = match &d.logo {
        Some(l) => Some((
            charger_image(&l.chemin, l.largeur, "logo", "fichier")?,
            pt(l.marge_haut),
            pt(l.marge_droite),
        )),
        None => None,
    };
    let mut reperes = Vec::new();
    for (r, en_bas, nom) in [(&d.entete_page, false, "entete_page"), (&d.pied_page, true, "pied_page")] {
        if let Some(r) = r {
            let base = Style::new().with_font_size(r.taille).with_color(couleur_genpdfi(r.couleur));
            for t in [&r.gauche, &r.centre, &r.droite] {
                verifier_texte(doc.context(), if r.gras { base.bold() } else { base }, t, nom)?;
            }
            reperes.push(ReperePose {
                repere: r.clone(),
                style: if r.gras { base.bold() } else { base },
                en_bas,
            });
        }
    }
    let veut_total = reperes.iter().any(|r| r.repere.emploie_total());
    let pages = Rc::new(Cell::new(0usize));
    doc.set_page_decorator(Decorateur {
        marges: Margins::trbl(
            pt(d.page.haut),
            pt(d.page.droite),
            pt(d.page.bas),
            pt(d.page.gauche),
        ),
        gauche: pt(d.page.gauche),
        droite: pt(d.page.droite),
        logo,
        reperes,
        total,
        pages: pages.clone(),
    });

    let largeur = pt(d.page.largeur_utile());
    let hauteur_page = pt(d.page.hauteur_utile());
    let style = Style::new().with_font_size(d.police.taille);
    let mut noeuds: Vec<Noeud> = Vec::new();
    let mut sauts = Vec::new();
    {
        let ctx = doc.context();
        for (i, blocs) in d.documents.iter().enumerate() {
            sauts.push(noeuds.len());
            let _ = i;
            for b in blocs {
                noeuds.push(construire(b, ctx, largeur, &d.police, hauteur_page, style, &cadre)?);
            }
            let debut = *sauts.last().expect("saut");
            fondre_marges(&mut noeuds[debut..]);
        }
    }
    // Chaque document commence sur une nouvelle page, sauf le premier.
    let mut depart = 0;
    let total = noeuds.len();
    let mut iter = noeuds.into_iter();
    for (k, &debut) in sauts.iter().enumerate() {
        let fin = sauts.get(k + 1).copied().unwrap_or(total);
        if k > 0 {
            doc.push(PageBreak::new());
        }
        for _ in debut..fin {
            doc.push(iter.next().expect("bloc"));
        }
        depart = fin;
    }
    let _ = depart;

    let mut octets = Vec::new();
    doc.render(&mut octets).map_err(traduire)?;
    Ok((octets, pages.get(), veut_total))
}
