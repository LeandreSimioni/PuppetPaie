//! La lecture des PDF existants : le nombre de pages (`--pages`) et le texte (`--texte`), et le
//! chargement commun à tous les modes qui relisent un PDF (`charger`).

use std::collections::BTreeMap;

use lopdf::Document;

use crate::description::Refus;

/// Lit un PDF en mémoire. Un fichier dont la table de références est fausse (« Microsoft Print To
/// PDF » écrit des positions décalées d'un octet, que lopdf ne retrouve pas) est réparé : on
/// reconstruit la table en cherchant les objets (`N G obj`) dans le fichier, comme font les
/// lecteurs de PDF.
pub fn charger(octets: &[u8]) -> Result<Document, Refus> {
    match Document::load_mem(octets) {
        Ok(doc) if !doc.get_pages().is_empty() => Ok(doc),
        premier => {
            if let Some(repare) = reparer(octets) {
                if let Ok(doc) = Document::load_mem(&repare) {
                    if !doc.get_pages().is_empty() {
                        return Ok(doc);
                    }
                }
            }
            match premier {
                Ok(doc) => Ok(doc),
                Err(e) => Err(Refus(format!("n'est pas un PDF lisible ({})", e))),
            }
        }
    }
}

/// Les nombres décimaux d'un début de ligne : `(n, g)` de « n g obj », et la position du mot `obj`.
fn debut_d_objet(b: &[u8], i: usize) -> Option<(u32, u16)> {
    let chiffres = |mut j: usize| {
        let debut = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j == debut || j - debut > 10 {
            return None;
        }
        let n: u64 = std::str::from_utf8(&b[debut..j]).ok()?.parse().ok()?;
        Some((n, j))
    };
    let (n, j) = chiffres(i)?;
    if b.get(j) != Some(&b' ') {
        return None;
    }
    let (g, k) = chiffres(j + 1)?;
    if b.get(k) != Some(&b' ') || b.get(k + 1..k + 4) != Some(b"obj") {
        return None;
    }
    // « obj » est suivi d'un blanc ou d'un délimiteur (et non de « ect » d'un autre mot).
    match b.get(k + 4) {
        Some(c) if c.is_ascii_alphanumeric() => None,
        _ => Some((u32::try_from(n).ok()?, u16::try_from(g).ok()?)),
    }
}

/// Le fichier avec une table de références neuve, ajoutée à la fin, bâtie sur la position réelle
/// des objets ; `None` quand il n'y a pas de queue classique (`trailer`) à reprendre.
fn reparer(b: &[u8]) -> Option<Vec<u8>> {
    let mut objets: BTreeMap<u32, (usize, u16)> = BTreeMap::new();
    let mut i = 0;
    while i < b.len() {
        if i == 0 || b[i - 1] == b'\n' || b[i - 1] == b'\r' {
            if let Some((n, g)) = debut_d_objet(b, i) {
                objets.insert(n, (i, g));
            }
        }
        i += 1;
    }
    if objets.is_empty() {
        return None;
    }
    // La dernière queue du fichier : son dictionnaire, jusqu'à `startxref`.
    let t = b.windows(7).rposition(|w| w == b"trailer")?;
    let fin = b[t..].windows(9).position(|w| w == b"startxref").map(|p| t + p).unwrap_or(b.len());
    let queue = &b[t..fin];
    let racine = queue.windows(5).any(|w| w == b"/Root");
    if !racine {
        return None;
    }
    let plus_grand = *objets.keys().last()?;
    let mut sortie = b.to_vec();
    sortie.push(b'\n');
    let debut_table = sortie.len();
    sortie.extend_from_slice(format!("xref\n0 {}\n", plus_grand + 1).as_bytes());
    sortie.extend_from_slice(b"0000000000 65535 f \n");
    for n in 1..=plus_grand {
        match objets.get(&n) {
            Some((pos, g)) => sortie.extend_from_slice(format!("{:010} {:05} n \n", pos, g).as_bytes()),
            None => sortie.extend_from_slice(b"0000000000 65535 f \n"),
        }
    }
    sortie.extend_from_slice(queue);
    sortie.extend_from_slice(format!("\nstartxref\n{}\n%%EOF\n", debut_table).as_bytes());
    Some(sortie)
}

/// Ouvre un PDF. Un fichier chiffré s'ouvre avec le mot de passe vide quand c'est possible
/// (`dechiffrer` : `true`) ; sans le déchiffrer, le nombre de pages se lit quand même.
pub fn ouvrir(chemin: &str, dechiffrer: bool) -> Result<Document, Refus> {
    let octets = std::fs::read(chemin).map_err(|e| Refus(format!("{} illisible ({})", chemin, e)))?;
    let mut doc = charger(&octets).map_err(|r| Refus(format!("{} {}", chemin, r.0)))?;
    if dechiffrer {
        crate::chiffrement::dechiffrer(&mut doc, b"").map_err(|r| Refus(format!("{} : {}", chemin, r.0)))?;
    }
    Ok(doc)
}

/// Le nombre de pages de chaque fichier, dans l'ordre.
pub fn pages(chemins: &[String]) -> Result<Vec<usize>, Refus> {
    chemins.iter().map(|c| Ok(ouvrir(c, false)?.get_pages().len())).collect()
}

/// Le texte de chaque page de `chemin`, et les avertissements (pages illisibles).
pub fn texte(chemin: &str) -> Result<(Vec<String>, Vec<String>), Refus> {
    let doc = ouvrir(chemin, true)?;
    Ok(crate::texte::texte_des_pages(&doc))
}
