//! OU VIVENT LES ENGAGEMENTS : `engagements/AAAA-MM.json`, le fichier du mois ou chaque engagement commence. Un
//! engagement est UN objet, qu'il dure un jour ou six mois ; il n'est ecrit que la. Ses paies n'y sont pas : chacune vit dans le
//! fichier de son mois (`paies`).
//!
//! Le fichier d'un mois : `{"mois", "taux_cachet", "engagements": [...]}` : le cachet par defaut du mois, que le
//! calcul lit pour chaque engagement du mois ; le mois ne porte rien d'autre.
//!
//! Un engagement porte la saisie de son premier mois, telle que l'ecran l'ecrit (salarie, spectacle, dates, lignes,
//! nature, metier...). Ce qui definit le contrat (`TERMES`, son salaire, sa duree du travail, son emploi...) vaut
//! jusqu'a ce qu'il change : un changement s'ecrit sous `termes`, une liste datee par terme (`[{"depuis": "AAAA-MM-JJ",
//! "valeur"}]`, la forme de la configuration ; `null` : retire), et vaut de son jour au changement suivant. Ce qui
//! n'appartient qu'a un mois d'apres le premier (ses lignes, ses conges pris, ses primes) s'ecrit sous `faits`, a ce
//! mois. Ce que l'engagement dit d'un mois, sa VUE (`Objet::vue`), se lit a la date : chaque terme a la fin du mois,
//! les faits du mois ; un mois sans faits d'un contrat d'usage prend les jours que son planning y met. Le salarie, les
//! bornes et le numero du contrat sont ceux de l'engagement (`PARTAGES`), dans tous ses mois.
//!
//! LE MOIS DE L'ECRAN (`vue_du_mois`) montre les engagements qui le touchent : ceux de son fichier, et ceux des fichiers
//! d'avant qui courent encore (`{"mois", "engagements": [...]}`) : ce que les calculs lisent. Une carte de l'ecran
//! ecrit son engagement, et lui seul (`ecrire_la_carte`).
//!
//! L'identite d'un engagement : son salarie et son premier jour, et son rang
//! parmi ceux du meme salarie et du meme jour (`identites`) : deux contrats d'usage du meme jour. Un contrat au mois
//! n'en a qu'un par salarie et par jour (`refuser_un_second_contrat_au_mois`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use droit::contexte::Contexte;
use socle::date::{dernier_du_mois, en_texte, mois_entre, premier_du_mois, Jour};
use socle::fichiers::vrai;
use socle::num::Resultat;

/// Le dossier des engagements, sous les donnees de la compagnie.
pub const DOSSIER: &str = "engagements";

/// Les deux dossiers que le stock tient, un fichier par mois : les engagements (`DOSSIER`), et les paies du mois
/// (`paies::DOSSIER`), que seul `paies` lit et ecrit. Un brouillon (`Brouillon`) les tient tous les deux.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Dossier {
    Engagements,
    Paies,
}

impl Dossier {
    fn nom(self) -> &'static str {
        match self {
            Dossier::Engagements => DOSSIER,
            Dossier::Paies => crate::paies::DOSSIER,
        }
    }
}

/// Les termes du contrat qui changent apres son premier mois : `{<terme>: [{"depuis", "valeur"}]}`.
pub const DATES: &str = "termes";

/// Ce que chaque mois d'apres le premier dit de lui seul : `{<mois>: {...}}`.
pub const FAITS: &str = "faits";

/// Ce que le moteur tient seul sur un engagement, jamais ce que la carte d'un mois ecrit telle quelle : ses termes
/// dates et les faits de ses mois (que la carte ecrit par `poser_le_mois`), sa validation, la validation qu'une
/// modification va remplacer, ses versions passees (`etats`), pour qui son numero de contrat a ete donne (`numeros`),
/// qu'il n'est plus actif (`NON_ACTIF`), ce que l'import a lu de lui (`importe::IMPORTE`).
pub const TENUS_PAR_LE_MOTEUR: [&str; 8] =
    [DATES, FAITS, "valide", "precedente", "versions", "numero_pour", NON_ACTIF, crate::importe::IMPORTE];

/// Ce qu'une version garde de l'engagement, en plus de sa saisie : ses termes dates et les faits de ses mois.
pub const SAISIS_PAR_MOIS: [&str; 2] = [DATES, FAITS];

/// Dans un engagement qui n'est plus actif (`etats::desactiver`) : `{"le", "version"}`, quand, et sa version (celle qui
/// suit la derniere validee : rien de paye). Il ne touche plus aucun mois (`Objet::couvre`) : il ne se calcule plus,
/// n'entre dans aucun document ni aucune declaration ; il se lit (`non_actifs_du_mois`).
pub const NON_ACTIF: &str = "non_actif";

/// Ce que la vue d'un mois porte et que le moteur deduit, qui ne s'ecrit pas : les jours du contrat entier (son premier
/// mois et son planning), que la DSN declare chaque mois (S21.G00.40.013) ; la date de l'avenant du mois ou un terme
/// change (`date_modification`) et, quand il change en cours de mois, les termes d'avant (`avant`), tires des termes
/// dates (`Objet::vue`).
pub const DEDUITS: [&str; 3] = ["jours_contrat", "date_modification", "avant"];

/// Ce que tous les mois d'un engagement partagent : le salarie, les bornes du contrat, son numero.
pub const PARTAGES: [&str; 4] = ["salarie", "debut", "fin", "numero"];

/// LES TERMES DU CONTRAT, qui valent jusqu'a ce qu'ils changent (`DATES`) : sa nature, son motif, sa duree du travail,
/// son salaire, son emploi, et ce que son contrat ecrit dit en plus (`CHAMPS_DU_CONTRAT_ECRIT` : un mois compare a un
/// autre pour un avenant lit les memes termes), son spectacle, son lieu, son etablissement et son metier. Ni ses lignes
/// (celles d'un contrat paye au mois se composent chaque mois), ni ce qui n'appartient qu'a un mois (conges pris,
/// absences, primes, rupture, date de paiement) : les faits du mois.
pub const TERMES: [&str; 21] = [
    "spectacle",
    "metier",
    "cadre",
    "lieu",
    "etablissement",
    "nature",
    "motif_recours",
    "quotite_contrat",
    "quotite_reference",
    "modalite_temps_travail",
    "salaire_mensuel",
    "emploi",
    "pcs_ese",
    "code_emploi_ft",
    "statut",
    "statut_conventionnel",
    "statut_categoriel_rc",
    "periode_essai_jours",
    "personne_remplacee",
    "repartition_horaire",
    "clauses_particulieres",
];

/// Le fichier des engagements qui commencent en `mois`.
pub fn chemin(racine: &Path, mois: &str) -> PathBuf {
    chemin_dans(racine, Dossier::Engagements, mois)
}

/// Le fichier de `mois` dans le dossier `d`.
pub(crate) fn chemin_dans(racine: &Path, d: Dossier, mois: &str) -> PathBuf {
    racine.join(d.nom()).join(format!("{}.json", mois))
}

/// Les mois qui ont un fichier d'engagements, du plus ancien au plus recent, ceux du brouillon ouvert compris.
pub fn fichiers(racine: &Path) -> Vec<String> {
    fichiers_dans(racine, Dossier::Engagements)
}

/// Les mois qui ont un fichier dans le dossier `d`, du plus ancien au plus recent, ceux du brouillon ouvert compris.
pub(crate) fn fichiers_dans(racine: &Path, d: Dossier) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(racine.join(d.nom()))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.strip_suffix(".json").filter(|m| socle::date::mois_valide(m)).map(String::from))
        .collect();
    BROUILLON.with(|b| v.extend(b.borrow().iter().filter(|(r, _)| r == racine).flat_map(|(_, f)| f.keys().filter(|(x, _)| *x == d).map(|(_, m)| m.clone()))));
    v.sort();
    v.dedup();
    v
}

/// Le fichier d'un mois, lu tel quel ; `None` s'il n'existe pas.
pub fn lire_fichier(racine: &Path, mois: &str) -> Resultat<Option<Map<String, Value>>> {
    Ok(lire_partage(racine, mois)?.map(|v| v.as_object().cloned().unwrap_or_default()))
}

thread_local! {
    /// Les fichiers ecrits dans le brouillon ouvert (`Brouillon`), pour sa racine : chacun par son dossier et son mois.
    static BROUILLON: std::cell::RefCell<Option<(PathBuf, BTreeMap<(Dossier, String), std::sync::Arc<Value>>)>> = const { std::cell::RefCell::new(None) };
}

/// UN BROUILLON : tant qu'il est ouvert, ce qui s'ecrit dans les engagements de `racine` et dans leurs paies (`reecrire`,
/// `paies`) reste en memoire, et toute lecture le voit. `ecrire` le pose sur le disque ; tombe sans, il n'a rien ecrit.
pub struct Brouillon(PathBuf);

impl Brouillon {
    pub fn ouvrir(racine: &Path) -> Brouillon {
        BROUILLON.with(|b| *b.borrow_mut() = Some((racine.to_path_buf(), BTreeMap::new())));
        Brouillon(racine.to_path_buf())
    }

    /// Ce que le brouillon tient s'ecrit sur le disque.
    pub fn ecrire(self) -> Resultat<()> {
        let fichiers = BROUILLON.with(|b| b.borrow_mut().take()).map(|(_, f)| f).unwrap_or_default();
        for ((d, mois), contenu) in fichiers {
            poser_dans(&self.0, d, &mois, &contenu)?;
        }
        Ok(())
    }

    /// Le fichier de `mois` du dossier `d` tel que le brouillon ouvert pour `racine` le tient.
    fn lu(racine: &Path, d: Dossier, mois: &str) -> Option<std::sync::Arc<Value>> {
        BROUILLON.with(|b| b.borrow().as_ref().filter(|(r, _)| r == racine).and_then(|(_, f)| f.get(&(d, mois.to_string())).cloned()))
    }

    /// Ecrit `contenu` comme fichier de `mois` du dossier `d` dans le brouillon ouvert pour `racine` ; faux : aucun ne
    /// l'est.
    fn tient(racine: &Path, d: Dossier, mois: &str, contenu: Value) -> bool {
        BROUILLON.with(|b| match b.borrow_mut().as_mut().filter(|(r, _)| r == racine) {
            Some((_, f)) => {
                f.insert((d, mois.to_string()), std::sync::Arc::new(contenu));
                true
            }
            None => false,
        })
    }
}

impl Drop for Brouillon {
    fn drop(&mut self) {
        BROUILLON.with(|b| *b.borrow_mut() = None);
    }
}

/// Ecrit `contenu` comme fichier d'engagements de `mois` (`poser_dans`).
pub(crate) fn poser(racine: &Path, mois: &str, contenu: &Value) -> Resultat<PathBuf> {
    poser_dans(racine, Dossier::Engagements, mois, contenu)
}

/// LE SEUL ENDROIT QUI ECRIT UN FICHIER D'ENGAGEMENTS OU DE PAIES : `contenu` devient le fichier de `mois` du dossier `d`,
/// dans le brouillon ouvert pour `racine` s'il y en a un (`Brouillon`), sinon sur le disque. Rend son chemin.
pub(crate) fn poser_dans(racine: &Path, d: Dossier, mois: &str, contenu: &Value) -> Resultat<PathBuf> {
    let p = chemin_dans(racine, d, mois);
    if Brouillon::tient(racine, d, mois, contenu.clone()) {
        return Ok(p);
    }
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| socle::fichiers::panne_fichier(&e, dir))?;
    }
    socle::ecriture::ecrire_json(&p, contenu, 2)?;
    Ok(p)
}

/// LE FICHIER D'ENGAGEMENTS DE `mois` QUI N'A PLUS RIEN (`{"mois", "engagements": []}` : un fichier que l'import avait
/// pose) s'en va : un mois sans fichier ni engagement n'a pas de saisie. Dans un brouillon, il s'ecrit vide.
pub(crate) fn retirer(racine: &Path, mois: &str) -> Resultat<PathBuf> {
    let vide = serde_json::json!({"mois": mois, "engagements": []});
    if Brouillon::tient(racine, Dossier::Engagements, mois, vide) {
        return Ok(chemin(racine, mois));
    }
    let p = chemin(racine, mois);
    socle::ecriture::effacer(&p)?;
    Ok(p)
}

/// Le fichier d'engagements d'un mois (`lire_dans`).
fn lire_partage(racine: &Path, mois: &str) -> Resultat<Option<std::sync::Arc<Value>>> {
    lire_dans(racine, Dossier::Engagements, mois)
}

/// Le fichier de `mois` du dossier `d` tel que le geste l'a lu ou ecrit (`socle::fichiers::json_partage`), ou tel que le
/// brouillon ouvert le tient : chaque lecture des engagements et des paies passe par lui. `None` s'il n'existe pas.
pub(crate) fn lire_dans(racine: &Path, d: Dossier, mois: &str) -> Resultat<Option<std::sync::Arc<Value>>> {
    if let Some(v) = Brouillon::lu(racine, d, mois) {
        return Ok(Some(v));
    }
    let p = chemin_dans(racine, d, mois);
    match socle::fichiers::json_partage(&p)? {
        Some(v) if !v.is_object() => Err(socle::echec::abime(&p, format!("un objet est attendu, lu {}", socle::valeur::nom_type(&v)))),
        v => Ok(v),
    }
}

/// Le fichier de `mois` ne porte-t-il que des engagements importes seuls (`Objet::importe_seul`) ? Un tel mois n'a pas de
/// saisie : il se lit. Un fichier illisible ou sans engagement n'est pas de l'import.
fn importes_seuls(racine: &Path, mois: &str) -> bool {
    let Ok(Some(f)) = lire_partage(racine, mois) else { return false };
    let es = f.get("engagements").and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
    !es.is_empty() && es.iter().all(|e| importe_seul(e, mois))
}

/// L'engagement `e`, du fichier de `fichier`, n'est-il qu'importe (`Objet::importe_seul`) ?
fn importe_seul(e: &Value, fichier: &str) -> bool {
    let verses = e.get(crate::importe::IMPORTE).and_then(|i| i.get("paies")).and_then(Value::as_object);
    e.get(crate::importe::IMPORTE).is_some_and(Value::is_object) && verses.and_then(|p| p.keys().max()).is_none_or(|m| m.as_str() >= fichier)
}

/// Le mois suivant `AAAA-MM`.
pub fn mois_apres(mois: &str) -> Resultat<String> {
    Ok(premier_du_mois(mois)?.mois_suivant())
}

// ------------------------------------------------------------------ l'objet

/// Un engagement tel qu'il est ecrit : la compagnie, le mois de son fichier (son premier mois), son rang dans le
/// fichier, et l'objet lui-meme, partage : le copier ne recopie pas ce qu'il porte (ses versions, ses paies) ; on ne
/// recopie que ce qu'on change (`brut_mut`).
#[derive(Debug, Clone)]
pub struct Objet {
    pub racine: PathBuf,
    pub fichier: String,
    pub rang: usize,
    /// Toujours un objet JSON.
    pub brut: std::sync::Arc<Value>,
}

impl Objet {
    /// Ses champs.
    pub fn champs(&self) -> &Map<String, Value> {
        static VIDE: std::sync::LazyLock<Map<String, Value>> = std::sync::LazyLock::new(Map::new);
        self.brut.as_object().unwrap_or(&VIDE)
    }

    /// L'objet a changer : recopie s'il est partage.
    pub fn brut_mut(&mut self) -> &mut Map<String, Value> {
        match std::sync::Arc::make_mut(&mut self.brut) {
            Value::Object(m) => m,
            autre => {
                *autre = Value::Object(Map::new());
                autre.as_object_mut().expect("un objet vient d'etre pose")
            }
        }
    }

    /// Le premier mois de l'engagement : celui de son fichier.
    pub fn premier_mois(&self) -> &str {
        &self.fichier
    }

    /// Le dernier mois d'un engagement borne (celui de sa fin) ; `None` pour un contrat sans terme.
    pub fn dernier_mois(&self) -> Resultat<Option<String>> {
        match self.brut.get("fin").filter(|f| vrai(f)) {
            Some(f) => Ok(Some(socle::date::d(f)?.mois_iso().max(self.fichier.clone()))),
            None if !self.mensuel() => {
                // Un contrat d'usage sans fin dite finit a son dernier jour joue ou planifie ; sans jour encore, a son
                // premier mois : seul un contrat paye au mois court sans terme.
                let v = &*self.brut;
                let mut jours: Vec<String> = crate::dates_engagement(&self.racine, &v)
                    .iter()
                    .chain(crate::planning_engagement(&self.racine, &v).iter())
                    .map(|j| match j {
                        Value::Object(m) => m.get("jour").map(en_texte).unwrap_or_default(),
                        autre => en_texte(autre),
                    })
                    .collect();
                jours.sort();
                let dernier = jours.last().map(|j| j.chars().take(7).collect::<String>()).filter(|m| socle::date::mois_valide(m));
                Ok(Some(dernier.map_or(self.fichier.clone(), |m| m.max(self.fichier.clone()))))
            }
            None => Ok(None),
        }
    }

    /// Un contrat paye au mois ?
    pub fn mensuel(&self) -> bool {
        droit::natures::nature(self.brut.get("nature").and_then(Value::as_str)).is_ok_and(|n| n.mensuelle())
    }

    /// Les mois que la saisie a touches : le premier, ceux qui ont des faits, et ceux ou un terme change.
    pub fn mois_saisis(&self) -> Vec<String> {
        let mut v = vec![self.fichier.clone()];
        if let Some(Value::Object(s)) = self.brut.get(FAITS) {
            v.extend(s.keys().filter(|m| m.as_str() > self.fichier.as_str()).cloned());
        }
        v.extend(self.changements().into_iter().map(|(_, j, _)| j.mois_iso()).filter(|m| m.as_str() > self.fichier.as_str()));
        v.sort();
        v.dedup();
        v
    }

    /// L'engagement touche-t-il `mois` ? Du premier mois a celui de sa fin ; sans terme, de son premier mois a toujours.
    /// Un engagement non actif ne touche plus aucun mois ; un engagement importe seul (`importe_seul`) n'en touche aucun de la saisie : il se
    /// lit (`importe`), rien ne le calcule.
    pub fn couvre(&self, mois: &str) -> Resultat<bool> {
        Ok(!self.est_non_actif() && !self.importe_seul() && self.dans_ses_bornes(mois)?)
    }

    /// Est-il importe (`importe::IMPORTE`) ? Ses mois repris le sont ; il peut continuer ici (`importe_seul`).
    pub fn est_importe(&self) -> bool {
        self.brut.get(crate::importe::IMPORTE).is_some_and(Value::is_object)
    }

    /// N'est-il qu'importe : tous ses mois sont repris d'un ancien logiciel, et il ne touche aucun mois d'ici. Un contrat
    /// de permanent qui continue ici vit dans le fichier de son premier mois neuf, apres tous ses mois verses
    /// (`importe::ecrire_tous`) : ce mois et les suivants sont de la saisie.
    pub fn importe_seul(&self) -> bool {
        importe_seul(&self.brut, &self.fichier)
    }

    /// N'est-il plus actif (`NON_ACTIF`) ?
    pub fn est_non_actif(&self) -> bool {
        self.brut.get(NON_ACTIF).is_some_and(Value::is_object)
    }

    /// `mois` est-il entre son premier mois et celui de sa fin, qu'il soit actif ou non ?
    fn dans_ses_bornes(&self, mois: &str) -> Resultat<bool> {
        if mois < self.fichier.as_str() {
            return Ok(false);
        }
        Ok(match self.dernier_mois()? {
            Some(d) => mois <= d.as_str(),
            None => true,
        })
    }

    /// Les mois de l'engagement jusqu'a `jusqu_a` : un engagement borne les a tous ; un contrat sans terme, ceux que la
    /// saisie a touches et ceux qui vont jusqu'a `jusqu_a`.
    pub fn mois_couverts(&self, jusqu_a: &str) -> Resultat<Vec<String>> {
        let fin = match self.dernier_mois()? {
            Some(d) => d,
            None => self.mois_saisis().last().cloned().unwrap_or_default().max(jusqu_a.to_string()).max(self.fichier.clone()),
        };
        Ok(mois_entre(&premier_du_mois(&self.fichier)?, &dernier_du_mois(&fin)?))
    }

    /// Valide ?
    pub fn est_valide(&self) -> bool {
        self.brut.get("valide").is_some_and(Value::is_object)
    }

    /// La saisie du premier mois : l'objet sans ce que le moteur tient seul.
    fn premiere_vue(&self) -> Map<String, Value> {
        // Seul ce qui reste se copie, dans l'ordre que laisse `Map::remove` (le dernier prend la place du retire).
        let mut v: Vec<(&String, &Value)> = self.champs().iter().collect();
        for k in TENUS_PAR_LE_MOTEUR {
            if let Some(i) = v.iter().position(|(c, _)| c.as_str() == k) {
                v.swap_remove(i);
            }
        }
        v.into_iter().map(|(c, x)| (c.clone(), x.clone())).collect()
    }

    /// Ce que l'engagement dit de `mois` : la saisie de ce mois-la, telle que l'ecran la montre et que le calcul la lit
    /// (`None` s'il ne le touche pas). Un engagement non actif dit encore ce qu'il etait, pour qu'il se lise. Elle se lit a
    /// la date, sans relire les mois d'entre-deux : la saisie du premier mois, ou, apres lui, ses termes et les faits du
    /// mois (`FAITS`) ; chaque terme y prend sa valeur au dernier jour du mois (`termes_au`).
    ///
    /// Un contrat paye au mois dont un terme change dans `mois` y porte la date de l'avenant (`date_modification`) : le
    /// document qui dit la modification la date de sa prise d'effet (code du travail R1221-40,
    /// `atelier/reference/sources/code-du-travail/articles-20261029.md`, l. 856), et la DSN date de meme le bloc
    /// S21.G00.41 (41.001). Un changement en cours de mois y porte aussi les termes de la veille (`avant` : le salaire et
    /// la duree du travail, `droit::periodes::TERMES_D_AVANT`), qui paient ses jours d'avant (`periodes::bornes`, chaque
    /// part par `droit::periodes::part_du_mois`).
    pub fn vue(&self, mois: &str) -> Resultat<Option<Map<String, Value>>> {
        if !self.dans_ses_bornes(mois)? {
            return Ok(None);
        }
        let premiere = self.premiere_vue();
        let mut v = if mois == self.fichier {
            premiere.clone()
        } else {
            let mut v: Map<String, Value> = TERMES.iter().chain(PARTAGES.iter()).filter_map(|k| premiere.get(*k).map(|x| (k.to_string(), x.clone()))).collect();
            match self.faits_de(mois) {
                Some(f) => v.extend(f.iter().filter(|(_, x)| !x.is_null()).map(|(k, x)| (k.clone(), x.clone()))),
                None if !self.mensuel() => {
                    v.insert("lignes".into(), Value::Array(self.lignes_du_planning(mois)?));
                }
                None => {}
            }
            v
        };
        self.termes_au(&mut v, &dernier_du_mois(mois)?)?;
        if mois > self.fichier.as_str() && self.mensuel() {
            self.avenant_du_mois(&mut v, mois)?;
        }
        for k in PARTAGES {
            match premiere.get(k) {
                Some(x) => v.insert(k.to_string(), x.clone()),
                None => v.remove(k),
            };
        }
        if mois > self.fichier.as_str() && !self.mensuel() {
            // Un contrat d'usage qui ne dit pas son debut commence a son premier jour joue, celui de son premier mois :
            // les mois suivants le declarent (S21.G00.40.001) et ne le prennent pas pour une embauche.
            let debut = crate::debut_brut(&self.racine, &Value::Object(premiere.clone()));
            if !v.contains_key("debut") && !debut.is_empty() {
                v.insert("debut".into(), Value::String(debut));
            }
            v.insert("jours_contrat".into(), Value::from(crate::jours_du_contrat(&self.racine, &Value::Object(premiere)) as i64));
        }
        Ok(Some(v))
    }

    /// Les faits que `mois` (apres le premier) dit de lui seul, s'il en dit.
    fn faits_de(&self, mois: &str) -> Option<&Map<String, Value>> {
        self.brut.get(FAITS).and_then(|f| f.get(mois)).and_then(Value::as_object)
    }

    /// Chaque changement d'un terme, du plus ancien au plus recent : le terme, son jour, sa valeur (`null` : retire).
    pub fn changements(&self) -> Vec<(String, Jour, Value)> {
        let mut out = Vec::new();
        for (k, liste) in self.brut.get(DATES).and_then(Value::as_object).into_iter().flatten() {
            for e in liste.as_array().into_iter().flatten() {
                if let Some(j) = e.get("depuis").and_then(|j| Jour::lire(&en_texte(j)).ok()) {
                    out.push((k.clone(), j, e.get("valeur").cloned().unwrap_or(Value::Null)));
                }
            }
        }
        out.sort_by_key(|(_, j, _)| *j);
        out
    }

    /// Pose dans `v` la valeur de chaque terme qui a change au plus tard le `jour` (la derniere a ce jour).
    fn termes_au(&self, v: &mut Map<String, Value>, jour: &Jour) -> Resultat<()> {
        for (k, j, x) in self.changements() {
            if j > *jour {
                continue;
            }
            if x.is_null() {
                v.remove(&k);
            } else {
                v.insert(k, x);
            }
        }
        Ok(())
    }

    /// Les termes du contrat au `jour` : ceux du premier mois, et chaque changement venu jusqu'a lui.
    fn termes_a(&self, jour: &Jour) -> Resultat<Map<String, Value>> {
        let premiere = self.premiere_vue();
        let mut v: Map<String, Value> = TERMES.iter().filter_map(|k| premiere.get(*k).map(|x| (k.to_string(), x.clone()))).collect();
        self.termes_au(&mut v, jour)?;
        Ok(v)
    }

    /// L'avenant de `mois` dans sa vue `v` (voir `vue`) : le dernier jour du mois ou un terme change, et, s'il n'est pas
    /// le 1er, les termes de la veille quand le salaire ou la duree du travail changent (l'avenant change autre chose :
    /// il se date, rien ne se coupe).
    fn avenant_du_mois(&self, v: &mut Map<String, Value>, mois: &str) -> Resultat<()> {
        let Some(j) = self.changements().into_iter().map(|(_, j, _)| j).filter(|j| j.mois_iso() == mois).max() else { return Ok(()) };
        v.insert("date_modification".into(), Value::String(j.iso()));
        if j.jour == 1 {
            return Ok(());
        }
        let a = self.termes_a(&j.plus_jours(-1)?)?;
        let terme = |x: &Map<String, Value>, k: &str| -> Resultat<Option<Value>> {
            match x.get(k).filter(|y| !y.is_null()) {
                Some(y) => Ok(Some(y.clone())),
                None if k == "quotite_contrat" => Ok(crate::conges::nature_de(x.get("nature"))?.quotite_reference_defaut.map(Value::from)),
                None => Ok(None),
            }
        };
        let mut avant = Map::new();
        let mut change = false;
        for k in droit::periodes::TERMES_D_AVANT {
            let (x, y) = (terme(&a, k)?, terme(v, k)?);
            change |= x.as_ref().map(canonique) != y.as_ref().map(canonique);
            if let Some(x) = x {
                avant.insert(k.to_string(), x);
            }
        }
        if change {
            v.insert("avant".into(), Value::Object(avant));
        }
        Ok(())
    }

    /// LES LIGNES D'UN MOIS SANS FAITS d'un contrat d'usage : les jours que les lignes du dernier mois qui en dit (le
    /// premier, ou un mois qui a ses faits) ont mis au planning de `mois` ; une ligne par ligne qui en a, a la meme
    /// rubrique, au meme taux et au meme lieu, ses jours de `mois` en dates et ceux d'apres au planning ; un cachet, une
    /// journee ou un service par date (une autre unite, des heures, ne se deduit pas d'un nombre de jours : quantite 0, a
    /// completer). Sans planning, aucune ligne.
    fn lignes_du_planning(&self, mois: &str) -> Resultat<Vec<Value>> {
        let dit = self.brut.get(FAITS).and_then(Value::as_object).and_then(|f| f.keys().filter(|m| m.as_str() < mois && m.as_str() > self.fichier.as_str()).max().cloned());
        let source = match &dit {
            Some(m) => self.faits_de(m).and_then(|f| f.get("lignes")),
            None => self.brut.get("lignes"),
        };
        let jour_de = |x: &Value| match x {
            Value::Object(m) => m.get("jour").map(en_texte).unwrap_or_default(),
            autre => en_texte(autre),
        };
        let mut lignes = Vec::new();
        for l in source.and_then(Value::as_array).into_iter().flatten() {
            let plan: Vec<Value> = l.get("planning").and_then(Value::as_array).cloned().unwrap_or_default();
            let dedans: Vec<Value> = plan.iter().filter(|x| jour_de(x).starts_with(mois)).cloned().collect();
            let apres: Vec<Value> = plan.iter().filter(|x| jour_de(x).get(..7).is_some_and(|m| m > mois)).cloned().collect();
            if dedans.is_empty() && apres.is_empty() {
                continue;
            }
            let rubrique = l.get("rubrique").map(en_texte).unwrap_or_else(|| "cachet".into());
            let unite = dernier_du_mois(mois).and_then(|j| droit::catalogue::rubrique(&self.racine, &rubrique, &j)).map(|r| r.unite).unwrap_or_default();
            let par_date = matches!(unite.as_str(), "cachet" | "jour" | "service");
            let mut n = Map::new();
            n.insert("rubrique".into(), Value::String(rubrique));
            n.insert("taux".into(), l.get("taux").cloned().unwrap_or(Value::Null));
            if let Some(lieu) = l.get("lieu").filter(|x| vrai(x)) {
                n.insert("lieu".into(), lieu.clone());
            }
            n.insert("quantite".into(), if par_date { Value::from(dedans.len()) } else { Value::from(0) });
            if let Some(h) = l.get("heures").filter(|x| par_date && vrai(x)) {
                n.insert("heures".into(), h.clone());
            }
            n.insert("dates".into(), Value::Array(dedans));
            n.insert("planning".into(), Value::Array(apres));
            lignes.push(Value::Object(n));
        }
        Ok(lignes)
    }
}

/// Une valeur telle qu'elle se compare : sa forme canonique (`canonique`), sans les listes vides (une ligne sans
/// `planning` et une ligne au planning vide disent la meme chose).
fn comparable(v: &Value) -> String {
    fn sans_vides(v: &Value) -> Value {
        match v {
            Value::Object(m) => Value::Object(m.iter().filter(|(_, x)| !x.as_array().is_some_and(Vec::is_empty)).map(|(k, x)| (k.clone(), sans_vides(x))).collect()),
            Value::Array(a) => Value::Array(a.iter().map(sans_vides).collect()),
            autre => autre.clone(),
        }
    }
    canonique(&sans_vides(v))
}

/// Tous les engagements, fichier par fichier, du plus ancien au plus recent, dans l'ordre de chaque fichier.
pub fn objets(racine: &Path) -> Resultat<Vec<Objet>> {
    Ok(tous(racine)?.1.objets.clone())
}

/// Les fichiers des engagements d'une compagnie tels qu'ils viennent d'etre lus : chaque mois, et ce que son fichier
/// contient (`socle::fichiers::json_partage` : la meme valeur tant que les octets sont les memes).
type Lus = Vec<(String, std::sync::Arc<Value>)>;

/// Deux lectures ont-elles trouve exactement les memes fichiers, aux memes octets ?
fn memes(a: &Lus, b: &Lus) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|((m, x), (n, y))| m == n && std::sync::Arc::ptr_eq(x, y))
}

fn lus(racine: &Path) -> Resultat<Lus> {
    let mut out = Vec::new();
    for f in fichiers(racine) {
        if let Some(v) = lire_partage(racine, &f)? {
            out.push((f, v));
        }
    }
    Ok(out)
}

/// Les engagements d'une compagnie tires de ses fichiers, et les bornes de chacun (son dernier mois), tirees avec lui :
/// lire un mois ne relit que les objets qui le touchent.
struct Tires {
    objets: Vec<Objet>,
    derniers: Vec<Result<Option<String>, String>>,
}

impl Tires {
    /// `mois` est-il entre le premier et le dernier mois de l'objet de rang `i` ?
    fn dans_ses_bornes(&self, i: usize, mois: &str) -> Resultat<bool> {
        let o = &self.objets[i];
        if mois < o.fichier.as_str() {
            return Ok(false);
        }
        Ok(match &self.derniers[i] {
            Ok(Some(d)) => mois <= d.as_str(),
            Ok(None) => true,
            Err(e) => return Err(e.clone()),
        })
    }

    /// L'objet de rang `i` touche-t-il `mois` (`Objet::couvre`) ?
    fn couvre(&self, i: usize, mois: &str) -> Resultat<bool> {
        let o = &self.objets[i];
        Ok(!o.est_non_actif() && !o.importe_seul() && self.dans_ses_bornes(i, mois)?)
    }

    /// Les objets qui touchent `mois`, dans l'ordre des fichiers.
    fn du_mois(&self, mois: &str) -> Resultat<Vec<&Objet>> {
        let mut out = Vec::new();
        for (i, o) in self.objets.iter().enumerate() {
            if o.fichier.as_str() <= mois && self.couvre(i, mois)? {
                out.push(o);
            }
        }
        Ok(out)
    }
}

/// TOUS LES ENGAGEMENTS D'UNE COMPAGNIE, tires une fois des memes fichiers : ce qu'on en avait tire ne sert que si ce
/// sont les memes lectures (`memes`) ; pendant un geste, les fichiers se lisent une fois (`socle::fichiers::geste`).
/// Partage : on n'y touche pas, et un objet se copie sans recopier ce qu'il porte (`Objet`).
fn tous(racine: &Path) -> Resultat<(Lus, std::sync::Arc<Tires>)> {
    type Gardes = std::sync::Mutex<std::collections::HashMap<PathBuf, (Lus, std::sync::Arc<Tires>)>>;
    static GARDES: std::sync::OnceLock<Gardes> = std::sync::OnceLock::new();
    let gardes = GARDES.get_or_init(Default::default);
    let fichiers = lus(racine)?;
    let avant = gardes.lock().unwrap_or_else(|p| p.into_inner()).get(racine).cloned();
    if let Some((vus, v)) = &avant {
        if memes(vus, &fichiers) {
            return Ok((fichiers, v.clone()));
        }
    }
    let mut objets = Vec::new();
    for (f, m) in &fichiers {
        // Les engagements d'un fichier qui n'a pas change depuis la lecture gardee sont ceux qu'elle a tires.
        let inchange = avant.as_ref().filter(|(vus, _)| vus.iter().any(|(n, y)| n == f && std::sync::Arc::ptr_eq(m, y)));
        if let Some((_, v)) = inchange {
            objets.extend(v.objets.iter().filter(|o| o.fichier == *f).cloned());
            continue;
        }
        for (rang, e) in m.get("engagements").and_then(Value::as_array).into_iter().flatten().enumerate() {
            if porte_ses_paies(e) {
                return Err(socle::echec::avec(
                    "Les engagements de cette compagnie portent encore leurs paies, une forme que ce logiciel ne lit plus : elle se refait, en l'ajoutant de nouveau et en reprenant ses mois (ses papiers s'importent). Rien n'est lu.",
                    serde_json::json!({"chemin": format!("{}/{}.json", DOSSIER, f)}),
                ));
            }
            if e.is_object() {
                objets.push(Objet { racine: racine.to_path_buf(), fichier: f.clone(), rang, brut: std::sync::Arc::new(e.clone()) });
            }
        }
    }
    let derniers = objets.iter().map(Objet::dernier_mois).collect();
    let out = std::sync::Arc::new(Tires { objets, derniers });
    socle::fichiers::ranger(gardes, racine.to_path_buf(), (fichiers.clone(), out.clone()));
    Ok((fichiers, out))
}

/// UN ENGAGEMENT NE PORTE PAS SES PAIES : elles vivent dans le fichier de leur mois (`paies`). Un engagement qui en porte
/// dans sa validation, celle qu'une modification remplace ou une version passee est d'une forme que le moteur ne lit pas.
fn porte_ses_paies(e: &Value) -> bool {
    let a_des_paies = |v: Option<&Value>| v.is_some_and(|v| v.get("paies").is_some());
    a_des_paies(e.get("valide"))
        || a_des_paies(e.get("precedente"))
        || e.get("versions").and_then(Value::as_array).is_some_and(|vs| vs.iter().any(|v| a_des_paies(Some(v))))
}

/// Les engagements qui touchent `mois`, dans l'ordre de l'ecran : ceux des fichiers d'avant qui courent encore, puis
/// ceux du mois.
pub fn objets_du_mois(racine: &Path, mois: &str) -> Resultat<Vec<Objet>> {
    Ok(tous(racine)?.1.du_mois(mois)?.into_iter().cloned().collect())
}

/// L'IDENTITE DE CHAQUE ENGAGEMENT D'UNE LISTE (le plan) : son salarie, son premier jour et son rang parmi ceux du meme
/// salarie qui commencent le meme jour (l'ordre de leur fichier, qui est le meme pour tous : le fichier de leur
/// premier mois). C'est elle que l'ecran tient pour designer une carte.
pub fn identites(objets: &[Objet]) -> Vec<Value> {
    let paires: Vec<(String, String)> = objets
        .iter()
        .map(|o| {
            let e = &*o.brut;
            (e.get("salarie").map(en_texte).unwrap_or_default(), crate::debut_brut(&o.racine, e))
        })
        .collect();
    paires
        .iter()
        .enumerate()
        .map(|(i, (s, d))| {
            let rang = paires[..i].iter().filter(|(s2, d2)| s2 == s && d2 == d).count();
            serde_json::json!({"salarie": s, "debut": d, "rang": rang})
        })
        .collect()
}

/// L'ENGAGEMENT QUE DESIGNE L'IDENTITE `id` (`identites`) dans l'ecran de `mois`, et sa place dans cet ecran (l'ordre
/// de `vue_du_mois`). Refus : une identite que l'ecran ne montre pas.
pub fn designe(racine: &Path, mois: &str, id: &Value) -> Resultat<(usize, Objet)> {
    let objets = objets_du_mois(racine, mois)?;
    let k = identites(&objets).iter().position(|x| x == id).ok_or_else(|| {
        format!("{} : l'écran ne montre pas cet engagement ; rechargez le mois. Rien n'est écrit.", crate::libelle_mois(mois))
    })?;
    Ok((k, objets.into_iter().nth(k).expect("la place vient de la liste")))
}

/// Les engagements non actifs (`NON_ACTIF`) qui toucheraient `mois` s'ils l'etaient encore : ce qui se lit d'eux.
pub fn non_actifs_du_mois(racine: &Path, mois: &str) -> Resultat<Vec<Objet>> {
    let t = tous(racine)?.1;
    let mut out = Vec::new();
    for (i, o) in t.objets.iter().enumerate() {
        if o.est_non_actif() && o.fichier.as_str() <= mois && t.dans_ses_bornes(i, mois)? {
            out.push(o.clone());
        }
    }
    Ok(out)
}

/// Efface l'objet `o` de son fichier (meme fichier, meme rang).
pub fn effacer(racine: &Path, o: &Objet) -> Resultat<PathBuf> {
    let mut contenu = lire_fichier(racine, &o.fichier)?.ok_or_else(|| format!("{} : le fichier de l'engagement a disparu", o.fichier))?;
    let Some(Value::Array(es)) = contenu.get_mut("engagements") else {
        return Err(socle::echec::a_la_cle("Un mois d'engagements ne se relit pas : il est abîmé.", "engagements", &o.fichier));
    };
    if o.rang >= es.len() || es[o.rang] != *o.brut {
        return Err(format!("{} : l'engagement {} a change sur le disque ; rien n'est efface", o.fichier, o.rang + 1));
    }
    es.remove(o.rang);
    poser(racine, &o.fichier, &Value::Object(contenu))
}

/// LE MOIS DE L'ECRAN : `{"mois", <defauts du mois>, "engagements": [la vue de chacun]}`, ou `None` quand aucun
/// engagement ne le touche et qu'il n'a pas de fichier. Un engagement valide y dit quand (`valide` : `{"le"}`).
pub fn vue_du_mois(racine: &Path, mois: &str) -> Resultat<Option<Value>> {
    Ok(vue_et_empreinte(racine, mois)?.map(|s| {
        socle::fichiers::noter_valeur(&cle_de_saisie(mois, None), s.1.clone());
        s.0.clone()
    }))
}

/// L'empreinte de la saisie de `mois` (`empreinte_de_saisie`) : celle que `vue_du_mois` note, ou, pour `salarie`, celle
/// que la paie de ses engagements du mois note ; `None` sans saisie.
pub fn empreinte_de_la_saisie_du_mois(racine: &Path, mois: &str, salarie: Option<&str>) -> Resultat<Option<String>> {
    Ok(vue_et_empreinte(racine, mois)?.map(|s| if salarie.is_none() { s.1.clone() } else { empreinte_de_saisie(&s.0, salarie) }))
}

/// La vue de `mois` et son empreinte, faites une fois pour les memes fichiers aux memes octets (`memes`) :
/// chaque paie qui se verifie relit la saisie des mois qu'elle a lus. La vue d'une ligne lit aussi l'unite de sa rubrique
/// (`droit::catalogue::rubrique`) : les deux catalogues en font partie.
pub(crate) fn vue_et_empreinte(racine: &Path, mois: &str) -> Resultat<Option<std::sync::Arc<(Value, String)>>> {
    type Saisie = Option<std::sync::Arc<(Value, String)>>;
    type Vues = std::sync::Mutex<std::collections::HashMap<(PathBuf, String), (Lus, Saisie)>>;
    static VUES: std::sync::OnceLock<Vues> = std::sync::OnceLock::new();
    static ABSENT: std::sync::OnceLock<std::sync::Arc<Value>> = std::sync::OnceLock::new();
    let vues = VUES.get_or_init(Default::default);
    let (mut lus, tous) = tous(racine)?;
    for p in [socle::fichiers::dossier_du_code().join("rubriques-base.json"), racine.join(socle::fichiers::CONFIG).join(droit::catalogue::RUBRIQUES_COMPAGNIE)] {
        // Absent, il est le meme d'une lecture a l'autre ; illisible, il ne l'est jamais (la vue se refait).
        let v = match socle::fichiers::json_partage(&p) {
            Ok(Some(v)) => v,
            Ok(None) => ABSENT.get_or_init(|| std::sync::Arc::new(Value::Null)).clone(),
            Err(_) => std::sync::Arc::new(Value::Null),
        };
        lus.push((p.to_string_lossy().into_owned(), v));
    }
    let cle = (racine.to_path_buf(), mois.to_string());
    if let Some((vus, s)) = vues.lock().unwrap_or_else(|p| p.into_inner()).get(&cle) {
        if memes(vus, &lus) {
            return Ok(s.clone());
        }
    }
    // Ce que le fichier du mois pose pour tous ; ses engagements, la vue les refait (ceux des mois d'avant qui courent).
    let fichier: Option<Map<String, Value>> = lire_partage(racine, mois)?
        .map(|v| v.as_object().into_iter().flatten().map(|(k, x)| (k.clone(), if k == "engagements" { Value::Null } else { x.clone() })).collect());
    let objets = tous.du_mois(mois)?;
    let saisie = if (fichier.is_none() || importes_seuls(racine, mois)) && objets.is_empty() {
        None
    } else {
        let mut data = fichier.unwrap_or_default();
        data.insert("mois".into(), Value::String(mois.to_string()));
        let mut engagements = Vec::new();
        for o in &objets {
            let vu = o.vue(mois)?;
            let Some(mut v) = vu else { continue };
            if let Some(le) = o.brut.get("valide").and_then(|x| x.get("le")) {
                v.insert("valide".into(), serde_json::json!({ "le": le }));
            }
            engagements.push(Value::Object(v));
        }
        data.insert("engagements".into(), Value::Array(engagements));
        let vue = Value::Object(data);
        let empreinte = empreinte_de_saisie(&vue, None);
        Some(std::sync::Arc::new((vue, empreinte)))
    };
    socle::fichiers::ranger(vues, cle, (lus, saisie.clone()));
    Ok(saisie)
}

/// La cle sous laquelle un calcul note la saisie d'un mois qu'il a lue (`socle::fichiers::suivre`) : `saisie <mois>`, ou
/// `saisie <mois> <salarie>` pour la saisie dont depend la paie de ce salarie dans son mois (`etats::calculer_les_rangs`).
pub fn cle_de_saisie(mois: &str, salarie: Option<&str>) -> String {
    match salarie {
        Some(s) => format!("saisie {} {}", mois, s),
        None => format!("saisie {}", mois),
    }
}

/// L'empreinte de la saisie d'un mois : ce qu'elle dit, sans ce que le moteur tient seul (la validation) ni le numero du
/// contrat (pose a l'envoi, il ne change aucun montant), les cles triees et un nombre ecrit comme un nombre. Deux parts,
/// `<le mois>:<ses engagements>` : ce que le mois pose pour tous (le taux du cachet), et ses
/// engagements ; la raison d'un recalcul dit laquelle a change (`etats::raison`). Seuls les engagements valides
/// comptent : une paie ecrite ne lit que ceux-la (`etats::calculer_les_rangs`), et un brouillon tape a cote d'elle ne
/// la fait pas recalculer. Avec `salarie`, seuls ses engagements comptent : la paie d'un engagement ne lit, des autres
/// engagements du mois, que ceux du meme salarie (`etats::calculer_les_rangs`).
pub fn empreinte_de_saisie(vue: &Value, salarie: Option<&str>) -> String {
    let mut v = vue.clone();
    if let Some(Value::Array(es)) = v.get_mut("engagements") {
        es.retain(|e| e.get("valide").is_some() && salarie.is_none_or(|s| e.get("salarie").and_then(Value::as_str) == Some(s)));
        for e in es.iter_mut() {
            if let Value::Object(m) = e {
                m.remove("valide");
                m.remove("numero");
            }
        }
    }
    let engagements = match v.as_object_mut() {
        Some(m) => {
            m.remove("_origine");
            m.remove("engagements").unwrap_or(Value::Null)
        }
        None => Value::Null,
    };
    let h = |x: &Value| socle::fichiers::sha256_hex(canonique(x).as_bytes());
    format!("{}:{}", h(&v), h(&engagements))
}

/// Une valeur ecrite sans ce qui ne change rien a ce qu'elle dit : les cles triees, un nombre ecrit comme un flottant
/// (`180` et `180.0` sont le meme taux : la page ecrit l'un, un fichier ancien l'autre).
pub fn canonique(v: &Value) -> String {
    match v {
        Value::Object(m) => {
            let mut cles: Vec<&String> = m.keys().collect();
            cles.sort();
            let corps: Vec<String> = cles.iter().map(|k| format!("{}:{}", Value::String((*k).clone()), canonique(&m[*k]))).collect();
            format!("{{{}}}", corps.join(","))
        }
        Value::Array(a) => format!("[{}]", a.iter().map(canonique).collect::<Vec<_>>().join(",")),
        Value::Number(n) => n.as_f64().map_or_else(|| n.to_string(), |f| format!("{:?}", f)),
        Value::String(t) if !t.is_empty() && t.bytes().all(|c| c.is_ascii_digit() || c == b'.' || c == b'-') && t.parse::<f64>().is_ok() => {
            format!("{:?}", t.parse::<f64>().unwrap_or_default())
        }
        autre => autre.to_string(),
    }
}

/// Les mois que la saisie a touches : les fichiers, et les mois que chaque engagement a saisis ou couvre jusqu'a sa fin ;
/// un contrat sans terme, jusqu'au dernier mois ecrit ou jusqu'au mois d'aujourd'hui s'il est plus loin : un CDI de
/// janvier que rien n'a saisi depuis a une paie, une DSN et des documents chaque mois qu'il court.
pub fn mois_saisis(racine: &Path) -> Resultat<Vec<String>> {
    let mut v: Vec<String> = fichiers(racine).into_iter().filter(|m| !importes_seuls(racine, m)).collect();
    let courant = droit::contexte::aujourdhui_sans_note()?.mois_iso();
    let horizon = v.iter().max().cloned().map(|h| h.max(courant));
    for o in objets(racine)?.into_iter().filter(|o| !o.importe_seul()) {
        v.extend(o.mois_saisis());
        let fin = match o.dernier_mois()? {
            Some(d) => Some(d),
            None => horizon.clone().filter(|h| h.as_str() >= o.fichier.as_str()),
        };
        if let Some(d) = fin {
            v.extend(mois_entre(&premier_du_mois(&o.fichier)?, &dernier_du_mois(&d)?));
        }
    }
    v.sort();
    v.dedup();
    Ok(v)
}

// ------------------------------------------------------------------ l'ecriture

/// L'ECRITURE D'UNE CARTE : l'engagement en saisie que designe l'identite `id` dans l'ecran de `mois` (`designe`)
/// prend `saisie`, ce que sa carte ecrit pour `mois` ; sans identite, l'engagement est neuf et commence dans le fichier
/// du mois (le fichier se cree avec ce que le mois dit par defaut, `defauts`). C'est la seule ecriture qui cree un
/// engagement. Rend l'identite de l'engagement ecrit (`identites`).
///
/// Ce que le moteur tient seul ne se prend pas de la saisie ; le numero du contrat et le nom du spectacle ecrits se
/// gardent (`poser_sur`). Refus, et rien n'est ecrit : un engagement valide (il se modifie d'un geste,
/// `etats::modifier`), une identite que l'ecran ne montre pas, un engagement du salarie et du premier jour d'un engagement
/// importe, des bornes qui laisseraient hors du contrat un mois ou il a des jours payes, un engagement neuf que l'ecran
/// de `mois` ne montrerait pas.
pub fn ecrire_la_carte(ctx: &Contexte, mois: &str, id: Option<&Value>, saisie: &Map<String, Value>, defauts: &Map<String, Value>) -> Resultat<Value> {
    let racine = &ctx.racine;
    let e = Value::Object(saisie.clone());
    if let Some(cle) = crate::cle_de_l_engagement(ctx, &e) {
        let debut = crate::debut_brut(racine, &e);
        if crate::importe::celui_de(racine, mois, &cle, &debut)?.is_some() {
            return Err(crate::importe::refus(&e.get("salarie").map(en_texte).unwrap_or(cle), &debut));
        }
    }
    let (fichier, rang) = match id {
        Some(id) => {
            let (_, mut o) = designe(racine, mois, id)?;
            if o.est_valide() {
                return Err(format!("{} : cet engagement est validé, il se modifie d'un geste. Rien n'est écrit.", crate::libelle_mois(mois)));
            }
            poser_sur(ctx, &mut o, mois, saisie, None)?;
            reecrire(racine, &o)?;
            (o.fichier, o.rang)
        }
        None => {
            refuser_un_second_contrat_au_mois(ctx, &e)?;
            let fichier = mois_d_ecriture(racine, mois, &e)?;
            let saisie = en_forme(saisie)?;
            let mut contenu = match lire_fichier(racine, &fichier)? {
                Some(c) => c,
                None => defauts.clone(),
            };
            contenu.insert("mois".into(), Value::String(fichier.clone()));
            let es = contenu.entry("engagements").or_insert_with(|| Value::Array(Vec::new()));
            let Value::Array(es) = es else { return Err(socle::echec::a_la_cle("Un mois d'engagements ne se relit pas : il est abîmé.", "engagements", mois)) };
            let rang = es.len();
            es.push(Value::Object(saisie));
            poser(racine, &fichier, &Value::Object(contenu))?;
            (fichier, rang)
        }
    };
    let objets = objets_du_mois(racine, mois)?;
    let k = objets
        .iter()
        .position(|o| o.fichier == fichier && o.rang == rang)
        .ok_or_else(|| format!("{} : l'engagement écrit ne tombe pas dans ce mois (ses dates le placent ailleurs).", crate::libelle_mois(mois)))?;
    Ok(identites(&objets).swap_remove(k))
}

/// UN CONTRAT AU MOIS N'A QU'UN ENGAGEMENT : une carte neuve d'un contrat paye au mois (CDI, CDD) dont le salarie a deja
/// un contrat au mois commence le meme jour (actif, et qui n'est pas qu'importe : `importe::refus` dit le sien) ne le refait pas, elle
/// refuse et n'ecrit rien ; cet engagement se designe et se modifie depuis sa carte. Le rang de l'identite (`identites`)
/// ne distingue que des contrats d'usage du meme jour (deux spectacles) ; deux contrats au mois du meme salarie
/// commencent deux jours differents (ils se suivent).
fn refuser_un_second_contrat_au_mois(ctx: &Contexte, e: &Value) -> Resultat<()> {
    let au_mois = |e: &Value| droit::natures::nature(e.get("nature").and_then(Value::as_str)).is_ok_and(|n| n.mensuelle());
    if !au_mois(e) {
        return Ok(());
    }
    let (Some(cle), debut) = (crate::cle_de_l_engagement(ctx, e), crate::debut_brut(&ctx.racine, e)) else { return Ok(()) };
    if debut.is_empty() {
        return Ok(());
    }
    let tires = tous(&ctx.racine)?.1;
    let Some(o) = tires.objets.iter().find(|o| {
        !o.est_non_actif()
            && !o.importe_seul()
            && au_mois(&o.brut)
            && crate::debut_brut(&ctx.racine, &o.brut) == debut
            && crate::cle_de_l_engagement(ctx, &o.brut).as_deref() == Some(cle.as_str())
    }) else {
        return Ok(());
    };
    let nature = droit::natures::nature(o.brut.get("nature").and_then(Value::as_str))?;
    let numero = o.brut.get("numero").filter(|n| vrai(n)).map_or_else(String::new, |n| format!(" (n° {})", en_texte(n)));
    Err(format!(
        "{} : son {} commencé le {}{} a déjà son engagement ; une carte neuve ne le refait pas, il se modifie depuis sa \
         carte. Un nouveau contrat commence un autre jour. Rien n'est écrit.",
        crate::salarie_nomme(ctx, e.get("salarie")),
        nature.libelle,
        Jour::lire(&debut).map_or(debut.clone(), |j| j.jj_mm_aaaa()),
        numero
    ))
}

/// LA SAISIE D'UN ENGAGEMENT VALIDE ROUVERT, POSEE SUR SON MOIS : l'engagement de rang `rang` de `mois`
/// (`objets_du_mois`), remis en saisie (`etats::modifier`), prend `saisie` (`poser_sur`) ; un terme qui y change vaut a
/// partir du `jour` de ce mois quand il est dit, du 1er sinon. Refus : un engagement valide, un jour qui n'est pas un
/// jour du contrat dans ce mois, ou des bornes qui laisseraient hors du contrat un mois ou il a des jours payes ; rien
/// n'est ecrit.
pub fn poser_la_saisie(ctx: &Contexte, mois: &str, rang: usize, saisie: &Map<String, Value>, jour: Option<u32>) -> Resultat<()> {
    let mut o = objet_du_mois(&ctx.racine, mois, rang)?.ok_or_else(|| format!("{} : pas d'engagement de rang {}", mois, rang + 1))?;
    if o.est_valide() {
        return Err(format!("{} : l'engagement de rang {} est valide, il ne prend pas de saisie", mois, rang + 1));
    }
    let le = match jour {
        None => None,
        Some(n) => {
            let premier = premier_du_mois(mois)?;
            let qui = crate::salarie_nomme(ctx, o.brut.get("salarie"));
            // AU PREMIER MOIS DE L'ENGAGEMENT, ses termes valent de son debut : un jour n'a rien a dater.
            if mois == o.fichier {
                return Err(format!(
                    "{} : {} est le premier mois de cet engagement, ce qu'il dit vaut depuis le début du contrat ; un jour \
                     n'y date rien. Pour que le contrat commence un autre jour, changez son début (« Début du contrat »). Rien n'est écrit.",
                    qui,
                    crate::libelle_mois(mois)
                ));
            }
            let j = Jour::nouveau(premier.an, premier.mois, n)
                .ok_or_else(|| format!("{} : « le {} {} » n'est pas un jour de ce mois. Rien n'est écrit.", qui, n, crate::libelle_mois(mois)))?;
            let debut = socle::date::d(o.brut.get("debut").unwrap_or(&Value::Null))?;
            let fin = o.brut.get("fin").filter(|f| vrai(f)).map(socle::date::d).transpose()?;
            if j < debut || fin.is_some_and(|f| j > f) {
                return Err(format!(
                    "{} : le contrat court du {}{} ; un changement au {} n'en est pas un jour. Rien n'est écrit.",
                    qui,
                    debut.jj_mm_aaaa(),
                    fin.map(|f| format!(" au {}", f.jj_mm_aaaa())).unwrap_or_default(),
                    j.jj_mm_aaaa()
                ));
            }
            // Le premier jour du contrat dans le mois : le changement couvre tout ce que le mois en paie.
            (j > debut).then_some(j)
        }
    };
    poser_sur(ctx, &mut o, mois, saisie, le)?;
    reecrire(&ctx.racine, &o)?;
    Ok(())
}

/// Ce que la carte ecrit pour `mois`, pose sur l'objet `o` : ce que le moteur tient seul ne se prend pas de la saisie,
/// le numero du contrat ecrit se garde, un spectacle nomme autrement que celui qui est ecrit (sa cle, son titre, son
/// numero d'objet) garde son nom ecrit.
fn poser_sur(ctx: &Contexte, o: &mut Objet, mois: &str, saisie: &Map<String, Value>, le: Option<Jour>) -> Resultat<()> {
    let mut saisie = en_forme(saisie)?;
    if !saisie.get("numero").is_some_and(vrai) {
        if let Some(n) = o.brut.get("numero").filter(|n| vrai(n)).cloned() {
            saisie.insert("numero".into(), n);
        }
    }
    let vue = o.vue(mois)?;
    // Le debut qu'un mois suivant d'un contrat d'usage deduit de son premier jour (`Objet::vue`) n'est pas une saisie.
    if mois != o.fichier && !o.champs().contains_key("debut") && saisie.get("debut").is_some_and(|d| vue.as_ref().and_then(|v| v.get("debut")) == Some(d)) {
        saisie.remove("debut");
    }
    if let Some(ecrit) = vue.and_then(|v| v.get("spectacle").filter(|s| vrai(s)).cloned()) {
        if saisie.get("spectacle").is_some_and(|s| *s != ecrit && meme_spectacle(ctx, s, &ecrit)) {
            saisie.insert("spectacle".into(), ecrit);
        }
    }
    poser_le_mois(o, mois, &saisie, le)
}

/// CE QU'UNE CARTE ECRIT, DANS LA FORME ECRITE : ni ce que le moteur tient seul ni ce qu'il deduit ; la nature
/// seulement si elle n'est pas celle par defaut (un engagement muet a la nature par defaut, `natures::defaut`) ; `cadre`
/// seulement s'il s'ecarte du defaut du metier au catalogue (`emplois::metier`). Une carte ecrite deux fois a
/// l'identique s'ecrit donc de meme, et sa relecture ne voit aucun changement.
/// L'apercu lit chaque carte dans cette forme aussi.
pub fn en_forme(saisie: &Map<String, Value>) -> Resultat<Map<String, Value>> {
    let mut saisie = saisie.clone();
    for k in TENUS_PAR_LE_MOTEUR.iter().chain(DEDUITS.iter()) {
        saisie.remove(*k);
    }
    if saisie.get("nature").is_none_or(|n| !vrai(n)) || saisie.get("nature").map(en_texte) == Some(droit::natures::defaut()?) {
        saisie.remove("nature");
    }
    let catalogue = match saisie.get("metier").filter(|m| vrai(m)) {
        Some(m) => droit::emplois::metier(&en_texte(m)).ok().flatten().map(|f| f.get("cadre_par_defaut").is_some_and(vrai)),
        None => None,
    };
    if catalogue.is_some() && saisie.get("cadre").and_then(Value::as_bool) == catalogue {
        saisie.remove("cadre");
    }
    if droit::natures::nature(saisie.get("nature").and_then(Value::as_str))?.mensuelle() {
        // UN PERMANENT N'A PAS DE SPECTACLE : le numero d'objet lui est interdit (CCH-11).
        saisie.remove("spectacle");
    } else if saisie.get("lignes").is_none() && saisie.get("debut").is_some_and(vrai) && saisie.get("fin").is_some_and(vrai) {
        // UN MOIS DU CONTRAT SANS CACHET : un contrat aux bornes dites, sans ligne, s'ecrit `lignes: []` -- la DSN porte
        // le contrat avec un versement a zero, sans bulletin.
        saisie.insert("lignes".into(), Value::Array(Vec::new()));
    }
    Ok(saisie)
}

/// `a` et `b` nomment-ils la meme fiche de spectacle (`Contexte::spectacle`) ?
fn meme_spectacle(ctx: &Contexte, a: &Value, b: &Value) -> bool {
    let cle = |v: &Value| ctx.spectacle(&en_texte(v)).ok().and_then(|f| f.get("_cle").cloned());
    cle(a).is_some_and(|x| Some(x) == cle(b))
}

/// Pose la saisie que l'ecran montre pour `mois` sur l'objet `o` : son premier mois s'il commence en `mois` ; sinon,
/// LE MOIS DE LA CARTE EST LE « A PARTIR DE » : chaque terme qu'elle dit autrement que l'engagement a la fin du mois
/// change a partir du `le` dit, du 1er du mois sinon (`dater`), et vaut jusqu'au changement suivant ; ce qu'elle dit
/// d'autre (ses lignes, ses conges, ses primes, sa rupture) est le fait du mois, qui ne s'ecrit que s'il dit autre chose
/// que ce que le mois tient seul (les jours que le planning y met). Ce que tous les mois partagent (`PARTAGES`) va a
/// l'objet. Les bornes ne laissent pas hors du contrat un mois ou il a des jours payes.
fn poser_le_mois(o: &mut Objet, mois: &str, saisie: &Map<String, Value>, le: Option<Jour>) -> Resultat<()> {
    let fin_du_mois = dernier_du_mois(mois)?;
    let actuels = o.termes_a(&fin_du_mois)?;
    for k in PARTAGES {
        match saisie.get(k).filter(|x| !x.is_null()) {
            Some(x) => o.brut_mut().insert(k.to_string(), x.clone()),
            None if k == "numero" => None,
            None => o.brut_mut().remove(k),
        };
    }
    // La fin que la carte dit d'un autre mois que le sien va au mois ou le contrat finit (`DE_LA_FIN`).
    let dernier = o.dernier_mois()?.filter(|d| d != mois);
    let (reste, de_la_fin): (Map<String, Value>, Map<String, Value>) = match &dernier {
        Some(_) => saisie.clone().into_iter().partition(|(k, x)| !DE_LA_FIN.contains(&k.as_str()) || x.is_null()),
        None => (saisie.clone(), Map::new()),
    };
    let saisie = &reste;
    if mois == o.fichier {
        let tenus: Vec<(&str, Option<Value>)> = TENUS_PAR_LE_MOTEUR.iter().map(|k| (*k, o.brut_mut().remove(*k))).collect();
        let mut neuf = saisie.clone();
        for k in PARTAGES {
            match o.brut.get(k) {
                Some(x) => neuf.insert(k.to_string(), x.clone()),
                None => neuf.remove(k),
            };
        }
        o.brut = std::sync::Arc::new(Value::Object(neuf));
        for (k, v) in tenus {
            if let Some(v) = v {
                o.brut_mut().insert(k.to_string(), v);
            }
        }
    } else {
        let depuis = match le {
            Some(j) => j,
            None => premier_du_mois(mois)?,
        };
        for k in TERMES {
            let nouveau = saisie.get(k).filter(|x| !x.is_null());
            if actuels.get(k).map(comparable) != nouveau.map(comparable) {
                dater(o, k, mois, depuis, nouveau.cloned())?;
            }
        }
        let faits: Map<String, Value> = saisie
            .iter()
            .filter(|(k, _)| ![&TERMES[..], &PARTAGES[..], &DEDUITS[..], &TENUS_PAR_LE_MOTEUR[..]].iter().any(|l| l.contains(&k.as_str())))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let seul = if o.mensuel() { Map::new() } else { Map::from_iter([("lignes".to_string(), Value::Array(o.lignes_du_planning(mois)?))]) };
        let tous = o.brut_mut().entry(FAITS.to_string()).or_insert_with(|| Value::Object(Map::new()));
        if let Value::Object(tous) = tous {
            if comparable(&Value::Object(faits.clone())) == comparable(&Value::Object(seul)) {
                tous.remove(mois);
            } else {
                tous.insert(mois.to_string(), Value::Object(faits));
                tous.sort_keys();
            }
        }
    }
    if let Some(d) = dernier.filter(|_| !de_la_fin.is_empty()) {
        poser_la_fin(o, &d, de_la_fin)?;
    }
    // Les bornes ne laissent pas hors du contrat un mois ou il a des jours payes.
    let mois_des_faits: Vec<String> = o.brut.get(FAITS).and_then(Value::as_object).map(|f| f.keys().cloned().collect()).unwrap_or_default();
    for m in mois_des_faits {
        if o.dans_ses_bornes(&m)? {
            continue;
        }
        if o.faits_de(&m).and_then(|f| f.get("lignes")).and_then(Value::as_array).is_some_and(|l| !l.is_empty()) {
            return Err(format!(
                "Le contrat irait du {} au {}, et {} y porte des jours payes : un mois de paie ne tient pas un contrat \
                 qui ne le couvre pas. Rien n'est ecrit. Retirer d'abord les lignes de {}, ou garder des bornes qui le \
                 couvrent.",
                o.brut.get("debut").map(en_texte).unwrap_or_default(),
                o.brut.get("fin").map(en_texte).unwrap_or_else(|| "?".into()),
                crate::libelle_mois(&m),
                crate::libelle_mois(&m)
            ));
        }
        if let Some(Value::Object(f)) = o.brut_mut().get_mut(FAITS) {
            f.remove(&m);
        }
    }
    for k in SAISIS_PAR_MOIS {
        if o.brut.get(k).and_then(Value::as_object).is_some_and(Map::is_empty) {
            o.brut_mut().remove(k);
        }
    }
    Ok(())
}

/// CE QUE LA FIN DU CONTRAT DIT D'ELLE : son motif, ses dates (notifiee, engagee, signee), son preavis, la transaction,
/// le dernier jour paye, et ce que le motif appelle (le droit a la retraite, le preavis non effectue, l'indemnite de fin
/// de contrat ecartee). Ce sont les faits du mois ou le contrat finit : la paie de ce mois-la et le signalement de fin de
/// contrat les lisent (S21.G00.62, S21.G00.63), aucun autre mois.
pub const DE_LA_FIN: [&str; 12] = [
    "motif_rupture",
    "notification_rupture",
    "engagement_licenciement",
    "signature_convention",
    "preavis",
    "transaction",
    "dernier_jour_paye",
    "en_droit_a_la_retraite",
    "preavis_non_effectue",
    "preavis_mois",
    "precarite_due",
    "precarite_pourquoi",
];

/// LA FIN DITE SUR LA CARTE D'UN AUTRE MOIS s'ecrit au mois `dernier`, celui ou le contrat finit (`DE_LA_FIN`) : au
/// premier mois s'il finit dans celui-ci, sinon parmi ses faits ; un contrat d'usage dont ce mois n'avait pas de faits
/// y garde les jours que son planning y met.
fn poser_la_fin(o: &mut Objet, dernier: &str, de_la_fin: Map<String, Value>) -> Resultat<()> {
    if dernier == o.fichier {
        o.brut_mut().extend(de_la_fin);
        return Ok(());
    }
    let mut faits = match o.faits_de(dernier) {
        Some(f) => f.clone(),
        None if o.mensuel() => Map::new(),
        None => Map::from_iter([("lignes".to_string(), Value::Array(o.lignes_du_planning(dernier)?))]),
    };
    faits.extend(de_la_fin);
    let tous = o.brut_mut().entry(FAITS.to_string()).or_insert_with(|| Value::Object(Map::new()));
    if let Value::Object(tous) = tous {
        tous.insert(dernier.to_string(), Value::Object(faits));
        tous.sort_keys();
    }
    Ok(())
}

/// UN TERME CHANGE A PARTIR DU `depuis` de `mois` : ce que le mois disait de lui s'efface (la carte du mois le redit), et
/// sa nouvelle valeur (`None` : retire) s'ecrit a ce jour, sauf si c'est deja la sienne la veille. Ce qui precede et les
/// changements des mois d'apres restent ce qu'ils sont.
fn dater(o: &mut Objet, terme: &str, mois: &str, depuis: Jour, valeur: Option<Value>) -> Resultat<()> {
    let jour_de = |e: &Value| e.get("depuis").and_then(|j| Jour::lire(&en_texte(j)).ok());
    let dates = o.brut_mut().entry(DATES.to_string()).or_insert_with(|| Value::Object(Map::new()));
    let Value::Object(dates) = dates else { return Err(socle::echec::cle("Un engagement ne se relit pas : il est abîmé.", DATES)) };
    let mut liste: Vec<Value> = dates.get(terme).and_then(Value::as_array).cloned().unwrap_or_default();
    liste.retain(|e| jour_de(e).is_none_or(|j| j.mois_iso() != mois));
    dates.insert(terme.to_string(), Value::Array(liste.clone()));
    let veille = o.termes_a(&depuis.plus_jours(-1)?)?;
    if veille.get(terme).map(comparable) != valeur.as_ref().map(comparable) {
        liste.push(serde_json::json!({"depuis": depuis.iso(), "valeur": valeur.unwrap_or(Value::Null)}));
        liste.sort_by_key(|e| jour_de(e));
    }
    let Some(Value::Object(dates)) = o.brut_mut().get_mut(DATES) else { return Err(socle::echec::cle("Un engagement ne se relit pas : il est abîmé.", DATES)) };
    if liste.is_empty() {
        dates.remove(terme);
    } else {
        dates.insert(terme.to_string(), Value::Array(liste));
    }
    Ok(())
}

impl Objet {
    /// L'engagement tel qu'une saisie gardee (`saisie`, celle d'une validation) le disait : meme fichier, meme rang.
    pub fn d_apres_la_saisie(&self, saisie: &Map<String, Value>) -> Objet {
        Objet { racine: self.racine.clone(), fichier: self.fichier.clone(), rang: self.rang, brut: std::sync::Arc::new(Value::Object(saisie.clone())) }
    }
}

/// L'engagement que designe, dans le mois `mois`, le rang `rang` de son ecran.
pub fn objet_du_mois(racine: &Path, mois: &str, rang: usize) -> Resultat<Option<Objet>> {
    Ok(objets_du_mois(racine, mois)?.into_iter().nth(rang))
}

/// Remplace l'objet `o` dans son fichier (meme fichier, meme rang), tel qu'il est maintenant.
pub fn reecrire(racine: &Path, o: &Objet) -> Resultat<PathBuf> {
    let mut contenu = lire_fichier(racine, &o.fichier)?.ok_or_else(|| format!("{} : le fichier de l'engagement a disparu", o.fichier))?;
    let Some(Value::Array(es)) = contenu.get_mut("engagements") else {
        return Err(socle::echec::a_la_cle("Un mois d'engagements ne se relit pas : il est abîmé.", "engagements", &o.fichier));
    };
    let place = es.get_mut(o.rang).ok_or_else(|| format!("{} : l'engagement {} a disparu", o.fichier, o.rang + 1))?;
    *place = (*o.brut).clone();
    poser(racine, &o.fichier, &Value::Object(contenu))
}

/// LE MOIS D'ENTREE DE LA COMPAGNIE DANS LE LOGICIEL : le premier mois qu'elle y paie, celui du plus ancien fichier qui
/// porte un engagement d'ici (qui n'est pas qu'importe, ni plus actif), ou `mois` s'il vient avant (la carte d'un premier mois, pas
/// encore ecrite). Les mois repris d'un ancien logiciel (`importe`) sont d'avant lui.
pub fn mois_d_entree(racine: &Path, mois: &str) -> Resultat<String> {
    let premier = tous(racine)?.1.objets.iter().find(|o| !o.importe_seul() && !o.est_non_actif()).map(|o| o.fichier.clone());
    Ok(premier.filter(|p| p.as_str() < mois).unwrap_or_else(|| mois.to_string()))
}

/// LE MOIS OU S'ECRIT L'ENGAGEMENT NEUF `e` que la carte de `mois` saisit : un engagement appartient a son premier mois.
/// Un contrat paye au mois dont le debut tombe dans un mois d'avant la carte, mais pas avant le logiciel
/// (`commence_avant_le_logiciel`), s'ecrit dans le mois de son debut, comme saisi sur sa carte : ce mois montre ce qui est
/// a faire (sa paie, sa DPAE). Sinon, `mois`.
pub fn mois_d_ecriture(racine: &Path, mois: &str, e: &Value) -> Resultat<String> {
    let mensuel = droit::natures::nature(e.get("nature").and_then(Value::as_str)).is_ok_and(|n| n.mensuelle());
    let debut = crate::debut_brut(racine, e);
    let premier = debut.get(..7).filter(|m| socle::date::mois_valide(m)).map(String::from);
    Ok(match premier {
        Some(p) if mensuel && p.as_str() < mois && !commence_avant_le_logiciel(racine, &debut, mois)? => p,
        _ => mois.to_string(),
    })
}

/// UN CONTRAT COMMENCE AVANT LE LOGICIEL : son debut (`AAAA-MM-JJ`) precede le mois d'entree de la compagnie
/// (`mois_d_entree`, vu du mois `mois`). Il a commence ailleurs : sa DPAE et son contrat se sont faits avant (rien a
/// declarer ni a signer ici), son numero se dit s'il a deja ete declare en DSN, et ses conges acquis avant se reprennent
/// de l'ancien bulletin (la fiche, `conges_reprise`).
/// Sa date de debut se declare telle quelle : S21.G00.40.001 est « la date du premier jour d'applicabilité du contrat »,
/// identifiante, sans autre borne que le dernier jour du mois declare (cahier technique DSN 2026.1, § 4.4.12,
/// `atelier/reference/norme-dsn/texte/chapitre-4-controles.txt` ; bloc S21.G00.40, `texte-2027/bloc-S21.G00.40.txt`) ;
/// son numero, identifiant aussi, se reporte a l'identique d'une DSN a la suivante (§ 2.1.2, `chapitre-2-principes-de-constitution.txt`).
pub fn commence_avant_le_logiciel(racine: &Path, debut: &str, mois: &str) -> Resultat<bool> {
    Ok(debut.get(..7).is_some_and(|m| socle::date::mois_valide(m)) && debut[..7] < *mois_d_entree(racine, mois)?)
}

/// Le jour `AAAA-MM-JJ` de `mois` qui est le premier, pour les bornes d'un contrat.
pub fn premier_jour(mois: &str) -> Resultat<Jour> {
    premier_du_mois(mois)
}

/// Le mois a-t-il une saisie : un fichier, ou un engagement qui le touche ?
pub fn a_une_saisie(racine: &Path, mois: &str) -> Resultat<bool> {
    if chemin(racine, mois).is_file() && !importes_seuls(racine, mois) {
        return Ok(true);
    }
    for o in objets(racine)? {
        if o.fichier.as_str() <= mois && o.couvre(mois)? {
            return Ok(true);
        }
    }
    Ok(false)
}
