//! Le cachet de l'entreprise, DESSINE depuis ses propres donnees -- en SVG
//!.
//!
//! ECRIT LE 22/09/2026, sur une remarque de l'editeur qui regardait son AER : ce
//! que le logiciel posait dans la case << Signature de l'employeur >> n'etait pas
//! une signature, c'etait un CACHET -- le fichier `gabarits/identite/
//! signature.png`, que `conges_spectacles.py` appelle lui-meme << un tampon a
//! l'italienne >>. L'attestation portait donc un tampon dans la case signature,
//! et RIEN dans la case cachet. Les deux cases etaient a l'envers l'une de
//! l'autre.
//!
//! POURQUOI LE DESSINER PLUTOT QUE DEMANDER UNE IMAGE. C'est la regle 0 : ce
//! logiciel n'est ecrit pour aucune compagnie en particulier. Un PNG ne vaut que
//! pour celle qui l'a fourni -- une compagnie qui installe PuppetPaie n'aurait,
//! sans cela, aucun cachet, ou pire, celui de la precedente. Tout ce qu'un cachet
//! porte est deja dans `entreprise.json`, verifie et date : la raison sociale,
//! l'adresse, le SIRET, l'APE, la licence d'entrepreneur de spectacles. Le
//! dessiner, c'est n'avoir rien a demander a personne.
//!
//! POURQUOI EN SVG. Un tampon est un DESSIN, pas une photo : il doit rester net
//! dans la case d'un formulaire comme a l'ecran, et la MEME image sert aux deux.
//! Le SVG se met a l'echelle sans baver, pese quelques centaines d'octets, et
//! c'est le meme texte qu'on envoie a la page de Configuration pour que la
//! compagnie VOIE son cachet avant de se demander s'il lui en faut un autre --
//! la demande de l'editeur, mot pour mot : << que les gens le trouvent beau et ne
//! passent pas trop de temps a faire le leur >>.
//!
//! ET LA COMPAGNIE GARDE LE SIEN SI ELLE EN A UN. Le dessin est le DEFAUT, pas
//! une obligation : celle qui a deja son tampon le pose dans Configuration, et
//! c'est lui qui part. Qu'elle le retire, et le dessin reprend la main -- rien a
//! reconfigurer, rien a se rappeler. C'est `image` qui arbitre, et il n'a qu'une
//! question : y a-t-il un fichier ?
//!
//! RIEN N'EST INVENTE. Une valeur absente d'`entreprise.json` ne s'ecrit pas :
//! la ligne saute. Un cachet qui afficherait un SIRET vide ou << None >> serait
//! pire que pas de cachet, et la case n'est exigee par aucun formulaire.

use std::path::Path;

use serde_json::{json, Value};

use droit::contexte::format_g;
use socle::date::{d, en_texte, Jour};
use socle::fichiers::vrai;

/// L'encre d'un tampon, et la geometrie du dessin. Le rapport 2:1 est celui
/// d'un tampon rectangulaire courant ; il tient dans la case cachet de l'AER
/// (160 x 79 points) sans rogner.
pub const ENCRE: &str = "#1a2e73";
pub const LARGEUR: f64 = 300.0;
pub const HAUTEUR: f64 = 150.0;
/// Le tampon ne touche pas le bord de la case.
pub const MARGE: f64 = 3.0;

/// Le numero de licence applicable, ou None.
///
/// `entreprise.json` la porte DATEE (une liste de {depuis, numero}) parce
/// qu'elle se renouvelle. `Contexte.licence` fait deja ce choix, mais ce
/// module ne recoit qu'un dictionnaire -- il le refait, a l'identique, et
/// sans lever : un cachet ne refuse pas de s'imprimer pour une licence.
pub fn licence_du_jour(ent: &Value, date_ref: Option<&Jour>) -> Option<String> {
    let val = ent.get("licence_spectacle").filter(|v| vrai(v))?;
    if let Value::String(s) = val {
        return Some(s.clone());
    }
    // Une licence qui n'est ni un texte ni la liste attendue : aucune.
    let liste = val.as_array()?;
    let mut tries: Vec<&Value> = liste.iter().collect();
    for e in &tries {
        e.as_object()?;
    }
    // `sorted(..., key=x.get("depuis") or "")` : tri stable, chaines comparees
    // par point de code (l'ordre des octets UTF-8 est le meme).
    let cle = |x: &&Value| -> String {
        match x.get("depuis") {
            Some(v) if vrai(v) => en_texte(v),
            _ => String::new(),
        }
    };
    tries.sort_by_key(cle);
    let mut retenu: Option<&Value> = None;
    for e in &tries {
        // `jour is None or _d(e["depuis"]) <= jour` : sans jour de reference,
        // la date n'est meme pas lue.
        let ok = match date_ref {
            None => true,
            Some(j) => d(e.get("depuis")?).ok()? <= *j,
        };
        if ok {
            retenu = e.get("numero");
        }
    }
    // `retenu or (val[-1].get("numero") ...)` ; l'appelant ne garde que ce qui
    // est vrai (`if lic:`), donc une valeur fausse vaut None des ici.
    let valeur = match retenu.filter(|v| vrai(v)) {
        Some(v) => Some(v),
        None => liste.last().and_then(|e| e.get("numero")),
    };
    valeur.filter(|v| vrai(v)).map(en_texte)
}

/// Ce que le cachet porte : le nom, le lieu, les numeros.
#[derive(Debug, Clone, PartialEq)]
pub struct Lignes {
    pub nom: String,
    pub lieu: Vec<String>,
    pub numeros: Vec<String>,
}

fn texte_de(ent: &Value, cle: &str) -> Option<String> {
    ent.get(cle).filter(|v| vrai(v)).map(en_texte)
}

/// `lignes` : rendu en donnees et non dessine, pour que cela se lise sans
/// ouvrir un fichier. Les trois groupes n'ont pas le meme poids a l'oeil : le
/// nom est ce qu'on lit, le reste est ce qu'on verifie.
pub fn lignes(ent: &Value, date_ref: Option<&Jour>) -> Lignes {
    let nom = texte_de(ent, "nom_affichage")
        .or_else(|| texte_de(ent, "raison_sociale"))
        .unwrap_or_default();
    let mut lieu = Vec::new();
    if let Some(adresse) = texte_de(ent, "adresse_affichage").or_else(|| texte_de(ent, "adresse")) {
        lieu.push(adresse);
    }
    let ou: Vec<String> = [texte_de(ent, "code_postal"), texte_de(ent, "commune_affichage").or_else(|| texte_de(ent, "commune"))]
        .into_iter()
        .flatten()
        .collect();
    if !ou.is_empty() {
        lieu.push(ou.join(" "));
    }
    let mut numeros = Vec::new();
    if let Some(s) = texte_de(ent, "siret") {
        numeros.push(format!("SIRET {}", s));
    }
    if let Some(s) = texte_de(ent, "naf") {
        numeros.push(format!("APE {}", s));
    }
    if let Some(l) = licence_du_jour(ent, date_ref) {
        numeros.push(format!("Licence {}", l));
    }
    Lignes { nom, lieu, numeros }
}

/// Le corps le plus grand auquel `texte` tient dans `large`.
///
/// Sans mesure de police reelle : une largeur moyenne par caractere suffit
/// pour du texte court, et le SVG n'a pas d'unite de mesure a nous rendre.
/// Mieux vaut un tampon un rien plus petit qu'un tampon qui deborde.
fn corps_qui_tient(texte: &str, large: f64, depart: f64, etroit: f64) -> f64 {
    if texte.is_empty() {
        return depart;
    }
    let tient = large / (texte.chars().count() as f64 * etroit);
    let m = if depart < tient { depart } else { tient };
    if 5.0 > m { 5.0 } else { m }
}

/// `xml.sax.saxutils.escape` : `&`, `<` et `>`, et rien d'autre.
fn echapper(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Le cachet, en SVG. None s'il n'y a rien a dessiner.
///
/// Un cadre double -- le trait epais dehors, le fin dedans --, le nom en
/// capitales, un filet, puis l'adresse et les numeros. C'est la mise en page
/// d'un tampon encreur, et elle est tenue ICI pour que la page de
/// Configuration et l'attestation montrent exactement la meme chose.
pub fn svg(ent: &Value, date_ref: Option<&Jour>) -> Option<String> {
    let t = lignes(ent, date_ref);
    if t.nom.is_empty() && t.lieu.is_empty() && t.numeros.is_empty() {
        return None;
    }

    let dedans = 16.0;
    let utile = LARGEUR - 2.0 * dedans;
    let milieu = LARGEUR / 2.0;
    let police = "Helvetica,Arial,sans-serif";

    // LA MISE EN PAGE SE CALCULE AVANT DE S'ECRIRE, et c'est ce qui evite le
    // defaut trouve au premier rendu : la ligne << Licence ... >> sortait par
    // le bas du cadre. Une compagnie qui porte un nom long, trois lignes
    // d'adresse ou une licence a rallonge n'est pas un cas rare -- c'est le
    // cas courant des que l'on quitte celle qui a servi a dessiner.
    //
    // On monte donc la liste de ce qu'il y a a ecrire (texte, corps, graisse,
    // blanc au-dessus), on mesure, et on comprime si besoin. Le resultat est
    // ensuite CENTRE verticalement : un tampon dont le texte pend vers le haut
    // ou vers le bas se voit tout de suite.
    let mut items: Vec<(Option<String>, f64, u32, f64)> = Vec::new();
    if !t.nom.is_empty() {
        items.push((Some(t.nom.to_uppercase()), corps_qui_tient(&t.nom, utile, 22.0, 0.62), 700, 0.0));
        items.push((None, 0.0, 0, 9.0)); // le filet
    }
    for (i, ligne) in t.lieu.iter().enumerate() {
        items.push((Some(ligne.clone()), corps_qui_tient(ligne, utile, 11.5, 0.52), 400, if i == 0 { 7.0 } else { 3.0 }));
    }
    for (i, ligne) in t.numeros.iter().enumerate() {
        items.push((Some(ligne.clone()), corps_qui_tient(ligne, utile, 10.0, 0.52), 400, if i == 0 { 6.0 } else { 2.5 }));
    }

    let haut_utile = HAUTEUR - 2.0 * dedans;
    let mut total = items.iter().fold(0.0, |s, (_, corps, _, blanc)| s + (blanc + corps));
    if total > haut_utile {
        // Tout retrecit du meme rapport : les proportions du tampon tiennent,
        // et rien ne sort du cadre.
        let k = haut_utile / total;
        items = items.into_iter().map(|(txt, corps, gras, blanc)| (txt, corps * k, gras, blanc * k)).collect();
        total = haut_utile;
    }

    let mut out = vec![
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" width=\"{}\" height=\"{}\">",
            format_g(LARGEUR),
            format_g(HAUTEUR),
            format_g(LARGEUR),
            format_g(HAUTEUR)
        ),
        format!(
            "<rect x=\"4\" y=\"4\" width=\"{}\" height=\"{}\" rx=\"12\" fill=\"none\" stroke=\"{}\" stroke-width=\"4\"/>",
            format_g(LARGEUR - 8.0),
            format_g(HAUTEUR - 8.0),
            ENCRE
        ),
        format!(
            "<rect x=\"11\" y=\"11\" width=\"{}\" height=\"{}\" rx=\"7\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.2\"/>",
            format_g(LARGEUR - 22.0),
            format_g(HAUTEUR - 22.0),
            ENCRE
        ),
    ];
    let mut y = dedans + (haut_utile - total) / 2.0;
    for (texte, corps, gras, blanc) in &items {
        y += blanc + corps;
        match texte {
            None => out.push(format!(
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"1\"/>",
                format_g(milieu - utile / 3.0),
                format_g(y),
                format_g(milieu + utile / 3.0),
                format_g(y),
                ENCRE
            )),
            Some(texte) => out.push(format!(
                "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" fill=\"{}\" font-family=\"{}\" font-size=\"{}\" font-weight=\"{}\">{}</text>",
                format_g(milieu),
                format_g(y),
                ENCRE,
                police,
                format_g(*corps),
                gras,
                echapper(texte)
            )),
        }
    }
    out.push("</svg>".into());
    Some(out.concat())
}

/// Une image (PNG, JPEG ou SVG) a poser dans `rect`, a `MARGE` du bord.
///
/// C'est la fabrique qui la pose (`fabrique::remplir`) : le plus grand
/// rectangle aux proportions de l'image qui tient dans la case, centre -- un
/// tampon 2:1 pose dans une case presque carree ne sort pas ovale. Un SVG y
/// reste VECTORIEL, net quelle que soit la taille de la case, la ou un PNG
/// agrandi bave. Un fichier illisible n'est pas pose en silence : la fabrique
/// refuse, et l'attestation ne sort pas sans qu'on sache pourquoi.
pub fn image_fichier(page: &Value, rect: &Value, fichier: &str) -> Value {
    json!({"page": page, "rect": rect, "marge": MARGE, "fichier": fichier})
}

/// Le cachet a poser dans `rect` de la page `page` : celui de la compagnie
/// s'il est pose, sinon le notre. C'est le SEUL endroit ou l'on choisit entre
/// les deux. Rend l'image telle que la fabrique la prend, ou None s'il n'y a
/// rien a dessiner.
///
/// Le dessin n'est pas un pis-aller : il porte les memes faits, tires de la
/// fiche, et il suit la fiche quand elle change. Un fichier pose l'emporte
/// simplement parce qu'une compagnie qui prend la peine d'apporter son tampon
/// veut le sien.
pub fn image(page: &Value, rect: &Value, ent: &Value, date_ref: Option<&Jour>, fichier: Option<&str>) -> Option<Value> {
    if let Some(f) = fichier {
        if Path::new(f).exists() {
            return Some(image_fichier(page, rect, f));
        }
    }
    let dessin = svg(ent, date_ref)?;
    Some(json!({"page": page, "rect": rect, "marge": MARGE, "svg": dessin}))
}
