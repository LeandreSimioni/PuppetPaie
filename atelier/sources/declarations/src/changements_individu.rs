//! Le bloc S21.G00.31 « Changements individu » (corrections DSN).
//!
//! Le NIR, le nom de famille, les prenoms et la date de naissance sont des donnees IDENTIFIANTES : elles se reportent
//! a l'identique d'une DSN mensuelle a la suivante, et tout changement se signale par le bloc changement « des la
//! premiere DSN portant le changement » (cahier technique 2026.1, § 2.1.1, p. 18 ; § 2.1.2.3.1, p. 20 :
//! « obligatoirement des la premiere DSN mensuelle dans laquelle est utilisee la nouvelle identite »). Le bloc 30 porte
//! la nouvelle valeur, le bloc 31 l'ANCIENNE (« Les rubriques doivent etre renseignees avec la valeur avant
//! changement », p. 177), datee en 31.001 ; plusieurs changements a la meme date font un seul bloc, des dates
//! differentes autant de blocs (p. 177) ; un bloc qui ne porte que sa date est interdit (CCH-12, p. 177). Textes :
//! `atelier/reference/norme-dsn/texte/bloc-S21.G00.31.txt`, `bloc-S21.G00.34.txt` (la fin de 31.011, p. 179),
//! `chapitre-2-principes-de-constitution.txt` (p. 18-21), `chapitre-4-controles.txt` (§ 4.4.12, p. 99 : 31.001 ne
//! depasse pas le dernier jour du mois principal declare).
//!
//! CE QUI A ETE DECLARE SE RELIT, IL NE SE RECALCULE PAS (regles 1 et 4) : l'identite de reference est celle de la fiche
//! que lit la paie declaree la plus recente du salarie, d'un mois anterieur au mois declare (`dsn::paies_declarees_du_mois`,
//! la version de l'engagement sur laquelle sa DSN est partie). La date du changement est celle que la fiche datee porte :
//! le `depuis` de la valeur qui suit la derniere qui porte l'ancienne valeur declaree (`droit::fiche::dater`). Une fiche qui
//! ne date pas le changement refuse : la date ne se devine pas.

use std::collections::BTreeMap;

use droit::contexte::Contexte;
use droit::fiche::{ChampsSalarie, Lue, Datee, FicheSalarie, FicheSalarieDatee};
use socle::date::Jour;
use crate::dsn::Fichier;
use socle::num::Resultat;
use crate::usages::strip;

/// UN IDENTIFIANT DE L'INDIVIDU : le champ de la fiche que le bloc 30 ecrit et que le bloc 31 peut porter (son nom, pour
/// lire son histoire dans la fiche datee), ses rubriques 30 et 31, son libelle, sa valeur telle que le bloc 30 l'ecrit
/// (la date de naissance en JJMMAAAA), et la case qui la porte.
pub struct Identifiant {
    pub champ: &'static str,
    pub r30: &'static str,
    pub r31: &'static str,
    pub libelle: &'static str,
    pub ecrite: fn(&ChampsSalarie) -> Option<String>,
    pub poser: fn(&mut ChampsSalarie, &ChampsSalarie),
    /// Ses valeurs dans la fiche datee, de la plus ancienne a la plus recente, telles que le bloc 30 les ecrit.
    pub datees: fn(&FicheSalarieDatee) -> Vec<(Option<Jour>, Option<String>)>,
}

/// Un texte de la fiche tel que le bloc 30 l'ecrit.
fn texte_ecrit<T: AsRef<str>>(c: &Option<T>) -> Option<String> {
    c.portee().map(|t| strip(t).to_string())
}

/// Les identifiants, dans l'ordre du bloc 31 (p. 177).
pub const IDENTIFIANTS: [Identifiant; 4] = [
    Identifiant {
        champ: "nir",
        r30: "S21.G00.30.001",
        r31: "S21.G00.31.008",
        libelle: "NIR",
        ecrite: |s| texte_ecrit(&s.nir),
        poser: |s, au| s.nir = au.nir.clone(),
        datees: |b| datees(&b.nir, texte_ecrit),
    },
    Identifiant {
        champ: "nom",
        r30: "S21.G00.30.002",
        r31: "S21.G00.31.009",
        libelle: "nom de famille",
        ecrite: |s| texte_ecrit(&s.nom),
        poser: |s, au| s.nom = au.nom.clone(),
        datees: |b| datees(&b.nom, texte_ecrit),
    },
    Identifiant {
        champ: "prenom",
        r30: "S21.G00.30.004",
        r31: "S21.G00.31.010",
        libelle: "prenoms",
        ecrite: |s| texte_ecrit(&s.prenom),
        poser: |s, au| s.prenom = au.prenom.clone(),
        datees: |b| datees(&b.prenom, texte_ecrit),
    },
    Identifiant {
        champ: "date_naissance",
        r30: "S21.G00.30.006",
        r31: "S21.G00.31.011",
        libelle: "date de naissance",
        ecrite: |s| s.date_naissance.dite().map(Jour::jjmmaaaa),
        poser: |s, au| s.date_naissance = au.date_naissance.clone(),
        datees: |b| datees(&b.date_naissance, |c| c.dite().map(Jour::jjmmaaaa)),
    },
];

/// L'identite d'un individu telle qu'une DSN l'a declaree : chaque case vide quand la DSN ne l'a pas portee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaree {
    /// Le mois principal declare (AAAA-MM) et le nom du fichier qui l'a portee.
    pub mois: String,
    pub fichier: String,
    pub nir: Option<String>,
    pub nom: Option<String>,
    pub prenoms: Option<String>,
    /// JJMMAAAA, comme 30.006.
    pub naissance: Option<String>,
    /// Le numero technique temporaire (30.020), s'il y en avait un.
    pub ntt: Option<String>,
    pub matricule: Option<String>,
}

impl Declaree {
    fn valeur(&self, champ: &str) -> Option<&str> {
        match champ {
            "nir" => self.nir.as_deref(),
            "nom" => self.nom.as_deref(),
            "prenom" => self.prenoms.as_deref(),
            _ => self.naissance.as_deref(),
        }
    }

    /// L'identite seule (sans le mois ni le fichier) : deux etablissements d'un meme mois doivent dire la meme.
    fn identite(&self) -> [&Option<String>; 5] {
        [&self.nir, &self.nom, &self.prenoms, &self.naissance, &self.ntt]
    }
}

/// Les valeurs d'une liste datee, de la plus ancienne a la plus recente, telles que le bloc 30 les ecrit (`ecrite` ;
/// `None` : une valeur qui ne ressemble a aucune valeur declaree).
fn datees<T>(liste: &Option<Vec<Datee<T>>>, ecrite: fn(&Option<T>) -> Option<String>) -> Vec<(Option<Jour>, Option<String>)> {
    liste.iter().flat_map(|l| droit::fiche::valeurs_datees(l)).map(|e| (e.depuis, ecrite(&e.valeur))).collect()
}

/// La fiche telle que le bloc 30 doit la lire pour le mois declare : ses identifiants tels qu'ils valaient le dernier
/// jour du mois (`fin`), lus dans la fiche datee (`brute`), le reste tel quel. Un changement d'identite date APRES le
/// mois declare n'entre pas dans sa DSN, sans quoi 31.001 depasserait le dernier jour du mois principal declare
/// (§ 4.4.12, p. 99) -- il ira dans la DSN du mois ou il prend effet.
pub fn identite_du_mois(salarie: &FicheSalarie, brute: Option<&FicheSalarieDatee>, fin: &Jour) -> Resultat<FicheSalarie> {
    let au: ChampsSalarie = brute.map(|b| b.au(fin)).unwrap_or_default();
    let mut sortie = salarie.clone();
    for i in &IDENTIFIANTS {
        (i.poser)(&mut sortie.champs, &au);
    }
    Ok(sortie)
}

/// Les identites declarees des mois anterieurs au mois declare, lues a la demande, du plus recent au plus ancien.
pub struct Declarations<'a> {
    ctx: &'a Contexte,
    departs: &'a engagement::departs::Departs,
    mois: Vec<String>,
    lus: BTreeMap<usize, Vec<Declaree>>,
}

impl<'a> Declarations<'a> {
    /// Les mois saisis avant `mois`.
    pub fn avant(ctx: &'a Contexte, mois: &str, departs: &'a engagement::departs::Departs) -> Resultat<Declarations<'a>> {
        let mut avant: Vec<String> = engagement::stock::mois_saisis(&ctx.racine)?.into_iter().map(String::from).filter(|m| m.as_str() < mois).collect();
        avant.sort();
        avant.reverse();
        Ok(Declarations { ctx, departs, mois: avant, lus: BTreeMap::new() })
    }

    /// Les individus que les paies du `i`-ieme mois qu'une DSN partie a declarees portent (`engagement::etats::paie_declaree`),
    /// avec la fiche que leur calcul a lue : l'identite que leur DSN a ecrite. Une paie qu'aucune DSN n'a encore declaree
    /// n'a rien dit de lui.
    fn lire(&mut self, i: usize) -> Resultat<&Vec<Declaree>> {
        if !self.lus.contains_key(&i) {
            let mois = &self.mois[i];
            let ctx_m = self.ctx.du_mois(mois)?;
            let mut paies = Vec::new();
            for e in engagement::etats::du_mois(self.ctx, mois, self.departs)? {
                if let Some(p) = e.valide().map(|v| engagement::etats::paie_declaree(&ctx_m, v.objet(), self.departs, mois)).transpose()?.flatten() {
                    paies.push(p);
                }
            }
            let mut tous = Vec::new();
            for p in paies {
                let s = &p.salarie;
                let nir = crate::ntt::nir_de(s);
                tous.push(Declaree {
                    mois: mois.clone(),
                    fichier: "la paie declaree".to_string(),
                    ntt: match nir {
                        None => Some(crate::ntt::numero(self.ctx.entreprise.siren.ecrite("siren")?, s)?),
                        Some(_) => None,
                    },
                    nir,
                    nom: texte_ecrit(&s.nom),
                    prenoms: texte_ecrit(&s.prenom),
                    naissance: s.date_naissance.dite().map(Jour::jjmmaaaa),
                    matricule: texte_ecrit(&s.code_salarie),
                });
            }
            self.lus.insert(i, tous);
        }
        Ok(&self.lus[&i])
    }

    /// L'identite que la derniere DSN qui le porte a declaree pour ce salarie, ou `None` s'il n'a jamais ete declare.
    /// Il se reconnait a un NIR que sa fiche (`brute`, datee) a porte, sinon a son matricule
    /// (30.019 ; il finit le NTT et ne change pas sans NIR, `moteur::routes::fiches::ecrire_fiche`).
    pub fn derniere(&mut self, salarie: &ChampsSalarie, brute: Option<&FicheSalarieDatee>) -> Resultat<Option<Declaree>> {
        let mut nirs: Vec<String> = crate::ntt::nir_de(salarie).into_iter().collect();
        nirs.extend(brute.into_iter().flat_map(IDENTIFIANTS[0].datees).filter_map(|(_, v)| v));
        let matricule = texte_ecrit(&salarie.code_salarie);
        for i in 0..self.mois.len() {
            let tous = self.lire(i)?;
            let mut siens: Vec<&Declaree> = tous.iter().filter(|d| d.nir.as_ref().is_some_and(|n| nirs.contains(n))).collect();
            if siens.is_empty() && matricule.is_some() {
                siens = tous.iter().filter(|d| d.matricule == matricule).collect();
            }
            let Some(premier) = siens.first() else { continue };
            if let Some(autre) = siens.iter().find(|d| d.identite() != premier.identite()) {
                return Err(format!(
                    "{} : la DSN de {} le declare deux fois, sous deux identites ({} et {}) : laquelle a ete declaree ne se \
devine pas. Rien n'est ecrit.",
                    qui(salarie),
                    premier.mois,
                    decrire(premier),
                    decrire(autre)
                ));
            }
            return Ok(Some((*premier).clone()));
        }
        Ok(None)
    }
}

/// L'identite que la derniere DSN mensuelle a declaree pour `salarie` (sa fiche lue), des mois saisis jusqu'a celui de
/// `jour` compris : ce que lit une DSN hors du mois (un signalement, une demande de taux) pour poser son NTT
/// (`ntt::poser_ntt`).
pub fn derniere_declaree(ctx: &Contexte, salarie: &droit::fiche::SalarieDuContrat, jour: &Jour, departs: &engagement::departs::Departs) -> Resultat<Option<Declaree>> {
    Declarations::avant(ctx, &jour.mois_civil().suivant().to_string(), departs)?.derniere(salarie, ctx.salarie_brut(salarie.id.as_str()))
}

fn qui(salarie: &ChampsSalarie) -> String {
    let nom = salarie.nomme().unwrap_or_else(|| "?".to_string());
    match salarie.code_salarie.portee() {
        Some(matricule) => format!("{} (matricule {})", nom, strip(matricule)),
        None => format!("{} (sans matricule)", nom),
    }
}

fn decrire(d: &Declaree) -> String {
    let dite = |c: &Option<String>| c.clone().unwrap_or_else(|| "?".to_string());
    format!(
        "{} {}, ne(e) le {}, {}",
        dite(&d.prenoms),
        dite(&d.nom),
        dite(&d.naissance),
        match &d.nir {
            Some(nir) => format!("NIR {}", nir),
            None => format!("NTT {}", dite(&d.ntt)),
        }
    )
}

/// Le jour d'effet que la fiche datee `brute` donne au changement de `champ` depuis la valeur `declaree` : le `depuis`
/// de la valeur qui suit la derniere qui la porte, au plus tard la fin du mois declare (`fin`).
fn date_d_effet(brute: Option<&FicheSalarieDatee>, identifiant: &Identifiant, declaree: &str, fin: &Jour) -> Resultat<Option<Jour>> {
    let valeurs = brute.map(identifiant.datees).unwrap_or_default();
    let mut meilleur: Option<Jour> = None;
    for paire in valeurs.windows(2) {
        let ((_, v), (suivant, _)) = (&paire[0], &paire[1]);
        if let Some(suivant) = suivant.filter(|s| s <= fin) {
            if v.as_deref() == Some(declaree) {
                meilleur = Some(suivant);
            }
        }
    }
    Ok(meilleur)
}

/// Ecrit, apres le bloc 30 de `salarie` (sa fiche telle que le bloc 30 l'a lue, `identite_du_mois`), les blocs 31 de
/// ce qui differe de l'identite `declaree` ; rien s'il n'a jamais ete declare. `mois` (AAAA-MM) et `fin` : le mois
/// declare et son dernier jour.
///
/// Le NIR qui ARRIVE (declare sans NIR, sous un NTT) n'a pas d'« ancien NIR » : la norme fait le lien par le NTT porte
/// a cote du NIR (30.020, p. 175 ; `ntt.rs`), pas par le bloc 31.
pub fn ecrire(f: &mut Fichier, salarie: &ChampsSalarie, brute: Option<&FicheSalarieDatee>, declaree: Option<&Declaree>, mois: &str, fin: &Jour) -> Resultat<()> {
    let Some(d) = declaree else { return Ok(()) };
    let mut par_date: BTreeMap<Jour, Vec<(&str, String)>> = BTreeMap::new();
    let mut sans_date: Vec<String> = Vec::new();
    for identifiant @ Identifiant { champ, r31, libelle, ecrite: valeur, .. } in &IDENTIFIANTS {
        let (champ, r31, libelle) = (*champ, *r31, *libelle);
        let maintenant = valeur(salarie);
        if d.valeur(champ) == maintenant.as_deref() {
            continue;
        }
        // Une case que la DSN n'a pas portee n'a pas d'ancienne valeur : le NIR qui arrive se lie par le NTT.
        let Some(avant) = d.valeur(champ) else { continue };
        if champ == "nir" && maintenant.is_none() {
            return Err(format!(
                "{} a ete declare(e) sous le NIR {} (DSN de {}, {}), et sa fiche n'a plus de NIR : un NIR declare ne se \
retire pas, il se reporte a l'identique d'une DSN a la suivante (cahier technique, § 2.1.1, p. 18). Remettre le NIR \
dans la fiche, ou le remplacer par le NIR corrige, date. Rien n'est ecrit.",
                qui(salarie),
                avant,
                d.mois,
                d.fichier
            ));
        }
        match date_d_effet(brute, identifiant, avant, fin)? {
            Some(j) => par_date.entry(j).or_default().push((r31, avant.to_string())),
            None => sans_date.push(format!(
                "  - {} : declare(e) \u{ab} {} \u{bb} dans la DSN de {} ({}), {} dans la fiche",
                libelle,
                avant,
                d.mois,
                d.fichier,
                match &maintenant {
                    Some(m) => format!("\u{ab} {} \u{bb}", m),
                    None => "rien".to_string(),
                }
            )),
        }
    }
    if !sans_date.is_empty() {
        return Err(format!(
            "La DSN de {} ne se fait pas : l'identite de {} a change depuis sa derniere declaration, et la fiche ne dit pas \
depuis quand.\n{}\nLa DSN declare l'ancienne valeur et la date du changement (bloc \u{ab} Changements individu \u{bb} \
S21.G00.31, cahier technique p. 177 ; \u{ab} obligatoirement des la premiere DSN mensuelle dans laquelle est utilisee \
la nouvelle identite \u{bb}, p. 20) : dater ce changement dans la fiche du salarie (y remettre l'ancienne valeur, \
enregistrer, puis saisir la nouvelle : l'ecran demande alors la date d'effet). Rien n'est ecrit.",
            mois,
            qui(salarie),
            sans_date.join("\n")
        ));
    }
    for (jour, rubriques) in par_date {
        f.add("S21.G00.31.001", jour.jjmmaaaa());
        for (r, v) in rubriques {
            f.add(r, v);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn declaree() -> Declaree {
        Declaree {
            mois: "2026-07".into(),
            fichier: "la paie declaree".into(),
            nir: Some("2901075999009".into()),
            nom: Some("CLAIRON".into()),
            prenoms: Some("LUCILE".into()),
            naissance: Some("02101990".into()),
            ntt: None,
            matricule: Some("6".into()),
        }
    }

    fn fin() -> Jour {
        Jour::lire("2026-09-30").unwrap()
    }

    fn lue(v: &Value) -> ChampsSalarie {
        droit::fiche::lire_champs(v, "la fiche").unwrap()
    }

    /// Une fiche datee : chaque valeur de tout temps, puis `apres` (champ, depuis, valeur).
    fn fiche(valeurs: Value, apres: &[(&str, &str, &str)]) -> FicheSalarieDatee {
        use droit::fiche::FicheDatee;
        let mut f = FicheSalarieDatee::neuve(&lue(&valeurs));
        for (champ, depuis, v) in apres {
            let ch = droit::fiche::lire_champs(&json!({"case": champ, "valeur": v}), "le changement").unwrap();
            f.changer(ch, Some(&Jour::lire(depuis).unwrap()), None);
        }
        f
    }

    #[test]
    fn rien_ne_change_rien_ne_s_ecrit() {
        let s = json!({"nir": "2901075999009", "nom": "CLAIRON", "prenom": "LUCILE", "date_naissance": "1990-10-02", "code_salarie": "6"});
        let b = fiche(s.clone(), &[]);
        let mut f = Fichier::nouveau();
        ecrire(&mut f, &lue(&s), Some(&b), Some(&declaree()), "2026-09", &fin()).unwrap();
        assert!(f.lignes.is_empty());
        ecrire(&mut f, &lue(&s), Some(&b), None, "2026-09", &fin()).unwrap();
        assert!(f.lignes.is_empty());
    }

    #[test]
    fn un_changement_date_porte_l_ancienne_valeur_et_un_non_date_refuse() {
        let s = json!({"nir": "2901075999009", "nom": "DUVAL", "prenom": "LUCILE", "date_naissance": "1990-10-02", "code_salarie": "6"});
        let s = lue(&s);
        let b = fiche(json!({"nom": "CLAIRON"}), &[("nom", "2026-09-15", "DUVAL")]);
        let mut f = Fichier::nouveau();
        ecrire(&mut f, &s, Some(&b), Some(&declaree()), "2026-09", &fin()).unwrap();
        assert_eq!(f.lignes, vec!["S21.G00.31.001,'15092026'", "S21.G00.31.009,'CLAIRON'"]);
        // Une fiche qui n'a jamais porte la valeur declaree ne date rien.
        let b = fiche(json!({"nom": "AUTRE"}), &[("nom", "2026-09-15", "DUVAL")]);
        let e = ecrire(&mut Fichier::nouveau(), &s, Some(&b), Some(&declaree()), "2026-09", &fin()).unwrap_err();
        assert!(e.contains("nom de famille") && e.contains("CLAIRON") && e.contains("DUVAL"), "{}", e);
    }

    #[test]
    fn un_nir_declare_ne_disparait_pas() {
        let s = json!({"nom": "CLAIRON", "prenom": "LUCILE", "date_naissance": "1990-10-02", "code_salarie": "6"});
        let e = ecrire(&mut Fichier::nouveau(), &lue(&s), Some(&fiche(s.clone(), &[])), Some(&declaree()), "2026-09", &fin()).unwrap_err();
        assert!(e.contains("n'a plus de NIR"), "{}", e);
    }

    #[test]
    fn l_identite_du_mois_ignore_un_changement_posterieur() {
        let s = json!({"nom": "DUVAL", "adresse": "NOUVELLE"});
        let b = fiche(json!({"nom": "CLAIRON", "adresse": "ANCIENNE"}), &[("nom", "2026-10-02", "DUVAL"), ("adresse", "2026-10-02", "NOUVELLE")]);
        let s = FicheSalarie { id: droit::contexte::IdSalarie::lire("DUVAL").unwrap(), champs: lue(&s) };
        let m = identite_du_mois(&s, Some(&b), &fin()).unwrap();
        assert_eq!(m.nom.portee(), Some("CLAIRON"));
        // Ce qui n'identifie pas se lit tel quel (la derniere valeur connue, § 2.1.2.4.2 par analogie).
        assert_eq!(m.adresse.portee(), Some("NOUVELLE"));
    }
}
