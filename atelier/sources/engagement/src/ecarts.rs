//! LES ECARTS DE CALCUL D'UN ENGAGEMENT IMPORTE : ce que ce moteur aurait calcule de la saisie qu'un autre logiciel a
//! payee, compare ligne a ligne a ce que son bulletin imprime. Le but est de voir ou deux logiciels lisent les textes
//! autrement ; l'engagement importe ne change pas (il se lit, il ne se recalcule pas : `importe`), et la paie calculee
//! ici ne s'ecrit nulle part.
//!
//! LA SAISIE REFAITE : le salarie (sa fiche, a la date de la periode ; le statut, la qualite de cadre et l'emploi que
//! le bulletin imprime passent devant les siens), le spectacle que le bulletin nomme, la nature que la DSN ou l'AEM atteste (muette : un CDDU, regle 6), les lignes de remuneration que le
//! bulletin imprime, par leur rubrique du catalogue (quantite, taux, heures), aux jours qu'il dit travailles (a defaut,
//! autant de jours de sa periode qu'il en compte : `comparer_une`), payee le
//! jour du versement. Elle se calcule comme la validation la calculerait (`charger_donnees`), avec les baremes et la
//! configuration a la date de la periode ; une compagnie qui n'a pas encore pose son taux AT/MP de l'annee lit celui que
//! le bulletin applique (`Contexte::avec_le_taux_atmp_lu`).
//!
//! LA COMPARAISON, PAR COTISATION : chaque ligne de cotisation du bulletin importe porte la cotisation que l'import lui a
//! donnee (`cotisation`, par son libelle) ; chaque ligne de la paie calculee, la sienne (`cotisation_de`).
//! Plusieurs lignes d'une meme cotisation s'additionnent (les tranches de l'Apec, les contributions de « Autres charges
//! dues par l'employeur ») ; leur base et leurs taux ne se comparent que quand chaque cote n'en a qu'une. Un bulletin au
//! modele de l'arrete du 25/02/2016 (art. 3) range la caisse des Conges Spectacles dans « Autres charges dues par
//! l'employeur » et la CET dans « Retraite complementaire, CEG et CET T1 » : quand le bulletin importe ne les imprime
//! pas a part, les notres y entrent aussi (`REPLIS`). Un montant compare a l'arrondi de chaque ligne pres (1 centime par
//! ligne additionnee), un taux a la milliemes de point pres (le bulletin en imprime trois decimales).
//!
//! CE QUI NE SE COMPARE PAS LE DIT (regle 1) : une periode sans bareme, une paie sans bulletin (la DSN seule ne porte
//! pas les lignes du bulletin), une ligne de remuneration soumise sans rubrique, une saisie que le calcul refuse (une
//! convention sans couche, une fiche incomplete) ; et, dans une paie comparee, les lignes de cotisation que la table ne
//! rattache a aucune cotisation.
//!
//! CE QUI SE REND NE DESIGNE PERSONNE : la cotisation, les libelles, la grandeur, les deux taux et l'ecart RELATIF d'une
//! base ou d'un montant (jamais le montant), la periode, la nature et le statut.

use std::collections::BTreeMap;
use std::path::Path;


use crate::calcul::LigneCotisation;
use crate::importe::{Importe, PaieVersee};
use droit::contexte::Contexte;
use socle::mois::Mois;
use socle::num::{dec_f64, Decimal};

/// Ce que l'on compare d'une ligne.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Grandeur {
    Base,
    TauxSalarie,
    TauxEmployeur,
    MontantSalarie,
    MontantEmployeur,
    /// Le bulletin importe n'imprime pas une cotisation que ce moteur calcule.
    AbsenteDuBulletin,
    /// Le bulletin importe imprime une cotisation que ce moteur ne calcule pas.
    AbsenteDuCalcul,
}

impl Grandeur {
    pub fn code(self) -> &'static str {
        match self {
            Grandeur::Base => "base",
            Grandeur::TauxSalarie => "taux_salarie",
            Grandeur::TauxEmployeur => "taux_employeur",
            Grandeur::MontantSalarie => "montant_salarie",
            Grandeur::MontantEmployeur => "montant_employeur",
            Grandeur::AbsenteDuBulletin => "absente_du_bulletin",
            Grandeur::AbsenteDuCalcul => "absente_du_calcul",
        }
    }

    /// En francais, pour un resume.
    pub fn dite(self) -> &'static str {
        match self {
            Grandeur::Base => "base",
            Grandeur::TauxSalarie => "taux salarial",
            Grandeur::TauxEmployeur => "taux employeur",
            Grandeur::MontantSalarie => "montant salarial",
            Grandeur::MontantEmployeur => "montant employeur",
            Grandeur::AbsenteDuBulletin => "calculée ici, absente du bulletin importé",
            Grandeur::AbsenteDuCalcul => "sur le bulletin importé, que ce moteur ne calcule pas",
        }
    }
}

/// Un ecart sur une cotisation (ou un total : `total_salarie`, `total_employeur`, `net_social`, `net_imposable`).
#[derive(Clone, Debug, PartialEq)]
pub struct Ecart {
    pub cotisation: String,
    /// Les libelles du bulletin importe pour cette cotisation (les notres quand il ne l'imprime pas).
    pub libelles: Vec<String>,
    pub grandeur: Grandeur,
    pub notre_taux: Option<Decimal>,
    pub leur_taux: Option<Decimal>,
    /// (le leur - le notre) / le notre, a 4 decimales, pour une base ou un montant ; `None` pour un taux, ou quand le
    /// notre est nul.
    pub ecart_relatif: Option<Decimal>,
}

/// La comparaison d'une paie importee (un mois de l'engagement).
#[derive(Clone, Debug)]
pub struct Comparaison {
    /// L'empreinte de la paie (`empreinte`).
    pub empreinte: String,
    pub mois: String,
    /// Le logiciel que le lecteur a reconnu au bulletin ; `None` : un autre.
    pub logiciel: Option<String>,
    pub nature: Option<String>,
    pub statut: Option<String>,
    /// Cadre (`true`) ou non cadre, quand le bulletin ou la fiche le dit.
    pub cadre: Option<bool>,
    /// D'ou vient le taux AT/MP du calcul : `compagnie` (celui qu'elle a pose pour l'annee) ou `bulletin` (celui que le
    /// bulletin importe applique : l'AT/MP n'est alors pas compare).
    pub taux_atmp: Option<&'static str>,
    /// Les ecarts ; `Err` : la paie ne se compare pas, et pourquoi.
    pub ecarts: Result<Vec<Ecart>, String>,
    /// La paie ne s'est pas comparee parce que le calcul a refuse la saisie (une configuration a completer, l'effectif
    /// de l'entreprise...) : elle se compare de nouveau quand la compagnie s'ouvre. Les autres raisons tiennent aux
    /// pieces importees et ne changent pas.
    pub a_refaire: bool,
    /// Les libelles de cotisation du bulletin que la table ne rattache a aucune cotisation : non compares.
    pub non_rattachees: Vec<String>,
}

/// Quand le bulletin importe n'imprime pas la premiere a part, elle se compare dans la seconde (le modele de l'arrete
/// du 25/02/2016, art. 3 : « Autres charges dues par l'employeur » ; la ligne « Retraite complementaire, CEG et CET »).
const REPLIS: [(&str, &str); 2] = [("conges_spec", "autres"), ("cet", "retraite_t1")];

/// La cotisation d'une ligne de la paie calculee, au grain ou un bulletin l'imprime : les contributions que
/// l'employeur paie seul et que le modele agrege (`autres`), les cotisations de la convention (`convention`), les
/// tranches de l'Apec et de la prevoyance ; les autres gardent leur identifiant du bareme.
pub fn cotisation_de(l: &LigneCotisation) -> String {
    let seule_employeur = l.taux_sal.is_none() && l.montant_sal.is_zero();
    match l.id.as_str() {
        "autres_1" | "autres_2" | "autres_3" | "autres_4" | "versement_mobilite" if seule_employeur => "autres".into(),
        "ccn" | "fcap" | "paritarisme" => "convention".into(),
        "apec_t1" | "apec_t2" => "apec".into(),
        id if id.starts_with("prevoyance") => "prevoyance".into(),
        id => id.into(),
    }
}

/// Une ligne de cotisation du bulletin importe.
#[derive(Clone, Debug, Default)]
struct Lue {
    libelle: String,
    base: Option<Decimal>,
    taux_salarie: Option<Decimal>,
    taux_employeur: Option<Decimal>,
    montant_salarie: Option<Decimal>,
    montant_employeur: Option<Decimal>,
}


/// Les comparaisons de chaque paie de l'engagement importe `imp`, mois par mois. `deja` : la paie de cette empreinte (`empreinte`) est
/// deja comparee, elle ne se compare pas deux fois.
pub fn comparer(racine: &Path, imp: &Importe, deja: &dyn Fn(&str) -> bool) -> Vec<Comparaison> {
    let brut = imp.objet().carte();
    let contrat = imp.contrat();
    let nature = imp.nature();
    // UN CONTRAT DONT LA NATURE RESTE A DIRE (`nature_a_dire`, celui d'un permanent : `import::nature`) ne se compare
    // pas encore : muet, il se calculerait en contrat d'usage (regle 6). Il se compare une fois sa nature dite.
    if imp.a_dire() {
        return Vec::new();
    }
    let paies = imp.paies();
    // Le nombre de jours travailles que le bulletin compte (son cumul « Jours ») : celui que la paie du mois porte (le
    // mois d'un permanent), sinon celui du contrat quand il n'a qu'une paie.
    let du_contrat = contrat.and_then(|c| c.jours).filter(|_| paies.len() == 1);
    paies
        .iter()
        .filter_map(|(mois, p)| {
            let empreinte = empreinte(imp, mois, p);
            if deja(&empreinte) {
                return None;
            }
            let logiciel = p.bulletin.as_ref().and_then(|b| b.logiciel.clone()).filter(|l| !l.is_empty());
            let mut c = Comparaison {
                empreinte,
                mois: mois.clone(),
                logiciel,
                nature: nature.clone(),
                statut: None,
                cadre: None,
                taux_atmp: None,
                ecarts: Ok(Vec::new()),
                a_refaire: false,
                non_rattachees: Vec::new(),
            };
            let jours_comptes = p.jours.or(du_contrat);
            c.ecarts = comparer_une(racine, brut, mois, p, nature.as_deref(), jours_comptes, &mut c);
            Some(c)
        })
        .collect()
}

fn comparer_une(
    racine: &Path,
    brut: &crate::saisie::Carte,
    mois: &str,
    p: &PaieVersee,
    nature: Option<&str>,
    jours_comptes: Option<i64>,
    c: &mut Comparaison,
) -> Result<Vec<Ecart>, String> {
    let fin = Mois::lire(mois)?.dernier_jour();
    let annee = &mois[..4];
    let mut ctx = Contexte::nouveau(racine, Some(fin)).map_err(|m| a_refaire(c, m))?;
    if !ctx.code.join("baremes").join(format!("{}.json", annee)).exists() {
        return Err(format!("aucun barème livré pour {} : la période ne se calcule pas ici", annee));
    }
    // La convention fait la couche du bareme (regle 8) : sans elle, le calcul n'est pas celui de la compagnie.
    if ctx.entreprise.idcc.texte().is_empty() {
        c.a_refaire = true;
        return Err("la convention collective de l'entreprise n'est pas renseignée : le calcul n'aurait pas sa couche".into());
    }
    let salarie = brut.salarie_dit().map(String::from).ok_or("l'engagement importé ne nomme pas son salarié")?;
    let fiche = ctx.salarie(&salarie).map_err(|m| a_refaire(c, m))?;
    // LE STATUT ET L'EMPLOI DE CETTE PAIE sont ceux que son bulletin imprime (`emploi_imprime`) ; ce qu'il ne dit pas
    // se lit dans la fiche. Un salarie artiste sur un contrat, non cadre au regime general sur un autre, se calcule
    // chaque fois sous le sien.
    let dit = |v: &Option<String>| v.clone().filter(|x| !x.is_empty());
    let imprime = |champ: fn(&crate::importe::EmploiImprime) -> &Option<String>| p.emploi_imprime.as_ref().and_then(|i| dit(champ(i)));
    c.statut = imprime(|i| &i.statut).or_else(|| fiche.statut.dite().map(|s| s.as_str().to_string()));
    c.cadre = imprime(|i| &i.statut_categoriel_rc).map(|s| s == "01").or_else(|| fiche.statut_categoriel_rc.dite().map(|s| *s == droit::fiche::StatutCategorielRc::Cadre));
    let Some(cotisations) = &p.cotisations else {
        return Err("la paie importée n'a pas de bulletin : la DSN seule ne porte pas ses lignes de cotisation".into());
    };
    // LE TAUX AT/MP est celui de l'etablissement, notifie par l'Assurance Maladie. Une compagnie qui n'a pas encore pose
    // le sien pour l'annee lit celui que le bulletin applique (son taux employeur, sinon son montant sur sa base ; un
    // bulletin qui n'imprime que le montant, sur l'assiette de la ligne AT/MP calculee ici), ramene au taux notifie par
    // le facteur que le bareme applique a ce contrat (`droit::bareme::bareme_atmp`), au centieme de point comme un taux
    // notifie. L'AT/MP n'est alors pas compare : il est l'entree.
    let an: i32 = annee.parse().map_err(|_| format!("mois illisible : {}", mois))?;
    let mut atmp_imprime: Option<(Option<Decimal>, Option<Decimal>, Option<Decimal>)> = None;
    if !racine.join(droit::contexte::BAREMES).join(format!("{}.json", annee)).exists() {
        let ligne = cotisations.iter().find(|l| l.cotisation.as_deref() == Some("atmp"));
        atmp_imprime = ligne
            .map(|l| (l.taux_employeur, l.montant_employeur, l.base))
            .filter(|(t, m, _)| t.is_some() || m.is_some_and(|m| !m.is_zero()));
        if atmp_imprime.is_some() {
            ctx = ctx.avec_le_taux_atmp_lu(an, Decimal::ONE);
        }
    }
    let atmp_lu = atmp_imprime.is_some();
    c.taux_atmp = Some(if atmp_lu { "bulletin" } else { "compagnie" });
    // Les jours travailles que le bulletin dit, chacun sur chaque ligne (les jours de l'engagement en sont la reunion).
    let mut jours: Vec<crate::DateDeLigne> = Vec::new();
    for d in &p.dates {
        let (mut j, b) = (d.debut, d.fin);
        while j <= b {
            jours.push(crate::DateDeLigne::Nue(j.iso()));
            j = j.plus_jours(1)?;
        }
    }
    // UN BULLETIN QUI N'IMPRIME PAS SES JOURS TRAVAILLES en dit la periode et le nombre (`jours_comptes`) :
    // - autant de jours que la periode : ce sont tous les siens ;
    // - moins, dans une periode qui tient dans un mois : le calcul ne lit des jours que leur nombre, leur mois et le
    //   premier. Le plafond d'un intermittent compte les jours remuneres mois par mois (BOSS, assiette generale, § 1090)
    //   ou 12 heures du plafond horaire de l'annee par journee (arrete du 24/01/1975, art. 3, al. 2 : `droit::plafond`),
    //   et l'engagement court se juge sur le debut et la fin du contrat ; le minimum conventionnel et le bareme se
    //   lisent au premier jour, qui est le debut de la periode. Le debut et les jours qui le suivent en tiennent lieu.
    // Sans ce nombre, une periode de plus d'un jour ne dit pas combien : la paie ne se compare pas.
    let mensuelle = droit::natures::nature(nature).is_ok_and(|n| n.mensuelle());
    if jours.is_empty() && !mensuelle {
        if let (Some(d), Some(f)) = (p.debut, p.fin) {
            let periode = f.moins(&d) + 1;
            let un_mois = (d.an, d.mois) == (f.an, f.mois);
            let n = if periode == 1 { Some(1) } else { jours_comptes.filter(|n| *n >= 1 && *n <= periode && (*n == periode || un_mois)) };
            let Some(n) = n else {
                return Err(match jours_comptes {
                    None => "le bulletin imprime sa période sans ses jours travaillés ni leur nombre".to_string(),
                    Some(n) if n > periode => "le bulletin compte plus de jours travaillés que sa période n'en a".to_string(),
                    Some(_) => "le bulletin ne dit pas lesquels de ses jours travaillés tombent dans chacun des mois de sa période".to_string(),
                });
            };
            for i in 0..n {
                jours.push(crate::DateDeLigne::Nue(d.plus_jours(i)?.iso()));
            }
        }
    }
    let mut saisies = Vec::new();
    let en_f64 = |d: Decimal| d.to_string().parse::<f64>().map_err(|e| e.to_string());
    for l in &p.lignes {
        let libelle = &l.libelle;
        let Some(r) = l.rubrique.as_deref().filter(|r| !r.is_empty()) else {
            if !l.soumis {
                continue;
            }
            return Err(format!("la ligne de rémunération « {} » ne se rattache à aucune rubrique du catalogue", libelle));
        };
        let quantite = l.quantite.ok_or_else(|| format!("la ligne « {} » n'imprime pas sa quantité", libelle))?;
        let taux = l.taux.ok_or_else(|| format!("la ligne « {} » n'imprime pas son taux", libelle))?;
        saisies.push(crate::LignePaie {
            heures: l.heures.map(|h| en_f64(h).map(crate::Chiffre::de_f64)).transpose()?,
            dates: jours.clone(),
            ..crate::LignePaie::de(r, crate::Chiffre::de_f64(en_f64(quantite)?), crate::Chiffre::de_f64(en_f64(taux)?))
        });
    }
    // Le spectacle que le bulletin nomme ; sans titre, celui dont la fiche porte son numero d'objet.
    let spectacle = (!mensuelle).then(|| {
        let par_objet = dit(&p.numero_objet).and_then(|n| ctx.spectacles.iter().find(|(_, f)| f.numero_objet.portee() == Some(n.as_str())).map(|(k, _)| k.clone()));
        dit(&p.spectacle).or(par_objet)
    });
    let e = crate::saisie::Carte {
        salarie: Some(salarie.to_string()),
        spectacle: spectacle.flatten(),
        nature: nature.map(|n| n.to_string()),
        debut: p.debut.map(|j| j.iso()),
        fin: p.fin.map(|j| j.iso()),
        emploi: imprime(|i| &i.emploi),
        pcs_ese: imprime(|i| &i.pcs_ese),
        code_emploi_ft: imprime(|i| &i.code_emploi_ft),
        statut: imprime(|i| &i.statut),
        statut_conventionnel: imprime(|i| &i.statut_conventionnel),
        statut_categoriel_rc: imprime(|i| &i.statut_categoriel_rc),
        lignes: Some(saisies),
        // Le jour ou l'ancien logiciel a verse : un fait (`Carte::importe`).
        date_paiement: p.verse_le.map(|j| j.iso()),
        importe: Some(true),
        ..Default::default()
    };
    let data = crate::saisie::VueDuMois { mois: socle::mois::Mois::lire(mois)?, taux_cachet: None, engagements: vec![e] };
    let mut saisie = crate::charger_donnees(&ctx, &data).map_err(|m| a_refaire(c, m))?;
    if let Some((taux, montant, base)) = atmp_imprime {
        // Calcule au taux notifie de 1 %, la ligne AT/MP porte le facteur du contrat, et son assiette.
        let ligne = saisie.paies.first().and_then(|p| p.lignes.iter().find(|l| l.id == "atmp"));
        let facteur = ligne.and_then(|l| l.taux_pat).filter(|f| *f > 0.0);
        let applique = taux.or_else(|| {
            let b = base.or(ligne.map(|l| l.base)).filter(|b| !b.is_zero())?;
            Some((montant? / b * Decimal::from(100)).round_dp(2))
        });
        if let (Some(f), Some(applique)) = (facteur, applique) {
            let notifie = (applique / dec_f64(f)?).round_dp(2);
            ctx = ctx.avec_le_taux_atmp_lu(an, notifie);
            saisie = crate::charger_donnees(&ctx, &data).map_err(|m| a_refaire(c, m))?;
        }
    }
    let paie = saisie.paies.first().ok_or("le calcul ne rend aucune paie")?;

    // Les deux cotes, par cotisation.
    let mut leurs: BTreeMap<String, Vec<Lue>> = BTreeMap::new();
    // Les cotisations que le bulletin imprime apres son total (les facultatives, les exonerations) : hors de ce total.
    let mut hors_total: Vec<String> = Vec::new();
    for l in cotisations {
        let lue = Lue {
            libelle: l.libelle.clone(),
            base: l.base,
            taux_salarie: l.taux_salarie,
            taux_employeur: l.taux_employeur,
            montant_salarie: l.montant_salarie,
            montant_employeur: l.montant_employeur,
        };
        if lue.montant_salarie.is_none() && lue.montant_employeur.is_none() {
            continue;
        }
        match l.cotisation.clone().filter(|c| !c.is_empty()) {
            Some(code) => {
                if l.hors_total && !hors_total.contains(&code) {
                    hors_total.push(code.clone());
                }
                leurs.entry(code).or_default().push(lue)
            }
            None => {
                if !c.non_rattachees.contains(&lue.libelle) {
                    c.non_rattachees.push(lue.libelle);
                }
            }
        }
    }
    let mut nos: BTreeMap<String, Vec<&LigneCotisation>> = BTreeMap::new();
    for l in &paie.lignes {
        nos.entry(cotisation_de(l)).or_default().push(l);
    }
    for (fin, gros) in REPLIS {
        if !leurs.contains_key(fin) {
            if let Some(v) = nos.remove(fin) {
                nos.entry(gros.into()).or_default().extend(v);
            }
        }
    }
    let mut ecarts = Vec::new();
    let mut codes: Vec<&String> = leurs.keys().chain(nos.keys()).collect();
    codes.sort();
    codes.dedup();
    let zero = Decimal::ZERO;
    for code in codes {
        if atmp_lu && code == "atmp" {
            continue;
        }
        let eux: &[Lue] = match leurs.get(code) {
            Some(v) => v,
            None => &[],
        };
        let nous: &[&LigneCotisation] = match nos.get(code) {
            Some(v) => v,
            None => &[],
        };
        let leur_sal: Decimal = eux.iter().filter_map(|l| l.montant_salarie).map(|x| x.abs()).sum();
        let leur_pat: Decimal = eux.iter().filter_map(|l| l.montant_employeur).map(|x| x.abs()).sum();
        let notre_sal: Decimal = nous.iter().map(|l| l.montant_sal.abs()).sum();
        let notre_pat: Decimal = nous.iter().map(|l| l.montant_pat.abs()).sum();
        let mut libelles: Vec<String> = Vec::new();
        for l in eux.iter().map(|l| l.libelle.clone()).chain(eux.is_empty().then(|| nous.iter().map(|l| l.libelle.clone())).into_iter().flatten()) {
            if !libelles.contains(&l) {
                libelles.push(l);
            }
        }
        let ecart = |grandeur, notre_taux, leur_taux, ecart_relatif| Ecart { cotisation: code.clone(), libelles: libelles.clone(), grandeur, notre_taux, leur_taux, ecart_relatif };
        let (chez_eux, chez_nous) = (leur_sal + leur_pat > zero, notre_sal + notre_pat > zero);
        if chez_nous && !chez_eux {
            ecarts.push(ecart(Grandeur::AbsenteDuBulletin, None, None, None));
            continue;
        }
        if chez_eux && !chez_nous {
            ecarts.push(ecart(Grandeur::AbsenteDuCalcul, None, None, None));
            continue;
        }
        if !chez_eux {
            continue;
        }
        let tolerance = Decimal::new(1, 2) * Decimal::from(eux.len().max(nous.len()) as i64);
        for (grandeur, leur, notre) in [(Grandeur::MontantSalarie, leur_sal, notre_sal), (Grandeur::MontantEmployeur, leur_pat, notre_pat)] {
            if (leur - notre).abs() > tolerance {
                ecarts.push(ecart(grandeur, None, None, relatif(leur, notre)));
            }
        }
        let ([l], [n]) = (eux, nous) else { continue };
        if let Some(b) = l.base.filter(|_| !n.masquer_base_et_taux) {
            if (b - n.base).abs() > Decimal::new(1, 2) {
                ecarts.push(ecart(Grandeur::Base, None, None, relatif(b, n.base)));
            }
        }
        for (grandeur, leur, notre) in [(Grandeur::TauxSalarie, l.taux_salarie, n.taux_sal), (Grandeur::TauxEmployeur, l.taux_employeur, n.taux_pat)] {
            // Un taux se compare quand chaque cote en a un : une ligne calculee sans taux (la reduction generale, un
            // montant fixe) n'en a pas, et ce que le bulletin imprime a sa place n'en est pas un ; le montant compare.
            let (Some(leur), Some(notre)) = (leur, notre.map(dec_f64).transpose()?) else { continue };
            if (leur - notre).abs() > Decimal::new(5, 4) {
                ecarts.push(ecart(grandeur, Some(notre), Some(leur), None));
            }
        }
    }
    // Les totaux du bulletin, sans ce qu'il imprime apres eux : l'ecart relatif seulement.
    let hors = |f: &dyn Fn(&LigneCotisation) -> Decimal| -> Decimal { paie.lignes.iter().filter(|l| hors_total.contains(&cotisation_de(l))).map(f).sum() };
    for (cle, notre, leur) in [
        ("cotisations_salariales", paie.total_sal - hors(&|l| l.montant_sal), p.cotisations_salariales),
        ("cotisations_patronales", paie.total_pat - hors(&|l| l.montant_pat), p.cotisations_patronales),
        ("net_social", paie.net_social, p.net_social),
        ("net_imposable", paie.net_imposable, p.net_imposable),
    ] {
        let Some(leur) = leur else { continue };
        if (leur - notre).abs() > Decimal::new(1, 2) {
            let grandeur = if cle == "cotisations_patronales" { Grandeur::MontantEmployeur } else { Grandeur::MontantSalarie };
            ecarts.push(Ecart { cotisation: cle.into(), libelles: Vec::new(), grandeur, notre_taux: None, leur_taux: None, ecart_relatif: relatif(leur, notre) });
        }
    }
    Ok(ecarts)
}

/// (leur - notre) / notre, a 4 decimales ; `None` quand le notre est nul.
fn relatif(leur: Decimal, notre: Decimal) -> Option<Decimal> {
    (!notre.is_zero()).then(|| ((leur - notre) / notre).round_dp(4))
}

/// Une raison de non-comparaison venue du calcul : la paie se comparera de nouveau.
fn a_refaire(c: &mut Comparaison, raison: String) -> String {
    c.a_refaire = true;
    raison
}

/// L'empreinte de la paie `mois` de l'engagement importe `imp` : ce qui la reconnait d'une comparaison a l'autre (son
/// salarie, ses bornes, son numero et ce que les pieces en disent, sans la date de l'import).
pub fn empreinte(imp: &Importe, mois: &str, p: &PaieVersee) -> String {
    let brut = imp.objet().carte();
    let paie = PaieVersee { importe_le: None, ..p.clone() };
    let graine = socle::ecriture::json_en_ligne(&(&brut.salarie, &brut.debut, &brut.fin, &brut.numero, mois, paie));
    socle::fichiers::sha256_hex(graine.as_bytes())[..16].to_string()
}
