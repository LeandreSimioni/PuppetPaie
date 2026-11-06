//! LA LECTURE D'UNE FORME : une donnee lue (un fichier, un bareme, la charge d'une route) est une structure derivee
//! (`serde::Deserialize`), dont chaque type porte `#[serde(deny_unknown_fields)]` (garde par
//! `moteur/tests/fichiers/structure.rs`) : une cle que la forme ne connait pas est refusee, sans exception. `lire` est la
//! seule fonction qui la lit ; son refus est une phrase du moteur, la cle fautive (son chemin dans la donnee) et ce qui ne
//! va pas a part (`echec::APart`).
//!
//! C'est un des trois modules du BORD (avec `ecriture` et `json_ascii`, qui ecrivent) : le seul endroit d'une couche
//! fermee ou le JSON se manipule sans forme (`moteur/tests/fichiers/structure.rs`, `une_couche_fermee_ne_lit_que_des_formes`).

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use crate::echec::APart;
use crate::num::Resultat;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// La donnee `v` lue en `T`. `phrase` est celle du refus, pour l'ecran (« Le barème livré ne se lit pas : le signaler au
/// développeur. ») ; le chemin de la cle et le detail vont a part.
pub fn lire<T: DeserializeOwned>(v: &Value, phrase: &str) -> Resultat<T> {
    serde_path_to_error::deserialize(v).map_err(|e| refus(phrase, None, &e))
}

/// Le texte JSON `texte` lu en `T` (`lire`, sans passer par une `Value`).
pub fn lire_texte<T: DeserializeOwned>(texte: &str, phrase: &str) -> Resultat<T> {
    let mut d = serde_json::Deserializer::from_str(texte);
    serde_path_to_error::deserialize(&mut d).map_err(|e| refus(phrase, None, &e))
}

/// Le fichier JSON `fichier` lu en `T` (`lire_texte`, sa lecture notee au registre d'un calcul suivi :
/// `fichiers::lire_texte`). Le disque qui refuse se dit par `echec::du_disque` ; une forme fausse par `phrase`, le
/// chemin du fichier, la cle et le detail a part.
pub fn lire_fichier<T: DeserializeOwned>(fichier: &Path, phrase: &str) -> Resultat<T> {
    let texte = crate::fichiers::lire_texte(fichier).map_err(|e| crate::echec::du_disque(fichier, &e))?;
    let mut d = serde_json::Deserializer::from_str(&texte);
    serde_path_to_error::deserialize(&mut d).map_err(|e| refus(phrase, Some(fichier), &e))
}

/// La longueur, en octets, de l'objet JSON qui ouvre `texte` (ce qu'un refus porte a part, `echec::avec`), s'il en
/// ouvre un.
pub fn objet_en_tete(texte: &str) -> Option<usize> {
    let mut lu = serde_json::Deserializer::from_str(texte).into_iter::<serde::de::IgnoredAny>();
    match (texte.starts_with('{'), lu.next()) {
        (true, Some(Ok(_))) => Some(lu.byte_offset()),
        _ => None,
    }
}

fn refus(phrase: &str, fichier: Option<&Path>, e: &serde_path_to_error::Error<serde_json::Error>) -> String {
    let cle = e.path().to_string();
    let detail = en_clair(&e.inner().to_string());
    crate::echec::avec(
        phrase,
        APart {
            chemin: fichier.map(|f| f.to_string_lossy().into_owned()),
            cle: Some(if cle == "." { String::new() } else { cle }),
            detail: Some(detail).filter(|d| !d.is_empty()),
            ..APart::default()
        },
    )
}

/// Le message de serde, dit en francais pour ce qu'il a de courant ; le reste tel quel.
fn en_clair(m: &str) -> String {
    let m = match m.split_once(" at line ") {
        Some((avant, _)) => avant,
        None => m,
    };
    if let Some(r) = m.strip_prefix("unknown field ") {
        let cle = match r.split_once(',') {
            Some((cle, _)) => cle,
            None => r,
        };
        return match r.split_once("expected ") {
            Some((_, a)) => format!("la clé {} n'est pas connue ici (attendues : {})", cle, a.replace("one of ", "")),
            None => format!("la clé {} n'est pas connue ici : aucune n'est attendue", cle),
        };
    }
    if let Some(r) = m.strip_prefix("missing field ") {
        return format!("la clé {} manque", r);
    }
    if let Some(r) = m.strip_prefix("invalid type: ") {
        return format!("la valeur n'a pas la forme attendue : {}", r.replace(", expected ", ", attendu "));
    }
    if let Some(r) = m.strip_prefix("unknown variant ") {
        return format!("la valeur {} n'est pas connue ici", r.replace(", expected one of ", " (attendues : ").replace(", expected ", " (attendue : ").replace("` or `", "` ou `") + ")");
    }
    m.to_string()
}

/// UN MONTANT OU UN TAUX EN DECIMAL : un entier garde son echelle nulle, un flottant sa representation la plus courte
/// (`1.0` reste `1.0`, `num::dec_f64`), un nombre ecrit en texte se lit (`num::dec_texte`).
/// `#[serde(deserialize_with = "socle::forme::decimal")]`.
pub fn decimal<'de, D: serde::Deserializer<'de>>(d: D) -> Result<crate::num::Decimal, D::Error> {
    struct EnDecimal;
    impl serde::de::Visitor<'_> for EnDecimal {
        type Value = crate::num::Decimal;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("un nombre")
        }
        fn visit_i64<E: serde::de::Error>(self, i: i64) -> Result<Self::Value, E> {
            Ok(i.into())
        }
        fn visit_u64<E: serde::de::Error>(self, u: u64) -> Result<Self::Value, E> {
            Ok(u.into())
        }
        fn visit_f64<E: serde::de::Error>(self, x: f64) -> Result<Self::Value, E> {
            crate::num::dec_f64(x).map_err(E::custom)
        }
        fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<Self::Value, E> {
            crate::num::dec_texte(s).map_err(E::custom)
        }
    }
    d.deserialize_any(EnDecimal)
}

/// `decimal`, quand la valeur peut manquer ou etre nulle (avec `#[serde(default)]`).
pub fn decimal_opt<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<crate::num::Decimal>, D::Error> {
    <Option<Nombre> as serde::Deserialize>::deserialize(d).map(|n| n.map(|n| n.0))
}

/// UN TEXTE QUI PEUT MANQUER : absent ou nul, il est vide (`None`) ; un texte vide `""` n'est pas le vide, il se refuse.
/// `#[serde(deserialize_with = "socle::forme::texte_opt")]` (la cle s'ecrit, nulle)..
pub fn texte_opt<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    match <Option<String> as serde::Deserialize>::deserialize(d)? {
        Some(t) if t.is_empty() => Err(serde::de::Error::custom("un texte vide ne dit rien : le vide s'écrit absent ou null")),
        t => Ok(t),
    }
}

/// Une cle qui peut manquer (rien ne change) ou etre nulle (elle s'efface) : `#[serde(default, deserialize_with =
/// "socle::forme::dite")]` sur un `Option<Option<T>>` -- absente, `None` ; nulle, `Some(None)`.
pub fn dite<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(d: D) -> Result<Option<Option<T>>, D::Error> {
    <Option<T> as serde::Deserialize>::deserialize(d).map(Some)
}

/// UN OBJET DONT L'ORDRE COMPTE (un catalogue montre dans l'ordre du fichier), lu en paires `(cle, valeur)` :
/// `#[serde(deserialize_with = "socle::forme::en_ordre")]` sur un `Vec<(String, T)>`. Une cle repetee refuse.
pub fn en_ordre<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(d: D) -> Result<Vec<(String, T)>, D::Error> {
    struct Visiteur<T>(std::marker::PhantomData<T>);
    impl<'de, T: serde::Deserialize<'de>> serde::de::Visitor<'de> for Visiteur<T> {
        type Value = Vec<(String, T)>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("un objet")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(self, mut m: A) -> Result<Self::Value, A::Error> {
            let mut out: Vec<(String, T)> = Vec::new();
            while let Some((k, v)) = m.next_entry::<String, T>()? {
                if out.iter().any(|(x, _)| *x == k) {
                    return Err(serde::de::Error::custom(format!("la clé {} est répétée", k)));
                }
                out.push((k, v));
            }
            Ok(out)
        }
    }
    d.deserialize_map(Visiteur(std::marker::PhantomData))
}

/// Un nombre decimal, lu comme `decimal`, la ou un champ ne se pose pas (une valeur de table).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Nombre(pub crate::num::Decimal);

impl<'de> serde::Deserialize<'de> for Nombre {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        decimal(d).map(Nombre)
    }
}

/// UNE VALEUR QUI PEUT NE RIEN DIRE : nulle (`null`), vide (`""`), ou dite, lue en `T` sans passer par une valeur
/// intermediaire (le chemin d'un refus va jusqu'a la cle fautive de `T`). C'est la lecture d'une case de fiche
/// (`droit::fiche::Case`), qui garde ces trois formes pour se reecrire telle qu'elle est lue.
#[derive(Debug, Clone, PartialEq)]
pub enum Remplie<T> {
    Nulle,
    Vide,
    Dite(T),
}

impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for Remplie<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::value::{BoolDeserializer, F64Deserializer, I64Deserializer, MapAccessDeserializer, SeqAccessDeserializer, StrDeserializer, U64Deserializer};
        struct Lue<T>(std::marker::PhantomData<T>);
        impl<'de, T: serde::Deserialize<'de>> serde::de::Visitor<'de> for Lue<T> {
            type Value = Remplie<T>;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("une valeur, nulle ou vide")
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(Remplie::Nulle)
            }
            fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(Remplie::Nulle)
            }
            fn visit_some<D: serde::Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
                d.deserialize_any(self)
            }
            fn visit_bool<E: serde::de::Error>(self, b: bool) -> Result<Self::Value, E> {
                T::deserialize(BoolDeserializer::<E>::new(b)).map(Remplie::Dite)
            }
            fn visit_i64<E: serde::de::Error>(self, i: i64) -> Result<Self::Value, E> {
                T::deserialize(I64Deserializer::<E>::new(i)).map(Remplie::Dite)
            }
            fn visit_u64<E: serde::de::Error>(self, u: u64) -> Result<Self::Value, E> {
                T::deserialize(U64Deserializer::<E>::new(u)).map(Remplie::Dite)
            }
            fn visit_f64<E: serde::de::Error>(self, x: f64) -> Result<Self::Value, E> {
                T::deserialize(F64Deserializer::<E>::new(x)).map(Remplie::Dite)
            }
            fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<Self::Value, E> {
                if s.is_empty() {
                    return Ok(Remplie::Vide);
                }
                T::deserialize(StrDeserializer::<E>::new(s)).map(Remplie::Dite)
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, a: A) -> Result<Self::Value, A::Error> {
                T::deserialize(SeqAccessDeserializer::new(a)).map(Remplie::Dite)
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, a: A) -> Result<Self::Value, A::Error> {
                T::deserialize(MapAccessDeserializer::new(a)).map(Remplie::Dite)
            }
        }
        d.deserialize_any(Lue(std::marker::PhantomData))
    }
}

/// UN NOMBRE TEL QU'IL EST ECRIT : un entier, un flottant, ou un texte qui se lit en decimal (`num::decimal_lu`, `12,5`
/// compris), garde tel quel pour se reecrire de meme (`droit::fiche::Montant`).
#[derive(Debug, Clone, PartialEq)]
pub enum NombreEcrit {
    Entier(i64),
    Naturel(u64),
    Flottant(f64),
    Texte(String),
}

impl<'de> serde::Deserialize<'de> for NombreEcrit {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Lu;
        impl serde::de::Visitor<'_> for Lu {
            type Value = NombreEcrit;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("un nombre")
            }
            fn visit_i64<E: serde::de::Error>(self, i: i64) -> Result<NombreEcrit, E> {
                Ok(NombreEcrit::Entier(i))
            }
            fn visit_u64<E: serde::de::Error>(self, u: u64) -> Result<NombreEcrit, E> {
                // Un naturel que `i64` tient est un entier ; au-dela, il reste naturel.
                Ok(if u <= i64::MAX as u64 { NombreEcrit::Entier(u as i64) } else { NombreEcrit::Naturel(u) })
            }
            fn visit_f64<E: serde::de::Error>(self, x: f64) -> Result<NombreEcrit, E> {
                Ok(NombreEcrit::Flottant(x))
            }
            fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<NombreEcrit, E> {
                match crate::num::decimal_lu(s) {
                    Some(_) => Ok(NombreEcrit::Texte(s.to_string())),
                    None => Err(E::custom(format!("{} n'est pas un nombre", crate::ecriture::json_en_ligne(s)))),
                }
            }
            fn visit_bool<E: serde::de::Error>(self, b: bool) -> Result<NombreEcrit, E> {
                Err(E::custom(format!("{} n'est pas un nombre", b)))
            }
        }
        d.deserialize_any(Lu)
    }
}

/// UN NOMBRE D'UNE DONNEE, garde tel qu'il s'ecrit (`130.0` reste `130.0`, `12` reste `12`) : il se relit et se
/// reecrit a l'identique. Un texte n'en est pas un.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "serde_json::Number", into = "serde_json::Number")]
pub struct NombreLu(serde_json::Number);

impl TryFrom<serde_json::Number> for NombreLu {
    type Error = std::convert::Infallible;
    fn try_from(n: serde_json::Number) -> Result<NombreLu, Self::Error> {
        Ok(NombreLu(n))
    }
}

impl From<NombreLu> for serde_json::Number {
    fn from(n: NombreLu) -> serde_json::Number {
        n.0
    }
}

impl NombreLu {
    /// Un flottant fini ; `None` pour l'infini ou NaN, qu'une donnee n'ecrit pas.
    pub fn de_f64(x: f64) -> Option<NombreLu> {
        serde_json::Number::from_f64(x).map(NombreLu)
    }

    pub fn as_f64(&self) -> Option<f64> {
        self.0.as_f64()
    }

    /// Le flottant du nombre : tout nombre d'une donnee en a un (un entier au-dela de 2^53 s'arrondit) ; NaN sinon.
    pub fn en_f64(&self) -> f64 {
        match self.0.as_f64() {
            Some(x) => x,
            None => f64::NAN,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        self.0.as_i64()
    }

    pub fn as_u64(&self) -> Option<u64> {
        self.0.as_u64()
    }
}

impl From<u64> for NombreLu {
    fn from(n: u64) -> NombreLu {
        NombreLu(n.into())
    }
}

impl From<i64> for NombreLu {
    fn from(n: i64) -> NombreLu {
        NombreLu(n.into())
    }
}

impl std::str::FromStr for NombreLu {
    type Err = String;
    fn from_str(t: &str) -> Result<NombreLu, String> {
        t.parse::<serde_json::Number>().map(NombreLu).map_err(|_| crate::num::illisible(t))
    }
}

impl std::fmt::Display for NombreLu {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Les trois cles qui expliquent un objet d'un fichier, sans rien dire au calcul : `quoi` (ce qu'il est), `note` (une
/// remarque, un pourquoi, une regle d'emploi), `source` (le texte depose qui le fonde).
pub const NOTES: [&str; 3] = ["quoi", "note", "source"];

/// UN OBJET DE FICHES ET SA NOTE (`config/salaries.json`, `config/spectacles.json`) : la note (`note`), et les fiches dans
/// l'ordre du fichier, chacune sous sa cle lue en `K`. Une cle repetee refuse. `#[serde(deserialize_with)]` ne s'y pose
/// pas : la forme l'appelle de son `Deserialize`.
pub fn fiches_et_note<'de, D, K, F>(d: D, note: &'static str) -> Result<(Option<String>, Vec<(K, F)>), D::Error>
where
    D: serde::Deserializer<'de>,
    K: serde::Deserialize<'de>,
    F: serde::Deserialize<'de>,
{
    use serde::de::IntoDeserializer;
    struct Lues<K, F>(&'static str, std::marker::PhantomData<(K, F)>);
    impl<'de, K: serde::Deserialize<'de>, F: serde::Deserialize<'de>> serde::de::Visitor<'de> for Lues<K, F> {
        type Value = (Option<String>, Vec<(K, F)>);
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("un objet de fiches")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(self, mut m: A) -> Result<Self::Value, A::Error> {
            let (mut note, mut fiches, mut vues) = (None, Vec::new(), std::collections::HashSet::new());
            while let Some(k) = m.next_key::<String>()? {
                if k == self.0 {
                    note = Some(m.next_value()?);
                } else if !vues.insert(k.clone()) {
                    return Err(serde::de::Error::custom(format!("la clé {} est répétée", k)));
                } else {
                    let cle = K::deserialize(k.as_str().into_deserializer())?;
                    fiches.push((cle, m.next_value()?));
                }
            }
            Ok((note, fiches))
        }
    }
    d.deserialize_map(Lues(note, std::marker::PhantomData))
}

/// `lire`, quand la phrase du refus nomme elle-meme la cle fautive : `phrase(chemin, detail)` la compose (le fichier
/// d'une compagnie, qu'elle corrige : `droit::fiche`).
pub fn lire_en_phrase<T: DeserializeOwned>(v: &Value, phrase: impl Fn(&str, &str) -> String) -> Resultat<T> {
    serde_path_to_error::deserialize(v).map_err(|e| refus_en_phrase(&phrase, &e))
}

/// `lire_en_phrase`, du fichier JSON `fichier` (sa lecture notee au registre d'un calcul suivi, `fichiers::lire_texte`) ;
/// le disque qui refuse se dit par `echec::du_disque`.
pub fn lire_fichier_en_phrase<T: DeserializeOwned>(fichier: &Path, phrase: impl Fn(&str, &str) -> String) -> Resultat<T> {
    let texte = crate::fichiers::lire_texte(fichier).map_err(|e| crate::echec::du_disque(fichier, &e))?;
    let mut d = serde_json::Deserializer::from_str(&texte);
    serde_path_to_error::deserialize(&mut d).map_err(|e| refus_en_phrase(&phrase, &e))
}

fn refus_en_phrase(phrase: &impl Fn(&str, &str) -> String, e: &serde_path_to_error::Error<serde_json::Error>) -> String {
    let chemin = e.path().to_string();
    let chemin = if chemin == "." { String::new() } else { chemin };
    let detail = en_clair(&e.inner().to_string());
    crate::echec::avec(phrase(&chemin, &detail), APart { cle: Some(chemin), detail: Some(detail).filter(|d| !d.is_empty()), ..APart::default() })
}

// ------------------------------------------------------------------ les formes partagees

/// Une forme lue, rangee par son type.
type Forme = Arc<dyn Any + Send + Sync>;

/// Ce que chaque fichier lu a donne, par son chemin et le type de sa forme : ses octets d'alors, et sa forme.
type CacheDesFormes = Mutex<HashMap<(PathBuf, TypeId), (Arc<Vec<u8>>, Forme)>>;

/// UN FICHIER LU EN SA FORME `T`, UNE FOIS PAR CONTENU : hors d'un geste, le fichier se relit a chaque fois (ses octets),
/// et la forme qu'on en avait tiree ne sert que si ses octets sont exactement ceux d'alors. Rien ne se lit perime, quel
/// que soit qui l'a ecrit et quand (une autre fenetre, une restauration, une cle USB aux dates a deux secondes pres) :
/// seule la lecture s'epargne. PENDANT UN GESTE (`geste`), il se lit une fois : la lecture du geste le garde, et ce que
/// le geste ecrit ou efface (`ecriture::ecrire_json`, `ecriture::effacer`) se relit. `None` s'il n'existe pas. Rien ne
/// se note au registre : l'appelant dit ce qu'il lit (les engagements se comptent par leur saisie, `engagement::stock`).
/// Partage : on ne la modifie pas ; deux lectures du meme contenu rendent la meme forme (`Arc::ptr_eq`). Un JSON
/// illisible se dit par `echec::du_disque`, une forme fausse par `phrase`, la cle et le detail a part (`lire`).
pub fn partagee<T: DeserializeOwned + Send + Sync + 'static>(chemin: &Path, phrase: &str) -> Resultat<Option<Arc<T>>> {
    static CACHE: OnceLock<CacheDesFormes> = OnceLock::new();
    let cle = (chemin.to_path_buf(), TypeId::of::<T>());
    let en_t = |f: Forme| f.downcast::<T>().map_err(|_| "Une forme partagée n'a pas le type de sa lecture : le signaler au développeur.".to_string());
    if let Some(lu) = GESTE.with(|g| g.borrow().as_ref().and_then(|l| l.get(&cle).cloned())) {
        return lu.map(en_t).transpose();
    }
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let octets = match std::fs::read(chemin) {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            garder_dans_le_geste(cle, None);
            return Ok(None);
        }
        Err(e) => return Err(crate::fichiers::panne_fichier(&e, chemin)),
    };
    let deja = crate::memoire::tenu(cache).get(&cle).filter(|(vus, _)| **vus == octets).map(|(_, f)| f.clone());
    let f = match deja {
        Some(f) => f,
        None => {
            let f: Forme = Arc::new(des_octets::<T>(chemin, &octets, phrase)?);
            crate::fichiers::ranger(cache, cle.clone(), (Arc::new(octets), f.clone()));
            f
        }
    };
    garder_dans_le_geste(cle, Some(f.clone()));
    en_t(f).map(Some)
}

/// Les octets JSON du fichier `chemin` lus en `T` : un JSON illisible se dit par `echec::du_disque`, une forme fausse par
/// `phrase`, la cle et le detail a part.
fn des_octets<T: DeserializeOwned>(chemin: &Path, octets: &[u8], phrase: &str) -> Resultat<T> {
    let mut d = serde_json::Deserializer::from_slice(octets);
    let t = serde_path_to_error::deserialize(&mut d).map_err(|e| match e.inner().classify() {
        serde_json::error::Category::Data => refus(phrase, None, &e),
        _ => crate::echec::du_disque(&chemin, e.inner()),
    })?;
    d.end().map_err(|e| crate::echec::du_disque(&chemin, &e))?;
    Ok(t)
}

thread_local! {
    /// LA LECTURE DU GESTE EN COURS (`geste`) : chaque forme que `partagee` a servie, par son chemin et son type (`None` :
    /// absent).
    static GESTE: RefCell<Option<HashMap<(PathBuf, TypeId), Option<Forme>>>> = const { RefCell::new(None) };
}

fn garder_dans_le_geste(cle: (PathBuf, TypeId), f: Option<Forme>) {
    GESTE.with(|g| {
        if let Some(l) = g.borrow_mut().as_mut() {
            l.insert(cle, f);
        }
    });
}

/// Le geste vient d'ecrire ou d'effacer `chemin` (`ecriture::ecrire_json`, `ecriture::effacer`) : ce qu'il en avait lu
/// ne vaut plus, il le relit.
pub(crate) fn change_pendant_le_geste(chemin: &Path) {
    GESTE.with(|g| {
        if let Some(l) = g.borrow_mut().as_mut() {
            l.retain(|(p, _), _| p != chemin);
        }
    });
}

/// UN GESTE (une route, l'etat d'un mois refait) LIT UNE FOIS ce que la compagnie porte et que `partagee` sert (les
/// engagements, leurs paies, les imprevus) : rendu par `geste`, la lecture vit tant que son garde vit, et tombe avec
/// lui ; un geste ouvert depuis un autre lit dans la lecture de celui-ci.
pub struct Geste(bool);

#[must_use = "la lecture du geste ne vit que tant que son garde vit"]
pub fn geste() -> Geste {
    Geste(GESTE.with(|g| {
        let mut g = g.borrow_mut();
        if g.is_some() {
            return false;
        }
        *g = Some(HashMap::new());
        true
    }))
}

impl Drop for Geste {
    fn drop(&mut self) {
        if self.0 {
            GESTE.with(|g| *g.borrow_mut() = None);
        }
    }
}
