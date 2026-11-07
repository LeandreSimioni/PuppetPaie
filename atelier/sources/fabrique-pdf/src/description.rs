//! La description validée : la forme donnée à la fabrique (`forme.rs`) devient ce que la mise en
//! page emploie, chaque valeur non dite remplacée par celle que la forme nomme.
//!
//! Toute erreur sort en une ligne de français, avec le numéro du bloc quand on le connaît.

use std::collections::BTreeMap;

use crate::forme as f;
use crate::modele;
use crate::style_contrat;

pub use crate::forme::{Alignement, PoliceFixe};

/// Un refus : une ligne de français, prête à être affichée telle quelle.
#[derive(Debug, Clone, PartialEq)]
pub struct Refus(pub String);

impl std::fmt::Display for Refus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Refus {}

pub fn refus<T>(message: impl Into<String>) -> Result<T, Refus> {
    Err(Refus(message.into()))
}

/// Le numéro du format que cette fabrique parle (`--version`). Le 2 ajoute les documents « modèle »
/// (FORMAT.md) ; les descriptions du format 1 sont toujours lues telles quelles. Le 3 ajoute les
/// modes de lecture et de retouche de PDF existants (`--pages`, `--texte`, `--marquer`,
/// `--verrouiller`) et le champ facultatif `largeur_max` de `texte_fixe` ; une description du
/// format 2 reste lue telle quelle.
pub const FORMAT: i64 = 3;

/// Le corps en dessous duquel `largeur_max` ne réduit pas un texte (6 pt : sous
/// cette taille la case est illisible, et c'est le signe que la donnée ne va pas).
pub const CORPS_PLANCHER: f32 = 6.0;

// ---------------------------------------------------------------------------
// Ce qui est validé.
// ---------------------------------------------------------------------------

pub type Rvb = (u8, u8, u8);

#[derive(Debug, Clone)]
pub struct Page {
    pub largeur: f32,
    pub hauteur: f32,
    pub haut: f32,
    pub droite: f32,
    pub bas: f32,
    pub gauche: f32,
}

impl Page {
    pub fn largeur_utile(&self) -> f32 {
        self.largeur - self.gauche - self.droite
    }
    pub fn hauteur_utile(&self) -> f32 {
        self.hauteur - self.haut - self.bas
    }
}

#[derive(Debug, Clone)]
pub struct Police {
    pub taille: f32,
    pub interligne: f32,
    pub couleur: Rvb,
}

#[derive(Debug, Clone)]
pub struct Logo {
    pub chemin: String,
    pub largeur: f32,
    pub marge_haut: f32,
    pub marge_droite: f32,
}

/// Un en-tête ou un pied de page : trois textes d'une ligne (`{page}` et `{pages}` y sont remplacés
/// par le numéro de la page et par le nombre de pages), à `distance` points du bord de la page (la
/// ligne de base). Un texte vide n'écrit rien.
#[derive(Debug, Clone)]
pub struct Repere {
    pub gauche: String,
    pub centre: String,
    pub droite: String,
    pub taille: f32,
    pub couleur: Rvb,
    pub gras: bool,
    pub distance: f32,
    pub premiere_page: bool,
    /// Une image posée au bout de la ligne, à droite, après le texte de droite.
    pub image: Option<ImageRepere>,
}

/// L'image d'un repère : `largeur` en points, hauteur selon les proportions ; son bas descend de
/// `descente` points sous la ligne de base (la jambe d'un mot dessiné).
#[derive(Debug, Clone)]
pub struct ImageRepere {
    pub chemin: String,
    pub largeur: f32,
    pub descente: f32,
}

impl Repere {
    /// Le total de pages ne se connaît qu'après une première mise en page (FORMAT.md).
    pub fn emploie_total(&self) -> bool {
        [&self.gauche, &self.centre, &self.droite].iter().any(|t| t.contains("{pages}"))
    }
}

#[derive(Debug, Clone)]
pub struct Morceau {
    pub texte: String,
    pub gras: bool,
    pub italique: bool,
    /// None : la couleur du bloc.
    pub couleur: Option<Rvb>,
}

/// Un trait : couleur et épaisseur.
pub type Filet = (Rvb, f32);

#[derive(Debug, Clone)]
pub struct CelluleTableau {
    /// Couleur et graisse déjà résolues (morceau, cellule, ligne, tableau).
    pub morceaux: Vec<Morceau>,
    pub etendue: usize,
    pub alignement: Alignement,
}

#[derive(Debug, Clone)]
pub struct LigneTableau {
    pub cellules: Vec<CelluleTableau>,
    pub fond: Option<Rvb>,
    pub taille: f32,
    pub filet_haut: Option<Filet>,
    pub filet_bas: Option<Filet>,
}

#[derive(Debug, Clone)]
pub enum Bloc {
    Paragraphe {
        morceaux: Vec<Morceau>,
        taille: f32,
        couleur: Rvb,
        alignement: Alignement,
        espace_avant: f32,
        espace_apres: f32,
        /// Marge à gauche, en points : le texte se met en page dans une largeur d'autant réduite.
        retrait: f32,
    },
    Espace(f32),
    Colonnes {
        filet: Option<(Rvb, f32)>,
        espace_avant: f32,
        marge_interne_haut: f32,
        colonnes: Vec<(f32, Vec<BlocEtiquete>)>,
    },
    Image {
        chemin: String,
        largeur: f32,
        /// Posée à l'endroit courant sans occuper de hauteur (FORMAT.md).
        flottante: bool,
    },
    /// Position absolue sur la page, en points depuis son coin haut-gauche : ne coule pas.
    TexteFixe {
        texte: String,
        x: f32,
        y: f32,
        taille: f32,
        police: PoliceFixe,
        gras: bool,
        couleur: Rvb,
        /// Gauche, Droite ou Centre, par rapport à `x`.
        alignement: Alignement,
        /// Largeur à ne pas dépasser, en points : au-delà, le corps est réduit pour que le texte
        /// tienne, sans descendre sous `CORPS_PLANCHER`.
        largeur_max: Option<f32>,
    },
    Trait {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        epaisseur: f32,
        couleur: Rvb,
    },
    ImageFixe {
        chemin: String,
        x: f32,
        y: f32,
        largeur: f32,
        hauteur: f32,
        centree: bool,
    },
    Tableau {
        /// Fraction de la largeur utile et alignement de chaque colonne.
        colonnes: Vec<(f32, Alignement)>,
        interligne: f32,
        /// haut, droite, bas, gauche : l'espace entre le bord d'une cellule et son texte.
        marge_cellule: [f32; 4],
        filet_lignes: Option<Filet>,
        filets_cellules: Option<Filet>,
        entete: Vec<LigneTableau>,
        lignes: Vec<LigneTableau>,
        espace_avant: f32,
        espace_apres: f32,
    },
}

impl Bloc {
    /// Un bloc à position absolue sur la page (il ne coule pas, n'occupe aucune hauteur).
    pub fn est_fixe(&self) -> bool {
        matches!(self, Bloc::TexteFixe { .. } | Bloc::Trait { .. } | Bloc::ImageFixe { .. })
    }
}

/// Un bloc et son étiquette d'erreur (« bloc 3, colonne 2, bloc 1 »).
#[derive(Debug, Clone)]
pub struct BlocEtiquete {
    pub etiquette: String,
    pub bloc: Bloc,
}

/// La description validée, telle que la mise en page l'emploie.
#[derive(Debug, Clone)]
pub struct Resolue {
    pub page: Page,
    pub police: Police,
    pub logo: Option<Logo>,
    pub titre: String,
    pub documents: Vec<Vec<BlocEtiquete>>,
    pub entete_page: Option<Repere>,
    pub pied_page: Option<Repere>,
}

// ---------------------------------------------------------------------------
// La validation.
// ---------------------------------------------------------------------------

/// Met une erreur de serde en français.
pub fn francais(e: &serde_json::Error) -> String {
    let brut = e.to_string();
    // serde_json ajoute « at line L column C » : on le retire du message et on le range à part.
    let (msg, lieu) = match brut.rfind(" at line ") {
        Some(i) => (brut[..i].to_string(), Some((e.line(), e.column()))),
        None => (brut.clone(), None),
    };
    let liste = |reste: &str| -> String {
        reste
            .split('`')
            .enumerate()
            .filter(|(i, _)| i % 2 == 1)
            .map(|(_, s)| s.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let nom = |reste: &str| -> String { reste.split('`').next().unwrap_or("").to_string() };
    let corps = if let Some(reste) = msg.strip_prefix("unknown field `") {
        let n = nom(reste);
        match reste.find("expected ") {
            Some(i) if !reste[i..].contains("there are no") => format!(
                "champ inconnu \"{}\" (champs connus : {})",
                n,
                liste(&reste[i..])
            ),
            _ => format!("champ inconnu \"{}\"", n),
        }
    } else if let Some(reste) = msg.strip_prefix("missing field `") {
        format!("champ absent \"{}\"", nom(reste))
    } else if let Some(reste) = msg.strip_prefix("unknown variant `") {
        let n = nom(reste);
        match reste.find("expected ") {
            Some(i) => format!("valeur inconnue \"{}\" (attendu : {})", n, liste(&reste[i..])),
            None => format!("valeur inconnue \"{}\"", n),
        }
    } else if let Some(reste) = msg.strip_prefix("invalid type: ") {
        match reste.split_once(", expected ") {
            Some((recu, attendu)) => format!(
                "valeur de mauvais type : {} reçu, {} attendu",
                traduit_type(recu),
                traduit_type(attendu)
            ),
            None => format!("valeur de mauvais type : {}", reste),
        }
    } else if let Some(reste) = msg.strip_prefix("invalid value: ") {
        format!("valeur invalide : {}", reste)
    } else if msg.starts_with("EOF while parsing") {
        "le JSON s'arrête trop tôt".to_string()
    } else if e.is_syntax() {
        "JSON mal formé".to_string()
    } else {
        msg
    };
    match lieu {
        Some((l, c)) if l > 0 => format!("{} (ligne {}, colonne {})", corps, l, c),
        _ => corps,
    }
}

fn traduit_type(t: &str) -> String {
    let t = t
        .replace("floating point", "nombre décimal")
        .replace("integer", "entier")
        .replace("string", "chaîne")
        .replace("boolean", "booléen")
        .replace("sequence", "liste")
        .replace("map", "objet")
        .replace("null", "null")
        .replace("unit value", "null")
        .replace("a nombre décimal", "un nombre décimal")
        .replace("f32", "un nombre")
        .replace("a chaîne", "une chaîne")
        .replace("a liste", "une liste")
        .replace("a booléen", "un booléen")
        .replace("a struct", "un objet")
        .replace("struct", "un objet");
    t
}

/// Lit « #RRGGBB » (majuscules ou minuscules).
pub fn couleur(texte: &str, ou: &str) -> Result<Rvb, Refus> {
    let bon = texte.len() == 7
        && texte.starts_with('#')
        && texte[1..].chars().all(|c| c.is_ascii_hexdigit());
    if !bon {
        return refus(format!(
            "{} : couleur \"{}\" invalide (attendu : \"#RRGGBB\")",
            ou, texte
        ));
    }
    let v = |i: usize| u8::from_str_radix(&texte[i..i + 2], 16).unwrap_or(0);
    Ok((v(1), v(3), v(5)))
}

fn fini(valeur: f32, nom: &str, ou: &str) -> Result<f32, Refus> {
    if valeur.is_finite() {
        Ok(valeur)
    } else {
        refus(format!("{} : {} n'est pas un nombre fini", ou, nom))
    }
}

fn positif_ou_nul(valeur: f32, nom: &str, ou: &str) -> Result<f32, Refus> {
    let v = fini(valeur, nom, ou)?;
    if v < 0.0 {
        refus(format!("{} : {} ne peut pas être négatif ({})", ou, nom, v))
    } else {
        Ok(v)
    }
}

/// Une longueur facultative : sans elle, 0.
fn longueur(valeur: Option<f32>, nom: &str, ou: &str) -> Result<f32, Refus> {
    positif_ou_nul(valeur.unwrap_or(0.0), nom, ou)
}

fn strictement_positif(valeur: f32, nom: &str, ou: &str) -> Result<f32, Refus> {
    let v = fini(valeur, nom, ou)?;
    if v <= 0.0 {
        refus(format!("{} : {} doit être positif ({})", ou, nom, v))
    } else {
        Ok(v)
    }
}

/// Une couleur facultative : sans elle, `heritee`.
fn couleur_ou(texte: &Option<String>, heritee: Rvb, ou: &str) -> Result<Rvb, Refus> {
    match texte {
        Some(c) => couleur(c, ou),
        None => Ok(heritee),
    }
}

/// Une taille facultative : sans elle, `heritee`.
fn taille_ou(valeur: Option<f32>, heritee: f32, nom: &str, ou: &str) -> Result<f32, Refus> {
    match valeur {
        Some(t) => strictement_positif(t, nom, ou),
        None => Ok(heritee),
    }
}

/// Les styles de modèle connus, pour le message d'erreur.
const STYLES: &str = "contrat, avenant, attestation";

fn morceau(m: &f::Morceau, majuscules: bool, ou: &str) -> Result<Morceau, Refus> {
    Ok(Morceau {
        texte: if majuscules { m.texte.to_uppercase() } else { m.texte.clone() },
        gras: m.gras.unwrap_or(false),
        italique: m.italique.unwrap_or(false),
        couleur: match &m.couleur {
            Some(c) => Some(couleur(c, ou)?),
            None => None,
        },
    })
}

fn lire_bloc(bloc: &f::Bloc, etiquette: &str, police: &Police) -> Result<Bloc, Refus> {
    match bloc {
        f::Bloc::Paragraphe(p) => {
            let majuscules = p.majuscules.unwrap_or(false);
            let mut morceaux = Vec::new();
            for m in &p.morceaux {
                morceaux.push(morceau(m, majuscules, etiquette)?);
            }
            Ok(Bloc::Paragraphe {
                morceaux,
                taille: taille_ou(p.taille, police.taille, "taille", etiquette)?,
                couleur: couleur_ou(&p.couleur, police.couleur, etiquette)?,
                alignement: p.alignement.unwrap_or(Alignement::Gauche),
                espace_avant: longueur(p.espace_avant, "espace_avant", etiquette)?,
                espace_apres: longueur(p.espace_apres, "espace_apres", etiquette)?,
                retrait: longueur(p.retrait, "retrait", etiquette)?,
            })
        }
        f::Bloc::Espace(e) => Ok(Bloc::Espace(positif_ou_nul(e.hauteur, "hauteur", etiquette)?)),
        f::Bloc::Image(i) => Ok(Bloc::Image {
            largeur: strictement_positif(i.largeur, "largeur", etiquette)?,
            chemin: i.chemin.clone(),
            flottante: i.flottante.unwrap_or(false),
        }),
        f::Bloc::Colonnes(c) => {
            if c.colonnes.is_empty() {
                return refus(format!("{} : une liste de colonnes vide", etiquette));
            }
            let filet = lire_filet(&c.filet_haut, etiquette)?;
            let mut somme = 0.0f32;
            let mut colonnes = Vec::new();
            for (k, col) in c.colonnes.iter().enumerate() {
                let ou_col = format!("{}, colonne {}", etiquette, k + 1);
                let l = strictement_positif(col.largeur, "largeur", &ou_col)?;
                somme += l;
                let mut blocs = Vec::new();
                for (j, b) in col.blocs.iter().flatten().enumerate() {
                    let et = format!("{}, bloc {}", ou_col, j + 1);
                    let bloc = lire_bloc(b, &et, police)?;
                    if matches!(bloc, Bloc::Tableau { .. }) {
                        return refus(format!(
                            "{} : un tableau ne se place pas dans des colonnes (il se coupe entre deux pages, elles non)",
                            et
                        ));
                    }
                    if bloc.est_fixe() {
                        return refus(format!(
                            "{} : un bloc à position fixe (texte_fixe, trait, image_fixe) ne se place pas dans des colonnes",
                            et
                        ));
                    }
                    blocs.push(BlocEtiquete { bloc, etiquette: et });
                }
                colonnes.push((l, blocs));
            }
            if (somme - 1.0).abs() > 0.001 {
                return refus(format!(
                    "{} : la somme des largeurs de colonnes vaut {}, elle doit valoir 1",
                    etiquette, somme
                ));
            }
            Ok(Bloc::Colonnes {
                filet,
                espace_avant: longueur(c.espace_avant, "espace_avant", etiquette)?,
                marge_interne_haut: longueur(c.marge_interne_haut, "marge_interne_haut", etiquette)?,
                colonnes,
            })
        }
        f::Bloc::Tableau(t) => lire_tableau(t, etiquette, police),
        f::Bloc::TexteFixe(t) => {
            let alignement = t.alignement.unwrap_or(Alignement::Gauche);
            if alignement == Alignement::Justifie {
                return refus(format!(
                    "{} : un texte à position fixe ne se justifie pas (gauche, droite ou centre)",
                    etiquette
                ));
            }
            Ok(Bloc::TexteFixe {
                x: fini(t.x, "x", etiquette)?,
                y: fini(t.y, "y", etiquette)?,
                taille: taille_ou(t.taille, police.taille, "taille", etiquette)?,
                couleur: couleur_ou(&t.couleur, police.couleur, etiquette)?,
                texte: t.texte.clone(),
                police: t.police.unwrap_or(PoliceFixe::Helvetica),
                gras: t.gras.unwrap_or(false),
                alignement,
                largeur_max: match t.largeur_max {
                    Some(v) => Some(strictement_positif(v, "largeur_max", etiquette)?),
                    None => None,
                },
            })
        }
        f::Bloc::Trait(t) => Ok(Bloc::Trait {
            x1: fini(t.x1, "x1", etiquette)?,
            y1: fini(t.y1, "y1", etiquette)?,
            x2: fini(t.x2, "x2", etiquette)?,
            y2: fini(t.y2, "y2", etiquette)?,
            epaisseur: strictement_positif(t.epaisseur.unwrap_or(f::EPAISSEUR), "epaisseur", etiquette)?,
            couleur: couleur(t.couleur.as_deref().unwrap_or(f::COULEUR), etiquette)?,
        }),
        f::Bloc::ImageFixe(i) => Ok(Bloc::ImageFixe {
            x: fini(i.x, "x", etiquette)?,
            y: fini(i.y, "y", etiquette)?,
            largeur: strictement_positif(i.largeur, "largeur", etiquette)?,
            hauteur: strictement_positif(i.hauteur, "hauteur", etiquette)?,
            centree: i.centree.unwrap_or(false),
            chemin: i.chemin.clone(),
        }),
    }
}

fn lire_filet(f: &Option<f::Filet>, ou: &str) -> Result<Option<Filet>, Refus> {
    match f {
        Some(x) => Ok(Some((
            couleur(x.couleur.as_deref().unwrap_or(f::COULEUR), ou)?,
            strictement_positif(x.epaisseur.unwrap_or(f::EPAISSEUR), "epaisseur", ou)?,
        ))),
        None => Ok(None),
    }
}

/// Une ligne de tableau : les couleurs, la graisse et la taille de la ligne descendent dans ses
/// cellules (celle du morceau, puis celle de la cellule, l'emportent).
fn lire_ligne(
    l: &f::Ligne,
    ou: &str,
    colonnes: &[(f32, Alignement)],
    taille: f32,
    couleur_table: Rvb,
) -> Result<LigneTableau, Refus> {
    let taille = taille_ou(l.taille, taille, "taille", ou)?;
    let couleur_ligne = couleur_ou(&l.couleur, couleur_table, ou)?;
    let fond = match &l.fond {
        Some(c) => Some(couleur(c, ou)?),
        None => None,
    };
    let gras_ligne = l.gras.unwrap_or(false);
    let mut cellules = Vec::new();
    let mut occupe = 0usize;
    for (k, v) in l.cellules.iter().enumerate() {
        let ou_c = format!("{}, cellule {}", ou, k + 1);
        let texte_seul;
        let decrite = match v {
            f::Cellule::Texte(s) => {
                texte_seul = f::CelluleDecrite {
                    texte: Some(s.clone()),
                    morceaux: None,
                    gras: None,
                    couleur: None,
                    alignement: None,
                    etendue: None,
                };
                &texte_seul
            }
            f::Cellule::Decrite(d) => d,
        };
        let etendue = decrite.etendue.unwrap_or(1);
        if etendue == 0 {
            return refus(format!("{} : etendue doit valoir au moins 1", ou_c));
        }
        if occupe + etendue > colonnes.len() {
            return refus(format!(
                "{} : la ligne occupe plus que les {} colonne(s) du tableau",
                ou_c,
                colonnes.len()
            ));
        }
        let couleur_cellule = couleur_ou(&decrite.couleur, couleur_ligne, &ou_c)?;
        let gras_cellule = decrite.gras.unwrap_or(false);
        let bruts: Vec<f::Morceau> = match (&decrite.texte, &decrite.morceaux) {
            (Some(_), Some(_)) => {
                return refus(format!("{} : \"texte\" et \"morceaux\" s'excluent", ou_c))
            }
            (Some(t), None) => vec![f::Morceau { texte: t.clone(), gras: None, italique: None, couleur: None }],
            (None, Some(m)) => m.clone(),
            (None, None) => Vec::new(),
        };
        let mut morceaux = Vec::new();
        for m in &bruts {
            morceaux.push(Morceau {
                texte: m.texte.clone(),
                gras: m.gras.unwrap_or(false) || gras_cellule || gras_ligne,
                italique: m.italique.unwrap_or(false),
                couleur: Some(couleur_ou(&m.couleur, couleur_cellule, &ou_c)?),
            });
        }
        cellules.push(CelluleTableau {
            morceaux,
            etendue,
            // Sans mot de la cellule, celui de la colonne où elle commence.
            alignement: decrite.alignement.unwrap_or(colonnes[occupe].1),
        });
        occupe += etendue;
    }
    Ok(LigneTableau {
        cellules,
        fond,
        taille,
        filet_haut: lire_filet(&l.filet_haut, ou)?,
        filet_bas: lire_filet(&l.filet_bas, ou)?,
    })
}

fn lire_tableau(t: &f::Tableau, etiquette: &str, police: &Police) -> Result<Bloc, Refus> {
    if t.colonnes.is_empty() {
        return refus(format!("{} : un tableau sans colonne", etiquette));
    }
    let mut colonnes = Vec::new();
    let mut somme = 0.0f32;
    for (k, c) in t.colonnes.iter().enumerate() {
        let ou_col = format!("{}, colonne {}", etiquette, k + 1);
        let l = strictement_positif(c.largeur, "largeur", &ou_col)?;
        somme += l;
        colonnes.push((l, c.alignement.unwrap_or(Alignement::Gauche)));
    }
    if (somme - 1.0).abs() > 0.001 {
        return refus(format!(
            "{} : la somme des largeurs de colonnes vaut {}, elle doit valoir 1",
            etiquette, somme
        ));
    }
    let taille = taille_ou(t.taille, police.taille, "taille", etiquette)?;
    let couleur_table = couleur_ou(&t.couleur, police.couleur, etiquette)?;
    let interligne = taille_ou(t.interligne, police.interligne, "interligne", etiquette)?;
    let ou_m = format!("{}, marge_cellule", etiquette);
    let marge_cellule = match &t.marge_cellule {
        Some(m) => [
            longueur(m.haut, "haut", &ou_m)?,
            longueur(m.droite, "droite", &ou_m)?,
            longueur(m.bas, "bas", &ou_m)?,
            longueur(m.gauche, "gauche", &ou_m)?,
        ],
        None => [0.0; 4],
    };
    let mut entete = Vec::new();
    for (i, l) in t.entete.iter().flatten().enumerate() {
        let ou = format!("{}, en-tête, ligne {}", etiquette, i + 1);
        entete.push(lire_ligne(l, &ou, &colonnes, taille, couleur_table)?);
    }
    let mut lignes = Vec::new();
    for (i, l) in t.lignes.iter().flatten().enumerate() {
        let ou = format!("{}, ligne {}", etiquette, i + 1);
        lignes.push(lire_ligne(l, &ou, &colonnes, taille, couleur_table)?);
    }
    Ok(Bloc::Tableau {
        colonnes,
        interligne,
        marge_cellule,
        filet_lignes: lire_filet(&t.filet_lignes, etiquette)?,
        filets_cellules: lire_filet(&t.filets_cellules, etiquette)?,
        entete,
        lignes,
        espace_avant: longueur(t.espace_avant, "espace_avant", etiquette)?,
        espace_apres: longueur(t.espace_apres, "espace_apres", etiquette)?,
    })
}

fn lire_repere(r: &Option<f::Repere>, nom: &str, police: &Police) -> Result<Option<Repere>, Refus> {
    let r = match r {
        Some(r) => r,
        None => return Ok(None),
    };
    let texte = |t: &Option<String>| t.clone().unwrap_or_default();
    let (gauche, centre, droite) = (texte(&r.gauche), texte(&r.centre), texte(&r.droite));
    for t in [&gauche, &centre, &droite] {
        if t.contains('\n') || t.contains('\r') {
            return refus(format!("{} : chaque texte tient sur une ligne (pas de \\n)", nom));
        }
    }
    Ok(Some(Repere {
        taille: strictement_positif(r.taille.unwrap_or(police.taille), "taille", nom)?,
        couleur: couleur_ou(&r.couleur, police.couleur, nom)?,
        distance: positif_ou_nul(r.distance.unwrap_or(f::DISTANCE_REPERE), "distance", nom)?,
        gauche,
        centre,
        droite,
        gras: r.gras.unwrap_or(false),
        premiere_page: r.premiere_page.unwrap_or(true),
        image: match &r.image {
            Some(i) => Some(ImageRepere {
                largeur: strictement_positif(i.largeur, "largeur", nom)?,
                descente: longueur(i.descente, "descente", nom)?,
                chemin: i.chemin.clone(),
            }),
            None => None,
        },
    }))
}

/// La page : celle que la description dit (chaque mesure non dite : A4, `MARGE`), sinon celle du style, sinon A4
/// aux marges `MARGE`.
fn lire_page(p: &Option<f::Page>, style: Option<style_contrat::Variante>) -> Result<Page, Refus> {
    let brute = match (p, style) {
        (Some(p), _) => {
            let m = p.marges.as_ref();
            let marge = |c: fn(&f::Marges) -> Option<f32>| m.and_then(c).unwrap_or(f::MARGE);
            Page {
                largeur: p.largeur.unwrap_or(f::PAGE_LARGEUR),
                hauteur: p.hauteur.unwrap_or(f::PAGE_HAUTEUR),
                haut: marge(|m| m.haut),
                droite: marge(|m| m.droite),
                bas: marge(|m| m.bas),
                gauche: marge(|m| m.gauche),
            }
        }
        (None, Some(_)) => style_contrat::page(),
        (None, None) => Page {
            largeur: f::PAGE_LARGEUR,
            hauteur: f::PAGE_HAUTEUR,
            haut: f::MARGE,
            droite: f::MARGE,
            bas: f::MARGE,
            gauche: f::MARGE,
        },
    };
    let page = Page {
        largeur: strictement_positif(brute.largeur, "largeur", "page")?,
        hauteur: strictement_positif(brute.hauteur, "hauteur", "page")?,
        haut: positif_ou_nul(brute.haut, "haut", "page, marges")?,
        droite: positif_ou_nul(brute.droite, "droite", "page, marges")?,
        bas: positif_ou_nul(brute.bas, "bas", "page, marges")?,
        gauche: positif_ou_nul(brute.gauche, "gauche", "page, marges")?,
    };
    if page.largeur_utile() <= 0.0 || page.hauteur_utile() <= 0.0 {
        return refus("page : les marges ne laissent aucune place au texte");
    }
    Ok(page)
}

/// La police : celle que la description dit (chaque valeur non dite : `TAILLE`, `INTERLIGNE`, `COULEUR`), sinon
/// celle du style, sinon `TAILLE`, `INTERLIGNE`, `COULEUR`.
fn lire_police(p: &Option<f::Police>, style: Option<style_contrat::Variante>) -> Result<Police, Refus> {
    let (taille, interligne, teinte) = match (p, style) {
        (Some(p), _) => (
            p.taille.unwrap_or(f::TAILLE),
            p.interligne.unwrap_or(f::INTERLIGNE),
            couleur(p.couleur.as_deref().unwrap_or(f::COULEUR), "police")?,
        ),
        (None, Some(_)) => {
            let q = style_contrat::police();
            (q.taille, q.interligne, q.couleur)
        }
        (None, None) => (f::TAILLE, f::INTERLIGNE, couleur(f::COULEUR, "police")?),
    };
    Ok(Police {
        taille: strictement_positif(taille, "taille", "police")?,
        interligne: strictement_positif(interligne, "interligne", "police")?,
        couleur: teinte,
    })
}

/// Valide une description : ce que la mise en page emploie.
pub fn resoudre(d: &f::Description) -> Result<Resolue, Refus> {
    // Le 3 est le numéro de la fabrique (`--version`) : une description qui le porte est celle
    // du format 2, avec `largeur_max` en plus. Les trois se lisent de la même façon.
    if !matches!(d.format, 1..=3) {
        return refus(format!(
            "format {} non pris en charge : cette fabrique lit les formats 1, 2 et 3",
            d.format
        ));
    }
    // Le style (« contrat ») donne la page et la police quand la description ne les dit pas.
    let style = d.style.map(|s| match s {
        f::Style::Contrat => style_contrat::Variante::Contrat,
        f::Style::Avenant => style_contrat::Variante::Avenant,
        f::Style::Attestation => style_contrat::Variante::Attestation,
    });
    let page = lire_page(&d.page, style)?;
    let police = lire_police(&d.police, style)?;
    let logo = match &d.logo {
        Some(l) => Some(Logo {
            largeur: strictement_positif(l.largeur, "largeur", "logo")?,
            marge_haut: longueur(l.marge_haut, "marge_haut", "logo")?,
            marge_droite: longueur(l.marge_droite, "marge_droite", "logo")?,
            chemin: l.chemin.clone(),
        }),
        None => None,
    };
    if d.documents.is_empty() {
        return refus("description : la liste \"documents\" est vide");
    }
    let plusieurs = d.documents.len() > 1;
    let mut documents = Vec::new();
    let mut titre_modele: Option<String> = None;
    for (n, doc) in d.documents.iter().enumerate() {
        let blocs_dits = match doc {
            f::Document::Modele(mb) => {
                let ou = if plusieurs { format!("document {}", n + 1) } else { "document".to_string() };
                let prefixe = if plusieurs { format!("document {}, ", n + 1) } else { String::new() };
                let (blocs, titre) = lire_modele(mb, d.format, style, &ou, prefixe, titre_modele.is_none())?;
                if titre_modele.is_none() {
                    titre_modele = titre;
                }
                documents.push(blocs);
                continue;
            }
            f::Document::Blocs(b) => b,
        };
        let mut blocs = Vec::new();
        for (i, b) in blocs_dits.iter().enumerate() {
            let etiquette = if plusieurs {
                format!("document {}, bloc {}", n + 1, i + 1)
            } else {
                format!("bloc {}", i + 1)
            };
            blocs.push(BlocEtiquete {
                bloc: lire_bloc(b, &etiquette, &police)?,
                etiquette,
            });
        }
        // La page d'un document à position fixe est fixe : ses blocs ne se mêlent pas à ceux qui
        // coulent (leur origine serait celle de l'endroit où l'on en est, pas celle de la page).
        if let Some(premier_fixe) = blocs.iter().position(|b| b.bloc.est_fixe()) {
            if let Some(autre) = blocs.iter().find(|b| !b.bloc.est_fixe()) {
                return refus(format!(
                    "{} : un document à position fixe ne mêle pas les blocs fixes (par exemple {}) et ceux qui coulent (par exemple {}) ; un document par page fixe",
                    autre.etiquette, blocs[premier_fixe].etiquette, autre.etiquette
                ));
            }
        }
        documents.push(blocs);
    }
    let entete_page = lire_repere(&d.entete_page, "entete_page", &police)?;
    let pied_page = lire_repere(&d.pied_page, "pied_page", &police)?;
    Ok(Resolue {
        page,
        police,
        logo,
        // Sans titre dit, celui du premier modèle (`texte_brut`, sans gras).
        titre: d.titre.clone().or(titre_modele).unwrap_or_default(),
        documents,
        entete_page,
        pied_page,
    })
}

/// Un document modèle mis en page selon le style : ses blocs, et son titre (`texte_brut`, sans gras) si `veut_titre`.
fn lire_modele(
    mb: &f::Modele,
    format: i64,
    style: Option<style_contrat::Variante>,
    ou: &str,
    prefixe: String,
    veut_titre: bool,
) -> Result<(Vec<BlocEtiquete>, Option<String>), Refus> {
    use style_contrat::Variante;
    if format < 2 {
        return refus(format!("{} : \"modele\" demande \"format\": 2", ou));
    }
    let variante = match style {
        Some(v) => v,
        None => {
            return refus(format!(
                "{} : \"modele\" demande un \"style\" au niveau de la description (styles connus : {})",
                ou, STYLES
            ))
        }
    };
    // Le modèle vient d'un fichier (`chemin`) ou de son texte (`texte`) : l'un des deux.
    let (m, nom_modele) = match (&mb.chemin, &mb.texte) {
        (Some(_), Some(_)) => {
            return refus(format!("{} : \"chemin\" et \"texte\" s'excluent (le modèle vient de l'un ou de l'autre)", ou))
        }
        (None, None) => {
            return refus(format!("{} : \"modele\" demande \"chemin\" (le fichier) ou \"texte\" (le modèle même)", ou))
        }
        (Some(c), None) => (modele::lire_fichier(c)?, c.clone()),
        (None, Some(t)) => (modele::lire_modele(t, "le modèle donné en texte")?, "le modèle donné en texte".to_string()),
    };
    let dites: &[f::Modification] = mb.modifications.as_deref().unwrap_or(&[]);
    if variante != Variante::Contrat && mb.remplace.is_some() {
        return refus(format!("{} : \"remplace\" ne va qu'avec le style \"contrat\"", ou));
    }
    if variante == Variante::Attestation && mb.signature.is_some() {
        return refus(format!(
            "{} : le style \"attestation\" n'a pas de case de signature (\"signature\")",
            ou
        ));
    }
    if variante != Variante::Attestation && mb.signature_employeur.is_some() {
        return refus(format!(
            "{} : \"signature_employeur\" ne va qu'avec le style \"attestation\"",
            ou
        ));
    }
    if variante != Variante::Avenant && !dites.is_empty() {
        return refus(format!("{} : \"modifications\" ne va qu'avec le style \"avenant\"", ou));
    }
    if variante == Variante::Avenant {
        if dites.is_empty() {
            return refus(format!("{} : rien n'a changé, pas d'avenant à écrire (\"modifications\" est vide)", ou));
        }
        if m.modification.is_empty() {
            return refus(format!("{} n'a pas de bloc MODIFICATION:.", nom_modele));
        }
        for (i, x) in dites.iter().enumerate() {
            if x.phrase.is_some() && !m.modification.iter().any(|b| b.contains("modification.phrase")) {
                return refus(format!("{} : modification {} : le bloc MODIFICATION: n'a pas de paragraphe {{{{modification.phrase}}}}", ou, i + 1));
            }
            if x.phrase.is_none() && x.numero.is_none() {
                return refus(format!("{} : modification {} : ni \"numero\" ni \"phrase\"", ou, i + 1));
            }
            if x.avant.len() != x.apres.len() {
                return refus(format!(
                    "{} : modification {} : \"avant\" et \"apres\" n'ont pas le même nombre de paragraphes",
                    ou,
                    i + 1
                ));
            }
        }
    }
    let modifications: Vec<style_contrat::Modification> = dites
        .iter()
        .map(|x| style_contrat::Modification {
            numero: x.numero.unwrap_or(0),
            article: x.article.clone(),
            avant: x.avant.clone(),
            apres: x.apres.clone(),
            phrase: x.phrase.clone(),
        })
        .collect();
    let valeurs: &BTreeMap<String, String> = &mb.valeurs;
    let c = style_contrat::Contrat {
        variante,
        valeurs,
        remplace: mb.remplace.as_ref().map(|r| (r.numero.as_str(), r.date.as_str())),
        signature: mb.signature.as_deref(),
        modifications: &modifications,
        signature_employeur: mb.signature_employeur.as_deref(),
    };
    let titre = if veut_titre { Some(modele::texte_brut(&m.titre, valeurs)?) } else { None };
    Ok((style_contrat::blocs(&m, &c, &prefixe)?, titre))
}
