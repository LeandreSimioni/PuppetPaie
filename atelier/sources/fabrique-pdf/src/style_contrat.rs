//! La mise en page d'un contrat CDDU : le modèle lu (`modele.rs`) devient des blocs.
//!
//! Chaque nombre ci-dessous est une mesure de la mise en page du contrat (le commentaire de
//! chaque constante dit laquelle).
//!
//! L'avenant et les deux papiers de fin de contrat (certificat de travail, reçu pour solde de
//! tout compte) ont le même papier, la même police et les mêmes titres : ce sont trois VARIANTES
//! de la même mise en page (`Variante`), qui ne diffèrent que par ce qui suit le préambule.

use crate::description::{Alignement, Bloc, BlocEtiquete, Morceau, Page, Police, Refus, Rvb, refus};
use crate::modele::{self, Modele, Valeurs};

pub const COULEUR_TEXTE: Rvb = (0x2C, 0x3E, 0x50); // body
const ROUGE: Rvb = (0xB2, 0x22, 0x22); // h1, h2, .label
const GRIS: Rvb = (0x55, 0x55, 0x55); // .sub
const GRIS_CLAIR: Rvb = (0x77, 0x77, 0x77); // .pt
const FILET: Rvb = (0xCC, 0xCC, 0xCC); // .sign td
/// Une ligne de `<br/>` dans une case : 7,5 pt x 1,25.
const LIGNE_CORPS: f32 = 7.5 * 1.25;
/// `.cite { margin-left: 12pt }`
const RETRAIT_CITATION: f32 = 12.0;
const SIGNATURE_LARGEUR: f32 = 92.0; // l'image fait 792 x 384 px
/// La case de la mention de signature de l'artiste (`documents::contrat::ecrire_contrat` contresigne) :
/// 46 pt de haut, filet gris à 0,5 pt, texte de 6,6 pt à 1,35, en retrait de 4 pt et 3 pt.
const MENTION_HAUTEUR: f32 = 46.0;
const MENTION_TAILLE: f32 = 6.6;
const MENTION_INTERLIGNE: f32 = 1.35;
const MENTION_FILET: Rvb = (0xB3, 0xB3, 0xB3);
const MENTION_MARGE: (f32, f32) = (4.0, 3.0);

/// La phrase d'un contrat qui annule et remplace, quand le modèle n'a pas de clé `ANNULE:`.
const ANNULE_DEFAUT: &str = "Le présent contrat annule et remplace le contrat n° {{remplace.numero}} du {{remplace.date}}, qui est réputé n'avoir jamais produit d'effet.";

/// La page et la police du contrat : A4 (595 x 842, le format A4 de PyMuPDF, `paper_rect("A4")`), marges de
/// la zone du chemin PyMuPDF — le haut laisse passer le logo — plus 7,5 pt à gauche et à droite
/// (la marge du `<body>` HTML).
pub fn page() -> Page {
    Page { largeur: 595.0, hauteur: 842.0, haut: 106.0, droite: 59.5, bas: 46.0, gauche: 59.5 }
}

pub fn police() -> Police {
    Police { taille: 7.5, interligne: 1.25, couleur: COULEUR_TEXTE }
}

/// Les trois mises en page (le `style` de la description : « contrat », « avenant »,
/// « attestation »).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variante {
    /// Titre, préambule, articles numérotés, mention finale, cases de signature.
    Contrat,
    /// Le contrat, plus un article « Modification de l'article n » par article modifié, qui cite
    /// l'ancien texte puis le nouveau ; les articles du modèle suivent,
    /// numérotés à la suite.
    Avenant,
    /// Certificat de travail, reçu pour solde de tout compte : des intitulés sans numéro, une
    /// valeur multiligne vaut plusieurs paragraphes, et pas de cases de signature — le reçu se
    /// signe de la main du salarié, sous sa mention.
    Attestation,
}

impl Variante {
    pub fn depuis_style(style: &str) -> Option<Variante> {
        match style {
            "contrat" => Some(Variante::Contrat),
            "avenant" => Some(Variante::Avenant),
            "attestation" => Some(Variante::Attestation),
            _ => None,
        }
    }
}

/// Un article du contrat modifié par l'avenant : son numéro et son titre dans le contrat, et les
/// paragraphes cités, l'ancien texte puis le nouveau, dans le même ordre.
pub struct Modification {
    pub numero: i64,
    pub article: String,
    pub avant: Vec<String>,
    pub apres: Vec<String>,
}

/// Ce que le moteur envoie d'un document, en plus du chemin du modèle.
pub struct Contrat<'a> {
    pub variante: Variante,
    pub valeurs: &'a Valeurs,
    /// (numero, date) du contrat que celui-ci annule et remplace (contrat seulement).
    pub remplace: Option<(&'a str, &'a str)>,
    /// L'image de la signature de l'employeur ; sans elle la case reste vide.
    pub signature: Option<&'a str>,
    /// Les articles modifiés (avenant seulement).
    pub modifications: &'a [Modification],
    /// L'image de la signature de l'employeur, posée en fin de document, à droite (attestation
    /// seulement : le certificat de travail s'établit signé ; le reçu, lui, se signe du salarié).
    pub signature_employeur: Option<&'a str>,
    /// Ce que l'artiste a signé d'un clic : qui, quand, de quelle adresse, quel code. Posé dans
    /// SA case, sans rien déplacer (contrat et avenant) ; sans lui la case reste vide.
    pub mention_artiste: Option<&'a str>,
}

fn para(
    morceaux: Vec<(String, bool)>,
    taille: Option<f32>,
    couleur: Option<Rvb>,
    alignement: Alignement,
    avant: f32,
    apres: f32,
) -> Bloc {
    Bloc::Paragraphe {
        morceaux: morceaux.into_iter().map(|(texte, gras)| Morceau { texte, gras, italique: false, couleur: None }).collect(),
        taille: taille.unwrap_or(7.5),
        couleur: couleur.unwrap_or(COULEUR_TEXTE),
        alignement,
        espace_avant: avant,
        espace_apres: apres,
        retrait: 0.0,
    }
}

/// Une citation : le paragraphe en italique, en retrait de 12 pt (`.cite` de l'avenant).
fn cite(morceaux: Vec<(String, bool)>) -> Bloc {
    let mut b = para(morceaux, None, None, Alignement::Justifie, 0.0, 2.6);
    if let Bloc::Paragraphe { morceaux, retrait, .. } = &mut b {
        for m in morceaux.iter_mut() {
            m.italique = true;
        }
        *retrait = RETRAIT_CITATION;
    }
    b
}

fn valeur<'a>(v: &'a Valeurs, cle: &str) -> Result<&'a str, Refus> {
    match v.get(cle) {
        Some(s) => Ok(s),
        None => refus(format!(
            "marqueur inconnu dans le modèle de contrat : {{{{{}}}}} (le bloc de signatures le demande)",
            cle
        )),
    }
}

/// Le bloc de signatures : deux colonnes à filet, celle de l'employeur avec sa signature sous sa
/// fonction (image flottante : elle recouvre les lignes vides sans rien pousser à la page
/// suivante). Sans fichier de signature, la case reste vide et le contrat se signe à la main.
fn signatures(v: &Valeurs, signature: Option<&str>, mention: Option<&str>) -> Result<Bloc, Refus> {
    let case = |libelle: &str, nom: &str, sous: &str| -> Vec<Bloc> {
        vec![
            para(vec![(libelle.to_string(), true)], Some(6.6), Some(ROUGE), Alignement::Gauche, 0.0, 0.0),
            para(vec![(nom.to_string(), false)], None, None, Alignement::Gauche, 0.0, 0.0),
            para(vec![(sous.to_string(), false)], Some(7.0), Some(GRIS_CLAIR), Alignement::Gauche, 0.0, 0.0),
        ]
    };
    let mut employeur = case(
        "L'employeur",
        valeur(v, "employeur.signataire")?,
        valeur(v, "employeur.signataire_fonction")?,
    );
    // La case du salarie dit qui il est au contrat : « L'artiste » d'un engagement, sauf si le contrat dit autre chose
    // (`salarie.case_signature`, « Le salarié » d'un contrat de permanent).
    let qualite = v.get("salarie.case_signature").map(String::as_str).unwrap_or("L'artiste");
    let mut artiste = case(qualite, valeur(v, "salarie.nom_complet")?, "signature");
    match signature {
        Some(chemin) => {
            employeur.push(Bloc::Espace(3.0));
            employeur.push(Bloc::Image {
                chemin: chemin.to_string(),
                largeur: SIGNATURE_LARGEUR,
                flottante: true,
            });
            employeur.push(Bloc::Espace(3.0 * LIGNE_CORPS - 3.0));
        }
        None => employeur.push(Bloc::Espace(3.0 * LIGNE_CORPS)),
    }
    match mention {
        // La case est posée 3 pt sous « signature » et ne prend aucune hauteur : le reste de
        // l'espace garde la case de l'artiste à la hauteur qu'elle a sans mention.
        Some(texte) => {
            artiste.push(Bloc::Espace(3.0));
            artiste.push(Bloc::Mention {
                lignes: texte.replace("\r\n", "\n").replace('\r', "\n").split('\n').map(str::to_string).collect(),
                taille: MENTION_TAILLE,
                interligne: MENTION_INTERLIGNE,
                couleur: COULEUR_TEXTE,
                filet: MENTION_FILET,
                hauteur: MENTION_HAUTEUR,
                marge: MENTION_MARGE,
            });
            artiste.push(Bloc::Espace(3.0 * LIGNE_CORPS - 3.0));
        }
        None => artiste.push(Bloc::Espace(3.0 * LIGNE_CORPS)),
    }
    let etiquetter = |blocs: Vec<Bloc>, col: usize| -> Vec<BlocEtiquete> {
        blocs
            .into_iter()
            .enumerate()
            .map(|(j, bloc)| BlocEtiquete {
                etiquette: format!("signatures, colonne {}, bloc {}", col, j + 1),
                bloc,
            })
            .collect()
    };
    Ok(Bloc::Colonnes {
        filet: Some((FILET, 0.5)),
        espace_avant: 4.0,
        marge_interne_haut: 5.0,
        colonnes: vec![(0.5, etiquetter(employeur, 1)), (0.5, etiquetter(artiste, 2))],
    })
}

/// Le bloc de l'employeur en fin d'attestation : nom, fonction, puis l'image (qui occupe sa
/// hauteur), dans la moitié droite de la page. Il ne se coupe pas entre deux pages.
fn signature_employeur(v: &Valeurs, image: &str) -> Result<Bloc, Refus> {
    let etiquetter = |blocs: Vec<Bloc>| -> Vec<BlocEtiquete> {
        blocs
            .into_iter()
            .enumerate()
            .map(|(j, bloc)| BlocEtiquete { etiquette: format!("signature de l'employeur, bloc {}", j + 1), bloc })
            .collect()
    };
    let droite = vec![
        para(vec![(valeur(v, "employeur.signataire")?.to_string(), false)], None, None, Alignement::Gauche, 0.0, 0.0),
        para(
            vec![(valeur(v, "employeur.signataire_fonction")?.to_string(), false)],
            Some(7.0),
            Some(GRIS_CLAIR),
            Alignement::Gauche,
            0.0,
            0.0,
        ),
        Bloc::Espace(3.0),
        Bloc::Image { chemin: image.to_string(), largeur: SIGNATURE_LARGEUR, flottante: false },
    ];
    Ok(Bloc::Colonnes {
        filet: None,
        espace_avant: 10.0,
        marge_interne_haut: 0.0,
        colonnes: vec![(0.6, Vec::new()), (0.4, etiquetter(droite))],
    })
}

/// Les blocs d'un document, dans l'ordre du modèle : titre, sous-titre, phrase d'annulation
/// (contrat), préambule, [articles modifiés (avenant),] articles, mention finale, [cases de
/// signature (contrat, avenant)]. `ou` préfixe les étiquettes d'erreur de mise en page
/// (« document 2 »).
pub fn blocs(m: &Modele, c: &Contrat, ou: &str) -> Result<Vec<BlocEtiquete>, Refus> {
    use Alignement::{Gauche, Justifie};
    let v = c.valeurs;
    let attestation = c.variante == Variante::Attestation;
    // Un paragraphe du modèle : un seul, ou un par ligne que la substitution a produite
    // (attestation : l'inventaire du reçu).
    let corps = |texte: &str, v: &Valeurs, avant: f32| -> Result<Vec<Bloc>, Refus> {
        if attestation {
            Ok(modele::lignes_substituees(texte, v)?
                .iter()
                .map(|l| para(modele::morceaux_substitues(l, false), None, None, Justifie, avant, 2.6))
                .collect())
        } else {
            Ok(vec![para(modele::morceaux(texte, v, false)?, None, None, Justifie, avant, 2.6)])
        }
    };
    let mut blocs = vec![para(modele::morceaux(&m.titre, v, true)?, Some(11.0), Some(ROUGE), Gauche, 10.0, 1.0)];
    if !m.soustitre.is_empty() {
        blocs.push(para(modele::morceaux(&m.soustitre, v, false)?, Some(6.8), Some(GRIS), Justifie, 0.0, 7.0));
    }
    if let Some((numero, date)) = c.remplace {
        let mut vr = v.clone();
        vr.insert("remplace.numero".to_string(), numero.to_string());
        vr.insert("remplace.date".to_string(), date.to_string());
        let defaut = [ANNULE_DEFAUT.to_string()];
        let phrases: &[String] = if m.annule.is_empty() { &defaut } else { &m.annule };
        for b in phrases {
            blocs.push(para(modele::morceaux(b, &vr, true)?, None, None, Justifie, 0.0, 6.0));
        }
    }
    for b in &m.preambule {
        blocs.extend(corps(b, v, 0.0)?);
    }
    // Les articles modifiés prennent les premiers numéros ; ceux du modèle les suivent.
    let mut n = 0;
    for modif in c.modifications {
        n += 1;
        let mut vm = v.clone();
        vm.insert("modification.numero".to_string(), modif.numero.to_string());
        vm.insert("modification.article".to_string(), modif.article.clone());
        let intitule = format!(
            "Article {} \u{2014} Modification de l'article {{{{modification.numero}}}} \u{ab} {{{{modification.article}}}} \u{bb}",
            n
        );
        blocs.push(para(modele::morceaux(&intitule, &vm, true)?, Some(7.7), Some(ROUGE), Gauche, 4.5, 1.0));
        for b in &m.modification {
            // Un paragraphe qui porte l'ancien ou le nouveau texte se répète pour chaque
            // paragraphe cité ; les autres se posent une fois.
            let textes = if b.contains("modification.avant") {
                Some(&modif.avant)
            } else if b.contains("modification.apres") {
                Some(&modif.apres)
            } else {
                None
            };
            match textes {
                Some(textes) => {
                    for t in textes {
                        let mut vt = vm.clone();
                        vt.insert("modification.avant".to_string(), t.clone());
                        vt.insert("modification.apres".to_string(), t.clone());
                        blocs.push(cite(modele::morceaux(b, &vt, false)?));
                    }
                }
                None => blocs.push(para(modele::morceaux(b, &vm, false)?, None, None, Justifie, 0.0, 2.6)),
            }
        }
    }
    for (titre, paragraphes) in m.articles.iter() {
        n += 1;
        let intitule = if attestation { titre.clone() } else { format!("Article {} \u{2014} {}", n, titre) };
        blocs.push(para(modele::morceaux(&intitule, v, true)?, Some(7.7), Some(ROUGE), Gauche, 4.5, 1.0));
        for b in paragraphes {
            blocs.extend(corps(b, v, 0.0)?);
        }
    }
    for b in &m.signatures {
        blocs.extend(corps(b, v, 6.0)?);
    }
    if !attestation {
        blocs.push(signatures(v, c.signature, c.mention_artiste)?);
    } else if let Some(image) = c.signature_employeur {
        blocs.push(signature_employeur(v, image)?);
    }
    Ok(blocs
        .into_iter()
        .enumerate()
        .map(|(i, bloc)| BlocEtiquete { etiquette: format!("{}bloc {}", ou, i + 1), bloc })
        .collect())
}
