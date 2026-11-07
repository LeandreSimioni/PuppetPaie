//! LE CONTRAT ECRIT, quelle que soit sa nature : ce que la loi exige qu'il dise, et les valeurs qui le disent.
//!
//! Le contrat a duree determinee « est etabli par ecrit » avec ses mentions (code du travail L. 1242-12,
//! `atelier/reference/sources/code-du-travail/xml/L1242-12.xml`) ; tout salarie recoit par ecrit les informations
//! principales de sa relation de travail (L. 1221-5-1 ; leur liste, R. 1221-34 ; le renvoi aux textes, R. 1221-35 :
//! `xml/L1221-5-1.xml`, `xml/R1221-34.xml`, `xml/R1221-35.xml`) ; le contrat a temps partiel dit sa duree et sa
//! repartition (L. 3123-6, `xml/L3123-6.xml`). Chaque nature de `contrats-base.json` dit son modele livre et ses
//! mentions obligatoires, chacune par son marqueur (`contrat_ecrit`) : un modele -- le livre ou celui que la compagnie
//! pose dans son dossier `gabarits/` -- qui n'en cite pas une ne fait pas de contrat (`refuser_les_mentions_absentes`).
//!
//! Ici, les valeurs propres au contrat d'un permanent (`marqueurs_au_mois` : son salaire au mois, sa duree du travail,
//! son motif et son terme), celles que tout contrat ecrit porte (`poser_les_mentions` : la categorie d'emploi, la
//! periode d'essai, les organismes, les mentions de renvoi, les clauses particulieres), et la liste des champs qu'un
//! modele peut citer (`champs_des_modeles`), que l'ecran montre a qui ecrit le sien.

use droit::fiche::Lue as _;
use std::path::Path;
use std::sync::Arc;

use crate::marqueurs::{Marqueur, ModeleLu, Valeurs};
use serde_json::{json, Value};

use crate::avenant::{de_devant, et, fait_le_brut, ligne_citee, remuneration_hors_brut};
use crate::bulletin::fmt_eur;
use socle::texte::{euros, virgule};
use droit::contexte::Contexte;
use droit::natures::{self, Nature};
use crate::valeur::{dec, en_texte, vrai};
use socle::num::{Decimal, Resultat};

/// La nature du contrat d'une paie calculee (sa forme JSON : `engagement.nature`).
pub fn nature_de_la_paie(paie: &Value) -> Resultat<Arc<Nature>> {
    match paie.pointer("/engagement/nature").and_then(Value::as_str) {
        Some(cle) => natures::nature(cle),
        None => Err(socle::fichiers::absent("engagement.nature")),
    }
}

/// Un texte de l'engagement (sa forme canonique), non vide.
fn texte<'a>(eng: &'a Value, cle: &str) -> Option<&'a str> {
    eng.get(cle).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty())
}

/// Ce que la saisie dit du contrat ecrit (`engagement::ContratEcrit`), non vide.
fn ecrit(eng: &Value, cle: &str) -> Option<String> {
    eng.pointer(&format!("/contrat_ecrit/{}", cle)).filter(|v| vrai(v)).map(en_texte).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Qui le contrat nomme, pour un refus.
fn qui(m: &Valeurs) -> String {
    m.get(&Marqueur::SalarieNomComplet).cloned().unwrap_or_else(|| "ce salarié".into())
}

/// Les valeurs du contrat d'un permanent, paye au mois, que les parties (`avenant::parties`) ne portent pas : le motif
/// et le terme d'un CDD, le lieu, la remuneration, sa periodicite, la duree du travail. `remuneration`, `brut`, `net` :
/// ceux de la paie calculee du mois.
#[allow(clippy::too_many_arguments)]
pub fn marqueurs_au_mois(eng: &Value, nat: &Nature, remuneration: &[Value], brut: &Value, net: &Value, m: &mut Valeurs) -> Resultat<()> {
    let nom = qui(m);
    // LE TERME (L1242-12, 2°) : un contrat a duree determinee le dit ; un CDI n'en a pas.
    match texte(eng, "fin") {
        Some(f) => {
            m.insert(Marqueur::ContratFin, socle::date::Jour::lire(f)?.jj_mm_aaaa());
        }
        None if !nat.duree_indeterminee => return Err(format!("Le contrat de {} ({}) n'a pas de date de fin : un contrat à durée déterminée dit son terme (L. 1242-12, 2°). Rien n'est écrit.", nom, nat.libelle)),
        None => {}
    }
    // LE MOTIF (L1242-12, premier alinea) et, pour un remplacement, la personne remplacee (1°).
    if nat.motif_recours_exige() {
        let code = texte(eng, "motif_recours").ok_or_else(|| format!("Le contrat de {} ne dit pas son motif de recours : un CDD « comporte la définition précise de son motif » (L. 1242-12). Le choisir dans l'engagement (Plus d'options › Motif de recours). Rien n'est écrit.", nom))?;
        let libelle = nat.libelle_motif_recours(code).ok_or_else(|| format!("Motif de recours {} inconnu pour un {}.", code, nat.libelle))?;
        m.insert(Marqueur::ContratMotif, minuscule_initiale(&libelle));
        if nat.motif_de_remplacement(code) {
            let qui_remplace = ecrit(eng, "personne_remplacee").ok_or_else(|| {
                format!("Le contrat de {} est conclu pour un remplacement : il comporte « le nom et la qualification professionnelle de la personne remplacée » (L. 1242-12, 1°). L'écrire dans l'engagement (Plus d'options › Personne remplacée). Rien n'est écrit.", nom)
            })?;
            m.insert(Marqueur::ContratPersonneRemplacee, format!(", en remplacement de {}", qui_remplace));
        } else {
            m.insert(Marqueur::ContratPersonneRemplacee, String::new());
        }
    }
    // LE LIEU DE TRAVAIL (R1221-34, 2°) : celui que l'engagement dit ; sans lui, le siege de l'employeur (INFERENCE : un
    // permanent travaille la ou la compagnie est etablie, sauf si son engagement dit autre chose).
    let lieu = match texte(eng, "lieu") {
        Some(l) => format!("à {}", l),
        None => format!(
            "au siège de l'employeur, {}, {} {}",
            m.get(&Marqueur::EmployeurAdresse).cloned().unwrap_or_default(),
            m.get(&Marqueur::EmployeurCodePostal).cloned().unwrap_or_default(),
            m.get(&Marqueur::EmployeurCommune).cloned().unwrap_or_default()
        ),
    };
    m.insert(Marqueur::ContratLieu, lieu);
    // LA REMUNERATION ET SES COMPOSANTES (L1242-12, 7° ; R1221-34, 11°) : le salaire mensuel de base (le taux de la ligne
    // de salaire de base, pas son montant du mois, que le prorata d'une entree en cours de mois reduit), puis ce qui
    // s'y ajoute et fait le brut. Un mois coupe par un avenant a une ligne de salaire de base par periode, dans l'ordre
    // des jours : le contrat dit celui de la derniere, ses termes a partir de l'avenant.
    let mut morceaux = Vec::new();
    let mut base = None;
    for l in remuneration.iter().filter(|l| fait_le_brut(l)) {
        if l.get("conges_pris").is_some_and(vrai) {
            continue;
        }
        if l.get("salaire_de_base").is_some_and(vrai) {
            base = Some(format!("un salaire mensuel brut de {}", euros(dec(l.get("taux").unwrap_or(&Value::Null))?)));
        } else if dec(l.get("montant").unwrap_or(&Value::Null))? > Decimal::ZERO {
            morceaux.push(ligne_citee(l)?);
        }
    }
    if let Some(b) = base {
        morceaux.insert(0, b);
    }
    if morceaux.is_empty() {
        return Err(format!("Le contrat de {} ne dit aucune rémunération : la paie de son premier mois n'a pas de salaire de base. Rien n'est écrit.", nom));
    }
    m.insert(Marqueur::RemunerationDetail, et(&morceaux));
    m.insert(Marqueur::RemunerationHorsBrut, remuneration_hors_brut(remuneration)?);
    m.insert(Marqueur::RemunerationBrut, fmt_eur(dec(brut)?));
    m.insert(Marqueur::RemunerationNetEstime, fmt_eur(dec(net)?));
    // LA PERIODICITE DU PAIEMENT (R1221-34, 11°) : une fois par mois (L3242-1). INFERENCE : par virement, comme le contrat
    // d'engagement le dit.
    m.insert(Marqueur::ContratPaiement, "chaque mois, par virement".into());
    m.insert(Marqueur::SalarieCaseSignature, "Le salarié".into());
    if let Some(p) = texte(eng, "date_paiement") {
        m.insert(Marqueur::ContratDatePaiement, socle::date::Jour::lire(p)?.jj_mm_aaaa());
    }
    // LA DUREE DU TRAVAIL (R1221-34, 12°) : la quotite du contrat, en heures par mois (S21.G00.40.013). Sous la duree de
    // reference, c'est un temps partiel, dont le contrat dit la repartition (L3123-6, 1°).
    let q = texte(eng, "quotite_contrat").or_else(|| texte(eng, "quotite")).ok_or_else(|| format!("Le contrat de {} ne dit pas sa durée du travail (heures par mois). Rien n'est écrit.", nom))?;
    let q = dec(&json!(q))?;
    // Le temps partiel comme la DSN le declare (S21.G00.40.014, `Engagement::modalite_temps_travail`) : la modalite
    // saisie, sinon une quotite sous la reference de l'entreprise, plafonnee a la duree legale.
    let legale = nat.quotite_reference_defaut().map(|r| dec(&json!(r))).transpose()?;
    let reference = match (texte(eng, "quotite_reference_saisie").map(|r| dec(&json!(r))).transpose()?, legale) {
        (Some(r), Some(l)) => r.min(l),
        (r, l) => r.or(l).unwrap_or(q),
    };
    let partiel = match texte(eng, "modalite_temps_travail_saisie") {
        Some(m) => m == "20",
        None => q < reference,
    };
    let mut duree = format!("{} heures par mois", virgule(q));
    if partiel {
        let repartition = ecrit(eng, "repartition_horaire").ok_or_else(|| {
            format!(
                "Le contrat de {} est à temps partiel ({} heures par mois, pour {} à temps plein) : il dit « la répartition de la durée du travail entre les jours de la semaine ou les semaines du mois » (L. 3123-6, 1°). L'écrire dans l'engagement (Plus d'options › Répartition des horaires). Rien n'est écrit.",
                nom,
                virgule(q),
                virgule(reference)
            )
        })?;
        duree = format!("{}, à temps partiel, réparties ainsi : {}", duree, repartition);
    } else if q.round_dp(2) == Decimal::new(15167, 2) {
        duree.push_str(", soit la durée légale de trente-cinq heures par semaine (article L. 3121-27 du code du travail)");
    }
    m.insert(Marqueur::ContratDureeTravail, duree);
    Ok(())
}

/// « Accroissement… » -> « accroissement… » : un libelle qui suit « pour ».
fn minuscule_initiale(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(p) => p.to_lowercase().chain(c).collect(),
        None => String::new(),
    }
}

/// Un organisme que le contrat nomme : son nom, et son adresse quand la donnee la porte.
struct Organisme {
    nom: String,
    adresse: Option<String>,
}

impl Organisme {
    /// « Audiens Retraite, 74 rue Jean Bleuzen, 92177 Vanves Cedex ».
    fn nom_et_adresse(&self) -> Option<String> {
        self.adresse.as_ref().map(|a| format!("{}, {}", self.nom, a))
    }
}

/// Les organismes de la compagnie : ses versements et ses adhesions (`Contexte::organismes`), et la partie normative
/// livree (`organismes-base.json`), qui porte les adresses et les organismes que la branche designe.
struct Organismes {
    compagnie: droit::organismes::Organismes,
    idcc: Option<String>,
}

impl Organismes {
    fn lire(ctx: &Contexte, ent: &droit::fiche::ChampsEntreprise) -> Resultat<Organismes> {
        Ok(Organismes { compagnie: ctx.organismes()?, idcc: ent.idcc.en_texte() })
    }

    /// L'organisme de ce siret : son nom, et l'adresse que la donnee livree lui donne.
    fn par_siret(&self, siret: &str) -> Option<Organisme> {
        let v = self.compagnie.base.versement(siret)?;
        Some(Organisme { nom: v.nom.clone(), adresse: v.adresse.clone() })
    }

    /// La caisse de retraite complementaire : l'organisme a qui la compagnie verse la cotisation du regime unifie
    /// Agirc-Arrco (code 131 du bloc S21.G00.81, `organismes-base.json`).
    fn retraite(&self) -> Option<Organisme> {
        let v = self.compagnie.versements().iter().find(|v| v.codes81.iter().any(|c| c == "131"))?;
        self.par_siret(&v.siret)
    }

    /// L'organisme de l'adhesion `cle` (la prevoyance, la sante) : celui que la compagnie a enregistre, sinon celui que
    /// la branche designe (`organisme_fixe` des contrats de `organismes-base.json`). `None` : elle n'a pas ce contrat.
    fn adhesion(&self, cle: &str) -> Option<Organisme> {
        let enregistre = self.compagnie.adhesions_prevoyance.iter().find(|a| a.cle == cle).map(|a| a.delegataire.as_str());
        let designe = || {
            self.compagnie.base.contrats_de(self.idcc.as_deref()).filter(|c| c.cle == cle && c.organisme_fixe).find_map(|c| c.organisme.as_deref())
        };
        self.par_siret(enregistre.or_else(designe)?)
    }
}

/// Les valeurs que tout contrat ecrit porte, quelle que soit sa nature : la categorie d'emploi (R1221-34, 3°), le motif
/// d'un contrat d'usage, la periode d'essai (L1242-12, 6° ; R1221-34, 7°), les organismes de retraite complementaire et
/// de prevoyance (L1242-12, 8° ; R1221-34, 14°), les mentions de renvoi (R1221-34, 8° a 10° et 14°, que R1221-35 permet
/// de dire par un renvoi aux textes), et les clauses particulieres -- celles de l'engagement, sinon celles que la
/// compagnie ecrit une fois dans sa configuration.
pub fn poser_les_mentions(ctx: &Contexte, ent: &droit::fiche::ChampsEntreprise, sal: &droit::fiche::ChampsSalarie, eng: &Value, nat: &Nature, m: &mut Valeurs) -> Resultat<()> {
    let nom = qui(m);
    // LA CATEGORIE D'EMPLOI : le classement conventionnel (S21.G00.40.002), sinon le statut cadre ou non cadre de la
    // retraite complementaire (S21.G00.40.003), sinon le statut du spectacle (artiste, technicien).
    let artiste = sal.statut.dite() == Some(&droit::fiche::Statut::Artiste);
    let categorie = match sal.statut_conventionnel.code().map(|c| natures::categorie_d_emploi(c, artiste)).transpose()?.flatten() {
        Some(c) => c,
        None => match sal.statut_categoriel_rc.code() {
            Some("01" | "02") => "cadre".to_string(),
            Some("04") => "non cadre".to_string(),
            _ => sal.statut.code().map(str::to_string).ok_or_else(|| {
                format!("La fiche de {} ne dit pas sa catégorie d'emploi, que le contrat écrit porte (R. 1221-34, 3°) : choisir son métier dans la liste, ou saisir son classement conventionnel sur sa fiche. Rien n'est écrit.", nom)
            })?,
        },
    };
    m.insert(Marqueur::SalarieCategorie, categorie);
    // LES PHRASES DE RENVOI de la nature (R. 1221-35) : une nature sans contrat ecrit n'en fait pas.
    let phrases = nat.phrases().ok_or_else(|| format!("La nature {} n'a pas de contrat écrit.", nat.libelle))?;
    if let Some(motif) = &phrases.motif {
        m.insert(Marqueur::ContratMotif, motif.clone());
    }
    m.insert(
        Marqueur::ContratPeriodeEssai,
        match ecrit(eng, "periode_essai_jours") {
            Some(j) => format!("{} jour{}", j, if j == "1" { "" } else { "s" }),
            None => "aucune".into(),
        },
    );
    // LES ORGANISMES : le nom ET l'adresse (L1242-12, 8°) ; une adresse que la donnee ne porte pas ne se devine pas.
    let org = Organismes::lire(ctx, ent)?;
    let retraite = org.retraite().ok_or_else(|| socle::echec::avec(format!("Le contrat de {} nomme sa caisse de retraite complémentaire (L. 1242-12, 8°), et la table livrée des versements n'en nomme aucune : le signaler au développeur. Rien n'est écrit.", nom), socle::echec::APart { fichier: Some("organismes-base.json".into()), cle: Some("131".into()), ..Default::default() }))?;
    let manque_adresse = |o: &Organisme| format!("Le contrat de {} donne « le nom et l'adresse » de {} (L. 1242-12, 8°), et cette adresse n'est pas connue du logiciel. Rien n'est écrit.", nom, o.nom);
    m.insert(Marqueur::OrganismesRetraite, retraite.nom_et_adresse().ok_or_else(|| manque_adresse(&retraite))?);
    let (cle_prevoyance, cle_sante) = if nat.permanent() { ("prevoyance_permanents", "sante_permanents") } else { ("interbranche", "tsintermittents") };
    let prevoyance = org.adhesion(cle_prevoyance);
    m.insert(
        Marqueur::OrganismesPrevoyance,
        match &prevoyance {
            Some(p) => p.nom_et_adresse().ok_or_else(|| manque_adresse(p))?,
            None => "aucun".into(),
        },
    );
    m.insert(Marqueur::MentionsFormation, phrases.formation.clone());
    m.insert(Marqueur::MentionsRupture, phrases.rupture.clone());
    let conges = phrases.conges.clone();
    let conges = if conges.contains("{congesspectacles}") { conges.replace("{congesspectacles}", &crate::conges_spectacles::adherent(ent)?) } else { conges };
    m.insert(Marqueur::MentionsConges, conges);
    // LES REGIMES OBLIGATOIRES ET LA PROTECTION SOCIALE COMPLEMENTAIRE (R1221-34, 14°) : le regime de base que la nature
    // declare (S21.G00.40.018, 200 : le regime general), la retraite complementaire, et les contrats collectifs.
    let regime = match nat.code("S21.G00.40.018").as_deref() {
        Some("200") => "Le salarié est affilié au régime général de la sécurité sociale".to_string(),
        autre => return Err(format!("Le contrat de {} : le régime de base {} de la nature {} n'a pas de libellé pour le contrat écrit.", nom, autre.unwrap_or("(aucun)"), nat.libelle)),
    };
    let aupres = |nom: &str| format!("auprès {}{}", de_devant(nom), nom);
    let mut protection = format!("{} ; sa retraite complémentaire est celle du régime Agirc-Arrco, {}", regime, aupres(&retraite.nom));
    let sante = org.adhesion(cle_sante);
    let collectifs: Vec<String> = match (&prevoyance, &sante) {
        (Some(p), Some(s)) if p.nom == s.nom => vec![format!("de prévoyance et de frais de santé {}", aupres(&p.nom))],
        _ => [prevoyance.as_ref().map(|p| format!("de prévoyance {}", aupres(&p.nom))), sante.as_ref().map(|s| format!("de frais de santé {}", aupres(&s.nom)))].into_iter().flatten().collect(),
    };
    if !collectifs.is_empty() {
        protection.push_str(&format!(". Il bénéficie des garanties collectives {}, prévues par la convention collective", et(&collectifs)));
    }
    protection.push('.');
    m.insert(Marqueur::MentionsProtectionSociale, protection);
    let clauses = ecrit(eng, "clauses_particulieres").or_else(|| ent.clauses_particulieres.remplie().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()));
    m.insert(Marqueur::ContratClausesParticulieres, clauses.unwrap_or_else(|| "Aucune.".into()));
    Ok(())
}

/// Le nom du modele livre du contrat ecrit de cette nature.
pub fn nom_du_modele(nat: &Nature) -> Resultat<String> {
    nat.modele_contrat().ok_or_else(|| format!("La nature {} n'a pas de contrat écrit.", nat.libelle))
}

/// Le modele est-il celui de la compagnie (son dossier `gabarits/`) plutot que celui du logiciel ?
pub fn est_propre(chemin: &Path) -> bool {
    !chemin.starts_with(socle::fichiers::dossier_du_code())
}

/// Les mentions que la loi exige d'un contrat de cette nature (`Nature::mentions_obligatoires`), chacune par son
/// marqueur, le texte qui l'exige, et si elle n'est due que dite.
pub fn mentions(nat: &Nature) -> Result<Vec<(Marqueur, String, bool)>, String> {
    nat.mentions_obligatoires()
        .into_iter()
        .flatten()
        .map(|x| {
            let m = Marqueur::lire(&x.marqueur).ok_or_else(|| socle::echec::avec(format!("La table livrée des natures exige du {} une mention que le logiciel ne connaît pas : le signaler au développeur.", nat.nom_du_contrat()), json!({"fichier": "contrats-base.json", "marqueur": x.marqueur})))?;
            Ok((m, x.texte.clone(), x.si_dite == Some(true)))
        })
        .collect()
}

/// LES MENTIONS QUE LA LOI EXIGE (`contrat_ecrit.mentions` de la nature) : un modele qui ne cite pas le marqueur de
/// l'une d'elles ferait un contrat qui la tait. Une mention `si_dite` n'est due que si sa valeur n'est pas vide (la
/// personne remplacee, pour un remplacement). `Err` : la phrase du refus, qui dit chaque
/// mention absente, le texte qui l'exige, et quoi faire.
pub fn refuser_les_mentions_absentes(modele: &Path, lu: &ModeleLu, nat: &Nature, valeurs: &Valeurs) -> Result<(), String> {
    let absentes: Vec<String> = mentions(nat)?
        .into_iter()
        .filter(|(cle, _, si_dite)| !(*si_dite && valeurs.get(cle).map_or("", String::as_str).trim().is_empty()))
        .filter(|(cle, _, _)| !lu.cite(*cle))
        .map(|(cle, pourquoi, _)| format!("{} ({})", cle, pourquoi))
        .collect();
    if absentes.is_empty() {
        return Ok(());
    }
    let nom = modele.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let (quel, quoi_faire) = if est_propre(modele) {
        (
            format!("Le modèle de contrat de la compagnie « {} » (dossier gabarits)", nom),
            "Ajouter ces champs à son texte (Configuration › Modèles de contrat dit lesquels et comment), ou retirer ce modèle pour reprendre celui du logiciel.",
        )
    } else {
        (format!("Le modèle de contrat livré « {} »", nom), "C'est un défaut du logiciel : le signaler.")
    };
    Err(format!(
        "{} ne porte pas ce que la loi exige d'un {} : {}. {} Rien n'est écrit.",
        quel,
        nat.nom_du_contrat(),
        absentes.join(" ; "),
        quoi_faire
    ))
}

/// LES CHAMPS QU'UN MODELE PEUT CITER, et ce que chacun imprime : ce que l'ecran montre a qui ecrit le modele de sa
/// compagnie. `(marqueur, ce qu'il imprime, natures ou il vaut)` ; « tous » : toute nature.
pub const CHAMPS: &[(Marqueur, &str, &str)] = &[
    (Marqueur::EmployeurRaisonSociale, "la raison sociale de la compagnie", "tous"),
    (Marqueur::EmployeurAdresse, "son adresse (la rue)", "tous"),
    (Marqueur::EmployeurCodePostal, "son code postal", "tous"),
    (Marqueur::EmployeurCommune, "sa commune", "tous"),
    (Marqueur::EmployeurSiret, "son SIRET", "tous"),
    (Marqueur::EmployeurNaf, "son code APE", "tous"),
    (Marqueur::EmployeurLicence, "sa licence d'entrepreneur de spectacles", "cddu_spectacle"),
    (Marqueur::EmployeurIdcc, "le numéro (IDCC) de sa convention collective", "tous"),
    (Marqueur::EmployeurConvention, "l'intitulé de sa convention collective", "tous"),
    (Marqueur::EmployeurCongesspectacles, "son numéro d'adhérent aux Congés Spectacles", "cddu_spectacle"),
    (Marqueur::EmployeurSignataire, "le prénom et le nom de qui signe pour elle", "tous"),
    (Marqueur::EmployeurSignataireFonction, "sa fonction", "tous"),
    (Marqueur::EmployeurQualiteDe, "« d' » ou « de », selon la fonction qui suit", "tous"),
    (Marqueur::SalarieNomComplet, "le prénom et le nom du salarié", "tous"),
    (Marqueur::SalarieNom, "son nom", "tous"),
    (Marqueur::SalariePrenom, "son prénom", "tous"),
    (Marqueur::SalarieDateNaissance, "sa date de naissance", "tous"),
    (Marqueur::SalarieLieuNaissance, "son lieu de naissance", "tous"),
    (Marqueur::SalarieAdresse, "son adresse (la rue)", "tous"),
    (Marqueur::SalarieCodePostal, "son code postal", "tous"),
    (Marqueur::SalarieCommune, "sa commune", "tous"),
    (Marqueur::SalarieNir, "son numéro de sécurité sociale", "tous"),
    (Marqueur::SalarieEmploi, "son emploi (le métier de l'engagement, ou de sa fiche)", "tous"),
    (Marqueur::SalarieQualiteDe, "« d' » ou « de », selon l'emploi qui suit", "tous"),
    (Marqueur::SalarieCategorie, "sa catégorie d'emploi (son classement conventionnel)", "tous"),
    (Marqueur::ContratNumero, "le numéro du contrat", "tous"),
    (Marqueur::ContratDebut, "la date d'embauche", "tous"),
    (Marqueur::ContratFin, "la date du terme", "cddu_spectacle cdd"),
    (Marqueur::ContratMotif, "le motif du contrat à durée déterminée", "cddu_spectacle cdd"),
    (Marqueur::ContratPersonneRemplacee, "« , en remplacement de » et la personne remplacée, pour un remplacement ; vide sinon", "cdd"),
    (Marqueur::ContratDatesTravail, "les dates de travail, et le lieu de chacune", "cddu_spectacle"),
    (Marqueur::ContratLieu, "le lieu de travail", "tous"),
    (Marqueur::ContratDatePaiement, "la date du paiement", "tous"),
    (Marqueur::ContratPaiement, "la périodicité et le mode de paiement", "cdd cdi"),
    (Marqueur::ContratDureeTravail, "la durée du travail (et, à temps partiel, sa répartition)", "cdd cdi"),
    (Marqueur::ContratPeriodeEssai, "la durée de la période d'essai, ou « aucune »", "tous"),
    (Marqueur::ContratClausesParticulieres, "les clauses particulières (celles de l'engagement, sinon celles de la configuration), ou « Aucune. »", "tous"),
    (Marqueur::RemunerationDetail, "la rémunération et ses composantes", "tous"),
    (Marqueur::RemunerationHorsBrut, "ce qui est versé en plus du brut (défraiements), vide sinon", "tous"),
    (Marqueur::RemunerationBrut, "le brut du (premier) mois", "tous"),
    (Marqueur::RemunerationNetEstime, "le net à payer estimé du (premier) mois", "tous"),
    (Marqueur::RemunerationPeriode, "« pour le mois de … » quand le contrat se poursuit après le mois", "cddu_spectacle"),
    (Marqueur::RemunerationCachets, "le nombre de cachets", "cddu_spectacle"),
    (Marqueur::RemunerationTauxCachet, "le taux du cachet, quand le contrat paie des cachets", "cddu_spectacle"),
    (Marqueur::SpectacleTitre, "le titre du spectacle", "cddu_spectacle"),
    (Marqueur::SpectacleObjet, "son numéro d'objet, entre parenthèses (vide sans numéro)", "cddu_spectacle"),
    (Marqueur::OrganismesRetraite, "le nom et l'adresse de la caisse de retraite complémentaire", "tous"),
    (Marqueur::OrganismesPrevoyance, "le nom et l'adresse de l'organisme de prévoyance, ou « aucun »", "tous"),
    (Marqueur::MentionsFormation, "le droit à la formation (un renvoi au code du travail)", "tous"),
    (Marqueur::MentionsConges, "les congés payés", "tous"),
    (Marqueur::MentionsRupture, "comment le contrat prend fin", "tous"),
    (Marqueur::MentionsProtectionSociale, "les régimes obligatoires et la protection sociale complémentaire", "tous"),
];

/// Les champs d'un modele de cette nature, chacun avec ce qu'il imprime et, s'il est obligatoire, le texte qui l'exige.
pub fn champs_des_modeles(nat: &Nature) -> Result<Vec<Value>, String> {
    let mentions = mentions(nat)?;
    Ok(CHAMPS
        .iter()
        .filter(|(_, _, ou)| *ou == "tous" || ou.split(' ').any(|n| n == nat.cle))
        .map(|(cle, quoi, _)| {
            let exige = mentions.iter().find(|(m, _, _)| *m == *cle);
            json!({
                "marqueur": cle.to_string(),
                "quoi": quoi,
                "obligatoire": exige.is_some(),
                "texte": exige.map(|(_, t, _)| t.clone()),
                "si_dite": exige.is_some_and(|(_, _, s)| *s),
            })
        })
        .collect())
}
