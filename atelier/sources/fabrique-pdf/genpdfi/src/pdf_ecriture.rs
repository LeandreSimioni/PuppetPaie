// Fichier modifié pour PuppetPaie (copie locale de genpdfi 0.2.7, Apache-2.0 OU MIT) :
// voir MODIFICATIONS.md à la racine de cette copie.
//! L'écriture d'un PDF au plus court (modification PuppetPaie, nouveau module).
//!
//! Deux écritures, et pas d'autre dans la fabrique :
//!
//! - `compact` : un PDF entier, en PDF 1.5 (ISO 32000-1, 7.5.7 et 7.5.8) : les objets qui ne sont pas
//!   des flux (dictionnaires des polices et de leurs largeurs, pages, champs de formulaire…) vont dans
//!   des flux d'objets comprimés, et la table des références est un flux comprimé avec le prédicteur
//!   PNG « Up ». Les flux eux-mêmes sont écrits tels quels : ce module ne recomprime rien.
//! - `ajouter` : une mise à jour incrémentale (ISO 32000-1, 7.5.6) : les octets d'avant restent en tête,
//!   octet pour octet, et seuls les objets neufs ou changés s'écrivent à la suite, avec une table qui
//!   suit la forme de celle d'avant (un flux si le fichier en a un, une table classique sinon).
//!
//! Le résultat ne dépend que de l'entrée : mêmes objets, mêmes octets.

use std::collections::BTreeMap;
use std::io::Write;

use printpdf::lopdf::xref::{XrefEntry, XrefType};
use printpdf::lopdf::{Dictionary, Document, Object, ObjectId, Stream, StringFormat};

/// Le nombre d'objets d'un flux d'objets : un lecteur décompresse le flux entier pour en lire un.
const PAR_FLUX: usize = 400;

/// L'objet `o` écrit en syntaxe PDF.
pub fn ecrire_objet(out: &mut Vec<u8>, o: &Object) {
    match o {
        Object::Null => out.extend_from_slice(b"null"),
        Object::Boolean(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
        Object::Integer(i) => out.extend_from_slice(i.to_string().as_bytes()),
        Object::Real(r) => out.extend_from_slice(reel(*r).as_bytes()),
        Object::Name(n) => nom(out, n),
        Object::String(t, StringFormat::Literal) => litteral(out, t),
        Object::String(t, StringFormat::Hexadecimal) => {
            out.push(b'<');
            for b in t {
                out.extend_from_slice(format!("{:02X}", b).as_bytes());
            }
            out.push(b'>');
        }
        Object::Array(a) => {
            out.push(b'[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 && separe(x) {
                    out.push(b' ');
                }
                ecrire_objet(out, x);
            }
            out.push(b']');
        }
        Object::Dictionary(d) => dictionnaire(out, d),
        Object::Stream(s) => {
            let mut d = s.dict.clone();
            d.set("Length", Object::Integer(s.content.len() as i64));
            dictionnaire(out, &d);
            out.extend_from_slice(b"stream\n");
            out.extend_from_slice(&s.content);
            out.extend_from_slice(b"\nendstream");
        }
        Object::Reference((n, g)) => out.extend_from_slice(format!("{} {} R", n, g).as_bytes()),
    }
}

/// Un réel au plus court : sans zéros ni point inutiles (`1.5`, `2`, `-0.25`).
fn reel(r: f32) -> String {
    if !r.is_finite() {
        return "0".into();
    }
    let s = format!("{}", r);
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

/// Un objet qui doit être séparé de celui qui le précède par un blanc.
fn separe(o: &Object) -> bool {
    matches!(o, Object::Null | Object::Boolean(_) | Object::Integer(_) | Object::Real(_) | Object::Reference(_))
}

fn nom(out: &mut Vec<u8>, n: &[u8]) {
    out.push(b'/');
    for &b in n {
        if b" \t\n\r\x0C()<>[]{}/%#".contains(&b) || !(33..=126).contains(&b) {
            out.extend_from_slice(format!("#{:02X}", b).as_bytes());
        } else {
            out.push(b);
        }
    }
}

fn litteral(out: &mut Vec<u8>, t: &[u8]) {
    out.push(b'(');
    for &b in t {
        match b {
            b'(' | b')' | b'\\' => {
                out.push(b'\\');
                out.push(b);
            }
            b'\r' => out.extend_from_slice(b"\\r"),
            _ => out.push(b),
        }
    }
    out.push(b')');
}

fn dictionnaire(out: &mut Vec<u8>, d: &Dictionary) {
    out.extend_from_slice(b"<<");
    for (k, v) in d.iter() {
        nom(out, k);
        if separe(v) {
            out.push(b' ');
        }
        ecrire_objet(out, v);
    }
    out.extend_from_slice(b">>");
}

fn comprimer(donnees: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    let _ = e.write_all(donnees);
    e.finish().unwrap_or_default()
}

/// Un objet que la table des références ou la structure du fichier ne laissent pas dans un flux d'objets :
/// un flux, un objet de génération non nulle, et ce que l'écriture refait elle-même.
fn hors_flux(id: ObjectId, o: &Object) -> bool {
    id.1 != 0 || matches!(o, Object::Stream(_))
}

/// Ce qu'une écriture refait elle-même et ne recopie pas : les flux d'objets et de références lus.
fn refait(o: &Object) -> bool {
    matches!(o.type_name(), Ok("ObjStm") | Ok("XRef"))
}

/// Les objets `objets` écrits à partir de l'octet `debut` de `out` : les flux et les objets à part un par un,
/// les autres dans des flux d'objets numérotés à partir de `libre`. Rend la table et le prochain numéro libre.
fn ecrire_objets(
    out: &mut Vec<u8>,
    objets: &BTreeMap<ObjectId, Object>,
    mut libre: u32,
    en_flux: bool,
) -> (BTreeMap<u32, XrefEntry>, u32) {
    let mut table = BTreeMap::new();
    let mut groupes: Vec<Vec<(u32, &Object)>> = vec![Vec::new()];
    for (id, o) in objets {
        if refait(o) {
            continue;
        }
        if !en_flux || hors_flux(*id, o) {
            table.insert(id.0, XrefEntry::Normal { offset: out.len() as u32, generation: id.1 });
            out.extend_from_slice(format!("{} {} obj\n", id.0, id.1).as_bytes());
            ecrire_objet(out, o);
            out.extend_from_slice(b"\nendobj\n");
        } else {
            if groupes.last().map_or(0, Vec::len) >= PAR_FLUX {
                groupes.push(Vec::new());
            }
            groupes.last_mut().unwrap().push((id.0, o));
        }
    }
    for groupe in groupes.into_iter().filter(|g| !g.is_empty()) {
        let numero = libre;
        libre += 1;
        let mut tete = Vec::new();
        let mut corps = Vec::new();
        for (i, (n, o)) in groupe.iter().enumerate() {
            if i > 0 {
                tete.push(b' ');
                corps.push(b'\n');
            }
            tete.extend_from_slice(format!("{} {}", n, corps.len()).as_bytes());
            ecrire_objet(&mut corps, o);
            table.insert(*n, XrefEntry::Compressed { container: numero, index: i as u16 });
        }
        // Un blanc après le dernier objet : un lecteur lit chaque objet jusqu'à un délimiteur.
        corps.push(b'\n');
        tete.push(b'\n');
        let premier = tete.len();
        tete.extend_from_slice(&corps);
        let mut d = Dictionary::new();
        d.set("Type", Object::Name(b"ObjStm".to_vec()));
        d.set("N", Object::Integer(groupe.len() as i64));
        d.set("First", Object::Integer(premier as i64));
        d.set("Filter", Object::Name(b"FlateDecode".to_vec()));
        table.insert(numero, XrefEntry::Normal { offset: out.len() as u32, generation: 0 });
        out.extend_from_slice(format!("{} 0 obj\n", numero).as_bytes());
        ecrire_objet(out, &Object::Stream(Stream::new(d, comprimer(&tete))));
        out.extend_from_slice(b"\nendobj\n");
    }
    (table, libre)
}

/// Le nombre d'octets qu'il faut pour écrire `v`.
fn largeur(v: u64) -> usize {
    let mut n = 1;
    while v >> (8 * n) != 0 {
        n += 1;
    }
    n
}

/// Le flux des références (ISO 32000-1, 7.5.8), numéro `numero`, écrit à la fin de `out` : les entrées de
/// `table`, groupées en sections contiguës, et le dictionnaire de queue `queue`.
fn flux_de_references(out: &mut Vec<u8>, mut table: BTreeMap<u32, XrefEntry>, numero: u32, mut queue: Dictionary) {
    let debut = out.len();
    table.insert(numero, XrefEntry::Normal { offset: debut as u32, generation: 0 });
    let champ = |e: &XrefEntry| -> (u8, u64, u64) {
        match e {
            XrefEntry::Normal { offset, generation } => (1, *offset as u64, *generation as u64),
            XrefEntry::Compressed { container, index } => (2, *container as u64, *index as u64),
            XrefEntry::Free => (0, 0, 0),
            XrefEntry::UnusableFree => (0, 0, 65535),
        }
    };
    let w2 = table.values().map(|e| largeur(champ(e).1)).max().unwrap_or(1);
    let w3 = table.values().map(|e| largeur(champ(e).2)).max().unwrap_or(1);
    let colonnes = 1 + w2 + w3;
    let mut sections: Vec<(u32, u32)> = Vec::new();
    let mut lignes: Vec<Vec<u8>> = Vec::new();
    for (n, e) in &table {
        match sections.last_mut() {
            Some((d, c)) if *d + *c == *n => *c += 1,
            _ => sections.push((*n, 1)),
        }
        let (t, a, b) = champ(e);
        let mut ligne = vec![t];
        ligne.extend_from_slice(&a.to_be_bytes()[8 - w2..]);
        ligne.extend_from_slice(&b.to_be_bytes()[8 - w3..]);
        lignes.push(ligne);
    }
    // Le prédicteur PNG « Up » : chaque ligne moins celle d'au-dessus (des numéros qui se suivent).
    let mut donnees = Vec::with_capacity(lignes.len() * (colonnes + 1));
    let zero = vec![0u8; colonnes];
    for (i, l) in lignes.iter().enumerate() {
        let haut = if i == 0 { &zero } else { &lignes[i - 1] };
        donnees.push(2);
        donnees.extend(l.iter().zip(haut.iter()).map(|(a, b)| a.wrapping_sub(*b)));
    }
    queue.set("Type", Object::Name(b"XRef".to_vec()));
    queue.set("Size", Object::Integer(numero as i64 + 1));
    queue.set("W", Object::Array(vec![Object::Integer(1), Object::Integer(w2 as i64), Object::Integer(w3 as i64)]));
    if sections != [(0, numero + 1)] {
        queue.set(
            "Index",
            Object::Array(sections.iter().flat_map(|(d, c)| [Object::Integer(*d as i64), Object::Integer(*c as i64)]).collect()),
        );
    }
    queue.set("Filter", Object::Name(b"FlateDecode".to_vec()));
    let mut parametres = Dictionary::new();
    parametres.set("Predictor", Object::Integer(12));
    parametres.set("Columns", Object::Integer(colonnes as i64));
    queue.set("DecodeParms", Object::Dictionary(parametres));
    out.extend_from_slice(format!("{} 0 obj\n", numero).as_bytes());
    ecrire_objet(out, &Object::Stream(Stream::new(queue, comprimer(&donnees))));
    out.extend_from_slice(format!("\nendobj\nstartxref\n{}\n%%EOF\n", debut).as_bytes());
}

/// Les clés de queue qu'une écriture reprend : la racine, les informations, l'identifiant (et le chiffrement).
fn queue_reprise(trailer: &Dictionary) -> Dictionary {
    let mut q = Dictionary::new();
    for k in [&b"Root"[..], b"Info", b"ID", b"Encrypt"] {
        if let Ok(v) = trailer.get(k) {
            q.set(k.to_vec(), v.clone());
        }
    }
    q
}

/// La version écrite : au moins 1.5 (les flux d'objets).
fn version(v: &str) -> String {
    let n: f32 = v.trim().parse().unwrap_or(1.5);
    if n < 1.5 {
        "1.5".into()
    } else {
        v.trim().to_string()
    }
}

/// Le document `doc` entier, au plus court (voir l'en-tête du module). Un document chiffré s'écrit à
/// l'ancienne (ses chaînes sont chiffrées objet par objet).
pub fn compact(doc: &Document) -> Vec<u8> {
    let chiffre = doc.trailer.get(b"Encrypt").is_ok();
    let mut out = Vec::new();
    out.extend_from_slice(format!("%PDF-{}\n", if chiffre { doc.version.clone() } else { version(&doc.version) }).as_bytes());
    out.extend_from_slice(b"%\xe2\xe3\xcf\xd3\n");
    let plus_grand = doc.objects.keys().map(|k| k.0).max().unwrap_or(0).max(doc.max_id);
    let (table, libre) = ecrire_objets(&mut out, &doc.objects, plus_grand + 1, !chiffre);
    if chiffre {
        table_classique(&mut out, &table, libre, queue_reprise(&doc.trailer), true);
    } else {
        flux_de_references(&mut out, table, libre, queue_reprise(&doc.trailer));
    }
    out
}

/// La table classique (ISO 32000-1, 7.5.4) des entrées `table`, puis la queue ; `entier` : un fichier entier
/// (l'entrée 0 et les numéros libres y figurent).
fn table_classique(out: &mut Vec<u8>, table: &BTreeMap<u32, XrefEntry>, taille: u32, mut queue: Dictionary, entier: bool) {
    let debut = out.len();
    let mut t = table.clone();
    if entier {
        t.insert(0, XrefEntry::UnusableFree);
        for n in 1..taille {
            t.entry(n).or_insert(XrefEntry::Free);
        }
    }
    let mut sections: Vec<(u32, Vec<String>)> = Vec::new();
    for (n, e) in &t {
        let ligne = match e {
            XrefEntry::Normal { offset, generation } => format!("{:010} {:05} n \n", offset, generation),
            XrefEntry::UnusableFree => "0000000000 65535 f \n".to_string(),
            _ => "0000000000 00000 f \n".to_string(),
        };
        match sections.last_mut() {
            Some((d, l)) if *d + l.len() as u32 == *n => l.push(ligne),
            _ => sections.push((*n, vec![ligne])),
        }
    }
    out.extend_from_slice(b"xref\n");
    for (d, l) in sections {
        out.extend_from_slice(format!("{} {}\n", d, l.len()).as_bytes());
        for x in l {
            out.extend_from_slice(x.as_bytes());
        }
    }
    queue.set("Size", Object::Integer(taille as i64));
    out.extend_from_slice(b"trailer\n");
    ecrire_objet(out, &Object::Dictionary(queue));
    out.extend_from_slice(format!("\nstartxref\n{}\n%%EOF\n", debut).as_bytes());
}

/// La mise à jour incrémentale de `avant` (les octets lus, `doc_avant` leur lecture) qui fait de `apres`
/// le document : les objets de `apres` neufs ou changés s'écrivent à la suite des octets d'avant, laissés
/// tels quels. La table suit celle d'avant : un flux si `avant` en a
/// un, une table classique sinon. Rien de changé : `avant` tel quel.
pub fn ajouter(avant: &[u8], doc_avant: &Document, apres: &Document) -> Vec<u8> {
    let mut changes: BTreeMap<ObjectId, Object> = BTreeMap::new();
    let mut a = Vec::new();
    let mut b = Vec::new();
    for (id, o) in &apres.objects {
        if refait(o) {
            continue;
        }
        let pareil = match doc_avant.objects.get(id) {
            Some(x) => {
                a.clear();
                b.clear();
                ecrire_objet(&mut a, x);
                ecrire_objet(&mut b, o);
                a == b
            }
            None => false,
        };
        if !pareil {
            changes.insert(*id, o.clone());
        }
    }
    if changes.is_empty() {
        return avant.to_vec();
    }
    let mut out = avant.to_vec();
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    let plus_grand = apres.objects.keys().map(|k| k.0).max().unwrap_or(0).max(apres.max_id).max(doc_avant.max_id);
    // Les objets de la mise à jour s'écrivent un par un, jamais dans un flux d'objets : un lecteur qui charge tous
    // les flux d'objets du fichier garde parfois la première version d'un objet (lopdf 0.31 le fait), alors qu'un
    // objet écrit à part l'emporte toujours.
    let en_flux = matches!(doc_avant.reference_table.cross_reference_type, XrefType::CrossReferenceStream)
        && doc_avant.trailer.get(b"Encrypt").is_err();
    let (table, libre) = ecrire_objets(&mut out, &changes, plus_grand + 1, false);
    let mut queue = queue_reprise(&apres.trailer);
    queue.set("Prev", Object::Integer(doc_avant.xref_start as i64));
    if en_flux {
        flux_de_references(&mut out, table, libre, queue);
    } else {
        table_classique(&mut out, &table, libre, queue, false);
    }
    out
}
