//! Les metiers du spectacle et la fiche telle qu'elle vaut pour un engagement (`emploi_de`, `emploi_du_mois`).
//!
//! Deux fichiers livres avec le code, chacun lu par sa forme, une fois tant qu'il ne change pas : `emplois-base.json`
//! (le catalogue, `BaseEmplois`) et `nomenclature-emplois-ft.json` (les codes France Travail, `Nomenclature`). Un
//! metier se lit par `metier`, qui rend un `Metier` : celui du catalogue, sinon celui de la nomenclature.
//!
//! De l'interface (l'accueil, `moteur::routes::ouverture`) : `catalogue`, `familles`,
//! `noms_des_conventions` et `controler_fiche`.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};
use socle::fichiers::{forme_en_cache, repr, FormeGardee};
use socle::num::Resultat;
use socle::date::Jour;

use crate::contexte::Contexte;
use crate::fiche::{Case, FicheSalarie, Statut};

/// `emplois-base.json` : le catalogue des metiers, dans l'ordre ou le deroulant les montre, et les noms courts des
/// conventions. `_perimetre`, `_regles` et `_sources` disent d'ou vient chaque champ.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseEmplois {
    #[serde(rename = "_quoi")]
    _quoi: String,
    #[serde(rename = "_perimetre")]
    _perimetre: String,
    #[serde(rename = "_regles")]
    _regles: BTreeMap<String, String>,
    #[serde(rename = "_sources")]
    _sources: Vec<String>,
    /// IDCC -> le nom court de la convention.
    #[serde(rename = "_conventions")]
    pub conventions: BTreeMap<String, String>,
    #[serde(deserialize_with = "socle::forme::en_ordre")]
    pub emplois: Vec<(String, FicheDuCatalogue)>,
}

/// Un metier du catalogue : ce que les declarations demandent (le libelle et sa forme declaree S21.G00.40.006, la
/// PCS-ESE 40.004, le statut artiste ou technicien, le statut conventionnel 40.002, cadre par defaut 40.003), sa famille,
/// les conventions dont le deroulant le montre, et la ligne de minima et de classification de chaque convention qui
/// le nomme (`_regles` de `emplois-base.json`).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FicheDuCatalogue {
    pub libelle: String,
    pub feminin: String,
    pub pcs_ese: String,
    pub statut: Statut,
    pub famille: String,
    pub cadre_par_defaut: bool,
    /// Le defaut cadre d'une convention qui le dit autrement (le dramaturge, cadre dans la 1285).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub cadre_selon_la_convention: BTreeMap<String, bool>,
    pub statut_conventionnel: String,
    pub minima: Minima,
    pub emploi: String,
    pub annexe: u8,
    /// L'annexe que la nomenclature France Travail donne au code, quand le catalogue en retient une autre.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annexe_nomenclature: Option<u8>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conventions: Vec<String>,
    /// Les conventions dont le deroulant le montre sans que leur liste de l'annexe 8 le nomme.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conventions_hors_liste: Vec<String>,
    /// IDCC -> le classement conventionnel du metier dans cette convention.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub classement: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classification_1285: Option<Classification1285>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classification_2642: Option<Fonction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classification_3090: Option<Classification3090>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classification_3097: Option<Classification3097>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(rename = "_2642", default, skip_serializing)]
    _note_2642: Option<String>,
    #[serde(rename = "_3097", default, skip_serializing)]
    _note_3097: Option<String>,
    #[serde(rename = "_un_code_deux_fonctions", default, skip_serializing)]
    _un_code_deux_fonctions: Option<String>,
}

/// La ligne de minima de chaque convention qui nomme le metier : la 3090 par annexe (annexe -> ligne), les autres en
/// une ligne.
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Minima {
    #[serde(rename = "3090", default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ccn_3090: BTreeMap<String, String>,
    #[serde(rename = "1285", default, skip_serializing_if = "Option::is_none")]
    pub ccn_1285: Option<String>,
    #[serde(rename = "2642", default, skip_serializing_if = "Option::is_none")]
    pub ccn_2642: Option<String>,
    #[serde(rename = "3097", default, skip_serializing_if = "Option::is_none")]
    pub ccn_3097: Option<String>,
}

impl Minima {
    /// La ligne de minima d'une convention qui n'en a qu'une (1285, 2642, 3097), ou `None`.
    pub fn ligne(&self, idcc: &str) -> Option<&str> {
        match idcc.trim() {
            "1285" => self.ccn_1285.as_deref(),
            "2642" => self.ccn_2642.as_deref(),
            "3097" => self.ccn_3097.as_deref(),
            _ => None,
        }
    }
}

/// La classification de la CCNEAC (1285) : le groupe de la grille des emplois autres qu'artistiques, et l'emploi
/// repere de l'article XI.3.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Classification1285 {
    pub groupe: Option<u32>,
    pub emploi_xi3: String,
}

/// La classification de la 3090 : la grille (technique, administratif) et le niveau.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Classification3090 {
    pub grille: String,
    pub niveau: String,
}

/// La fonction du metier dans la grille de la 2642.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Fonction {
    pub fonction: String,
}

/// La fonction du metier dans la grille de la 3097 (titre II), et l'emploi repere du titre IV qui donne le salaire d'un
/// permanent (`minima-conventionnels.json`, `3097._permanents` : seuls les metiers qui le disent sont compares).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Classification3097 {
    pub fonction: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub titre_iv: Option<String>,
}

/// `nomenclature-emplois-ft.json` : les codes emploi France Travail (S21.G00.40.005), leur libelle et leur annexe.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Nomenclature {
    #[serde(rename = "_quoi")]
    _quoi: String,
    #[serde(rename = "_source")]
    _source: String,
    #[serde(rename = "_version")]
    _version: String,
    #[serde(rename = "_evolution_2016")]
    _evolution_2016: String,
    pub emplois: BTreeMap<String, EmploiFt>,
}

/// Un code de la nomenclature : son libelle et son annexe (8 technicien, 10 artiste).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EmploiFt {
    pub libelle: String,
    pub annexe: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evolution_2016: Option<String>,
}

const CATALOGUE_ILLISIBLE: &str = "Le catalogue des métiers livré avec le logiciel ne se lit pas : le signaler au développeur.";

/// LA SEULE LECTURE D'`emplois-base.json`.
pub fn base() -> Resultat<Arc<BaseEmplois>> {
    static CACHE: OnceLock<FormeGardee<BaseEmplois>> = OnceLock::new();
    forme_en_cache(&CACHE, "emplois-base.json", CATALOGUE_ILLISIBLE, Ok)
}

/// LA SEULE LECTURE DE `nomenclature-emplois-ft.json`.
pub fn nomenclature() -> Resultat<Arc<Nomenclature>> {
    static CACHE: OnceLock<FormeGardee<Nomenclature>> = OnceLock::new();
    forme_en_cache(&CACHE, "nomenclature-emplois-ft.json", "La nomenclature des emplois France Travail livrée avec le logiciel ne se lit pas : le signaler au développeur.", Ok)
}

/// Premiere lettre en majuscule, le reste en minuscules.
fn capitaliser(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(p) => p.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
    }
}

/// Un metier, par son code : celui du catalogue, ou, hors catalogue, celui de la nomenclature France Travail (un code
/// et une annexe ; ni PCS, ni niveau, ni minima).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Metier {
    pub code: String,
    #[serde(flatten)]
    pub fiche: FicheDuMetier,
}

/// D'ou vient un metier : le catalogue, ou la nomenclature seule.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum FicheDuMetier {
    Catalogue(FicheDuCatalogue),
    Nomenclature(EmploiFt),
}

impl Metier {
    /// Sa fiche du catalogue, ou `None` hors catalogue.
    pub fn au_catalogue(&self) -> Option<&FicheDuCatalogue> {
        match &self.fiche {
            FicheDuMetier::Catalogue(f) => Some(f),
            FicheDuMetier::Nomenclature(_) => None,
        }
    }

    /// Le libelle lisible : celui du catalogue, ou celui de la nomenclature en casse ordinaire.
    pub fn libelle(&self) -> String {
        match &self.fiche {
            FicheDuMetier::Catalogue(f) => f.libelle.clone(),
            FicheDuMetier::Nomenclature(n) => capitaliser(&n.libelle),
        }
    }

    /// Le libelle au feminin (hors catalogue, le meme).
    pub fn feminin(&self) -> String {
        match &self.fiche {
            FicheDuMetier::Catalogue(f) => f.feminin.clone(),
            FicheDuMetier::Nomenclature(n) => capitaliser(&n.libelle),
        }
    }

    /// La forme declaree de l'emploi (S21.G00.40.006).
    pub fn emploi(&self) -> &str {
        match &self.fiche {
            FicheDuMetier::Catalogue(f) => &f.emploi,
            FicheDuMetier::Nomenclature(n) => &n.libelle,
        }
    }

    /// L'annexe de l'assurance chomage : 8 (technicien) ou 10 (artiste).
    pub fn annexe(&self) -> u8 {
        match &self.fiche {
            FicheDuMetier::Catalogue(f) => f.annexe,
            FicheDuMetier::Nomenclature(n) => n.annexe,
        }
    }

    /// Le statut : celui du catalogue ; hors catalogue, artiste pour l'annexe 10, technicien sinon.
    pub fn statut(&self) -> Statut {
        match &self.fiche {
            FicheDuMetier::Catalogue(f) => f.statut,
            FicheDuMetier::Nomenclature(n) if n.annexe == 10 => Statut::Artiste,
            FicheDuMetier::Nomenclature(_) => Statut::Technicien,
        }
    }

    /// La famille du catalogue ; « Hors catalogue » sinon.
    pub fn famille(&self) -> &str {
        self.au_catalogue().map_or("Hors catalogue", |f| f.famille.as_str())
    }

    /// La PCS-ESE du catalogue (hors catalogue : aucune).
    pub fn pcs_ese(&self) -> Option<&str> {
        self.au_catalogue().map(|f| f.pcs_ese.as_str())
    }

    /// La classification 3090 du catalogue, ou `None`.
    pub fn classification_3090(&self) -> Option<&Classification3090> {
        self.au_catalogue().and_then(|f| f.classification_3090.as_ref())
    }

    /// La liste de l'annexe 8 de la convention `idcc` nomme-t-elle ce metier ? Ce n'est pas le deroulant : un metier
    /// porte dans `conventions` les conventions dont le deroulant le montre, dont celles qui le portent sans que leur liste
    /// le nomme (`conventions_hors_liste`, et la regle de lecture des listes 6 et 7 dans `_regles` de `emplois-base.json`).
    /// Un metier hors catalogue ne dit pas sur quelle liste il est : non.
    pub fn nomme_par_l_annexe8(&self, idcc: &str) -> bool {
        let idcc = idcc.trim();
        self.au_catalogue().is_some_and(|f| !idcc.is_empty() && f.conventions.iter().any(|c| c == idcc) && !f.conventions_hors_liste.iter().any(|c| c == idcc))
    }

    /// Le metier est-il cadre, faute d'engagement qui le dise, dans la convention `idcc` ? `cadre_selon_la_convention`
    /// l'emporte pour la convention qu'il nomme (le dramaturge, cadre dans la 1285, art. XI.2.1, que la grille 3090 ne
    /// nomme pas : `_regles.cadre_par_defaut` de `emplois-base.json`) ; sinon `cadre_par_defaut`. Hors catalogue : non.
    pub fn cadre_par_defaut(&self, idcc: &str) -> bool {
        self.au_catalogue().is_some_and(|f| f.cadre_selon_la_convention.get(idcc.trim()).copied().unwrap_or(f.cadre_par_defaut))
    }

    /// Le defaut cadre de ce metier change-t-il d'une convention a l'autre (`cadre_selon_la_convention`) ? Une carte qui le
    /// dit garde alors ce qu'elle dit (`engagement::stock::en_forme`).
    pub fn cadre_selon_la_convention(&self) -> bool {
        self.au_catalogue().is_some_and(|f| !f.cadre_selon_la_convention.is_empty())
    }

    /// Le statut conventionnel DSN (40.002) du metier declare NON cadre. L'artiste-interprete : 05, par inference
    /// (`_regles.statut_conventionnel` de `emplois-base.json` : « Non-cadres » de la 3090, art. 6.3, hors de la categorie
    /// cadre de la 1285, art. XI.2.1 ; 05 par l'exemple de la fiche net-entreprises 3344, l. 364-366).
    pub fn statut_non_cadre(&self) -> Option<String> {
        if let Some(st) = self.au_catalogue().map(|f| f.statut_conventionnel.as_str()).filter(|s| !s.is_empty() && *s != "04") {
            return Some(st.to_string());
        }
        if self.statut() == Statut::Artiste {
            return Some("05".into());
        }
        let c = self.classification_3090()?;
        if c.niveau.starts_with("cadre") || c.niveau.starts_with("agent") {
            return Some("05".into());
        }
        Some(if c.grille == "administratif" { "06" } else { "07" }.into())
    }
}

/// Un metier par son code (blancs et casse ignores), catalogue d'abord, nomenclature ensuite. `None` si le code n'existe
/// nulle part.
pub fn metier(code: &str) -> Resultat<Option<Metier>> {
    let code = code.trim().to_uppercase();
    if code.is_empty() {
        return Ok(None);
    }
    if let Some((_, f)) = base()?.emplois.iter().find(|(c, _)| *c == code) {
        return Ok(Some(Metier { code, fiche: FicheDuMetier::Catalogue(f.clone()) }));
    }
    Ok(nomenclature()?.emplois.get(&code).cloned().map(|n| Metier { code, fiche: FicheDuMetier::Nomenclature(n) }))
}

/// LES SEPT CHAMPS DECLARES POUR UN METIER, tels que la fiche du salarie les ecrit : l'emploi declare et son libelle,
/// les codes PCS-ESE et France Travail, le statut, les statuts conventionnel (S21.G00.40.002) et categoriel de retraite
/// complementaire (S21.G00.40.003).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ChampsDuMetier {
    pub emploi: String,
    pub emploi_affichage: String,
    pub pcs_ese: Option<String>,
    pub code_emploi_ft: Option<String>,
    pub statut: String,
    pub statut_conventionnel: Option<String>,
    pub statut_categoriel_rc: &'static str,
}

/// Les sept champs declares pour ce metier (`emploi_affichage` compris).
/// `cadre` a `None` prend le defaut du catalogue dans la convention `idcc` (`Metier::cadre_par_defaut`). Refuse un code
/// inconnu.
pub fn resoudre(code: &str, cadre: Option<bool>, feminin: bool, idcc: &str) -> Resultat<ChampsDuMetier> {
    let m = metier(code)?.ok_or_else(|| {
        format!(
            "Metier {} inconnu : ni dans le catalogue (emplois-base.json), ni \
             dans la nomenclature France Travail (nomenclature-emplois-ft.json).",
            repr(code))
    })?;
    let est_cadre = cadre.unwrap_or_else(|| m.cadre_par_defaut(idcc));
    let libelle = Some(if feminin { m.feminin() } else { m.libelle() }).filter(|l| !l.is_empty()).unwrap_or_else(|| m.libelle());
    Ok(ChampsDuMetier {
        emploi: m.emploi().to_string(),
        emploi_affichage: libelle,
        pcs_ese: m.pcs_ese().map(str::to_string),
        code_emploi_ft: Some(m.code.clone()),
        statut: m.statut().as_str().to_string(),
        statut_conventionnel: if est_cadre { Some("04".into()) } else { m.statut_non_cadre() },
        statut_categoriel_rc: if est_cadre { "01" } else { "04" },
    })
}

/// Les cases de `UNE_VALEUR_PAR_MOIS` d'une fiche, dans son ordre, chacune telle qu'ecrite : `None` quand elle n'est ni
/// dite ni vide.
fn valeurs_par_mois(f: &FicheSalarie) -> [(&'static str, Option<String>); 5] {
    fn portee<T: std::fmt::Display>(c: &Case<T>) -> Option<String> {
        match c {
            Case::Absente | Case::Nulle => None,
            Case::Vide => Some(String::new()),
            Case::Dite(v) => Some(v.to_string()),
        }
    }
    [
        (UNE_VALEUR_PAR_MOIS[0], portee(&f.statut_categoriel_rc)),
        (UNE_VALEUR_PAR_MOIS[1], portee(&f.statut_conventionnel)),
        (UNE_VALEUR_PAR_MOIS[2], portee(&f.statut)),
        (UNE_VALEUR_PAR_MOIS[3], portee(&f.code_emploi_ft)),
        (UNE_VALEUR_PAR_MOIS[4], portee(&f.pcs_ese)),
    ]
}

/// Ce qu'un contrat paye au mois ne laisse pas changer en cours de mois : ce qui choisit la pile du bareme (le statut
/// artiste / technicien, la categorie cadre, le metier qui la dit a defaut de la fiche) et ce que le bloc 40 porte avec
/// une rubrique en bloc 41 (40.002, 40.003, 40.004). Le libelle de l'emploi n'en est pas : la DSN declare « la derniere
/// valeur connue » du mois (cahier technique 2026.1, § 2.1.2.4.2, p. 22).
const UNE_VALEUR_PAR_MOIS: [&str; 5] = ["statut_categoriel_rc", "statut_conventionnel", "statut", "code_emploi_ft", "pcs_ese"];

/// La fiche salarie `cle` d'un contrat PAYE AU MOIS (CDI, CDD de droit commun), telle qu'elle vaut pour la periode payee
/// (`debut`, `fin`) : lue a la fin de la periode. Un permanent passe cadre, change de poste, obtient ou perd sa RQTH en
/// cours de contrat : la paie et la DSN du mois ou cela prend effet le disent.
///
/// UN CHANGEMENT DANS LA PERIODE REFUSE (`UNE_VALEUR_PAR_MOIS`) : une paie se calcule sur une seule pile de bareme
/// (regle 6). La paie d'un mois ou ces valeurs changent se decoupe en periodes elementaires aux jours du changement
/// (`jours_de_changement`, `periodes::bornes`, norme § 2.1.3.2, p. 28), et chaque periode lit sa fiche ici, sur ses
/// bornes : le refus ne garde que l'appel sur une periode qui n'aurait pas ete coupee. Le statut BOETH, qui ne change
/// pas la paie, peut changer en cours de mois (40.072 se renseigne « pour le contrat concerne dans la periode
/// declaree », p. 211 : celui de la fin).
pub fn emploi_du_mois(ctx: &Contexte, cle: &str, debut: &Jour, fin: &Jour) -> Resultat<FicheSalarie> {
    let au_debut = ctx.salarie_au(cle, debut)?;
    let a_la_fin = ctx.salarie_au(cle, fin)?;
    for ((c, avant), (_, apres)) in valeurs_par_mois(&au_debut).into_iter().zip(valeurs_par_mois(&a_la_fin)) {
        if avant == apres {
            continue;
        }
        let le = ctx.salarie_brut(cle).map_or_else(Vec::new, |b| b.depuis(c)).into_iter().flatten().filter(|j| j > debut && j <= fin).last();
        let premier = Jour { jour: 1, ..*debut };
        let suivant = Jour { jour: 1, ..premier.plus_jours(31)? };
        let texte = |v: &Option<String>| v.as_deref().map_or_else(|| "(vide)".to_string(), repr);
        let qui = [a_la_fin.prenom.texte(), a_la_fin.nom.texte()].iter().filter(|s| !s.trim().is_empty()).map(|s| s.to_string()).collect::<Vec<_>>().join(" ");
        return Err(socle::echec::a_la_cle(&format!(
            "{} : la fiche change une case le {} ({} -> {}), dans une periode de paie qui n'est pas coupee a ce jour-la : une \
periode se calcule sur une seule pile de bareme. Dater le changement au premier jour d'un mois sur la fiche : le {} (il \
vaut pour tout ce mois-ci) ou le {} (il vaut a partir du mois suivant).",
            if qui.is_empty() { "Ce salarie".to_string() } else { qui },
            le.map_or_else(|| "?".to_string(), |j| j.jj_mm_aaaa()),
            texte(&avant),
            texte(&apres),
            premier.jj_mm_aaaa(),
            suivant.jj_mm_aaaa()
        ), &c, ""));
    }
    Ok(a_la_fin)
}

/// Les jours, apres `debut` et au plus tard `fin` (la periode payee d'un contrat paye au mois), ou la fiche `cle` change
/// une valeur qui choisit la pile du bareme ou que le bloc 40 porte avec une rubrique en bloc 41 (`UNE_VALEUR_PAR_MOIS`) :
/// le `depuis` d'une de ces valeurs, quand la fiche de la veille et celle du jour different. C'est la que la paie du mois
/// se decoupe en periodes elementaires (`periodes::bornes` ; norme DSN 2026.1, § 2.1.3.2, p. 28 : « passage de non
/// cadre a cadre en cours de periode de paie »). Vide : le mois se paie d'un seul tenant.
pub fn jours_de_changement(ctx: &Contexte, cle: &str, debut: &Jour, fin: &Jour) -> Resultat<Vec<Jour>> {
    let Some(brute) = ctx.salarie_brut(cle) else { return Ok(Vec::new()) };
    let mut jours: Vec<Jour> = Vec::new();
    for c in UNE_VALEUR_PAR_MOIS {
        jours.extend(brute.depuis(c).into_iter().flatten().filter(|j| j > debut && j <= fin));
    }
    jours.sort();
    jours.dedup();
    let mut out = Vec::new();
    for j in jours {
        let (avant, apres) = (brute.au(&j.plus_jours(-1)?), brute.au(&j));
        if !avant.memes_cases(&apres, &UNE_VALEUR_PAR_MOIS) {
            out.push(j);
        }
    }
    Ok(out)
}

/// Ce que `emploi_de` lit d'un engagement.
#[derive(Debug, Clone, Default)]
pub struct EmploiEngagement {
    pub emploi: Option<String>,
    /// Le libelle lisible que l'engagement a resolu.
    pub emploi_affichage: Option<String>,
    pub pcs_ese: Option<String>,
    pub code_emploi_ft: Option<String>,
    pub statut: Option<String>,
    pub statut_conventionnel: Option<String>,
    pub statut_categoriel_rc: Option<String>,
}

/// La fiche salarie `sal` (lue a la date de la periode) telle qu'elle vaut POUR CET ENGAGEMENT : ce que l'engagement
/// dit l'emporte ; le reste vient de la fiche.
pub fn emploi_de(sal: &FicheSalarie, eng: &EmploiEngagement) -> Resultat<FicheSalarie> {
    let mut out = sal.clone();
    let dit = |v: &Option<String>| v.as_ref().filter(|s| !s.is_empty()).cloned();
    // Un choix que l'engagement dit se lit comme la fiche le lirait (`fiche::Statut`...) : hors de la liste, il se refuse.
    fn choix<T: serde::de::DeserializeOwned>(v: &str, cle: &str) -> Resultat<T> {
        use serde::de::IntoDeserializer;
        let ecrit: serde::de::value::StrDeserializer<serde::de::value::Error> = v.into_deserializer();
        T::deserialize(ecrit)
            .map_err(|_| format!("L'engagement dit « {} » pour {}, qui n'est pas un choix de la fiche du salarié.", v, cle.replace('_', " ")))
    }
    let pris = [&eng.emploi, &eng.pcs_ese, &eng.code_emploi_ft, &eng.statut, &eng.statut_conventionnel, &eng.statut_categoriel_rc];
    let affichage = dit(&eng.emploi_affichage);
    if pris.iter().all(|v| dit(v).is_none()) && affichage.is_none() {
        return Ok(out);
    }
    if let Some(v) = dit(&eng.emploi) {
        out.emploi = Case::Dite(v);
    }
    if let Some(v) = dit(&eng.pcs_ese) {
        out.pcs_ese = Case::Dite(v);
    }
    if let Some(v) = dit(&eng.code_emploi_ft) {
        out.code_emploi_ft = Case::Dite(v);
    }
    if let Some(v) = dit(&eng.statut) {
        out.statut = Case::Dite(choix(&v, "statut")?);
    }
    if let Some(v) = dit(&eng.statut_conventionnel) {
        out.statut_conventionnel = Case::Dite(choix(&v, "statut_conventionnel")?);
    }
    if let Some(v) = dit(&eng.statut_categoriel_rc) {
        out.statut_categoriel_rc = Case::Dite(choix(&v, "statut_categoriel_rc")?);
    }
    // Un metier choisi sur la ligne : son libelle lisible aussi, tel que
    // l'engagement l'a resolu (au feminin si la fiche le dit) ; a defaut, le
    // libelle du catalogue.
    if let Some(a) = affichage {
        out.emploi_affichage = Case::Dite(a);
    } else if let Some(code) = dit(&eng.code_emploi_ft) {
        if sal.code_emploi_ft.dite() != Some(&code) {
            out.emploi_affichage = match resoudre(&code, None, false, "") {
                Ok(r) => Case::Dite(r.emploi_affichage),
                Err(_) => Case::Absente,
            };
        }
    }
    Ok(out)
}

// ------------------------------------------------------------- l'interface

/// Un metier du catalogue tel qu'un deroulant le montre : son code, ses libelles, sa famille, son statut et son annexe,
/// sa PCS-ESE, s'il est cadre par defaut dans la convention de la compagnie, son niveau 3090, et les conventions dont le
/// deroulant le montre.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MetierDuDeroulant {
    pub code: String,
    pub libelle: String,
    pub feminin: String,
    pub famille: String,
    pub statut: &'static str,
    pub annexe: u8,
    pub pcs_ese: String,
    pub cadre_par_defaut: bool,
    pub niveau_3090: Option<String>,
    pub conventions: Vec<String>,
}

/// Le catalogue du spectacle vivant, le defaut cadre de chaque metier lu dans la convention `idcc`, pour un
/// deroulant, dans l'ordre du fichier.
pub fn catalogue(idcc: &str) -> Resultat<Vec<MetierDuDeroulant>> {
    Ok(base()?
        .emplois
        .iter()
        .map(|(code, f)| {
            let m = Metier { code: code.clone(), fiche: FicheDuMetier::Catalogue(f.clone()) };
            MetierDuDeroulant {
                cadre_par_defaut: m.cadre_par_defaut(idcc),
                code: code.clone(),
                libelle: f.libelle.clone(),
                feminin: f.feminin.clone(),
                famille: f.famille.clone(),
                statut: f.statut.as_str(),
                annexe: f.annexe,
                pcs_ese: f.pcs_ese.clone(),
                niveau_3090: f.classification_3090.as_ref().map(|c| c.niveau.clone()),
                conventions: f.conventions.clone(),
            }
        })
        .collect())
}

/// Le metier du catalogue qu'un intitule imprime nomme (un bulletin d'un logiciel precedent : « MUSICIEN »), compare
/// par `norme` (sans casse ni accents) a son libelle, a son feminin et a sa forme declaree. `(code, feminin)` quand un
/// seul metier porte cet intitule ; `None` quand aucun, ou plusieurs (« SCRIPTE ») : on ne choisit pas entre eux.
pub fn par_intitule(intitule: &str, norme: impl Fn(&str) -> String) -> Resultat<Option<(String, bool)>> {
    let cherche = norme(intitule);
    if cherche.is_empty() {
        return Ok(None);
    }
    let mut trouves: Vec<(String, bool)> = Vec::new();
    for (code, f) in &base()?.emplois {
        let (libelle, feminin, emploi) = (norme(&f.libelle), norme(&f.feminin), norme(&f.emploi));
        if [&libelle, &feminin, &emploi].contains(&&cherche) {
            trouves.push((code.clone(), feminin == cherche && libelle != cherche));
        }
    }
    Ok(if trouves.len() == 1 { trouves.pop() } else { None })
}

/// IDCC -> le nom court de la convention.
pub fn noms_des_conventions() -> Resultat<BTreeMap<String, String>> {
    Ok(base()?.conventions.clone())
}

/// Les familles du catalogue, dans l'ordre ou les
/// montrer : les artistes d'abord, parce que c'est le cas de presque toutes
/// les paies.
pub fn familles() -> Resultat<Vec<String>> {
    const ORDRE: [&str; 11] = [
        "Artistes", "Musique", "Cirque et rue", "Mise en scène et création", "Régie et plateau", "Son", "Lumière",
        "Décor et construction", "Costume, coiffure, maquillage", "Production et administration", "Audiovisuel dans le spectacle",
    ];
    let vues: std::collections::BTreeSet<String> = base()?.emplois.iter().map(|(_, f)| f.famille.clone()).collect();
    let mut out: Vec<String> = ORDRE.iter().filter(|f| vues.contains(**f)).map(|f| f.to_string()).collect();
    out.extend(vues.iter().filter(|f| !ORDRE.contains(&f.as_str())).cloned());
    Ok(out)
}

/// Ce qu'une fiche salarie declare (son code emploi, son statut, sa PCS-ESE ; vide : elle ne le dit pas), confronte au
/// catalogue. Des phrases d'avertissement, jamais d'erreur : la fiche peut avoir raison contre le catalogue.
pub fn controler_fiche(code_emploi_ft: &str, statut: &str, pcs_ese: &str) -> Resultat<Vec<String>> {
    let mut avis = Vec::new();
    let code = code_emploi_ft.trim().to_uppercase();
    if code.is_empty() {
        return Ok(avis);
    }
    let Some(m) = metier(&code)? else {
        avis.push(format!("Code emploi France Travail {} inconnu de la nomenclature (S21.G00.40.005).", repr(&code)));
        return Ok(avis);
    };
    let libelle = m.libelle();
    if !statut.is_empty() && statut != m.statut().as_str() {
        avis.push(format!(
            "Le code emploi {} ({}) est de l'annexe {}, donc {} ; la fiche dit {}.",
            code, libelle, m.annexe(), m.statut(), repr(statut)
        ));
    }
    if let Some(attendu) = m.pcs_ese().filter(|p| !p.is_empty() && !pcs_ese.is_empty()) {
        if pcs_ese.to_lowercase() != attendu.to_lowercase() {
            avis.push(format!("PCS-ESE {} pour {} ; le catalogue attend {} (fiche Insee de la rubrique).", pcs_ese, libelle, attendu));
        }
    }
    Ok(avis)
}
