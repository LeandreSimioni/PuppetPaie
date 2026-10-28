//! La veille des conventions collectives par les livraisons open data KALI de la DILA (`moteur/kali.py`, retire au lot K,
//! porte au lot S1). Un outil de l'EDITEUR (`moteur sources --verifier`), jamais d'une compagnie.
//!
//! Legifrance refuse les robots (403). Mais la DILA publie chaque soir, en open data et sans compte, ce qui a change dans
//! la base KALI (les conventions collectives) : une archive de quelques centaines de Ko par jour, a
//! `https://echanges.dila.gouv.fr/OPENDATA/KALI/` (`KALI_AAAAMMJJ-HHMMSS.tar.gz`). L'index garde environ un an de
//! livraisons. La base entiere existe aussi (180 Mo) : on ne la prend jamais, on n'en a pas besoin.
//!
//! Dans une livraison, deux choses nous interessent :
//!   - `conteneur/.../KALICONT*.xml` : la fiche d'une convention (IDCC, titre, et la liste de TOUS ses textes attaches,
//!     avec leur titre). Elle n'est livree que le jour ou la convention bouge. C'est le premier signal.
//!   - `texte/version/.../KALITEXT*.xml` : un texte modifie ce jour-la (un avenant nouveau, un arrete d'extension qui
//!     change son etat). Le texte ne dit pas a quelle convention il appartient : on le sait par la liste du conteneur,
//!     vue dans cette livraison ou une precedente, ou parce que le manifeste des sources le cite deja.
//!
//! La liste des textes d'une convention ne vient que de sa fiche : a la premiere veille on remonte l'index a rebours
//! jusqu'a la trouver (une fois), pour savoir ensuite rattacher un texte modifie a sa convention.
//!
//! La lecture est incrementale : chaque livraison lue est resumee une fois dans `atelier/reference/sources/veille-kali.json` (ce
//! qui touche NOS conventions, rien d'autre), et n'est jamais retelechargee. Le rapport est recalcule a chaque fois
//! depuis la date de releve de la source : un signal vu hier ne disparait pas parce qu'on a relance la commande. Le
//! fichier est celui qu'ecrivait le Python (memes cles) : la veille reprend ou il s'etait arrete.
//!
//! Ce module ne decide rien : il rend ce qui a bouge, `sources::verifier` en fait un etat, la commande l'affiche.

use std::path::Path;

use serde_json::{json, Map, Value};

pub const INDEX: &str = "https://echanges.dila.gouv.fr/OPENDATA/KALI/";
pub const CACHE: &str = "veille-kali.json";
/// Un texte connu par le manifeste, pas par une fiche.
pub const CITE: &str = "(cite par sources.json)";

type R<T> = Result<T, String>;

/// Ce qu'on telecharge : une adresse, ses octets (ou la panne, en une phrase).
pub type Telecharger<'a> = dyn FnMut(&str) -> R<Vec<u8>> + 'a;

/// Les estampilles `AAAAMMJJ-HHMMSS` de l'index, sans doublon, dans l'ordre (`entrepot_dila::livraisons`, que l'entrepot KALI
/// lit aussi).
pub fn livraisons(index_html: &str) -> Vec<String> {
    crate::entrepot_dila::livraisons(index_html, "KALI")
}

/// `AAAAMMJJ...` -> `AAAA-MM-JJ`.
pub fn jour(livraison: &str) -> String {
    let c = |a: usize, b: usize| livraison.get(a..b).unwrap_or("");
    format!("{}-{}-{}", c(0, 4), c(4, 6), c(6, 8))
}

// ------------------------------------------------------------------ l'archive

/// Les fichiers d'une archive `.tar.gz` : (nom, octets). Le format tar se lit dans `entrepot_dila::LecteurTar`, sans
/// dependance (le meme lecteur fait defiler l'archive globale de LEGI en flux).
pub fn fichiers_tar_gz(octets: &[u8]) -> R<Vec<(String, Vec<u8>)>> {
    let mut tar = crate::entrepot_dila::archive(octets);
    let mut out = Vec::new();
    while let Some(m) = tar.suivant()? {
        if m.fichier {
            out.push((m.nom, tar.lire()?));
        }
    }
    Ok(out)
}

fn texte_de(n: roxmltree::Node, chemin: &[&str]) -> String {
    let mut courant = n;
    for nom in chemin {
        match courant.children().find(|c| c.is_element() && c.tag_name().name() == *nom) {
            Some(c) => courant = c,
            None => return String::new(),
        }
    }
    courant.text().unwrap_or("").trim().to_string()
}

/// Ce qu'une livraison contient : les conteneurs (par identifiant : `idcc`, `titre`, `etat`, `textes` attaches) et les
/// textes modifies (par identifiant : `titre`, `date_texte`, `modifie_le`, `etat`, `extension`, et `_xml`, le nom du
/// fichier dans l'archive). Tout, sans filtrer : c'est l'appelant qui sait ce qu'il suit.
pub fn lire_livraison(octets: &[u8]) -> R<(Map<String, Value>, Map<String, Value>)> {
    let mut conteneurs = Map::new();
    let mut textes = Map::new();
    for (nom, xml) in fichiers_tar_gz(octets)? {
        if !nom.ends_with(".xml") || !(nom.contains("/conteneur/") || nom.contains("/texte/version/")) {
            continue;
        }
        let t = String::from_utf8(xml).map_err(|_| format!("{} : pas de l'UTF-8", nom))?;
        let doc = roxmltree::Document::parse(&t).map_err(|e| format!("{} illisible : {}", nom, e))?;
        let racine = doc.root_element();
        let ident = texte_de(racine, &["META", "META_COMMUN", "ID"]);
        match racine.tag_name().name() {
            "IDCC" => {
                let meta = ["META", "META_SPEC", "META_CONTENEUR"];
                let champ = |c: &str| texte_de(racine, &[meta[0], meta[1], meta[2], c]);
                let mut liens = Map::new();
                for l in racine.descendants().filter(|n| n.has_tag_name("LIEN_TXT")) {
                    if let Some(id) = l.attribute("idtxt").filter(|s| !s.is_empty()) {
                        liens.insert(id.to_string(), json!(l.attribute("titretxt").unwrap_or("").trim()));
                    }
                }
                conteneurs.insert(ident, json!({"idcc": champ("NUM"), "titre": champ("TITRE"), "etat": champ("ETAT"), "textes": liens}));
            }
            "TEXTE_VERSION" => {
                let chron = |c: &str| texte_de(racine, &["META", "META_SPEC", "META_TEXTE_CHRONICLE", c]);
                let vers = |c: &str| texte_de(racine, &["META", "META_SPEC", "META_TEXTE_VERSION", c]);
                let titre = Some(vers("TITRE")).filter(|t| !t.is_empty()).unwrap_or_else(|| vers("TITREFULL"));
                textes.insert(
                    ident,
                    json!({
                        "titre": titre, "date_texte": chron("DATE_TEXTE"), "modifie_le": chron("DERNIERE_MODIFICATION"),
                        "etat": vers("ETAT"), "extension": vers("LIBELLE_EXTENSION"), "_xml": nom,
                    }),
                );
            }
            _ => {}
        }
    }
    Ok((conteneurs, textes))
}

// ------------------------------------------------------------------ la veille

fn charger_cache(p: &Path) -> R<Map<String, Value>> {
    if p.exists() {
        let t = std::fs::read_to_string(p).map_err(|e| format!("{} : {}", p.display(), e))?;
        return match serde_json::from_str::<Value>(&t) {
            Ok(Value::Object(m)) => Ok(m),
            Ok(_) => Err(format!("{} n'est pas un objet JSON : le deplacer force la relecture", p.display())),
            Err(e) => Err(format!("{} illisible ({}) : le deplacer force la relecture", p.display(), e)),
        };
    }
    let mut m = Map::new();
    m.insert(
        "_quoi".into(),
        json!("Les livraisons open data KALI deja lues, resumees a ce qui touche les conventions suivies dans sources.json. Une livraison lue n'est jamais retelechargee ; supprimer ce fichier force la relecture."),
    );
    m.insert("livraisons".into(), json!({}));
    m.insert("textes_connus".into(), json!({}));
    Ok(m)
}

/// `setdefault(cle, {})` sur un objet, qui doit en etre un.
fn objet<'a>(m: &'a mut Map<String, Value>, cle: &str, defaut: Value) -> R<&'a mut Value> {
    let v = m.entry(cle.to_string()).or_insert(defaut);
    if v.is_null() {
        *v = json!({});
    }
    Ok(v)
}

fn tous_cites(connus: &Value) -> bool {
    connus.as_object().is_none_or(|m| m.values().all(|v| v.as_str() == Some(CITE)))
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

/// Une convention suivie : son conteneur, son IDCC, les textes que le manifeste cite.
pub struct Suivi {
    pub conteneur: String,
    pub idcc: String,
    pub textes: Vec<String>,
}

/// Retourne voir ce qui a bouge, pour chaque convention suivie, depuis une date (`depuis`, ISO : la releve la plus
/// ancienne ; les livraisons de ce jour-la et d'avant ne comptent pas). `relever` : poser le XML des textes touches dans
/// `dossier/kali/`.
///
/// Rend `{"livraisons_lues", "nouvelles", "trou", "amorces", "evenements": {conteneur: [...]}}`. Une panne de l'index ou
/// d'une archive se rend en `Err` : c'est a l'appelant de dire « injoignable » ; rien n'est alors ecrit.
pub fn veiller(dossier: &Path, suivis: &[Suivi], depuis: &str, relever: bool, telecharger: &mut Telecharger) -> R<Value> {
    let cache_p = dossier.join(CACHE);
    let mut cache = charger_cache(&cache_p)?;
    {
        let connus = objet(&mut cache, "textes_connus", json!({}))?;
        let connus = connus.as_object_mut().ok_or("veille-kali.json : « textes_connus » n'est pas un objet")?;
        for s in suivis {
            let c = connus.entry(s.conteneur.clone()).or_insert_with(|| json!({}));
            let c = c.as_object_mut().ok_or("veille-kali.json : une convention de « textes_connus » n'est pas un objet")?;
            for t in &s.textes {
                c.entry(t.clone()).or_insert_with(|| json!(CITE));
            }
        }
    }

    let seuil = format!("{}-999999", depuis.replace('-', ""));
    let index = String::from_utf8_lossy(&telecharger(INDEX)?).into_owned();
    let toutes = livraisons(&index);
    let (Some(premiere), Some(derniere)) = (toutes.first().cloned(), toutes.last().cloned()) else {
        return Err("l'index KALI ne liste aucune livraison : la page a change ?".into());
    };
    let a_lire: Vec<&String> = toutes.iter().filter(|l| **l > seuil).collect();

    // Ce qu'on a couvert sans trou : de la premiere veille a la derniere livraison vue. Si l'index ne remonte plus
    // jusque-la, des livraisons sont sorties sans avoir ete lues -- on le dit, et on s'en souvient.
    let s8 = |t: &str| t.get(..8).unwrap_or(t).to_string();
    let couv = cache.get("couverture").filter(|c| !c.is_null()).cloned();
    let mut trous: Vec<Value> = cache.get("trous").and_then(Value::as_array).cloned().unwrap_or_default();
    match couv {
        None => {
            if premiere > seuil {
                trous.push(json!({"de": s8(&seuil), "a": s8(&premiere)}));
            }
            cache.insert("couverture".into(), json!({"de": std::cmp::min(&seuil, &premiere), "a": derniere}));
        }
        Some(c) if premiere.as_str() > s(&c["a"]) => {
            trous.push(json!({"de": s8(s(&c["a"])), "a": s8(&premiere)}));
            cache.insert("couverture".into(), json!({"de": c["de"], "a": derniere}));
        }
        Some(mut c) => {
            let a = std::cmp::max(s(&c["a"]).to_string(), derniere.clone());
            c["a"] = json!(a);
            cache.insert("couverture".into(), c);
        }
    }
    let mut trou = Value::Null;
    for t in &trous {
        if s(&t["a"]) > s8(&seuil).as_str() {
            trou = json!(format!(
                "des livraisons entre le {} et le {} sont sorties de l'index sans avoir ete lues : verifier la page a la main pour cette periode",
                jour(s(&t["de"])),
                jour(s(&t["a"]))
            ));
        }
    }
    cache.insert("trous".into(), Value::Array(trous));

    // La liste de reference d'une convention (ses textes attaches) ne vient que d'une fiche livree. Tant qu'on n'en a pas
    // vu, on ne saurait pas rattacher un texte modifie a la convention -- sauf ceux que le manifeste cite. Alors, une
    // fois, on remonte l'index a rebours jusqu'a trouver la derniere fiche de chaque convention suivie.
    let mut a_amorcer: Vec<&str> =
        suivis.iter().map(|s| s.conteneur.as_str()).filter(|c| tous_cites(&cache["textes_connus"][*c])).collect();
    a_amorcer.dedup();
    for l in toutes.iter().filter(|l| **l <= seuil).rev() {
        if a_amorcer.is_empty() {
            break;
        }
        let (conteneurs, _) = lire_livraison(&telecharger(&format!("{}KALI_{}.tar.gz", INDEX, l))?)?;
        for cont in a_amorcer.clone() {
            if let Some(c) = conteneurs.get(cont) {
                if let (Some(connus), Some(liens)) = (cache["textes_connus"][cont].as_object_mut(), c["textes"].as_object()) {
                    for (k, v) in liens {
                        connus.insert(k.clone(), v.clone());
                    }
                }
                let n = c["textes"].as_object().map_or(0, Map::len);
                objet(&mut cache, "amorces", json!({}))?[cont] = json!({"livraison": jour(l), "textes": n, "titre": c["titre"]});
                a_amorcer.retain(|x| *x != cont);
            }
        }
    }

    let mut nouvelles = Vec::new();
    for l in a_lire {
        if cache.get("livraisons").and_then(|x| x.get(l.as_str())).is_some() {
            continue;
        }
        let octets = telecharger(&format!("{}KALI_{}.tar.gz", INDEX, l))?;
        let (conteneurs, textes) = lire_livraison(&octets)?;
        let mut resume_c = Map::new();
        let mut resume_t = Map::new();
        for suivi in suivis {
            let cont = suivi.conteneur.as_str();
            if let Some(c) = conteneurs.get(cont) {
                // La premiere fiche vue donne la liste de reference : rien n'y est « nouveau », on ne connaissait pas la
                // liste d'avant.
                let premiere = tous_cites(&cache["textes_connus"][cont]);
                let anciens = cache["textes_connus"][cont].as_object().cloned().unwrap_or_default();
                let liens = c["textes"].as_object().cloned().unwrap_or_default();
                let nouveaux: Map<String, Value> =
                    if premiere { Map::new() } else { liens.iter().filter(|(k, _)| !anciens.contains_key(*k)).map(|(k, v)| (k.clone(), v.clone())).collect() };
                resume_c.insert(cont.to_string(), json!({"idcc": c["idcc"], "etat": c["etat"], "premiere": premiere, "nouveaux": nouveaux}));
                if let Some(connus) = cache["textes_connus"][cont].as_object_mut() {
                    for (k, v) in liens {
                        connus.insert(k, v);
                    }
                }
            }
        }
        for (ident, t) in &textes {
            for suivi in suivis {
                let cont = suivi.conteneur.as_str();
                if cache["textes_connus"][cont].get(ident).is_some() {
                    let mut t = t.clone();
                    t["conteneur"] = json!(cont);
                    if relever {
                        poser(dossier, l, ident, s(&t["_xml"]), &octets)?;
                    }
                    resume_t.insert(ident.clone(), t);
                }
            }
        }
        for t in resume_t.values_mut() {
            if let Some(o) = t.as_object_mut() {
                o.shift_remove("_xml");
            }
        }
        objet(&mut cache, "livraisons", json!({}))?[l.as_str()] = json!({"conteneurs": resume_c, "textes": resume_t});
        nouvelles.push(l.clone());
    }

    std::fs::create_dir_all(dossier).map_err(|e| format!("{} : {}", dossier.display(), e))?;
    let cache = Value::Object(cache);
    socle::ecriture::ecrire_json(&cache_p, &cache, 1)?;

    let mut evenements = Map::new();
    for suivi in suivis {
        evenements.entry(suivi.conteneur.clone()).or_insert_with(|| json!([]));
    }
    let mut lues: Vec<&String> = cache["livraisons"].as_object().map(|m| m.keys().collect()).unwrap_or_default();
    lues.sort();
    for l in lues {
        if *l <= seuil {
            continue;
        }
        let r = &cache["livraisons"][l.as_str()];
        for (cont, c) in r["conteneurs"].as_object().into_iter().flatten() {
            // Une fiche relivree sans texte nouveau n'apprend rien de plus que les textes modifies le meme jour : on ne la
            // rapporte pas.
            let nouveaux = c["nouveaux"].as_object().is_some_and(|m| !m.is_empty());
            let premiere = c["premiere"].as_bool() == Some(true);
            if let Some(Value::Array(ev)) = evenements.get_mut(cont) {
                if nouveaux || premiere {
                    let n = cache["textes_connus"][cont.as_str()].as_object().map_or(0, Map::len);
                    ev.push(json!({
                        "livraison": jour(l), "genre": "convention", "ident": cont,
                        "titre": if premiere {
                            format!("premiere fiche de la convention vue : {} texte(s) attache(s), liste de reference posee", n)
                        } else {
                            format!("fiche de la convention relivree (etat {})", s(&c["etat"]))
                        },
                        "premiere": premiere, "nouveaux": c["nouveaux"],
                    }));
                }
            }
        }
        for (ident, t) in r["textes"].as_object().into_iter().flatten() {
            if let Some(Value::Array(ev)) = evenements.get_mut(s(&t["conteneur"])) {
                let mut e = t.clone();
                e["livraison"] = json!(jour(l));
                e["genre"] = json!("texte");
                e["ident"] = json!(ident);
                ev.push(e);
            }
        }
    }
    Ok(json!({
        "livraisons_lues": toutes.iter().filter(|l| **l > seuil).map(|l| jour(l)).collect::<Vec<_>>(),
        "nouvelles": nouvelles.iter().map(|l| jour(l)).collect::<Vec<_>>(),
        "trou": trou,
        "amorces": cache.get("amorces").cloned().unwrap_or_else(|| json!({})),
        "evenements": evenements,
    }))
}

/// Le XML d'un texte touche, garde a cote des sources : on le relira, et on ne le perd pas quand la livraison sort de
/// l'index.
fn poser(dossier: &Path, livraison: &str, ident: &str, membre: &str, octets: &[u8]) -> R<()> {
    let Some((_, xml)) = fichiers_tar_gz(octets)?.into_iter().find(|(n, _)| n == membre) else {
        return Err(format!("{} absent de la livraison {}", membre, livraison));
    };
    let cible = dossier.join("kali").join(format!("{}-{}.xml", ident, jour(livraison)));
    std::fs::create_dir_all(cible.parent().unwrap_or(dossier)).map_err(|e| e.to_string())?;
    std::fs::write(&cible, xml).map_err(|e| format!("{} : {}", cible.display(), e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Une archive `.tar.gz` minimale (ustar), pour les essais : (nom, contenu).
    pub(crate) fn archive(fichiers: &[(&str, &str)]) -> Vec<u8> {
        let mut tar = Vec::new();
        for (nom, contenu) in fichiers {
            let mut h = [0u8; 512];
            h[..nom.len()].copy_from_slice(nom.as_bytes());
            h[100..107].copy_from_slice(b"0000644");
            h[124..135].copy_from_slice(format!("{:011o}", contenu.len()).as_bytes());
            h[156] = b'0';
            h[257..263].copy_from_slice(b"ustar\0");
            h[148..156].copy_from_slice(b"        ");
            let somme: u32 = h.iter().map(|&c| c as u32).sum();
            h[148..155].copy_from_slice(format!("{:06o}\0", somme).as_bytes());
            tar.extend_from_slice(&h);
            tar.extend_from_slice(contenu.as_bytes());
            tar.resize(tar.len().div_ceil(512) * 512, 0);
        }
        tar.extend_from_slice(&[0u8; 1024]);
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(&tar).unwrap();
        gz.finish().unwrap()
    }

    pub(crate) fn conteneur(id: &str, idcc: &str, textes: &[(&str, &str)]) -> String {
        let liens: String = textes.iter().map(|(i, t)| format!(r#"<LIEN_TXT idtxt="{}" titretxt="{}"/>"#, i, t)).collect();
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<IDCC><META><META_COMMUN><ID>{}</ID></META_COMMUN><META_SPEC><META_CONTENEUR><TITRE>Convention {}</TITRE><ETAT>VIGUEUR_ETEN</ETAT><NUM>{}</NUM></META_CONTENEUR></META_SPEC></META><STRUCTURE_TXT><TM niv=\"1\">{}</TM></STRUCTURE_TXT></IDCC>",
            id, idcc, idcc, liens
        )
    }

    pub(crate) fn texte(id: &str, titre: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<TEXTE_VERSION><META><META_COMMUN><ID>{}</ID></META_COMMUN><META_SPEC><META_TEXTE_CHRONICLE><DATE_TEXTE>2026-09-01</DATE_TEXTE><DERNIERE_MODIFICATION>2026-10-01</DERNIERE_MODIFICATION></META_TEXTE_CHRONICLE><META_TEXTE_VERSION><TITRE>{}</TITRE><ETAT>VIGUEUR_NON_ETEN</ETAT><LIBELLE_EXTENSION/></META_TEXTE_VERSION></META_SPEC></META></TEXTE_VERSION>",
            id, titre
        )
    }

    #[test]
    fn l_index_donne_les_livraisons_dans_l_ordre_sans_doublon() {
        let index = r#"<a href="KALI_20260102-210000.tar.gz">x</a> <a href="Freemium_kali_global_20250713-140000.tar.gz">y</a>
            <a href="KALI_20260101-210000.tar.gz">z</a> <a href="KALI_20260102-210000.tar.gz">x</a>"#;
        assert_eq!(livraisons(index), ["20260101-210000", "20260102-210000"]);
        assert_eq!(jour("20260102-210000"), "2026-01-02");
    }

    #[test]
    fn une_livraison_se_lit_conteneurs_et_textes_les_autres_fichiers_ignores() {
        let a = archive(&[
            ("20261001-210000/kali/global/conteneur/KALI/CONT/KALICONT1.xml", &conteneur("KALICONT1", "3090", &[("KALITEXT1", "Salaires 2026")])),
            ("20261001-210000/kali/global/texte/version/KALI/TEXT/KALITEXT9.xml", &texte("KALITEXT9", "Avenant formation")),
            ("20261001-210000/kali/global/texte/struct/KALI/TEXT/KALITEXT9.xml", "<pas lu/>"),
            ("20261001-210000/liste_suppression_kali.dat", "rien"),
        ]);
        let (c, t) = lire_livraison(&a).unwrap();
        assert_eq!(c["KALICONT1"], json!({"idcc": "3090", "titre": "Convention 3090", "etat": "VIGUEUR_ETEN", "textes": {"KALITEXT1": "Salaires 2026"}}));
        assert_eq!(t.len(), 1);
        assert_eq!((t["KALITEXT9"]["titre"].clone(), t["KALITEXT9"]["etat"].clone()), (json!("Avenant formation"), json!("VIGUEUR_NON_ETEN")));
    }

    /// La premiere veille remonte l'index jusqu'a la fiche de la convention (l'amorce), puis lit ce qui suit le releve ; un
    /// texte attache a la convention apres la fiche de reference est « nouveau », et le cache garde ce qui a ete lu.
    #[test]
    fn la_veille_amorce_la_liste_puis_rapporte_les_mouvements_et_ne_relit_rien() {
        let d = std::env::temp_dir().join(format!("paie-kali-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let avant = archive(&[("a/conteneur/KALICONT1.xml", &conteneur("KALICONT1", "3090", &[("KALITEXT1", "Salaires 2025")]))]);
        let apres = archive(&[
            ("b/conteneur/KALICONT1.xml", &conteneur("KALICONT1", "3090", &[("KALITEXT1", "Salaires 2025"), ("KALITEXT2", "Salaires 2026")])),
            ("b/texte/version/KALITEXT2.xml", &texte("KALITEXT2", "Salaires minimaux 2026")),
            ("b/texte/version/KALITEXT8.xml", &texte("KALITEXT8", "Un texte d'une autre convention")),
        ]);
        let index = r#"href="KALI_20260901-210000.tar.gz" href="KALI_20261001-210000.tar.gz""#;
        let vus = std::cell::RefCell::new(Vec::new());
        let mut tele = |u: &str| -> R<Vec<u8>> {
            vus.borrow_mut().push(u.to_string());
            Ok(if u == INDEX { index.as_bytes().to_vec() } else if u.contains("20260901") { avant.clone() } else { apres.clone() })
        };
        let suivis = [Suivi { conteneur: "KALICONT1".into(), idcc: "3090".into(), textes: vec!["KALITEXT1".into()] }];
        let r = veiller(&d, &suivis, "2026-09-15", true, &mut tele).unwrap();
        assert_eq!(r["livraisons_lues"], json!(["2026-10-01"]));
        assert_eq!(r["amorces"]["KALICONT1"]["livraison"], json!("2026-09-01"));
        let ev = r["evenements"]["KALICONT1"].as_array().unwrap();
        assert_eq!(ev[0]["nouveaux"], json!({"KALITEXT2": "Salaires 2026"}));
        assert_eq!((ev[1]["ident"].clone(), ev[1]["genre"].clone()), (json!("KALITEXT2"), json!("texte")));
        assert_eq!(ev.len(), 2, "le texte d'une autre convention n'est pas rapporte : {:?}", ev);
        assert!(d.join("kali/KALITEXT2-2026-10-01.xml").is_file(), "--relever pose le XML du texte touche");
        // Une seconde veille : la livraison est dans le cache, seul l'index est relu ; le rapport est le meme.
        vus.borrow_mut().clear();
        let r2 = veiller(&d, &suivis, "2026-09-15", false, &mut tele).unwrap();
        assert_eq!(*vus.borrow(), [INDEX]);
        assert_eq!(r2["evenements"], r["evenements"]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn un_index_vide_est_une_panne_et_rien_n_est_ecrit() {
        let d = std::env::temp_dir().join(format!("paie-kali-vide-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let mut tele = |_: &str| -> R<Vec<u8>> { Ok(b"<html></html>".to_vec()) };
        let suivis = [Suivi { conteneur: "KALICONT1".into(), idcc: "3090".into(), textes: vec![] }];
        assert!(veiller(&d, &suivis, "2026-09-15", false, &mut tele).unwrap_err().contains("aucune livraison"));
        assert!(!d.join(CACHE).exists());
    }
}
