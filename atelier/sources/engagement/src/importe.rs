//! L'ENGAGEMENT IMPORTE : un engagement que l'import a tire des pieces d'un autre logiciel (les DSN deposees, les AEM,
//! les bulletins de paie). Il vit avec les autres, dans `engagements/AAAA-MM.json`, le fichier du mois ou il commence,
//! et porte `importe` :
//!
//! ```text
//! "importe": {"le", "contrat"?: {ce que l'AEM ou le bulletin atteste}, "paies": {"AAAA-MM": {ce qui a ete verse}}}
//! ```
//!
//! Il est valide et ses documents sont partis : ce qui en est parti, c'est l'import lui-meme (les pieces de l'autre
//! logiciel). IL SE LIT, IL NE SE MODIFIE PAS ET NE SE RECALCULE JAMAIS : il ne touche aucun mois de la saisie
//! (`stock::Objet::couvre`), donc aucun calcul, aucune declaration ni aucun document qui part ne le prend ; l'ecran ne peut ni le
//! reecrire ni en poser un double (`stock::ecrire_la_carte`). Son numero de contrat est celui que la DSN deposee lui
//! a donne (S21.G00.40.009), jamais un numero tire : il compte parmi les numeros pris (`numeros`).
//!
//! Ce qui le lit : les cumuls des annexes 8 et 10 (`cumul`, par ce que le contrat atteste), le mois tenu avant
//! PuppetPaie (`contrats_lus`, `paies_lues` : une ligne par contrat atteste, une ligne par paie versee), et les etats,
//! qui lisent chacune de ses paies dans les mots du moteur (`Importe::paie`, par `etats::paies_du_mois`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::calcul::{Importee, LigneCotisation, Paie, PlafondTempsPartiel};
use crate::stock::{self, Objet};
use droit::contexte::Contexte;
use socle::date::Jour;
use socle::num::Decimal;
use socle::date::en_texte;
use socle::fichiers::vrai;
use socle::num::Resultat;

/// Dans un engagement importe : ce que l'import a lu de lui.
pub const IMPORTE: &str = "importe";

/// Un engagement importe.
#[derive(Debug, Clone)]
pub struct Importe {
    objet: Objet,
}

impl Importe {
    /// L'engagement `o` s'il est importe.
    pub fn de(o: Objet) -> Option<Importe> {
        o.est_importe().then_some(Importe { objet: o })
    }

    /// L'engagement tel qu'il est ecrit.
    pub fn objet(&self) -> &Objet {
        &self.objet
    }

    fn lu(&self) -> &Map<String, Value> {
        static VIDE: std::sync::OnceLock<Map<String, Value>> = std::sync::OnceLock::new();
        self.objet.brut.get(IMPORTE).and_then(Value::as_object).unwrap_or_else(|| VIDE.get_or_init(Map::new))
    }

    fn texte(&self, k: &str) -> String {
        self.objet.brut.get(k).filter(|x| vrai(x)).map(en_texte).unwrap_or_default()
    }

    /// Quand il a ete importe.
    pub fn le(&self) -> String {
        self.lu().get("le").map(en_texte).unwrap_or_default()
    }

    /// Ce que l'AEM ou le bulletin atteste du contrat (heures, cachets, jours, brut, numero d'objet...), s'il l'est.
    pub fn contrat(&self) -> Option<&Map<String, Value>> {
        self.lu().get("contrat").and_then(Value::as_object)
    }

    /// Ce qui a ete verse, mois par mois.
    pub fn paies(&self) -> BTreeMap<String, &Map<String, Value>> {
        self.lu().get("paies").and_then(Value::as_object).into_iter().flatten().filter_map(|(m, p)| Some((m.clone(), p.as_object()?))).collect()
    }

    /// LE CONTRAT D'USAGE QUI CONTINUE ICI : le dernier mois verse par l'ancien logiciel, quand le contrat que la DSN
    /// deposee declare finit apres lui (S21.G00.40.010, sa fin previsionnelle : `norme-dsn/texte/bloc-S21.G00.40.txt`).
    /// Les mois qui suivent se paient ici, sous le numero et les dates que cette DSN lui a donnes (regle 4). Un contrat
    /// qu'une AEM atteste est fini (l'AEM se remet a la fin du contrat) ; un contrat paye au mois a sa propre reprise,
    /// sa nature dite (`import::nature`) ; sans numero, rien ne se reprend.
    pub fn continue_apres(&self) -> Option<String> {
        let dernier = self.paies().into_keys().max()?;
        let fin = self.texte("fin");
        let contrat = self.contrat();
        let lu = |k: &str| contrat.and_then(|c| c.get(k)).filter(|x| vrai(x)).map(en_texte).unwrap_or_default();
        let paye_au_mois = droit::natures::nature(Some(&lu("nature"))).is_ok_and(|n| n.mensuelle()) || vrai(contrat.and_then(|c| c.get("nature_a_dire")).unwrap_or(&Value::Null));
        let continue_ = fin.len() >= 7 && &fin[..7] > dernier.as_str() && lu("atteste_par") != "AEM" && !paye_au_mois && !self.texte("numero").is_empty();
        continue_.then_some(dernier)
    }

    /// Les mois qu'il touche : de son debut a sa fin (au dernier mois verse, s'il continue ici : `continue_apres`), et
    /// ceux ou il a ete verse. Un contrat sans fin (un CDI) ne touche que les mois ou il a ete verse : des mois d'avant
    /// le premier bulletin repris, les pieces ne disent rien.
    pub fn mois(&self) -> Vec<String> {
        let (debut, fin) = (self.texte("debut"), self.texte("fin"));
        let mut v: Vec<String> = self.paies().into_keys().collect();
        if debut.len() >= 7 {
            let fin = match self.continue_apres() {
                Some(dernier) => format!("{}-01", dernier),
                None if fin.len() >= 7 => fin,
                None if !v.is_empty() => {
                    v.sort();
                    return v;
                }
                None => debut.clone(),
            };
            let mut m = debut[..7].to_string();
            while m.as_str() <= &fin[..7] {
                v.push(m.clone());
                match stock::mois_apres(&m) {
                    Ok(n) => m = n,
                    Err(_) => break,
                }
            }
        }
        v.sort();
        v.dedup();
        v
    }

    /// Le contrat atteste, en une ligne : `{"salarie", "debut", "fin", <ce qu'il atteste>}`.
    pub fn ligne_du_contrat(&self) -> Option<Value> {
        let c = self.contrat()?;
        let mut l = self.bornes();
        l.extend(c.iter().map(|(k, v)| (k.clone(), v.clone())));
        Some(Value::Object(l))
    }

    /// Les paies versees, une ligne par mois : `{"mois", "salarie", "debut", "fin", "contrat"?, <ce qui a ete verse>}`.
    pub fn lignes_des_paies(&self) -> Vec<Value> {
        self.paies()
            .into_iter()
            .map(|(m, p)| {
                let mut l = self.bornes();
                l.insert("mois".into(), Value::String(m));
                l.extend(p.iter().map(|(k, v)| (k.clone(), v.clone())));
                Value::Object(l)
            })
            .collect()
    }

    fn bornes(&self) -> Map<String, Value> {
        let mut l = Map::new();
        for k in ["salarie", "debut", "fin"] {
            l.insert(k.into(), Value::String(self.texte(k)));
        }
        if let Some(n) = self.objet.brut.get("numero").filter(|n| vrai(n)) {
            l.insert("contrat".into(), n.clone());
        }
        l.insert("importe_le".into(), Value::String(self.le()));
        l
    }
}

/// Les montants d'une paie importee, tels que ses pieces les nomment, et la grandeur de la paie qu'ils sont.
const MONTANTS: [&str; 7] = ["brut", "cotisations_salariales", "cotisations_patronales", "net_imposable", "net_social", "cout_employeur", "pas"];

impl Importe {
    /// LA PAIE DE `mois`, DANS LES MOTS DU MOTEUR : ce que les pieces de l'ancien logiciel disent de ce qui a ete verse,
    /// range comme le calcul range le sien (`calcul::Paie`), et rien de calcule. L'engagement porte l'identite importee
    /// (salarie, bornes, numero, nature : muette, un CDDU, regle 6), le spectacle dont la fiche a son numero d'objet,
    /// l'emploi que le bulletin imprime, les lignes de remuneration lues et le jour du versement. Les lignes de
    /// cotisation sont celles du bulletin, sous la cotisation que l'import leur a donnee (`cotisation`).
    ///
    /// LES BASES se rangent ou le bareme de la paie les range : la base d'une ligne de cotisation du bulletin sous la
    /// base que le bareme donne a cette cotisation (`rubriques_bulletin`, `base`) ; une base que la DSN deposee declare
    /// (bloc 78, son code) sous celle que le bareme y declare (`cotisations_dsn`, `base78`), quand il n'en declare
    /// qu'une. Le bulletin passe devant la DSN, comme l'import le fait pour le brut et les nets.
    ///
    /// Ce que les pieces ne disent pas vaut zero et se nomme (`Importee::non_dits`) : un etat le laisse vide.
    pub fn paie(&self, ctx: &Contexte, mois: &str) -> Resultat<Paie> {
        let paies = self.paies();
        let p = *paies.get(mois).ok_or_else(|| format!("{} : aucune paie importee en {}", self.texte("salarie"), mois))?;
        let lu = |k: &str| p.get(k).filter(|x| vrai(x)).map(en_texte).filter(|x| !x.trim().is_empty());
        let montant = |k: &str| lu(k).and_then(|x| socle::num::decimal_lu(x.trim()));
        let contrat = self.contrat();
        let du_contrat = |k: &str| contrat.and_then(|c| c.get(k)).filter(|x| vrai(x)).map(en_texte);
        let zero = Decimal::new(0, 2);
        let mut non_dits: Vec<&'static str> = MONTANTS.iter().copied().filter(|k| montant(k).is_none()).collect();

        let nature = du_contrat("nature").or_else(|| Some(self.texte("nature")).filter(|n| !n.is_empty()));
        let nat = droit::natures::nature(nature.as_deref())?;
        let ctx = ctx.du_mois(mois)?;
        let cle = self.texte("salarie");
        let sal = ctx.salarie(&cle)?;
        let objet = lu("numero_objet").or_else(|| du_contrat("numero_objet")).unwrap_or_default().to_uppercase();
        let spectacle = ctx.spectacles.iter().find(|(_, f)| !objet.is_empty() && f.get("numero_objet").map(en_texte).map(|n| n.to_uppercase()) == Some(objet.clone())).map(|(k, _)| k.clone());
        let spec = match &spectacle {
            Some(k) => ctx.spectacle(k)?,
            None => Value::Null,
        };
        let imprime = p.get("emploi_imprime").and_then(Value::as_object);
        let emploi = |k: &str| imprime.and_then(|m| m.get(k)).filter(|x| vrai(x)).map(en_texte);
        let jour = |t: String| Jour::lire(&t);
        let debut = jour(self.texte("debut")).or_else(|_| jour(lu("debut").unwrap_or_default()))?;
        let fin = jour(self.texte("fin")).or_else(|_| jour(lu("fin").unwrap_or_default())).ok();
        let mut dates_travail = Vec::new();
        for d in p.get("dates").and_then(Value::as_array).into_iter().flatten() {
            let (Ok(mut j), Ok(b)) = (jour(d.get("debut").map(en_texte).unwrap_or_default()), jour(d.get("fin").map(en_texte).unwrap_or_default())) else { continue };
            while j <= b {
                dates_travail.push(j);
                j = j.plus_jours(1)?;
            }
        }
        let brut = montant("brut").unwrap_or(zero);
        // LES LIGNES DE REMUNERATION QUE LES PIECES DISENT : celles du bulletin ; a defaut, le brut que la DSN declare.
        let mut lignes: Vec<Value> = p.get("lignes").and_then(Value::as_array).cloned().unwrap_or_default();
        if lignes.is_empty() {
            lignes.push(serde_json::json!({"libelle": "brut déclaré", "montant": brut.to_string(), "soumis": true}));
        }
        let quotite = lu("quotite").and_then(|q| q.replace(',', ".").parse::<f64>().ok());
        let mut engagement = crate::Engagement {
            salarie: cle.clone(),
            nom: socle::texte::salarie_nomme_ou(Some(&sal), &cle),
            spectacle: spectacle.clone(),
            dates_travail,
            lignes,
            debut,
            fin,
            numero: Some(self.texte("numero")).filter(|n| !n.is_empty()).or_else(|| p.get("bulletin").and_then(|b| b.get("contrat")).filter(|x| vrai(x)).map(en_texte)),
            mois: mois.to_string(),
            planning: Vec::new(),
            planning_lieux: BTreeMap::new(),
            jours: 0,
            jours_contrat: 0,
            date_paiement: lu("verse_le"),
            taux_cachet: None,
            cachets: 0,
            heures: None,
            lieu: String::new(),
            lieux: BTreeMap::new(),
            emploi: emploi("emploi"),
            emploi_affichage: emploi("emploi_affichage"),
            pcs_ese: emploi("pcs_ese"),
            code_emploi_ft: emploi("code_emploi_ft"),
            statut: emploi("statut"),
            statut_conventionnel: emploi("statut_conventionnel"),
            statut_categoriel_rc: emploi("statut_categoriel_rc"),
            etablissement: None,
            nature: nature.clone(),
            quotite_contrat: quotite,
            quotite_reference_saisie: None,
            modalite_temps_travail_saisie: None,
            motif_recours: None,
            motif_rupture: None,
            notification_rupture: None,
            preavis: None,
            dernier_jour_paye: None,
            date_modification: None,
            transaction: None,
            suspensions: Vec::new(),
            avant: None,
            periode: None,
            ecrit: Map::new(),
        };
        let remuneration = remuneration_lue(&engagement, &ctx)?;
        if engagement.lignes.iter().all(|l| l.get("rubrique").is_none_or(|r| !vrai(r))) {
            non_dits.extend(["cachets", "heures"]);
        }
        engagement.cachets = remuneration.iter().filter(|r| r.unite == "cachet").map(|r| r.quantite).sum::<Decimal>().trunc().to_string().parse().unwrap_or(0);
        let heures: Decimal = remuneration.iter().map(|r| r.heures_annexes).sum();
        engagement.heures = (!heures.is_zero()).then(|| heures.to_string().parse().unwrap_or(0.0));
        engagement.jours = engagement.dates_travail.len() as i64;
        engagement.jours_contrat = engagement.jours;

        // Les lignes de cotisation du bulletin, sous leur cotisation ; une ligne que l'import n'a pas qualifiee garde
        // son libelle, sans cotisation.
        let taux = |x: Option<&Value>| x.filter(|v| vrai(v)).and_then(|v| socle::num::decimal_lu(en_texte(v).trim())).and_then(|d| d.to_string().parse::<f64>().ok());
        let mt = |x: Option<&Value>| x.filter(|v| vrai(v)).and_then(|v| socle::num::decimal_lu(en_texte(v).trim()));
        let cotisations: Vec<LigneCotisation> = p
            .get("cotisations")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|l| LigneCotisation {
                id: l.get("cotisation").filter(|x| vrai(x)).map(en_texte).unwrap_or_default(),
                famille: l.get("famille").filter(|x| vrai(x)).map(en_texte).unwrap_or_default(),
                libelle: l.get("libelle").map(en_texte).unwrap_or_default(),
                base: mt(l.get("base")).unwrap_or(zero),
                taux_pat: taux(l.get("taux_employeur")),
                montant_pat: mt(l.get("montant_employeur")).unwrap_or(zero),
                taux_sal: taux(l.get("taux_salarie")),
                montant_sal: mt(l.get("montant_salarie")).unwrap_or(zero),
                masquer_base_et_taux: false,
            })
            .collect();

        // Les bases, ou le bareme de la paie les range.
        let mut bases: BTreeMap<String, Decimal> = BTreeMap::new();
        bases.insert("brut".into(), brut);
        let fin_periode = engagement.fin_periode()?;
        let caisse = droit::caisse_conges::affilie(&nat, &sal, &engagement.debut, engagement.fin.as_ref())?;
        let bar = crate::calcul::bareme_du_salarie(&ctx, &sal, &nat, &fin_periode, caisse, false)?;
        let liste = |k: &str| bar.get(k).and_then(Value::as_array).cloned().unwrap_or_default();
        for l in cotisations.iter().filter(|l| !l.id.is_empty() && !l.base.is_zero()) {
            if let Some(cle) = liste("rubriques_bulletin").iter().find(|r| r.get("id").map(en_texte) == Some(l.id.clone())).and_then(|r| r.get("base")).map(en_texte) {
                bases.entry(cle).or_insert(l.base);
            }
        }
        for (code, m) in p.get("bases").and_then(Value::as_object).into_iter().flatten() {
            let Some(m) = socle::num::decimal_lu(en_texte(m).trim()) else { continue };
            let mut cles: Vec<String> = liste("cotisations_dsn")
                .iter()
                .filter(|c| c.get("bloc78").map(en_texte).as_deref() == Some(code.as_str()))
                .filter_map(|c| c.get("base78").map(en_texte))
                .filter(|k| k != "zero")
                .collect();
            cles.sort();
            cles.dedup();
            if let [cle] = &cles[..] {
                bases.entry(cle.clone()).or_insert(m);
            }
        }
        let base_csg = bases.get("csg").copied().unwrap_or(zero);
        let plafond = ["brut_plafonne", "t1_mensuelle"].iter().find_map(|k| bases.get(*k)).filter(|t| **t < brut).copied();
        if plafond.is_none() {
            non_dits.push("plafond");
        }
        let pas = montant("pas");
        // Le net avant impot : celui que le bulletin imprime, sinon le net paye ou le net verse plus l'impot preleve.
        let net_a_payer = montant("net_avant_impot").or_else(|| Some(montant("net_a_payer").or_else(|| montant("net_verse"))? + pas?));
        if net_a_payer.is_none() {
            non_dits.push("net_a_payer");
        }

        let tp = PlafondTempsPartiel::NonConcerne;
        let mut paie = Paie::nouveau(engagement, sal, spec, brut, base_csg, plafond.unwrap_or(zero), 0.0, zero, tp);
        let c = paie.ecrire();
        c.lignes = cotisations;
        c.remuneration = remuneration;
        c.total_sal = montant("cotisations_salariales").unwrap_or(zero);
        c.total_pat = montant("cotisations_patronales").unwrap_or(zero);
        c.net_a_payer = net_a_payer.unwrap_or(zero);
        c.net_social = montant("net_social").unwrap_or(zero);
        c.net_imposable = montant("net_imposable").unwrap_or(zero);
        c.cout_employeur = montant("cout_employeur").unwrap_or(zero);
        c.numero_bulletin = p.get("bulletin").and_then(|b| b.get("numero")).filter(|x| vrai(x)).map(en_texte);
        c.bases = bases;
        c.pas = pas.map(|prelevement| droit::pas::Versement {
            taux: montant("taux_pas").and_then(|t| t.to_string().parse::<f64>().ok()).map_or(Value::Null, Value::from),
            type_taux: lu("type_pas").map_or(Value::Null, Value::String),
            identifiant: lu("identifiant_pas").map_or(Value::Null, Value::String),
            abattement: zero,
            montant_soumis: montant("net_imposable").unwrap_or(zero),
            prelevement,
        });
        c.importee = Some(Importee { le: self.le(), non_dits });
        Ok(paie)
    }
}

/// LES PAIES IMPORTEES DE `mois`, dans les mots du moteur (`Importe::paie`), une par engagement importe paye ce mois.
pub fn paies_du_mois(ctx: &Contexte, mois: &str) -> Resultat<Vec<Paie>> {
    du_mois(&ctx.racine, mois)?.iter().filter(|i| i.paies().contains_key(mois)).map(|i| i.paie(ctx, mois)).collect()
}

/// Tous les engagements importes, fichier par fichier.
pub fn tous(racine: &Path) -> Resultat<Vec<Importe>> {
    Ok(stock::objets(racine)?.into_iter().filter_map(Importe::de).collect())
}

/// Les engagements importes qui touchent `mois`.
pub fn du_mois(racine: &Path, mois: &str) -> Resultat<Vec<Importe>> {
    Ok(tous(racine)?.into_iter().filter(|i| i.mois().iter().any(|m| m == mois)).collect())
}

/// Les contrats attestes de tous les engagements importes, une ligne chacun (`Importe::ligne_du_contrat`).
pub fn contrats_lus(racine: &Path) -> Resultat<Vec<Value>> {
    Ok(tous(racine)?.iter().filter_map(Importe::ligne_du_contrat).collect())
}

/// Les paies versees de tous les engagements importes, une ligne par mois (`Importe::lignes_des_paies`).
pub fn paies_lues(racine: &Path) -> Resultat<Vec<Value>> {
    Ok(tous(racine)?.iter().flat_map(Importe::lignes_des_paies).collect())
}

/// LA DSN DE `mois` QUE L'ANCIEN LOGICIEL A DEPOSEE : une paie importee de ce mois qu'une DSN deposee declare
/// (`declare_par: "DSN"`), en une ligne (`Importe::lignes_des_paies` : `depose_le`, `fichier`...), qu'une saisie touche
/// aussi le mois ou non. Cette DSN n'est pas faite ici et ne se refait pas ici (regle 4) : ce que la saisie ajoute a
/// ce mois se declare dans une DSN suivante, comme un contrat ajoute apres la DSN de son mois.
pub fn dsn_deposee(racine: &Path, mois: &str) -> Resultat<Option<Value>> {
    Ok(du_mois(racine, mois)?
        .iter()
        .flat_map(Importe::lignes_des_paies)
        .find(|p| p.get("mois").map(en_texte).as_deref() == Some(mois) && p.get("declare_par").map(en_texte).as_deref() == Some("DSN")))
}

/// L'engagement importe du salarie `cle` (la cle de sa fiche) qui commence le `debut` et touche `mois`, s'il y en a un.
pub fn celui_de(racine: &Path, mois: &str, cle: &str, debut: &str) -> Resultat<Option<Importe>> {
    Ok(du_mois(racine, mois)?.into_iter().find(|i| i.texte("salarie") == cle && i.texte("debut") == debut))
}

/// Les contrats d'usage importes qui continuent en `mois` (`Importe::continue_apres`) : verses avant lui par l'ancien
/// logiciel, ils finissent en `mois` ou apres.
pub fn qui_continuent(racine: &Path, mois: &str) -> Resultat<Vec<Importe>> {
    Ok(tous(racine)?
        .into_iter()
        .filter(|i| i.continue_apres().is_some_and(|d| d.as_str() < mois) && i.texte("fin").get(..7).is_some_and(|f| f >= mois))
        .collect())
}

/// Le contrat importe que l'engagement `e` de la saisie continue : meme salarie, meme debut, meme numero, et il continue
/// apres son dernier mois verse (`continue_apres`).
pub fn continue_par(racine: &Path, e: &Map<String, Value>) -> Resultat<Option<Importe>> {
    let t = |k: &str| e.get(k).filter(|x| vrai(x)).map(en_texte).unwrap_or_default();
    let (salarie, debut, numero) = (t("salarie"), t("debut"), t("numero"));
    if salarie.is_empty() || debut.is_empty() || numero.is_empty() {
        return Ok(None);
    }
    Ok(tous(racine)?.into_iter().find(|i| {
        i.continue_apres().is_some() && i.texte("salarie") == salarie && i.texte("debut") == debut && i.texte("numero") == numero
    }))
}

/// LE CONTRAT `numero` DE `salarie` COMMENCE LE `debut` VIENT-IL DE L'ANCIEN LOGICIEL ? Un engagement importe l'atteste
/// (un permanent : `Importe::atteste`), ou il continue un contrat d'usage importe (`continue_par`). L'ecran le dit sur
/// la carte. Faux quand rien ne se lit.
pub fn vient_d_un_ancien_logiciel(racine: &Path, salarie: &str, debut: &str, numero: &str) -> bool {
    let tous = tous(racine).unwrap_or_default();
    tous.iter().any(|i| i.atteste(salarie, debut, numero) || (i.continue_apres().is_some() && i.texte("salarie") == salarie && i.texte("debut") == debut && i.texte("numero") == numero))
}

impl Importe {
    /// ATTESTE-T-IL LE CONTRAT `numero` DE `salarie` COMMENCE LE `debut` ? Le contrat d'un permanent qui court encore
    /// porte le debut et le numero que la DSN de l'ancien logiciel lui a donnes (`contrat_debut`, `numero` :
    /// `import::nature`) ; chacun de ses bulletins repris l'atteste, et l'engagement qui le continue ici est le meme
    /// contrat (S21.G00.40.001 et .009, `norme-dsn/texte/bloc-S21.G00.40.txt`), pas une copie.
    pub fn atteste(&self, salarie: &str, debut: &str, numero: &str) -> bool {
        let du_contrat = self.contrat().and_then(|c| c.get("contrat_debut")).filter(|x| vrai(x)).map(en_texte);
        !numero.is_empty()
            && self.texte("salarie") == salarie
            && self.texte("numero") == numero
            && du_contrat.unwrap_or_else(|| self.texte("debut")) == debut
    }

    /// LES JOURS DU CONTRAT ENTIER (S21.G00.40.013 d'un contrat d'usage : `Engagement::quotite`) d'un contrat qui continue
    /// ici, `propres` etant ceux que la saisie lui donne : les jours que les bulletins importes impriment, plus les siens ;
    /// a defaut d'un bulletin pour chaque mois verse, la quotite que la DSN deposee a declaree pour ce contrat (elle le
    /// dit pour le contrat entier, `norme-dsn/texte/bloc-S21.G00.40.txt`). `None` quand aucune piece ne les donne.
    pub fn jours_du_contrat_continue(&self, propres: usize) -> Option<i64> {
        let paies = self.paies();
        let jours = |p: &Map<String, Value>| -> Option<i64> {
            let dates = p.get("dates")?.as_array()?;
            let mut n = 0i64;
            for d in dates {
                let lire = |k: &str| socle::date::Jour::lire(&d.get(k).map(en_texte).unwrap_or_default()).ok();
                let (a, b) = (lire("debut")?, lire("fin")?);
                n += b.moins(&a) + 1;
            }
            (n > 0).then_some(n)
        };
        let lus: Option<Vec<i64>> = paies.values().map(|p| jours(p)).collect();
        if let Some(l) = lus.filter(|l| !l.is_empty()) {
            return Some(l.iter().sum::<i64>() + propres as i64);
        }
        let dernier = paies.into_values().last()?;
        dernier.get("quotite").map(en_texte).and_then(|q| q.trim().replace(',', ".").parse::<f64>().ok()).filter(|q| *q > 0.0 && q.fract() == 0.0).map(|q| q as i64)
    }
}

/// Le refus d'un geste sur un engagement importe : il se lit, il ne se modifie pas.
pub fn refus(qui: &str, debut: &str) -> String {
    let jour = socle::date::Jour::lire(debut).map(|j| j.jj_mm_aaaa()).unwrap_or_else(|_| debut.to_string());
    format!(
        "L'engagement de {} du {} est importé : il se lit tel que l'ancien logiciel l'a déclaré, il ne se modifie pas, ne se \
         recalcule pas et ne se supprime pas. Rien n'est écrit.",
        qui, jour
    )
}

/// ECRIT LES ENGAGEMENTS IMPORTES, tous : chacun dans le fichier du mois ou il commence (son `debut`, sinon le premier
/// mois ou il a ete verse), apres les engagements de la saisie. Ceux qui y etaient s'en vont ; rien d'autre ne change.
/// Rend les fichiers ecrits.
pub fn ecrire_tous(racine: &Path, importes: Vec<Map<String, Value>>) -> Resultat<Vec<PathBuf>> {
    let mut par_mois: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for e in importes {
        let debut = e.get("debut").map(en_texte).unwrap_or_default();
        let premier = e.get(IMPORTE).and_then(|i| i.get("paies")).and_then(Value::as_object).and_then(|p| p.keys().next().cloned());
        let mois = if debut.len() >= 7 { debut[..7].to_string() } else { premier.ok_or("un engagement importe sans debut ni paie")? };
        par_mois.entry(mois).or_default().push(Value::Object(e));
    }
    let mut touches: Vec<String> = tous(racine)?.iter().map(|i| i.objet.fichier.clone()).collect();
    touches.extend(par_mois.keys().cloned());
    touches.sort();
    touches.dedup();
    let mut ecrits = Vec::new();
    for f in touches {
        let existe = stock::lire_fichier(racine, &f)?;
        let neufs = par_mois.remove(&f).unwrap_or_default();
        let mut contenu = existe.clone().unwrap_or_default();
        contenu.insert("mois".into(), Value::String(f.clone()));
        let mut es: Vec<Value> =
            contenu.get("engagements").and_then(Value::as_array).into_iter().flatten().filter(|e| !e.get(IMPORTE).is_some_and(Value::is_object)).cloned().collect();
        es.extend(neufs);
        contenu.insert("engagements".into(), Value::Array(es));
        if existe.as_ref() == Some(&contenu) {
            continue;
        }
        ecrits.push(stock::poser(racine, &f, &Value::Object(contenu))?);
    }
    Ok(ecrits)
}

/// CE QUE L'IMPORT COMPLETE DE LUI-MEME : ce que les pieces ne disaient pas et que la personne dit apres l'import (la
/// nature d'un contrat de permanent, son numero). `completer` recoit chaque engagement importe tel qu'il est ecrit et
/// dit s'il l'a change ; ceux-la se reecrivent. Rend combien ont change.
pub fn completer(racine: &Path, mut completer: impl FnMut(&mut Map<String, Value>) -> bool) -> Resultat<usize> {
    let mut tous: Vec<Map<String, Value>> = tous(racine)?.into_iter().map(|i| i.objet.champs().clone()).collect();
    let mut changes = 0usize;
    for e in tous.iter_mut() {
        if completer(e) {
            changes += 1;
        }
    }
    if changes > 0 {
        ecrire_tous(racine, tous)?;
    }
    Ok(changes)
}

/// LES LIGNES DE REMUNERATION QU'UN BULLETIN IMPORTE IMPRIME, dans les mots du moteur : une ligne que l'import a
/// rattachee a une rubrique du catalogue (`rubrique`, avec sa quantite et son taux) prend ce que le catalogue dit
/// d'elle (son unite, ses heures pour les annexes 8 et 10, son assiette : `calcul::lignes_brut`) ; son montant reste
/// celui du bulletin. Une ligne sans rubrique garde son libelle et son montant, rien de plus.
fn remuneration_lue(e: &crate::Engagement, ctx: &Contexte) -> Resultat<Vec<crate::calcul::LigneBrut>> {
    let mut out = Vec::new();
    for l in &e.lignes {
        let montant = l.get("montant").filter(|x| vrai(x)).and_then(|x| socle::num::decimal_lu(en_texte(x).trim())).unwrap_or_default();
        let rattachee = ["rubrique", "quantite", "taux"].iter().all(|k| l.get(*k).is_some_and(vrai));
        if rattachee {
            let mut seule = e.clone();
            seule.lignes = vec![l.clone()];
            for mut b in crate::calcul::lignes_brut(&seule, ctx)? {
                b.montant = montant;
                out.push(b);
            }
            continue;
        }
        let soumis = l.get("soumis") != Some(&Value::Bool(false));
        out.push(crate::calcul::LigneBrut {
            code: String::new(),
            libelle: l.get("libelle").map(en_texte).unwrap_or_default(),
            unite: String::new(),
            quantite: Decimal::ZERO,
            taux: Decimal::ZERO,
            montant,
            heures_annexes: Decimal::ZERO,
            soumis,
            imposable: soumis,
            conges_spectacles: soumis,
            salaire_de_base: false,
            retabli: false,
            conges_pris: false,
            dsn_bloc52: None,
            arret_de_travail: false,
            dsn_autres_bruts: None,
            precision: None,
        });
    }
    Ok(out)
}
