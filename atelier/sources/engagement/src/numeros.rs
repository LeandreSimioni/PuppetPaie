//! LE NUMERO D'UN CONTRAT (regle 4 : il ne se recalcule pas, il se lit). Il identifie le contrat en DSN
//! (S21.G00.40.009), unique par individu et par etablissement dans une declaration (CCH-12,
//! `atelier/reference/norme-dsn/texte-2027/bloc-S21.G00.40.txt`) ; aucune suite continue n'est exigee.
//!
//! Il s'ecrit dans l'engagement a sa validation (`a_la_validation`, que seule `etats::valider` appelle), puis tout ce qui
//! part -- le contrat a la signature ou imprime, la DPAE, la DSN, le bulletin -- le lit la. Le numero donne est le suivant
//! du plus haut que portent, pour le meme salarie et la meme annee, les engagements et ce qui est parti (`departs`)
//! (`derniers_pris`) : un engagement supprime avant tout envoi rend le sien.
//!
//! Une revalidation garde le numero. Tant que rien n'est parti sous lui, un engagement dont le salarie ou le debut a
//! change en recoit un nouveau ; des qu'un envoi l'a porte, il ne change plus. Un numero venu avec la saisie (un contrat
//! repris d'un autre logiciel) reste le sien, sauf sur la copie d'un engagement plus ancien qui le porte deja.

use std::collections::HashMap;


use serde_json::{json, Value};

use droit::contexte::{Contexte, IdSalarie};
use socle::date::en_texte;
use socle::fichiers::vrai;
use socle::num::Resultat;

use crate::departs::Departs;
use crate::stock::{self, Objet};

/// Pour qui le numero a ete donne : la cle du salarie et le debut du contrat a ce moment-la, et quand.
const POUR: &str = "numero_pour";

/// UN NUMERO DE CONTRAT, tel que la DSN le declare en S21.G00.40.009 : de 5 a 20 caracteres (`X [5,20]`,
/// `atelier/reference/norme-dsn/texte-2027/bloc-S21.G00.40.txt` l. 527-551), pris parmi [a-z], [A-Z], [0-9], le trait
/// d'union, l'espace, le point, l'arobase, le slash, l'underscore et les lettres accentuees de la liste (`ACCENTUES`),
/// sans espace au debut ni a la fin (§ 4.4.11, `chapitre-4-controles.txt` l. 760-780). Il ne se donne qu'ici
/// (`former`, regle 4) ; ailleurs il se lit tel qu'il est deja ecrit (`lire`, ou `serde` : la meme chaine), d'un
/// engagement, d'une charge, d'une DSN deposee. Ni `From<&str>` ni `Default` : un texte quelconque n'en est pas un.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct NumeroContrat(String);

/// Les caracteres accentues qu'un numero de contrat admet (§ 4.4.11).
const ACCENTUES: [char; 8] = ['à', 'é', 'è', 'ê', 'ë', 'ï', 'ô', 'ç'];

impl NumeroContrat {
    /// Un numero de contrat ecrit `texte` ; toute autre forme que celle de la norme se refuse.
    pub fn lire(texte: &str) -> Resultat<NumeroContrat> {
        let n = texte.chars().count();
        let admis = |c: char| c.is_ascii_alphanumeric() || "- .@/_".contains(c) || ACCENTUES.contains(&c);
        if !(5..=20).contains(&n) || !texte.chars().all(admis) || texte.starts_with(' ') || texte.ends_with(' ') {
            return Err(format!(
                "« {} » n'est pas un numéro de contrat de la DSN (S21.G00.40.009) : de 5 à 20 lettres, chiffres, points, \
                 traits d'union, arobases, barres obliques, tirets bas ou espaces, sans espace au début ni à la fin.",
                texte
            ));
        }
        Ok(NumeroContrat(texte.to_string()))
    }

    /// Le numero d'une valeur JSON (un engagement, une charge, une ligne du journal) : absent, nul ou vide, aucun ; une
    /// autre forme que celle de la norme se refuse.
    pub fn de_la_valeur(v: Option<&Value>) -> Resultat<Option<NumeroContrat>> {
        match v.filter(|n| vrai(n)) {
            None => Ok(None),
            Some(n) => NumeroContrat::lire(&en_texte(n)).map(Some),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// L'annee (2000 + les deux chiffres) et le rang d'un numero que `former` a donne (`FA1.26.16`) ; `None` pour un
    /// numero d'une autre forme (un contrat repris d'un autre logiciel).
    pub fn annee_et_rang(&self) -> Option<(i32, i64)> {
        let (tete, rang) = self.0.rsplit_once('.')?;
        if rang.is_empty() || !rang.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let (_, yy) = tete.rsplit_once('.')?;
        if yy.len() != 2 || !yy.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        Some((2000 + yy.parse::<i32>().ok()?, rang.parse().ok()?))
    }
}

impl std::ops::Deref for NumeroContrat {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for NumeroContrat {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for NumeroContrat {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for NumeroContrat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::str::FromStr for NumeroContrat {
    type Err = String;
    fn from_str(s: &str) -> Resultat<NumeroContrat> {
        NumeroContrat::lire(s)
    }
}

impl TryFrom<String> for NumeroContrat {
    type Error = String;
    fn try_from(s: String) -> Resultat<NumeroContrat> {
        NumeroContrat::lire(&s)
    }
}

impl From<NumeroContrat> for String {
    fn from(n: NumeroContrat) -> String {
        n.0
    }
}

impl From<NumeroContrat> for Value {
    fn from(n: NumeroContrat) -> Value {
        Value::String(n.0)
    }
}

impl PartialEq<str> for NumeroContrat {
    fn eq(&self, autre: &str) -> bool {
        self.0 == autre
    }
}

impl PartialEq<&str> for NumeroContrat {
    fn eq(&self, autre: &&str) -> bool {
        self.0 == *autre
    }
}

impl PartialEq<String> for NumeroContrat {
    fn eq(&self, autre: &String) -> bool {
        self.0 == *autre
    }
}

impl PartialEq<NumeroContrat> for &str {
    fn eq(&self, autre: &NumeroContrat) -> bool {
        *self == autre.0
    }
}

impl PartialEq<NumeroContrat> for String {
    fn eq(&self, autre: &NumeroContrat) -> bool {
        *self == autre.0
    }
}

/// Le numero de rang `seq` de l'annee `annee`, pour le salarie de cette fiche : `<initiales><code>.<AA>.<rang>`. Le seul
/// qui fabrique un numero de contrat (regle 4) ; des initiales ou un code qui le sortiraient de la forme de la norme le
/// font refuser (`NumeroContrat::lire`).
pub fn former(fiche: &Value, annee: i32, seq: i64) -> Resultat<NumeroContrat> {
    let champ = |k: &str| fiche.get(k).map(en_texte).ok_or_else(|| socle::fichiers::absent(k));
    let an = annee.to_string();
    NumeroContrat::lire(&format!("{}{}.{}.{:02}", champ("initiales_contrat")?, champ("code_salarie")?, &an[2.min(an.len())..], seq))
}

/// Le dernier rang pris en `annee`, par salarie (la cle de sa fiche) : le plus grand des numeros de cette annee que
/// portent ses engagements, ou sous lesquels quelque chose est parti pour lui (`departs`).
fn derniers_pris(ctx: &Contexte, annee: i32, departs: &Departs) -> Resultat<HashMap<IdSalarie, i64>> {
    derniers_pris_sauf(ctx, annee, None, departs)
}

/// La meme, sans l'engagement `sauf` (son fichier et son rang) : celui qui recoit un nouveau numero.
fn derniers_pris_sauf(ctx: &Contexte, annee: i32, sauf: Option<(&str, usize)>, departs: &Departs) -> Resultat<HashMap<IdSalarie, i64>> {
    let mut pris: HashMap<IdSalarie, i64> = HashMap::new();
    let mut noter = |cle: IdSalarie, numero: &NumeroContrat| {
        if let Some((a, seq)) = numero.annee_et_rang() {
            if a == annee {
                let v = pris.entry(cle).or_insert(0);
                *v = (*v).max(seq);
            }
        }
    };
    for o in stock::objets(&ctx.racine)? {
        if sauf == Some((o.fichier.as_str(), o.rang)) {
            continue;
        }
        let (Some(cle), Some(n)) = (crate::cle_de_l_engagement(ctx, &o.brut), NumeroContrat::de_la_valeur(o.brut.get("numero"))?) else { continue };
        noter(cle, &n);
    }
    for (cle, n) in departs.engagements() {
        noter(IdSalarie::lire(cle)?, &NumeroContrat::lire(n)?);
    }
    Ok(pris)
}

/// LE PLUS HAUT NUMERO DE BULLETIN `N/AAAA` DE `annee` QU'UN BULLETIN IMPORTE IMPRIME (0 sans aucun) : la suite des
/// bulletins de l'annee continue apres lui, et deux bulletins de la meme annee ne portent jamais le meme numero. Un
/// numero d'une autre forme n'est pas un rang de cette suite : il ne se lit pas comme tel.
pub fn dernier_bulletin_importe(racine: &std::path::Path, annee: i64) -> Resultat<i64> {
    let rang = |v: Option<&Value>| -> Option<i64> {
        let t = v.filter(|x| vrai(x)).map(en_texte)?;
        let (n, an) = t.trim().split_once('/')?;
        let ok = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
        (ok(n) && an.len() == 4 && ok(an) && an.parse::<i64>().ok()? == annee).then(|| n.parse().ok()).flatten()
    };
    let mut haut = 0;
    for i in crate::importe::tous(racine)? {
        let du_contrat = rang(i.contrat().and_then(|c| c.get("numero_bulletin")));
        let des_paies = i.paies().into_values().filter_map(|p| rang(p.get("bulletin").and_then(|b| b.get("numero")))).max();
        haut = haut.max(du_contrat.unwrap_or(0)).max(des_paies.unwrap_or(0));
    }
    Ok(haut)
}

/// Un envoi a-t-il porte le numero `n` de l'engagement `o` (du salarie `cle`) ? Ce qui est parti sous lui (`departs`),
/// ou une version de lui gardee parce qu'un envoi l'avait emportee.
fn porte_par_un_envoi(o: &Objet, cle: &str, n: &str, departs: &Departs) -> bool {
    if o.brut.get("versions").and_then(Value::as_array).is_some_and(|v| !v.is_empty()) {
        return true;
    }
    !departs.de(cle, n).is_empty()
}


/// Un engagement ecrit avant `o` (plus ancien fichier, ou plus haut dans le meme) porte-t-il deja le numero `n` ? Une
/// copie d'engagement emporte le numero de l'original : il reste a l'original.
/// Le contrat importe que `o` continue porte le sien : c'est le meme contrat, pas une copie -- le contrat d'usage qui
/// continue ici (`importe::continue_par`).
fn porte_par_un_autre(ctx: &Contexte, o: &Objet, n: &str) -> Resultat<bool> {
    let continue_ = crate::importe::continue_par(&ctx.racine, o.champs())?.map(|i| (i.objet().fichier.clone(), i.objet().rang));
    Ok(stock::objets(&ctx.racine)?.into_iter().any(|x| {
        (x.fichier.as_str(), x.rang) < (o.fichier.as_str(), o.rang)
            && continue_.as_ref() != Some(&(x.fichier.clone(), x.rang))
            && x.brut.get("numero").map(en_texte).as_deref() == Some(n)
    }))
}

/// LE NUMERO A LA VALIDATION de l'engagement `o` (`etats::valider`), pose sur lui ; `le` : l'instant de la validation.
/// Il garde celui qu'il porte, sauf si son salarie ou son debut a change depuis qu'il l'a recu et qu'aucun envoi ne l'a
/// porte : il en recoit alors un nouveau, comme un engagement qui n'en a pas.
pub(crate) fn a_la_validation(ctx: &Contexte, o: &mut Objet, le: &str, departs: &Departs) -> Resultat<()> {
    let e = &*o.brut;
    let cle = ctx.cle_du_salarie(&e.get("salarie").map(en_texte).unwrap_or_default())?;
    let fiche = ctx.salarie(&cle)?;
    let debut = crate::debut_brut(&ctx.racine, e);
    let porte = NumeroContrat::de_la_valeur(o.brut.get("numero"))?;
    // UN NUMERO PARTI NE CHANGE PLUS : celui de la validation que la modification remplace, des qu'un envoi l'a porte ;
    // celui qu'une DSN de l'ancien logiciel a declare pour un contrat repris (`importe::numero_declare`).
    let parti = NumeroContrat::de_la_valeur(o.brut.get(crate::etats::PRECEDENTE).and_then(|p| p.pointer("/saisie/numero")))?;
    let parti = match parti.filter(|p| porte_par_un_envoi(o, &cle, p, departs)) {
        Some(p) => Some(p),
        None => crate::importe::numero_declare(&o.brut)?,
    };
    if let Some(parti) = parti {
        if porte.as_ref().is_some_and(|n| *n != parti) {
            return Err(format!(
                "{} : ce contrat est parti sous le n° {} ; un numéro parti ne change plus (il identifie le contrat en DSN, \
                 S21.G00.40.009). Rien n'est écrit.",
                crate::salarie_nomme(ctx, o.brut.get("salarie")),
                parti
            ));
        }
    }
    if let Some(n) = porte {
        let garde = match o.brut.get(POUR) {
            Some(p) => {
                let meme = p.get("salarie").map(en_texte).as_deref() == Some(cle.as_str()) && p.get("debut").map(en_texte) == Some(debut.clone());
                meme || porte_par_un_envoi(o, &cle, &n, departs)
            }
            // Un numero venu avec la saisie (un contrat repris d'un autre logiciel, sous la forme qu'il avait) : le sien,
            // sauf s'il est la copie d'un engagement plus ancien.
            None => !porte_par_un_autre(ctx, o, &n)?,
        };
        if garde {
            return Ok(());
        }
    }
    let annee: i32 = debut.get(..4).and_then(|a| a.parse().ok()).ok_or_else(|| format!("date illisible : {}", debut))?;
    let dernier = derniers_pris_sauf(ctx, annee, Some((o.fichier.as_str(), o.rang)), departs)?.get(&cle).copied().unwrap_or(0);
    o.brut_mut().insert("numero".into(), former(&fiche, annee, dernier + 1)?.into());
    o.brut_mut().insert(POUR.into(), json!({"salarie": cle, "debut": debut, "le": le}));
    Ok(())
}

/// Le numero de contrat que la paie `p`, sans numero, porterait : la suite des numeros `pris` pour son salarie dans
/// l'annee du debut du contrat (`derniers_pris`, lus une fois par annee), en memoire -- celui que sa validation lui
/// donnerait (`a_la_validation`).
fn proposer(ctx: &Contexte, p: &mut crate::calcul::Contenu, pris: &mut HashMap<i32, HashMap<IdSalarie, i64>>, departs: &Departs) -> Resultat<()> {
    let annee = p.engagement.debut.an;
    let pris = match pris.entry(annee) {
        std::collections::hash_map::Entry::Occupied(x) => x.into_mut(),
        std::collections::hash_map::Entry::Vacant(x) => x.insert(derniers_pris(ctx, annee, departs)?),
    };
    let cle = ctx.cle_du_salarie(&p.engagement.salarie)?;
    let dernier = pris.entry(cle).or_insert(0);
    *dernier += 1;
    p.engagement.numero = Some(former(&p.salarie, annee, *dernier)?);
    Ok(())
}

/// LES NUMEROS DE CONTRAT QUE DES PAIES PORTERAIENT, en memoire : un engagement qui porte deja le sien le garde ; un
/// engagement qui n'a pas encore le sien (pas encore valide) recoit celui que sa validation lui donnerait (`proposer`).
pub fn numeroter_les_contrats(ctx: &Contexte, paies: &mut [crate::calcul::Paie], departs: &Departs) -> Resultat<()> {
    let mut pris = HashMap::new();
    for paie in paies.iter_mut() {
        if paie.engagement.numero.is_some() {
            continue;
        }
        proposer(ctx, paie.ecrire(), &mut pris, departs)?;
    }
    Ok(())
}

/// LES NUMEROS QUE DES PAIES PORTERAIENT, en memoire : les numeros de contrat (`numeroter_les_contrats`) ; une paie dont le bulletin est
/// parti garde son numero de bulletin, une autre recoit le `prochain` (`N/AAAA`), puis le suivant. Un mois sans paie n'a pas de
/// bulletin. Rend le prochain numero de bulletin libre.
pub fn numeroter(ctx: &Contexte, paies: &mut [crate::calcul::Paie], annee: i32, prochain: i64, departs: &Departs) -> Resultat<i64> {
    let mut n_bull = prochain;
    let mut pris = HashMap::new();
    for paie in paies.iter_mut() {
        let p = paie.ecrire();
        if p.engagement.numero.is_none() {
            proposer(ctx, p, &mut pris, departs)?;
        }
        if p.engagement.sans_paie() || p.numero_bulletin.as_deref().is_some_and(|n| !n.is_empty()) {
            continue;
        }
        p.numero_bulletin = Some(format!("{}/{}", n_bull, annee));
        n_bull += 1;
    }
    Ok(n_bull)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un numero de contrat : de 5 a 20 caracteres (S21.G00.40.009, `X [5,20]`), du jeu du § 4.4.11, sans espace aux
    /// bords ; le JSON garde la meme chaine. `former` refuse des initiales qui l'en sortiraient.
    #[test]
    fn un_numero_de_contrat_a_la_forme_de_la_norme() {
        for bon in ["FA1.26.13", "C-001", "a b/c_d@é.ç", "12345678901234567890"] {
            assert_eq!(NumeroContrat::lire(bon).map(String::from), Ok(bon.to_string()));
        }
        for faux in ["", "C1", "FA1 ", " FA1.26", "123456789012345678901", "FA1.26#3", "FÀ1.26.13"] {
            assert!(NumeroContrat::lire(faux).is_err(), "{:?} n'est pas un numero de contrat", faux);
        }
        let n = NumeroContrat::lire("FA1.26.13").unwrap();
        assert_eq!((n.annee_et_rang(), serde_json::to_value(&n).unwrap()), (Some((2026, 13)), serde_json::json!("FA1.26.13")));
        assert!(serde_json::from_value::<NumeroContrat>(serde_json::json!("C1")).is_err());
        let fiche = |initiales: &str| serde_json::json!({"initiales_contrat": initiales, "code_salarie": "1"});
        assert_eq!(former(&fiche("FA"), 2026, 3).map(String::from), Ok("FA1.26.03".to_string()));
        assert!(former(&fiche("F#"), 2026, 3).is_err());
    }
}
