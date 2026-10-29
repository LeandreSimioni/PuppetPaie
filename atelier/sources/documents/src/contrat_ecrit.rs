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

use std::path::Path;
use std::sync::Arc;

use fabrique_pdf::modele::Valeurs;
use serde_json::{json, Value};

use crate::avenant::{de_devant, et, fait_le_brut, jj_mm_aaaa, ligne_citee, remuneration_hors_brut};
use crate::bulletin::fmt_eur;
use droit::contexte::Contexte;
use droit::natures::{self, Nature};
use socle::date::en_texte;
use socle::fichiers::vrai;
use socle::num::{dec, Decimal, Resultat};

/// La nature du contrat d'une paie calculee (sa forme JSON : `engagement.nature`).
pub fn nature_de_la_paie(paie: &Value) -> Resultat<Arc<Nature>> {
    natures::nature(paie.pointer("/engagement/nature").and_then(Value::as_str))
}

/// Un texte de l'engagement (sa forme canonique), non vide.
fn texte<'a>(eng: &'a Value, cle: &str) -> Option<&'a str> {
    eng.get(cle).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty())
}

/// Ce que la saisie dit du contrat ecrit (`engagement::CHAMPS_DU_CONTRAT_ECRIT`), non vide.
fn ecrit(eng: &Value, cle: &str) -> Option<String> {
    eng.pointer(&format!("/contrat_ecrit/{}", cle)).filter(|v| vrai(v)).map(en_texte).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Qui le contrat nomme, pour un refus.
fn qui(m: &Valeurs) -> String {
    m.get("salarie.nom_complet").cloned().unwrap_or_else(|| "ce salarié".into())
}

/// « 151,67 » : un nombre d'heures, a la francaise.
fn heures(x: Decimal) -> String {
    let t = fmt_eur(x);
    t.strip_suffix(",00").map(str::to_string).unwrap_or(t)
}

/// Les valeurs du contrat d'un permanent, paye au mois, que les parties (`avenant::parties`) ne portent pas : le motif
/// et le terme d'un CDD, le lieu, la remuneration, sa periodicite, la duree du travail. `remuneration`, `brut`, `net` :
/// ceux de la paie calculee du mois.
#[allow(clippy::too_many_arguments)]
pub fn marqueurs_au_mois(ent: &Value, sal: &Value, eng: &Value, nat: &Nature, remuneration: &[Value], brut: &Value, net: &Value, m: &mut Valeurs) -> Resultat<()> {
    let nom = qui(m);
    // LE TERME (L1242-12, 2°) : un contrat a duree determinee le dit ; un CDI n'en a pas.
    match texte(eng, "fin") {
        Some(f) => {
            m.insert("contrat.fin".into(), jj_mm_aaaa(f)?);
        }
        None if !nat.duree_indeterminee => return Err(format!("Le contrat de {} ({}) n'a pas de date de fin : un contrat à durée déterminée dit son terme (L. 1242-12, 2°). Rien n'est écrit.", nom, nat.libelle)),
        None => {}
    }
    // LE MOTIF (L1242-12, premier alinea) et, pour un remplacement, la personne remplacee (1°).
    if nat.motif_recours_exige() {
        let code = texte(eng, "motif_recours").ok_or_else(|| format!("Le contrat de {} ne dit pas son motif de recours : un CDD « comporte la définition précise de son motif » (L. 1242-12). Le choisir dans l'engagement (Plus d'options › Motif de recours). Rien n'est écrit.", nom))?;
        let libelle = nat.libelle_motif_recours(code).ok_or_else(|| format!("Motif de recours {} inconnu pour un {}.", code, nat.libelle))?;
        m.insert("contrat.motif".into(), minuscule_initiale(&libelle));
        if nat.motif_de_remplacement(code) {
            let qui_remplace = ecrit(eng, "personne_remplacee").ok_or_else(|| {
                format!("Le contrat de {} est conclu pour un remplacement : il comporte « le nom et la qualification professionnelle de la personne remplacée » (L. 1242-12, 1°). L'écrire dans l'engagement (Plus d'options › Personne remplacée). Rien n'est écrit.", nom)
            })?;
            m.insert("contrat.personne_remplacee".into(), format!(", en remplacement de {}", qui_remplace));
        } else {
            m.insert("contrat.personne_remplacee".into(), String::new());
        }
    }
    // LE LIEU DE TRAVAIL (R1221-34, 2°) : celui que l'engagement dit ; sans lui, le siege de l'employeur (INFERENCE : un
    // permanent travaille la ou la compagnie est etablie, sauf si son engagement dit autre chose).
    let lieu = match texte(eng, "lieu") {
        Some(l) => format!("à {}", l),
        None => format!(
            "au siège de l'employeur, {}, {} {}",
            m.get("employeur.adresse").cloned().unwrap_or_default(),
            m.get("employeur.code_postal").cloned().unwrap_or_default(),
            m.get("employeur.commune").cloned().unwrap_or_default()
        ),
    };
    m.insert("contrat.lieu".into(), lieu);
    // LA REMUNERATION ET SES COMPOSANTES (L1242-12, 7° ; R1221-34, 11°) : le salaire mensuel de base (le taux de la ligne
    // de salaire de base, pas son montant du mois, que le prorata d'une entree en cours de mois reduit), puis ce qui
    // s'y ajoute et fait le brut.
    let mut morceaux = Vec::new();
    for l in remuneration.iter().filter(|l| fait_le_brut(l)) {
        if l.get("conges_pris").is_some_and(vrai) {
            continue;
        }
        if l.get("salaire_de_base").is_some_and(vrai) {
            morceaux.insert(0, format!("un salaire mensuel brut de {} €", fmt_eur(dec(l.get("taux").unwrap_or(&Value::Null))?)));
        } else if dec(l.get("montant").unwrap_or(&Value::Null))? > Decimal::ZERO {
            morceaux.push(ligne_citee(l)?);
        }
    }
    if morceaux.is_empty() {
        return Err(format!("Le contrat de {} ne dit aucune rémunération : la paie de son premier mois n'a pas de salaire de base. Rien n'est écrit.", nom));
    }
    m.insert("remuneration.detail".into(), et(&morceaux));
    m.insert("remuneration.hors_brut".into(), remuneration_hors_brut(remuneration)?);
    m.insert("remuneration.brut".into(), fmt_eur(dec(brut)?));
    m.insert("remuneration.net_estime".into(), fmt_eur(dec(net)?));
    // LA PERIODICITE DU PAIEMENT (R1221-34, 11°) : une fois par mois (L3242-1). INFERENCE : par virement, comme le contrat
    // d'engagement le dit.
    m.insert("contrat.paiement".into(), "chaque mois, par virement".into());
    m.insert("salarie.case_signature".into(), "Le salarié".into());
    if let Some(p) = texte(eng, "date_paiement") {
        m.insert("contrat.date_paiement".into(), jj_mm_aaaa(p)?);
    }
    // LA DUREE DU TRAVAIL (R1221-34, 12°) : la quotite du contrat, en heures par mois (S21.G00.40.013). Sous la duree de
    // reference, c'est un temps partiel, dont le contrat dit la repartition (L3123-6, 1°).
    let q = texte(eng, "quotite_contrat").or_else(|| texte(eng, "quotite")).ok_or_else(|| format!("Le contrat de {} ne dit pas sa durée du travail (heures par mois). Rien n'est écrit.", nom))?;
    let q = dec(&json!(q))?;
    // Le temps partiel comme la DSN le declare (S21.G00.40.014, `Engagement::modalite_temps_travail`) : la modalite
    // saisie, sinon une quotite sous la reference de l'entreprise, plafonnee a la duree legale.
    let legale = nat.quotite_reference_defaut.map(|r| dec(&json!(r))).transpose()?;
    let reference = match (texte(eng, "quotite_reference_saisie").map(|r| dec(&json!(r))).transpose()?, legale) {
        (Some(r), Some(l)) => r.min(l),
        (r, l) => r.or(l).unwrap_or(q),
    };
    let partiel = match texte(eng, "modalite_temps_travail_saisie") {
        Some(m) => m == "20",
        None => q < reference,
    };
    let mut duree = format!("{} heures par mois", heures(q));
    if partiel {
        let repartition = ecrit(eng, "repartition_horaire").ok_or_else(|| {
            format!(
                "Le contrat de {} est à temps partiel ({} heures par mois, pour {} à temps plein) : il dit « la répartition de la durée du travail entre les jours de la semaine ou les semaines du mois » (L. 3123-6, 1°). L'écrire dans l'engagement (Plus d'options › Répartition des horaires). Rien n'est écrit.",
                nom,
                heures(q),
                heures(reference)
            )
        })?;
        duree = format!("{}, à temps partiel, réparties ainsi : {}", duree, repartition);
    } else if q.round_dp(2) == Decimal::new(15167, 2) {
        duree.push_str(", soit la durée légale de trente-cinq heures par semaine (article L. 3121-27 du code du travail)");
    }
    m.insert("contrat.duree_travail".into(), duree);
    let _ = (ent, sal);
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

/// Une phrase de renvoi de la nature (`contrat_ecrit.phrases`).
fn phrase(nat: &Nature, cle: &str) -> Option<String> {
    nat.contrat_ecrit.get("phrases").and_then(|p| p.get(cle)).and_then(Value::as_str).map(str::to_string)
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

/// Les organismes de la compagnie : ses versements et ses adhesions (`config/organismes.json`, `Contexte::organismes`),
/// et la partie normative livree (`organismes-base.json`), qui porte les adresses.
struct Organismes {
    compagnie: Option<Value>,
    base: Value,
    idcc: String,
}

impl Organismes {
    fn lire(ctx: &Contexte, ent: &Value) -> Resultat<Organismes> {
        let base = socle::fichiers::lire_json(&ctx.code.join("organismes-base.json"))?;
        Ok(Organismes { compagnie: ctx.organismes()?, base, idcc: ent.get("idcc").map(en_texte).unwrap_or_default() })
    }

    fn versements(&self) -> Vec<&Value> {
        fn de(v: &Value) -> Vec<&Value> {
            v.get("versements").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default()
        }
        let mut tous = self.compagnie.as_ref().map(de).unwrap_or_default();
        tous.extend(de(&self.base));
        tous
    }

    /// L'organisme de ce siret : son nom, et l'adresse que la compagnie ou la donnee livree lui donne.
    fn par_siret(&self, siret: &str) -> Option<Organisme> {
        let siens: Vec<&Value> = self.versements().into_iter().filter(|v| v.get("siret").and_then(Value::as_str) == Some(siret)).collect();
        let nom = siens.iter().find_map(|v| v.get("nom").and_then(Value::as_str))?;
        let adresse = siens.iter().find_map(|v| v.get("adresse").and_then(Value::as_str)).map(str::to_string);
        Some(Organisme { nom: nom.to_string(), adresse })
    }

    /// La caisse de retraite complementaire : l'organisme a qui la compagnie verse la cotisation du regime unifie
    /// Agirc-Arrco (code 131 du bloc S21.G00.81, `organismes-base.json`).
    fn retraite(&self) -> Option<Organisme> {
        let siret = self.versements().into_iter().find(|v| v.get("codes81").and_then(Value::as_array).is_some_and(|c| c.iter().any(|x| x.as_str() == Some("131"))))?.get("siret")?.as_str()?.to_string();
        self.par_siret(&siret)
    }

    /// L'organisme de l'adhesion `cle` (la prevoyance, la sante) : celui que la compagnie a enregistre, sinon celui que
    /// la branche designe (`organisme_fixe` des contrats de `organismes-base.json`). `None` : elle n'a pas ce contrat.
    fn adhesion(&self, cle: &str) -> Option<Organisme> {
        let enregistre = self.compagnie.as_ref().and_then(|c| c.get("adhesions_prevoyance")).and_then(Value::as_array).and_then(|a| {
            a.iter().find(|x| x.get("cle").and_then(Value::as_str) == Some(cle)).and_then(|x| x.get("delegataire").and_then(Value::as_str))
        });
        let designe = || {
            ["tous", self.idcc.as_str()].iter().find_map(|k| {
                self.base.pointer(&format!("/contrats/{}", k)).and_then(Value::as_array).and_then(|a| {
                    a.iter()
                        .find(|x| x.get("cle").and_then(Value::as_str) == Some(cle) && x.get("organisme_fixe").is_some_and(vrai))
                        .and_then(|x| x.get("organisme").and_then(Value::as_str))
                })
            })
        };
        self.par_siret(enregistre.or_else(designe)?)
    }
}

/// Les valeurs que tout contrat ecrit porte, quelle que soit sa nature : la categorie d'emploi (R1221-34, 3°), le motif
/// d'un contrat d'usage, la periode d'essai (L1242-12, 6° ; R1221-34, 7°), les organismes de retraite complementaire et
/// de prevoyance (L1242-12, 8° ; R1221-34, 14°), les mentions de renvoi (R1221-34, 8° a 10° et 14°, que R1221-35 permet
/// de dire par un renvoi aux textes), et les clauses particulieres -- celles de l'engagement, sinon celles que la
/// compagnie ecrit une fois dans sa configuration.
pub fn poser_les_mentions(ctx: &Contexte, ent: &Value, sal: &Value, eng: &Value, nat: &Nature, m: &mut Valeurs) -> Resultat<()> {
    let nom = qui(m);
    // LA CATEGORIE D'EMPLOI : le classement conventionnel (S21.G00.40.002), sinon le statut cadre ou non cadre de la
    // retraite complementaire (S21.G00.40.003), sinon le statut du spectacle (artiste, technicien).
    let lu = |c: &str| sal.get(c).filter(|v| vrai(v)).map(en_texte);
    let categorie = match lu("statut_conventionnel").map(|c| natures::categorie_d_emploi(&c)).transpose()?.flatten() {
        Some(c) => c,
        None => match lu("statut_categoriel_rc").as_deref() {
            Some("01" | "02") => "cadre".to_string(),
            Some("04") => "non cadre".to_string(),
            _ => lu("statut").ok_or_else(|| {
                format!("La fiche de {} ne dit pas sa catégorie d'emploi, que le contrat écrit porte (R. 1221-34, 3°) : choisir son métier dans la liste, ou saisir son classement conventionnel sur sa fiche. Rien n'est écrit.", nom)
            })?,
        },
    };
    m.insert("salarie.categorie".into(), categorie);
    if let Some(motif) = phrase(nat, "motif") {
        m.insert("contrat.motif".into(), motif);
    }
    m.insert(
        "contrat.periode_essai".into(),
        match ecrit(eng, "periode_essai_jours") {
            Some(j) => format!("{} jour{}", j, if j == "1" { "" } else { "s" }),
            None => "aucune".into(),
        },
    );
    // LES ORGANISMES : le nom ET l'adresse (L1242-12, 8°) ; une adresse que la donnee ne porte pas ne se devine pas.
    let org = Organismes::lire(ctx, ent)?;
    let retraite = org.retraite().ok_or_else(|| format!("Le contrat de {} nomme sa caisse de retraite complémentaire (L. 1242-12, 8°), et la compagnie ne verse à aucune (Configuration › Organismes et contrats). Rien n'est écrit.", nom))?;
    let manque_adresse = |o: &Organisme| format!("Le contrat de {} donne « le nom et l'adresse » de {} (L. 1242-12, 8°), et cette adresse n'est pas connue du logiciel. Rien n'est écrit.", nom, o.nom);
    m.insert("organismes.retraite".into(), retraite.nom_et_adresse().ok_or_else(|| manque_adresse(&retraite))?);
    let (cle_prevoyance, cle_sante) = if nat.permanent() { ("prevoyance_permanents", "sante_permanents") } else { ("interbranche", "tsintermittents") };
    let prevoyance = org.adhesion(cle_prevoyance);
    m.insert(
        "organismes.prevoyance".into(),
        match &prevoyance {
            Some(p) => p.nom_et_adresse().ok_or_else(|| manque_adresse(p))?,
            None => "aucun".into(),
        },
    );
    for (cle, texte) in [("mentions.formation", phrase(nat, "formation")), ("mentions.rupture", phrase(nat, "rupture"))] {
        m.insert(cle.into(), texte.unwrap_or_default());
    }
    let conges = phrase(nat, "conges").unwrap_or_default();
    let conges = if conges.contains("{congesspectacles}") { conges.replace("{congesspectacles}", &crate::conges_spectacles::adherent(ent)?) } else { conges };
    m.insert("mentions.conges".into(), conges);
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
    m.insert("mentions.protection_sociale".into(), protection);
    let clauses = ecrit(eng, "clauses_particulieres").or_else(|| ent.get("clauses_particulieres").filter(|v| vrai(v)).map(en_texte).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()));
    m.insert("contrat.clauses_particulieres".into(), clauses.unwrap_or_else(|| "Aucune.".into()));
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

/// LES MENTIONS QUE LA LOI EXIGE (`contrat_ecrit.mentions` de la nature) : un modele qui ne cite pas le marqueur de
/// l'une d'elles ferait un contrat qui la tait. Une mention `si_dite` n'est due que si sa valeur n'est pas vide (la
/// personne remplacee, pour un remplacement). `Err` : la phrase du refus, qui dit chaque
/// mention absente, le texte qui l'exige, et quoi faire.
pub fn refuser_les_mentions_absentes(modele: &Path, texte: &str, nat: &Nature, valeurs: &Value) -> Result<(), String> {
    let absentes: Vec<String> = nat
        .mentions_obligatoires()
        .into_iter()
        .filter(|(cle, _, si_dite)| !(*si_dite && valeurs.get(cle).and_then(Value::as_str).unwrap_or_default().trim().is_empty()))
        .filter(|(cle, _, _)| !crate::contrat::cite(texte, cle))
        .map(|(cle, pourquoi, _)| format!("{{{{{}}}}} ({})", cle, pourquoi))
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
pub const CHAMPS: &[(&str, &str, &str)] = &[
    ("employeur.raison_sociale", "la raison sociale de la compagnie", "tous"),
    ("employeur.adresse", "son adresse (la rue)", "tous"),
    ("employeur.code_postal", "son code postal", "tous"),
    ("employeur.commune", "sa commune", "tous"),
    ("employeur.siret", "son SIRET", "tous"),
    ("employeur.naf", "son code APE", "tous"),
    ("employeur.licence", "sa licence d'entrepreneur de spectacles", "cddu_spectacle"),
    ("employeur.idcc", "le numéro (IDCC) de sa convention collective", "tous"),
    ("employeur.convention", "l'intitulé de sa convention collective", "tous"),
    ("employeur.congesspectacles", "son numéro d'adhérent aux Congés Spectacles", "cddu_spectacle"),
    ("employeur.signataire", "le prénom et le nom de qui signe pour elle", "tous"),
    ("employeur.signataire_fonction", "sa fonction", "tous"),
    ("employeur.qualite_de", "« d' » ou « de », selon la fonction qui suit", "tous"),
    ("salarie.nom_complet", "le prénom et le nom du salarié", "tous"),
    ("salarie.nom", "son nom", "tous"),
    ("salarie.prenom", "son prénom", "tous"),
    ("salarie.date_naissance", "sa date de naissance", "tous"),
    ("salarie.lieu_naissance", "son lieu de naissance", "tous"),
    ("salarie.adresse", "son adresse (la rue)", "tous"),
    ("salarie.code_postal", "son code postal", "tous"),
    ("salarie.commune", "sa commune", "tous"),
    ("salarie.nir", "son numéro de sécurité sociale", "tous"),
    ("salarie.emploi", "son emploi (le métier de l'engagement, ou de sa fiche)", "tous"),
    ("salarie.qualite_de", "« d' » ou « de », selon l'emploi qui suit", "tous"),
    ("salarie.categorie", "sa catégorie d'emploi (son classement conventionnel)", "tous"),
    ("contrat.numero", "le numéro du contrat", "tous"),
    ("contrat.debut", "la date d'embauche", "tous"),
    ("contrat.fin", "la date du terme", "cddu_spectacle cdd"),
    ("contrat.motif", "le motif du contrat à durée déterminée", "cddu_spectacle cdd"),
    ("contrat.personne_remplacee", "« , en remplacement de » et la personne remplacée, pour un remplacement ; vide sinon", "cdd"),
    ("contrat.dates_travail", "les dates de travail, et le lieu de chacune", "cddu_spectacle"),
    ("contrat.lieu", "le lieu de travail", "tous"),
    ("contrat.date_paiement", "la date du paiement", "tous"),
    ("contrat.paiement", "la périodicité et le mode de paiement", "cdd cdi"),
    ("contrat.duree_travail", "la durée du travail (et, à temps partiel, sa répartition)", "cdd cdi"),
    ("contrat.periode_essai", "la durée de la période d'essai, ou « aucune »", "tous"),
    ("contrat.clauses_particulieres", "les clauses particulières (celles de l'engagement, sinon celles de la configuration), ou « Aucune. »", "tous"),
    ("remuneration.detail", "la rémunération et ses composantes", "tous"),
    ("remuneration.hors_brut", "ce qui est versé en plus du brut (défraiements), vide sinon", "tous"),
    ("remuneration.brut", "le brut du (premier) mois", "tous"),
    ("remuneration.net_estime", "le net à payer estimé du (premier) mois", "tous"),
    ("remuneration.periode", "« pour le mois de … » quand le contrat se poursuit après le mois", "cddu_spectacle"),
    ("remuneration.cachets", "le nombre de cachets", "cddu_spectacle"),
    ("remuneration.taux_cachet", "le taux du cachet, quand le contrat paie des cachets", "cddu_spectacle"),
    ("spectacle.titre", "le titre du spectacle", "cddu_spectacle"),
    ("spectacle.numero_objet", "son numéro d'objet", "cddu_spectacle"),
    ("organismes.retraite", "le nom et l'adresse de la caisse de retraite complémentaire", "tous"),
    ("organismes.prevoyance", "le nom et l'adresse de l'organisme de prévoyance, ou « aucun »", "tous"),
    ("mentions.formation", "le droit à la formation (un renvoi au code du travail)", "tous"),
    ("mentions.conges", "les congés payés", "tous"),
    ("mentions.rupture", "comment le contrat prend fin", "tous"),
    ("mentions.protection_sociale", "les régimes obligatoires et la protection sociale complémentaire", "tous"),
];

/// Les champs d'un modele de cette nature, chacun avec ce qu'il imprime et, s'il est obligatoire, le texte qui l'exige.
pub fn champs_des_modeles(nat: &Nature) -> Vec<Value> {
    let mentions = nat.mentions_obligatoires();
    CHAMPS
        .iter()
        .filter(|(_, _, ou)| *ou == "tous" || ou.split(' ').any(|n| n == nat.cle))
        .map(|(cle, quoi, _)| {
            let exige = mentions.iter().find(|(m, _, _)| m == cle);
            json!({
                "marqueur": format!("{{{{{}}}}}", cle),
                "quoi": quoi,
                "obligatoire": exige.is_some(),
                "texte": exige.map(|(_, t, _)| t.clone()),
                "si_dite": exige.is_some_and(|(_, _, s)| *s),
            })
        })
        .collect()
}
