//! `courriels-fictifs` (`moteur/simulation.courriels_json`) : les courriels joues de la compagnie fictive
//! (`courriels-simules/*.eml`), tels que le salarie les recoit -- et, s'ils portent un bouton, la reponse toute
//! ecrite que ce bouton prepare. `dernier` : le repere a redonner la fois suivante.
//!
//! Lecture MIME par `mailparse` (bibliotheque eprouvee) a la place de `email` de Python. Ecarts possibles, notes :
//! un en-tete `From`/`To` est rendu tel que `mailparse` le decode (le Python le re-serialise : guillemets autour
//! d'un nom a point ou virgule) ; les pieces jointes sont les feuilles du message qui ne sont ni le texte ni le HTML
//! du corps (`iter_attachments`).

use std::collections::HashMap;
use std::path::Path;

use mailparse::{parse_mail, MailHeaderMap, ParsedMail};
use serde_json::{json, Value};

use droit::contexte::Contexte;
use crate::courriel::{BoiteSimulee, Courriel, ErreurCourriel};

const DOSSIER: &str = "courriels-simules";

/// Les courriels joues, du plus ancien au plus recent : (nom, octets). Un fichier illisible est ignore.
pub fn courriels(racine: &Path) -> Vec<(String, Vec<u8>)> {
    let dossier = racine.join(DOSSIER);
    let Ok(lecture) = std::fs::read_dir(&dossier) else { return vec![] };
    let mut noms: Vec<String> = lecture
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".eml"))
        .collect();
    noms.sort();
    noms.into_iter()
        .filter_map(|n| {
            let o = std::fs::read(dossier.join(&n)).ok()?;
            parse_mail(&o).ok()?;
            Some((n, o))
        })
        .collect()
}

pub fn nom_de_fichier(p: &ParsedMail) -> Option<String> {
    let d = p.get_content_disposition();
    d.params.get("filename").cloned().or_else(|| p.ctype.params.get("name").cloned())
}

pub fn feuilles<'a>(p: &'a ParsedMail<'a>, sortie: &mut Vec<&'a ParsedMail<'a>>) {
    if p.subparts.is_empty() {
        sortie.push(p);
    } else {
        for s in &p.subparts {
            feuilles(s, sortie);
        }
    }
}

/// `binascii.a2b_qp` : les retours a la ligne restent ceux du fichier (`mailparse` les rend tous en CRLF).
fn decoder_qp(o: &[u8]) -> Vec<u8> {
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut b = Vec::with_capacity(o.len());
    let mut i = 0;
    while i < o.len() {
        if o[i] != b'=' {
            b.push(o[i]);
            i += 1;
        } else if o.get(i + 1) == Some(&b'\n') {
            i += 2;
        } else if o.get(i + 1) == Some(&b'\r') && o.get(i + 2) == Some(&b'\n') {
            i += 3;
        } else if let (Some(h), Some(l)) = (o.get(i + 1).and_then(|c| hex(*c)), o.get(i + 2).and_then(|c| hex(*c))) {
            b.push(h * 16 + l);
            i += 3;
        } else {
            b.push(b'=');
            i += 1;
        }
    }
    b
}

/// `part.get_content()` d'une feuille texte : le corps decode, sans toucher aux fins de ligne.
pub fn texte_de(f: &ParsedMail) -> String {
    use mailparse::body::Body;
    let charset = f.ctype.charset.to_lowercase();
    if let Body::Base64(b) = f.get_body_encoded() {
        // Un corps base64 se decode comme `get_payload(decode=True)` : indulgent, et JAMAIS vide faute de base64 valide (le
        // texte brut, lignes jointes, si la longueur est impossible). `mailparse` rend une erreur la ou le Python rend un texte.
        let octets = decoder_b64_python(b.get_raw());
        let synthetique = [format!("Content-Type: text/plain; charset=\"{}\"\r\nContent-Transfer-Encoding: binary\r\n\r\n", f.ctype.charset).into_bytes(), octets].concat();
        return parse_mail(&synthetique).ok().and_then(|m| m.get_body().ok()).unwrap_or_default();
    }
    if matches!(charset.as_str(), "" | "us-ascii" | "utf-8" | "utf8") {
        if let Body::QuotedPrintable(b) = f.get_body_encoded() {
            return String::from_utf8_lossy(&decoder_qp(b.get_raw())).into_owned();
        }
    }
    let t = f.get_body().unwrap_or_default();
    match f.get_body_encoded() {
        // `mailparse` rend CRLF les fins de ligne d'un corps quoted-printable ; Python garde celles du fichier.
        Body::QuotedPrintable(b) if !b.get_raw().contains(&b'\r') => t.replace("\r\n", "\n"),
        _ => t,
    }
}

const ALPHABET_B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// `binascii.a2b_base64` : `strict` = `validate=True` (tout signe hors alphabet est une erreur) ; sinon les signes hors alphabet
/// sont ignores. `None` = `binascii.Error`.
fn a2b_base64(donnees: &[u8], strict: bool) -> Option<Vec<u8>> {
    let valeur = |c: u8| ALPHABET_B64.iter().position(|x| *x == c).map(|v| v as u32);
    let (mut sortie, mut tampon, mut bits, mut quad, mut pads) = (Vec::new(), 0u32, 0u32, 0usize, 0usize);
    let mut fini = false;
    for (i, &c) in donnees.iter().enumerate() {
        if c == b'=' {
            if quad >= 2 && quad + pads + 1 >= 4 {
                if strict && i + 1 < donnees.len() {
                    return None; // des donnees apres le bourrage
                }
                fini = true;
                break;
            }
            pads += 1;
            continue;
        }
        let Some(v) = valeur(c) else {
            if strict {
                return None;
            }
            continue;
        };
        pads = 0;
        tampon = (tampon << 6) | v;
        bits += 6;
        quad = (quad + 1) % 4;
        if bits >= 8 {
            bits -= 8;
            sortie.push((tampon >> bits) as u8);
            tampon &= (1 << bits) - 1;
        }
    }
    if !fini && quad != 0 {
        return None;
    }
    Some(sortie)
}

/// `email._header_value_parser.decode_b` (ce que `get_payload(decode=True)` fait d'un corps base64) : d'abord strict avec le
/// bourrage complete, puis indulgent, puis indulgent avec `==` ; si la longueur est impossible, le texte lui-meme.
pub fn decoder_b64_python(brut: &[u8]) -> Vec<u8> {
    let encode: Vec<u8> = brut.iter().copied().filter(|c| *c != b'\n' && *c != b'\r').collect();
    let reste = encode.len() % 4;
    let mut complete = encode.clone();
    if reste != 0 {
        complete.extend_from_slice(&b"==="[..4 - reste]);
    }
    if let Some(o) = a2b_base64(&complete, true) {
        return o;
    }
    if let Some(o) = a2b_base64(&encode, false) {
        return o;
    }
    let mut deux = encode.clone();
    deux.extend_from_slice(b"==");
    if let Some(o) = a2b_base64(&deux, false) {
        return o;
    }
    encode
}

/// Le texte d'une feuille `text/<sous>` sans nom de fichier : la premiere.
fn corps(feuilles: &[&ParsedMail], sous: &str) -> String {
    for f in feuilles {
        if f.ctype.mimetype == format!("text/{}", sous) && nom_de_fichier(f).is_none() {
            return texte_de(f);
        }
    }
    String::new()
}

/// `html.unescape`, pour ce qu'un lien `mailto:` porte.
fn deseschapper(t: &str) -> String {
    let mut out = String::new();
    let mut reste = t;
    while let Some(i) = reste.find('&') {
        out.push_str(&reste[..i]);
        let suite = &reste[i..];
        let fin = suite.find(';');
        let rempl = fin.and_then(|f| {
            let e = &suite[1..f];
            let c = match e {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ => e
                    .strip_prefix("#x")
                    .or_else(|| e.strip_prefix("#X"))
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| e.strip_prefix('#').and_then(|d| d.parse().ok()))
                    .and_then(char::from_u32),
            }?;
            Some((c, f))
        });
        match rempl {
            Some((c, f)) => {
                out.push(c);
                reste = &suite[f + 1..];
            }
            None => {
                out.push('&');
                reste = &suite[1..];
            }
        }
    }
    out.push_str(reste);
    out
}

/// `urllib.parse.unquote_plus`.
fn decoder(t: &str) -> String {
    let o = t.as_bytes();
    let mut b: Vec<u8> = Vec::with_capacity(o.len());
    let mut i = 0;
    while i < o.len() {
        match o[i] {
            b'+' => b.push(b' '),
            b'%' if i + 2 < o.len() => {
                match std::str::from_utf8(&o[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()).ok_or(()) {
                    Ok(v) => {
                        b.push(v);
                        i += 2;
                    }
                    Err(_) => b.push(b'%'),
                }
            }
            c => b.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&b).into_owned()
}

/// `_bouton_mailto` : (objet, corps) du lien `mailto:` du premier HTML qui en porte un.
pub fn bouton(feuilles: &[&ParsedMail]) -> Option<(String, String)> {
    let re = regex::Regex::new(r#"href="(mailto:[^"]+)""#).ok()?;
    for f in feuilles {
        if f.ctype.mimetype != "text/html" {
            continue;
        }
        let html = texte_de(f);
        if let Some(c) = re.captures(&html) {
            let lien = deseschapper(&c[1]);
            let requete = lien.split_once('?').map(|(_, q)| q.split('#').next().unwrap_or("")).unwrap_or("");
            let (mut sujet, mut texte) = (None::<String>, None::<String>);
            for paire in requete.split('&') {
                let Some((k, v)) = paire.split_once('=') else { continue };
                if v.is_empty() {
                    continue;
                }
                match decoder(k).as_str() {
                    "subject" if sujet.is_none() => sujet = Some(decoder(v)),
                    "body" if texte.is_none() => texte = Some(decoder(v)),
                    _ => {}
                }
            }
            return Some((sujet.unwrap_or_default(), texte.unwrap_or_default()));
        }
    }
    None
}

/// `_pages` : le nombre de pages d'une piece PDF, 0 pour une autre ou une illisible.
fn pages(f: &ParsedMail) -> usize {
    if f.ctype.mimetype != "application/pdf" {
        return 0;
    }
    let Ok(octets) = f.get_body_raw() else { return 0 };
    fabrique_pdf::lecture::charger(&octets).map(|d| d.get_pages().len()).unwrap_or(0)
}

/// `str(m["From"])` : les adresses comme Python les re-serialise (un nom a point, virgule, arobase... entre guillemets).
fn adresses(m: &ParsedMail, champ: &str) -> String {
    let brut = m.headers.get_first_value(champ).unwrap_or_default();
    let Some(h) = m.headers.get_first_header(champ) else { return brut };
    let Ok(liste) = mailparse::addrparse_header(h) else { return brut };
    let mut vus = vec![];
    for a in liste.iter() {
        match a {
            mailparse::MailAddr::Single(i) => match i.display_name.as_deref().filter(|n| !n.is_empty()) {
                Some(n) => {
                    let n = if n.chars().any(|c| "()<>@,:;.\\\"[]".contains(c)) {
                        format!("\"{}\"", n.replace('\\', "\\\\").replace('"', "\\\""))
                    } else {
                        n.to_string()
                    };
                    vus.push(format!("{} <{}>", n, i.addr));
                }
                None => vus.push(i.addr.clone()),
            },
            mailparse::MailAddr::Group(_) => return brut,
        }
    }
    vus.join(", ")
}

/// Les courriels joues de la compagnie fictive ecrits apres `apres` (un nom de fichier ; "" : tous), tels que la page
/// les montre.
pub fn courriels_joues(racine: &Path, apres: &str) -> Value {
    let (mut sortie, mut dernier) = (vec![], String::new());
    for (nom, octets) in courriels(racine) {
        dernier = nom.clone();
        if nom.as_str() <= apres {
            continue;
        }
        let Ok(m) = parse_mail(&octets) else { continue };
        let mut fs = vec![];
        feuilles(&m, &mut fs);
        let en_tete = |c: &str| m.headers.get_first_value(c).unwrap_or_default();
        let b = bouton(&fs);
        let est_multipart = m.ctype.mimetype.starts_with("multipart/") && m.ctype.mimetype != "multipart/alternative";
        let pieces: Vec<Value> = fs
            .iter()
            .filter(|f| est_multipart && !(f.ctype.mimetype.starts_with("text/") && nom_de_fichier(f).is_none()))
            .map(|f| json!({"nom": nom_de_fichier(f), "pages": pages(f)}))
            .collect();
        sortie.push(json!({
            "nom": nom,
            "de": adresses(&m, "From"),
            "a": adresses(&m, "To"),
            "sujet": en_tete("Subject"),
            "texte": corps(&fs, "plain"),
            "html": corps(&fs, "html"),
            "pieces": pieces,
            "bouton": b.map(|(s, c)| json!({"sujet": s, "corps": c})),
        }));
    }
    json!({"courriels": sortie, "dernier": dernier})
}


// ------------------------------------------------------------------ la boite qui repond

/// Les courriels joues, par leur `Message-ID` (`{(m.get("Message-ID") or "").strip(): m for _, m in courriels(ctx)}`) : le
/// dernier d'un meme identifiant l'emporte.
pub fn courriels_par_identifiant(racine: &Path) -> HashMap<String, Vec<u8>> {
    let mut envoyes = HashMap::new();
    for (_, octets) in courriels(racine) {
        if let Ok(m) = parse_mail(&octets) {
            envoyes.insert(crate::courriel_lecture::identifiant_de(&m), octets.clone());
        }
    }
    envoyes
}

/// `_bouton_mailto(demande)` pour la demande dont on connait les identifiants (`a["envoyes"]`) : (objet, corps) du lien
/// `mailto:` de son courriel joue, s'il y en a un.
pub fn bouton_de_la_demande(envoyes: &HashMap<String, Vec<u8>>, identifiants: &[String]) -> Option<(String, String)> {
    let octets = identifiants.iter().find_map(|i| envoyes.get(i))?;
    let m = parse_mail(octets).ok()?;
    let mut f = Vec::new();
    feuilles(&m, &mut f);
    bouton(&f)
}

/// `simulation._reponse` : le courriel que la personne fictive renvoie a la boite (de `de` a `a`), date de maintenant,
/// identifiant `<ident@fictive.puppetpaie>`.
pub fn reponse_jouee(de: &str, a: &str, sujet: &str, corps: &str, ident: &str) -> Result<Vec<u8>, String> {
    // CHOIX DE L'UTILISATEUR (07/10/2026) : dans une compagnie fictive, une adresse de salarie AVEC NOM D'AFFICHAGE (`Nom <adresse>`) est
    // acceptee, comme chez le Python (`EmailMessage` ecrit l'en-tete tel quel). `envoi::en_mime_avec` n'ecrit que des adresses
    // simples (H3a) : le message se fabrique avec l'adresse nue, puis l'en-tete `From` est remis tel que le Python l'ecrit.
    let (nu, rendu) = adresse_d_affichage(de);
    let message = crate::envoi::Message {
        cle: ident.to_string(),
        nom: nu.clone(),
        destinataire: a.to_string(),
        sujet: sujet.to_string(),
        corps: corps.to_string(),
        ..Default::default()
    };
    let t = socle::temps::secondes().floor() as i64;
    let mut frontiere = String::new;
    let octets = crate::envoi::en_mime_avec(
        &message,
        &nu,
        crate::envoi::Hasards { date: crate::envoi::date_de(t), message_id: format!("<{}@fictive.puppetpaie>", ident), frontiere: &mut frontiere },
    )?;
    let entete_nu = format!("From: {}", nu);
    match rendu {
        Some(rendu) if octets.starts_with(entete_nu.as_bytes()) => {
            let mut sortie = format!("From: {}", rendu).into_bytes();
            sortie.extend_from_slice(&octets[entete_nu.len()..]);
            Ok(sortie)
        }
        _ => Ok(octets),
    }
}

/// `Nom <adresse>` -> (l'adresse nue, l'en-tete `From` tel que le Python l'ecrit) ; une adresse simple -> (elle-meme, `None`). Le
/// nom d'affichage ASCII s'ecrit tel quel ; un nom qui ne l'est pas, en mot encode `=?utf-8?b?...?=` (court : le Python choisit
/// `q` ou `b` et plie les longs noms, ce que cette fonction ne fait pas).
fn adresse_d_affichage(de: &str) -> (String, Option<String>) {
    let de = de.trim();
    let (Some(debut), true) = (de.rfind('<'), de.ends_with('>')) else { return (de.to_string(), None) };
    let nu = de[debut + 1..de.len() - 1].trim().to_string();
    let nom = de[..debut].trim();
    if nom.is_empty() {
        return (nu.clone(), Some(format!("<{}>", nu)));
    }
    let nom_ecrit = if nom.is_ascii() { nom.to_string() } else { format!("=?utf-8?b?{}?=", crate::envoi::base64_brut(nom.as_bytes())) };
    (nu.clone(), Some(format!("{} <{}>", nom_ecrit, nu)))
}

/// `simulation._BoiteQuiRepond` : la boite de la compagnie fictive, vue par une releve. Chaque code attendu y trouve sa
/// reponse, fabriquee d'avance. Ce qu'on y envoie (le contrat contresigne) s'ecrit comme tout courriel joue.
pub struct BoiteQuiRepond {
    boite: BoiteSimulee,
    reponses: HashMap<String, Vec<u8>>,
}

impl BoiteQuiRepond {
    pub fn nouvelle(ctx: &Contexte) -> BoiteQuiRepond {
        BoiteQuiRepond { boite: BoiteSimulee::du_contexte(ctx), reponses: HashMap::new() }
    }

    /// Fabrique d'avance la reponse a ce code.
    pub fn repondre(&mut self, code: &str, octets: Vec<u8>) {
        self.reponses.insert(code.to_string(), octets);
    }
}

impl Courriel for BoiteQuiRepond {
    fn compte(&self) -> &str {
        Courriel::compte(&self.boite)
    }

    fn racine(&self) -> Option<&Path> {
        Courriel::racine(&self.boite)
    }

    fn envoyer(&mut self, message_mime: &[u8]) -> Result<String, ErreurCourriel> {
        self.boite.envoyer(message_mime)
    }

    /// Le code cherche, s'il a sa reponse : `[code]`.
    fn rechercher(&mut self, requete: &str, _maximum: usize) -> Result<Vec<String>, ErreurCourriel> {
        let code = requete.trim_matches('"');
        Ok(if self.reponses.contains_key(code) { vec![code.to_string()] } else { Vec::new() })
    }

    fn message_brut(&mut self, identifiant: &str) -> Result<Vec<u8>, ErreurCourriel> {
        Ok(self.reponses.get(identifiant).cloned().unwrap_or_default())
    }

    fn fermer(&mut self) {}
}
