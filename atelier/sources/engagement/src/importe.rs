//! L'ENGAGEMENT IMPORTE : un engagement que l'import a tire des pieces d'un autre logiciel (les DSN deposees, les AEM,
//! les bulletins de paie). Il vit avec les autres, dans `engagements/AAAA-MM.json`, le fichier du mois ou il commence,
//! et porte `importe` (`Pieces`) : quand il a ete importe, le contrat qu'une AEM, un bulletin ou un certificat atteste
//! (`ContratAtteste`), et ce qui a ete verse, mois par mois (`PaieVersee`). Cette zone se lit et s'ecrit par sa forme,
//! et par elle seule (`Pieces::lire`, `Pieces::en_valeur`) ; l'engagement la lit une fois, avec son fichier
//! (`stock::Objet::importe`).
//!
//! Ses mois sont valides et leurs documents sont partis : ce qui en est parti, c'est l'import lui-meme (les pieces de
//! l'autre logiciel). SES MOIS REPRIS SE LISENT, ILS NE SE MODIFIENT PAS ET NE SE RECALCULENT JAMAIS : ils ne sont
//! aucun mois de la saisie (`stock::Objet::couvre`), donc aucun calcul, aucune declaration ni aucun document qui part
//! ne les prend ; l'ecran ne peut ni les reecrire ni en poser un double (`stock::ecrire_la_carte`). Son numero de
//! contrat est celui que la DSN deposee lui a donne (S21.G00.40.009), jamais un numero tire : il compte parmi les
//! numeros pris (`numeros`).
//!
//! UN CONTRAT DE PERMANENT EST UN ENGAGEMENT, QU'IL AIT COMMENCE AILLEURS OU ICI. Les bulletins d'un permanent attestent
//! chacun le mois qu'ils paient, tous du meme contrat : ils font un engagement importe, dont ses paies sont les mois.
//! Paye le dernier mois repris et sans terme avant le mois qui suit (`continue_ici`), il continue ici : il vit dans le
//! fichier de son premier mois neuf (`ecrire_tous`), son mois d'entree, et ce mois et les suivants se saisissent et
//! se calculent comme ceux de tout engagement, sous la meme identite et le meme numero -- la DSN declare le contrat
//! sous le numero (S21.G00.40.009) et la date de debut (.001) que l'ancien logiciel lui a donnes, reportes a
//! l'identique d'une DSN a la suivante (`norme-dsn/texte/bloc-S21.G00.40.txt` ; cahier technique § 2.1.2,
//! `norme-dsn/texte/chapitre-2-principes-de-constitution.txt`). Il ne se modifie pas avant ce mois : ce qui le definit
//! se lit a partir de lui.
//!
//! Ce qui le lit : les cumuls des annexes 8 et 10 (`cumul`, par ce que le contrat atteste), le mois tenu avant
//! PuppetPaie (`contrats_lus`, `paies_lues` : un contrat atteste, une paie versee), et les etats, qui lisent chacune de
//! ses paies dans les mots du moteur (`Importe::paie`, par `etats::paies_du_mois`). L'import manipule les memes formes,
//! avec les bornes de leur engagement (`ContratLu`, `PaieLue`), de la lecture des pieces a l'ecriture (`Neuf`).

use std::collections::BTreeMap;

use droit::fiche::Lue;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::calcul::{Importee, LigneCotisation, Paie, PlafondTempsPartiel};
use crate::numeros::NumeroContrat;
use crate::saisie::Carte;
use crate::stock::{self, Objet};
use droit::contexte::Contexte;
use socle::date::Jour;
use socle::num::{Decimal, Resultat};

/// Dans un engagement importe : ce que l'import a lu de lui (`Pieces`).
pub const IMPORTE: &str = "importe";

/// Dans la ligne du contrat qu'un bulletin atteste : sa nature reste a dire (le bulletin d'un permanent ne la porte pas).
pub const A_DIRE: &str = "nature_a_dire";

// ------------------------------------------------------------------ la forme

/// LA ZONE `importe` D'UN ENGAGEMENT : le jour de l'import, le contrat atteste, s'il l'est, et les paies versees, par
/// mois (`AAAA-MM`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pieces {
    pub le: String,
    pub contrat: Option<ContratAtteste>,
    pub paies: BTreeMap<String, PaieVersee>,
}

/// La piece qui atteste un contrat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Attestation {
    #[serde(rename = "AEM")]
    Aem,
    #[serde(rename = "bulletin")]
    Bulletin,
    #[serde(rename = "certificat")]
    Certificat,
}

/// CE QU'UNE AEM, UN BULLETIN OU UN CERTIFICAT CONGES SPECTACLES ATTESTE DU CONTRAT (ses bornes sont celles de
/// l'engagement) : ce qu'il compte, son brut, son numero d'objet, la piece et sa page. Le contrat entier d'un permanent ne
/// garde ni compteurs, ni brut, ni piece : ce qu'un bulletin atteste de son seul mois reste a la paie de ce mois.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContratAtteste {
    pub atteste_par: Attestation,
    pub numero_objet: Option<String>,
    pub heures: Option<i64>,
    pub cachets: Option<i64>,
    pub jours: Option<i64>,
    /// Les heures d'enseignement que les heures comptent (annexe 10).
    pub heures_enseignement: Option<i64>,
    pub brut: Option<Decimal>,
    /// La base conge qu'un certificat Congés Spectacles porte, pour le contrat entier.
    pub base_conge: Option<Decimal>,
    /// Le bulletin d'un permanent ne dit pas la nature de son contrat : elle se demande apres l'import (`import::nature`).
    pub nature_a_dire: bool,
    /// La nature dite, quand l'import relit un contrat dont elle a ete dite.
    pub nature: Option<String>,
    pub numero_bulletin: Option<String>,
    /// Le numero du contrat qu'un bulletin imprime (il se lit, il ne se tire pas : regle 4).
    pub numero_contrat: Option<String>,
    pub numero_attestation: Option<String>,
    pub numero_certificat: Option<String>,
    /// Le jour que la piece porte (sa signature), tel qu'il s'y lit.
    pub atteste_le: Option<String>,
    /// La copie de la piece, sous `historique/bulletins/`, et sa page.
    pub fichier: Option<String>,
    pub page: Option<u64>,
    pub importe_le: Option<String>,
}

/// Ce qui a declare une paie versee.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Declaration {
    #[serde(rename = "DSN")]
    Dsn,
    #[serde(rename = "bulletin")]
    Bulletin,
}

/// CE QUI A ETE VERSE UN MOIS : ce que la DSN deposee declare (bloc 50, ses bases du bloc 78), complete par le bulletin
/// de ce mois, qui fait foi sur ce qu'il imprime (`import::bulletins::completer`). `contrat` est ce qui la reconnait d'un
/// import a l'autre : le numero que la DSN declare (S21.G00.51.010), sinon le bulletin ou sa copie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaieVersee {
    pub contrat: String,
    pub declare_par: Declaration,
    pub nir: Option<String>,
    pub numero_objet: Option<String>,
    pub debut: Option<Jour>,
    pub fin: Option<Jour>,
    /// La quotite que la DSN deposee declare pour le contrat (S21.G00.40.013).
    pub quotite: Option<Decimal>,
    pub brut: Option<Decimal>,
    pub net_imposable: Option<Decimal>,
    /// Le net verse de la DSN (50.004), qui n'est pas le net a payer.
    pub net_verse: Option<Decimal>,
    pub net_a_payer: Option<Decimal>,
    pub net_avant_impot: Option<Decimal>,
    pub net_social: Option<Decimal>,
    pub cotisations_salariales: Option<Decimal>,
    pub cotisations_patronales: Option<Decimal>,
    pub cout_employeur: Option<Decimal>,
    pub pas: Option<Decimal>,
    pub taux_pas: Option<Decimal>,
    pub type_pas: Option<String>,
    pub identifiant_pas: Option<String>,
    pub verse_le: Option<Jour>,
    /// Le depot de la DSN qui la declare, et la copie de son fichier.
    pub depose_le: Option<String>,
    pub fichier: Option<String>,
    /// Les bases assujetties que la DSN declare, par code du bloc 78.
    pub bases: BTreeMap<String, Decimal>,
    /// Ce que le bulletin d'un mois atteste de ce mois, quand le contrat entier ne le garde pas.
    pub heures: Option<i64>,
    pub cachets: Option<i64>,
    pub jours: Option<i64>,
    pub spectacle: Option<String>,
    pub analytique: Option<String>,
    pub lignes: Vec<LigneImprimee>,
    pub dates: Vec<DatesTravaillees>,
    /// Les lignes de cotisation du bulletin ; `None` : aucun bulletin ne les a donnees.
    pub cotisations: Option<Vec<CotisationImprimee>>,
    pub emploi_imprime: Option<EmploiImprime>,
    pub bulletin: Option<BulletinSource>,
    pub importe_le: Option<String>,
}

/// Une ligne de remuneration que le bulletin imprime ; `rubrique` : celle du catalogue a laquelle l'import l'a rattachee.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LigneImprimee {
    pub quantite: Option<Decimal>,
    pub libelle: String,
    pub taux: Option<Decimal>,
    pub montant: Option<Decimal>,
    pub soumis: bool,
    pub rubrique: Option<String>,
    pub heures: Option<Decimal>,
}

impl LigneImprimee {
    /// La ligne de paie qu'elle dit a l'engagement importe, quand l'import l'a rattachee a une rubrique : sa quantite, son
    /// taux et ses heures (son libelle et son montant restent ceux du bulletin, `remuneration_lue`).
    pub fn en_ligne_de_paie(&self) -> Option<crate::LignePaie> {
        let rubrique = self.rubrique.clone().filter(|r| !r.trim().is_empty())?;
        Some(crate::LignePaie::Importee(crate::ligne::Importee { rubrique, quantite: self.quantite, taux: self.taux, heures: self.heures }))
    }
}

/// Des jours travailles que le bulletin imprime, de `debut` a `fin`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatesTravaillees {
    pub debut: Jour,
    pub fin: Jour,
}

/// Une ligne de cotisation que le bulletin imprime ; `cotisation` : celle du bareme que l'import lui a donnee.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CotisationImprimee {
    pub famille: Option<String>,
    pub libelle: String,
    pub cotisation: Option<String>,
    pub base: Option<Decimal>,
    pub taux_employeur: Option<Decimal>,
    pub montant_employeur: Option<Decimal>,
    pub taux_salarie: Option<Decimal>,
    pub montant_salarie: Option<Decimal>,
    pub hors_total: bool,
}

/// Le statut et l'emploi que CE bulletin imprime, en champs de la fiche (`import::bulletins::statut_et_metier`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmploiImprime {
    pub emploi: Option<String>,
    pub emploi_affichage: Option<String>,
    pub pcs_ese: Option<String>,
    pub code_emploi_ft: Option<String>,
    pub statut: Option<String>,
    pub statut_conventionnel: Option<String>,
    pub statut_categoriel_rc: Option<String>,
}

/// Le bulletin qui a complete la paie : sa copie, sa page, le logiciel qui l'a fait, son numero, le numero du contrat
/// qu'il imprime.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BulletinSource {
    pub fichier: String,
    pub page: u64,
    pub logiciel: Option<String>,
    pub numero: Option<String>,
    pub contrat: Option<String>,
}

/// UN CONTRAT ATTESTE, AVEC LES BORNES DE SON ENGAGEMENT : la ligne que l'import manipule, de la lecture de la piece a
/// l'engagement ecrit (`Neuf`). `salarie` est la cle de la fiche (`<a creer>` tant qu'elle n'existe pas) ; `numero`, le
/// numero de contrat de l'engagement qui le porte. Elle se rend a la page telle qu'elle s'ecrit (`Serialize`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ContratLu {
    pub salarie: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debut: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fin: Option<String>,
    #[serde(rename = "contrat", skip_serializing_if = "Option::is_none")]
    pub numero: Option<String>,
    #[serde(flatten)]
    pub contrat: ContratAtteste,
}

/// UNE PAIE VERSEE, AVEC SON MOIS ET LE SALARIE DE SON ENGAGEMENT : la ligne de paie que l'import manipule, de la
/// lecture de la DSN ou du bulletin a l'engagement ecrit.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PaieLue {
    pub mois: String,
    pub salarie: String,
    #[serde(flatten)]
    pub paie: PaieVersee,
}

impl PaieLue {
    /// Ce qui la reconnait d'un import a l'autre : (mois, salarie, contrat).
    pub fn cle(&self) -> (String, String, String) {
        (self.mois.clone(), self.salarie.clone(), self.paie.contrat.clone())
    }

    /// La meme, sans ce qui dit quand elle a ete importee : deux lectures des memes pieces se comparent ainsi.
    pub fn sans_date_d_import(&self) -> PaieLue {
        let mut p = self.clone();
        p.paie.importe_le = None;
        p
    }
}

impl PaieVersee {
    /// Une paie dont on ne sait encore que ce qui la reconnait et ce qui la declare.
    pub fn vide(contrat: String, declare_par: Declaration) -> PaieVersee {
        PaieVersee {
            contrat,
            declare_par,
            nir: None,
            numero_objet: None,
            debut: None,
            fin: None,
            quotite: None,
            brut: None,
            net_imposable: None,
            net_verse: None,
            net_a_payer: None,
            net_avant_impot: None,
            net_social: None,
            cotisations_salariales: None,
            cotisations_patronales: None,
            cout_employeur: None,
            pas: None,
            taux_pas: None,
            type_pas: None,
            identifiant_pas: None,
            verse_le: None,
            depose_le: None,
            fichier: None,
            bases: BTreeMap::new(),
            heures: None,
            cachets: None,
            jours: None,
            spectacle: None,
            analytique: None,
            lignes: Vec::new(),
            dates: Vec::new(),
            cotisations: None,
            emploi_imprime: None,
            bulletin: None,
            importe_le: None,
        }
    }
}

impl ContratAtteste {
    /// Un contrat dont on ne sait encore que ce qui l'atteste.
    pub fn vide(atteste_par: Attestation) -> ContratAtteste {
        ContratAtteste {
            atteste_par,
            numero_objet: None,
            heures: None,
            cachets: None,
            jours: None,
            heures_enseignement: None,
            brut: None,
            base_conge: None,
            nature_a_dire: false,
            nature: None,
            numero_bulletin: None,
            numero_contrat: None,
            numero_attestation: None,
            numero_certificat: None,
            atteste_le: None,
            fichier: None,
            page: None,
            importe_le: None,
        }
    }
}

// ------------------------------------------------------------------ l'engagement importe

/// Un engagement importe : l'engagement tel qu'il est ecrit, ses pieces avec lui.
#[derive(Debug, Clone)]
pub struct Importe {
    objet: Objet,
    /// Le contrat d'un permanent : paye au mois, ou de nature encore a dire ; lu une fois, avec sa nature.
    permanent: bool,
}

impl Importe {
    /// L'engagement `o` s'il est importe ; une nature dite que le moteur ne connait pas se refuse.
    pub fn de(o: Objet) -> Resultat<Option<Importe>> {
        if !o.est_importe() {
            return Ok(None);
        }
        let mut i = Importe { objet: o, permanent: true };
        if let Some(n) = i.nature() {
            i.permanent = droit::natures::nature(&n)?.mensuelle();
        }
        Ok(Some(i))
    }

    /// L'engagement tel qu'il est ecrit.
    pub fn objet(&self) -> &Objet {
        &self.objet
    }

    /// Ce que l'import a lu de lui.
    pub fn pieces(&self) -> &Pieces {
        self.objet.pieces().expect("un engagement importe a ses pieces (`de`)")
    }

    /// La carte de l'engagement lui-meme (son salarie, ses bornes, son numero, sa nature).
    fn carte(&self) -> &Carte {
        self.objet.carte()
    }

    /// Quand il a ete importe.
    pub fn le(&self) -> &str {
        &self.pieces().le
    }

    /// Ce que l'AEM, le bulletin ou le certificat atteste du contrat, s'il l'est.
    pub fn contrat(&self) -> Option<&ContratAtteste> {
        self.pieces().contrat.as_ref()
    }

    /// Ce qui a ete verse, mois par mois.
    pub fn paies(&self) -> &BTreeMap<String, PaieVersee> {
        &self.pieces().paies
    }

    /// La nature du contrat : celle qui a ete dite (l'engagement), sinon celle que la piece atteste ; une piece qui ne la
    /// porte pas et ne la laisse pas a dire est celle d'un contrat d'usage : son bulletin porte une marque du spectacle
    /// (numero d'objet ou Congés Spectacles, `moteur::import::bulletins` ; seul celui qui n'en a aucune la laisse a dire).
    /// `None` : elle reste a dire (`a_dire`).
    pub fn nature(&self) -> Option<String> {
        match self.nature_dite() {
            Some(n) => Some(n),
            None if self.contrat().is_some_and(|c| c.nature_a_dire) => None,
            None => Some(crate::carte::CDDU.to_string()),
        }
    }

    /// LA NATURE SOUS LAQUELLE CE QUI A ETE VERSE SE RANGE (`paie`) : la sienne (`nature`) ; tant qu'elle reste a dire,
    /// celle d'un contrat d'usage, sous laquelle l'import range la paie que l'ancien logiciel a versee : ses montants sont
    /// ceux des pieces, rien ne s'y calcule. Un contrat a dire ne se compare pas (`ecarts::comparer`) et attend sa nature
    /// (`import::nature`).
    pub fn nature_rangee(&self) -> String {
        match self.nature() {
            Some(n) => n,
            None => crate::carte::CDDU.to_string(),
        }
    }

    /// La nature dite (l'engagement) ou attestee (la piece) ; `None` : ni l'un ni l'autre ne la porte.
    fn nature_dite(&self) -> Option<String> {
        self.carte().nature.clone().filter(|n| !n.is_empty()).or_else(|| self.contrat().and_then(|c| c.nature.clone()))
    }

    /// Sa nature reste-t-elle a dire ? Le bulletin d'un permanent ne la porte pas (`nature_a_dire`), et personne ne l'a
    /// dite.
    pub fn a_dire(&self) -> bool {
        self.nature().is_none()
    }

    /// Le contrat d'un permanent : paye au mois, ou de nature encore a dire.
    pub fn permanent(&self) -> bool {
        self.permanent
    }

    /// LE CONTRAT D'USAGE QUI CONTINUE ICI : le dernier mois verse par l'ancien logiciel, quand le contrat que la DSN
    /// deposee declare finit apres lui (S21.G00.40.010, sa fin previsionnelle : `norme-dsn/texte/bloc-S21.G00.40.txt`).
    /// Les mois qui suivent se paient ici, sous le numero et les dates que cette DSN lui a donnes (regle 4). Un contrat
    /// qu'une AEM atteste est fini (l'AEM se remet a la fin du contrat) ; un contrat paye au mois a sa propre reprise,
    /// sa nature dite (`import::nature`) ; sans numero, rien ne se reprend.
    pub fn continue_apres(&self) -> Option<String> {
        let dernier = self.paies().keys().max()?.clone();
        let apres = self.carte().fin.as_deref().and_then(|f| f.get(..7)).is_some_and(|f| f > dernier.as_str());
        let par_aem = self.contrat().is_some_and(|c| c.atteste_par == Attestation::Aem);
        let continue_ = apres && !par_aem && !self.permanent() && self.carte().numero.as_deref().is_some_and(|n| !n.is_empty());
        continue_.then_some(dernier)
    }

    /// Les mois qu'il touche : de son debut a sa fin (au dernier mois verse, s'il continue ici : `continue_apres`), et
    /// ceux ou il a ete verse. Le contrat d'un permanent ne touche que les mois ou il a ete verse : des mois d'avant le
    /// premier bulletin repris, les pieces ne disent rien, et ceux d'apres le dernier sont ceux d'ici.
    pub fn mois(&self) -> Vec<String> {
        let mut v: Vec<String> = self.paies().keys().cloned().collect();
        if let (Some(debut), false) = (self.carte().debut.as_deref().filter(|d| d.len() >= 7), self.permanent()) {
            let fin = match (self.continue_apres(), self.carte().fin.as_deref().filter(|f| f.len() >= 7)) {
                (Some(dernier), _) => format!("{}-01", dernier),
                (None, Some(fin)) => fin.to_string(),
                (None, None) if !v.is_empty() => {
                    v.sort();
                    return v;
                }
                (None, None) => debut.to_string(),
            };
            let mut m = debut[..7].to_string();
            while m.as_str() <= &fin[..7] {
                v.push(m.clone());
                match socle::mois::Mois::du_texte(&m) {
                    Some(x) => m = x.suivant().to_string(),
                    None => break,
                }
            }
        }
        v.sort();
        v.dedup();
        v
    }

    /// Le contrat atteste, avec les bornes de l'engagement. La nature dite est celle du contrat : il n'attend plus rien.
    pub fn contrat_lu(&self) -> Resultat<Option<ContratLu>> {
        let Some(contrat) = self.contrat() else { return Ok(None) };
        let mut contrat = contrat.clone();
        if let Some(n) = self.nature_dite() {
            contrat.nature_a_dire = false;
            contrat.nature = Some(n);
        }
        contrat.importe_le = contrat.importe_le.or_else(|| Some(self.le().to_string()).filter(|l| !l.is_empty()));
        let numero = self.carte().numero.clone().filter(|n| !n.is_empty());
        Ok(Some(ContratLu { salarie: self.cle_ecrite()?, debut: self.carte().debut.clone(), fin: self.carte().fin.clone(), numero, contrat }))
    }

    /// Les paies versees, une par mois, avec le salarie de l'engagement.
    pub fn paies_lues(&self) -> Resultat<Vec<PaieLue>> {
        self.paies().iter().map(|(m, p)| self.paie_lue(m, p)).collect()
    }

    fn paie_lue(&self, mois: &str, p: &PaieVersee) -> Resultat<PaieLue> {
        let mut paie = p.clone();
        paie.importe_le = paie.importe_le.or_else(|| Some(self.le().to_string()).filter(|l| !l.is_empty()));
        Ok(PaieLue { mois: mois.to_string(), salarie: self.cle_ecrite()?, paie })
    }

    /// La cle de la fiche de son salarie, ecrite ; un engagement importe qui ne la dit pas se refuse.
    fn cle_ecrite(&self) -> Resultat<String> {
        let o = &self.objet;
        o.carte().salarie.as_ref().map(ToString::to_string).ok_or_else(|| format!("l'engagement importé {} (rang {}) ne dit pas son salarié", o.fichier, o.rang))
    }
}

/// UN MONTANT QUE LES PIECES DISENT, ou zero (au centime) quand elles le taisent : une paie importee le nomme parmi ses
/// `non_dits` (`Importe::paie`), et un etat le laisse vide.
fn tu_vaut_zero(x: Option<Decimal>) -> Decimal {
    x.into_iter().fold(Decimal::new(0, 2), |_, m| m)
}

/// Les montants d'une paie importee, tels que ses pieces les nomment, et la grandeur de la paie qu'ils sont.
fn montants(p: &PaieVersee) -> [(&'static str, Option<Decimal>); 7] {
    [
        ("brut", p.brut),
        ("cotisations_salariales", p.cotisations_salariales),
        ("cotisations_patronales", p.cotisations_patronales),
        ("net_imposable", p.net_imposable),
        ("net_social", p.net_social),
        ("cout_employeur", p.cout_employeur),
        ("pas", p.pas),
    ]
}

impl Importe {
    /// LA PAIE DE `mois`, DANS LES MOTS DU MOTEUR : ce que les pieces de l'ancien logiciel disent de ce qui a ete verse,
    /// range comme le calcul range le sien (`calcul::Paie`), et rien de calcule. L'engagement porte l'identite importee
    /// (salarie, bornes, numero, nature : `nature_rangee`), le spectacle dont la fiche a son numero d'objet,
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
        let p = self.paies().get(mois).ok_or_else(|| format!("{} (rang {}) : aucune paie importee en {}", self.objet.fichier, self.objet.rang, mois))?;
        let contrat = self.contrat();
        let zero = Decimal::new(0, 2);
        let mut non_dits: Vec<&'static str> = montants(p).iter().filter(|(_, m)| m.is_none()).map(|(k, _)| *k).collect();

        let nature = self.nature_rangee();
        let nat = droit::natures::nature(&nature)?;
        let ctx = ctx.du_mois(mois)?;
        let cle = self.carte().salarie.as_ref().ok_or_else(|| socle::fichiers::absent("salarie"))?;
        let sal = crate::calcul::convention_de_l_employeur(&ctx, crate::fiche_du_salarie(&ctx, cle)?);
        let objet: Option<String> = p.numero_objet.clone().or_else(|| contrat.and_then(|c| c.numero_objet.clone())).filter(|o| !o.is_empty()).map(|o| o.to_uppercase());
        let spectacle = ctx.spectacles.iter().find(|(_, f)| objet.is_some() && f.numero_objet.portee().map(str::to_uppercase) == objet).map(|(k, _)| k.clone());
        let spec = match &spectacle {
            Some(k) => Some(crate::fiche_du_spectacle(&ctx, k)?),
            None => None,
        };
        // L'emploi que le bulletin imprime ; un bulletin qui ne l'imprime pas n'en dit rien.
        let imprime = |champ: fn(&EmploiImprime) -> &Option<String>| p.emploi_imprime.as_ref().and_then(|i| champ(i).clone());
        let debut = match (self.carte().debut.as_deref(), p.debut) {
            (Some(d), _) => Jour::lire(d)?,
            (None, Some(j)) => j,
            (None, None) => return Err(format!("{} (rang {}) : ni l'engagement ni la paie de {} ne disent le début du contrat", self.objet.fichier, self.objet.rang, mois)),
        };
        // Le contrat d'un permanent finit a sa fin, dite ou declaree, jamais a celle du mois que paie un bulletin.
        let fin = match (self.carte().fin.as_deref().and_then(Jour::du_texte), self.permanent()) {
            (Some(f), _) => Some(f),
            (None, true) => None,
            (None, false) => p.fin,
        };
        let mut dates_travail = Vec::new();
        for d in &p.dates {
            let mut j = d.debut;
            while j <= d.fin {
                dates_travail.push(j);
                j = j.plus_jours(1)?;
            }
        }
        let brut = tu_vaut_zero(p.brut);
        // LES LIGNES DE REMUNERATION QUE LES PIECES DISENT : celles du bulletin ; a defaut, le brut que la DSN declare.
        let mut lignes_lues = p.lignes.clone();
        if lignes_lues.is_empty() {
            lignes_lues.push(LigneImprimee { libelle: "brut déclaré".into(), montant: Some(brut), soumis: true, ..Default::default() });
        }
        let quotite = p.quotite.map(socle::num::flottant);
        let mut engagement = crate::Engagement {
            salarie: cle.clone(),
            nom: crate::qui_de(&sal)?,
            spectacle: spectacle.clone(),
            dates_travail,
            lignes: lignes_lues.iter().filter_map(LigneImprimee::en_ligne_de_paie).collect(),
            debut,
            fin,
            numero: match NumeroContrat::du_texte(self.carte().numero.as_deref())? {
                Some(n) => Some(n),
                None => p.bulletin.as_ref().and_then(|b| b.contrat.as_deref()).filter(|n| !n.trim().is_empty()).map(NumeroContrat::lire).transpose()?,
            },
            mois: mois.to_string(),
            planning: Vec::new(),
            planning_lieux: BTreeMap::new(),
            jours: 0,
            jours_contrat: 0,
            date_paiement: p.verse_le.map(|j| j.iso()),
            versement: Default::default(),
            taux_cachet: None,
            cachets: 0,
            heures: None,
            lieu: String::new(),
            lieux: BTreeMap::new(),
            emploi: imprime(|i| &i.emploi),
            emploi_affichage: imprime(|i| &i.emploi_affichage),
            pcs_ese: imprime(|i| &i.pcs_ese),
            code_emploi_ft: imprime(|i| &i.code_emploi_ft),
            statut: imprime(|i| &i.statut),
            statut_conventionnel: imprime(|i| &i.statut_conventionnel),
            statut_categoriel_rc: imprime(|i| &i.statut_categoriel_rc),
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
            ecrit: crate::ContratEcrit::default(),
        };
        let remuneration = remuneration_lue(&engagement, &lignes_lues, &ctx)?;
        if lignes_lues.iter().all(|l| l.rubrique.as_deref().is_none_or(str::is_empty)) {
            non_dits.extend(["cachets", "heures"]);
        }
        let cachets = remuneration.iter().filter(|r| r.unite == "cachet").map(|r| r.quantite).sum::<Decimal>().trunc().to_string();
        engagement.cachets = cachets.parse().map_err(|_| socle::num::illisible(&cachets))?;
        let heures: Decimal = remuneration.iter().map(|r| r.heures_annexes).sum();
        engagement.heures = (!heures.is_zero()).then(|| crate::conges::flot(heures));
        engagement.jours = engagement.dates_travail.len() as i64;
        engagement.jours_contrat = engagement.jours;

        // Les lignes de cotisation du bulletin, sous leur cotisation ; une ligne que l'import n'a pas qualifiee garde
        // son libelle, sans cotisation.
        let taux = |x: Option<Decimal>| x.map(socle::num::flottant);
        let cotisations: Vec<LigneCotisation> = p
            .cotisations
            .iter()
            .flatten()
            .map(|l| LigneCotisation {
                id: l.cotisation.clone().filter(|c| !c.is_empty()),
                famille: l.famille.clone().filter(|f| !f.is_empty()),
                libelle: l.libelle.clone(),
                base: tu_vaut_zero(l.base),
                taux_pat: taux(l.taux_employeur),
                montant_pat: tu_vaut_zero(l.montant_employeur),
                taux_sal: taux(l.taux_salarie),
                montant_sal: tu_vaut_zero(l.montant_salarie),
                masquer_base_et_taux: false,
            })
            .collect();

        // Les bases, ou le bareme de la paie les range ; ses taux se lisent sur les pieces (`calcul::bareme_pour_ranger`).
        let mut bases: BTreeMap<String, Decimal> = BTreeMap::new();
        bases.insert("brut".into(), brut);
        let fin_periode = engagement.fin_periode()?;
        let caisse = droit::caisse_conges::affilie(&nat, &sal, &engagement.debut, engagement.fin.as_ref())?;
        let bar = crate::calcul::bareme_pour_ranger(&ctx, &sal, &nat, &fin_periode, caisse, droit::majoration_chomage::due(&nat, engagement.motif_recours.as_deref(), &engagement.debut, engagement.fin.as_ref()))?;
        for (l, id) in cotisations.iter().filter_map(|l| l.id.as_deref().map(|id| (l, id))).filter(|(l, _)| !l.base.is_zero()) {
            if let Some(cle) = bar.rubrique(id).and_then(|r| r.base.clone()) {
                bases.entry(cle).or_insert(l.base);
            }
        }
        for (code, m) in &p.bases {
            let mut cles: Vec<String> = bar.cotisations_dsn.iter().filter(|c| c.bloc78 == *code).map(|c| c.base78.clone()).filter(|k| k != "zero").collect();
            cles.sort();
            cles.dedup();
            if let [cle] = &cles[..] {
                bases.entry(cle.clone()).or_insert(*m);
            }
        }
        let base_csg = tu_vaut_zero(bases.get(crate::calcul::CSG).copied());
        let plafond = ["brut_plafonne", "t1_mensuelle"].iter().find_map(|k| bases.get(*k)).filter(|t| **t < brut).copied();
        if plafond.is_none() {
            non_dits.push("plafond");
        }
        // Le net avant impot : celui que le bulletin imprime, sinon le net paye ou le net verse plus l'impot preleve.
        let net_a_payer = p.net_avant_impot.or_else(|| Some(p.net_a_payer.or(p.net_verse)? + p.pas?));
        if net_a_payer.is_none() {
            non_dits.push("net_a_payer");
        }

        let tp = PlafondTempsPartiel::NonConcerne;
        let mut paie = Paie::nouveau(engagement, sal, spec, brut, base_csg, tu_vaut_zero(plafond), 0.0, zero, tp);
        let c = paie.ecrire();
        c.lignes = cotisations;
        c.remuneration = remuneration;
        c.total_sal = tu_vaut_zero(p.cotisations_salariales);
        c.total_pat = tu_vaut_zero(p.cotisations_patronales);
        c.net_a_payer = tu_vaut_zero(net_a_payer);
        c.net_social = tu_vaut_zero(p.net_social);
        c.net_imposable = tu_vaut_zero(p.net_imposable);
        c.cout_employeur = tu_vaut_zero(p.cout_employeur);
        c.numero_bulletin = p.bulletin.as_ref().and_then(|b| b.numero.clone()).filter(|n| !n.is_empty());
        c.bases = bases;
        c.pas = p.pas.map(|prelevement| droit::pas::Versement {
            taux: p.taux_pas.map(socle::num::flottant).map(droit::fiche::Montant::Flottant),
            type_taux: p.type_pas.clone(),
            identifiant: p.identifiant_pas.clone(),
            abattement: zero,
            montant_soumis: tu_vaut_zero(p.net_imposable),
            prelevement,
        });
        c.importee = Some(Importee { le: self.le().to_string(), non_dits });
        Ok(paie)
    }
}

/// LES PAIES IMPORTEES DE `mois`, dans les mots du moteur (`Importe::paie`), une par engagement importe paye ce mois.
pub fn paies_du_mois(ctx: &Contexte, mois: &str) -> Resultat<Vec<Paie>> {
    du_mois(&ctx.racine, mois)?.iter().filter(|i| i.paies().contains_key(mois)).map(|i| i.paie(ctx, mois)).collect()
}

/// Tous les engagements importes, fichier par fichier.
pub fn tous(racine: &Path) -> Resultat<Vec<Importe>> {
    Ok(stock::objets(racine)?.into_iter().map(Importe::de).collect::<Resultat<Vec<_>>>()?.into_iter().flatten().collect())
}

/// Les engagements importes qui touchent `mois`.
pub fn du_mois(racine: &Path, mois: &str) -> Resultat<Vec<Importe>> {
    Ok(tous(racine)?.into_iter().filter(|i| i.mois().iter().any(|m| m == mois)).collect())
}

/// Les contrats attestes de tous les engagements importes (`Importe::contrat_lu`).
pub fn contrats_lus(racine: &Path) -> Resultat<Vec<ContratLu>> {
    Ok(tous(racine)?.iter().map(Importe::contrat_lu).collect::<Resultat<Vec<_>>>()?.into_iter().flatten().collect())
}

/// Les paies versees de tous les engagements importes, une par mois (`Importe::paies_lues`).
pub fn paies_lues(racine: &Path) -> Resultat<Vec<PaieLue>> {
    Ok(tous(racine)?.iter().map(Importe::paies_lues).collect::<Resultat<Vec<_>>>()?.into_iter().flatten().collect())
}

/// LA DSN DE `mois` QUE L'ANCIEN LOGICIEL A DEPOSEE : une paie importee de ce mois qu'une DSN deposee declare
/// (`declare_par: "DSN"` : son `depose_le`, son `fichier`), qu'une saisie touche aussi le mois ou non. Cette DSN n'est
/// pas faite ici et ne se refait pas ici (regle 4) : ce que la saisie ajoute a ce mois se declare dans une DSN suivante,
/// comme un contrat ajoute apres la DSN de son mois.
pub fn dsn_deposee(racine: &Path, mois: &str) -> Resultat<Option<PaieLue>> {
    let trouve = tous(racine)?.iter().find_map(|i| i.paies().get(mois).filter(|p| p.declare_par == Declaration::Dsn).map(|p| i.paie_lue(mois, p)));
    trouve.transpose()
}

/// L'engagement importe du salarie `cle` (la cle de sa fiche) qui commence le `debut` et touche `mois`, s'il y en a un.
pub fn celui_de(racine: &Path, mois: &str, cle: &str, debut: &str) -> Resultat<Option<Importe>> {
    Ok(du_mois(racine, mois)?.into_iter().find(|i| i.carte().salarie.as_deref() == Some(cle) && i.carte().debut.as_deref() == Some(debut)))
}

/// Les contrats d'usage importes qui continuent en `mois` (`Importe::continue_apres`) : verses avant lui par l'ancien
/// logiciel, ils finissent en `mois` ou apres.
pub fn qui_continuent(racine: &Path, mois: &str) -> Resultat<Vec<Importe>> {
    Ok(tous(racine)?
        .into_iter()
        .filter(|i| i.continue_apres().is_some_and(|d| d.as_str() < mois) && i.carte().fin.as_deref().and_then(|f| f.get(..7)).is_some_and(|f| f >= mois))
        .collect())
}

/// Le contrat importe que l'engagement `e` de la saisie continue : meme salarie, meme debut, meme numero, et il continue
/// apres son dernier mois verse (`continue_apres`).
pub fn continue_par(racine: &Path, e: &Carte) -> Resultat<Option<Importe>> {
    let (Some(debut), Some(numero), Some(salarie)) = (e.debut.as_deref().filter(|d| !d.is_empty()), e.numero.as_deref().filter(|n| !n.is_empty()), &e.salarie) else { return Ok(None) };
    Ok(tous(racine)?.into_iter().find(|i| {
        i.continue_apres().is_some() && i.carte().salarie.as_ref() == Some(salarie) && i.carte().debut.as_deref() == Some(debut) && i.carte().numero.as_deref() == Some(numero)
    }))
}

/// LE CONTRAT `numero` DE `salarie` COMMENCE LE `debut` VIENT-IL DE L'ANCIEN LOGICIEL ? Un engagement importe porte ces
/// salarie, debut et numero : c'est lui, ou le contrat d'usage qu'il continue ici (`continue_par`). L'ecran le dit sur
/// la carte. Faux sans numero.
pub fn vient_d_un_ancien_logiciel(racine: &Path, salarie: &str, debut: &str, numero: Option<&str>) -> Resultat<bool> {
    let Some(numero) = numero.filter(|n| !n.is_empty()) else { return Ok(false) };
    Ok(tous(racine)?.iter().any(|i| {
        i.carte().salarie.as_deref() == Some(salarie) && i.carte().debut.as_deref() == Some(debut) && i.carte().numero.as_deref() == Some(numero)
    }))
}

/// LE NUMERO QU'UNE DSN DE L'ANCIEN LOGICIEL A DECLARE pour l'engagement `o` : celui de sa derniere paie importee
/// qu'une DSN deposee declare (`declare_par: "DSN"`, S21.G00.40.009). Cette DSN est partie : le numero est lu, il ne se
/// dit plus et ne change plus (`numeros::a_la_validation`). `None` pour un engagement d'ici, ou repris des seuls bulletins.
/// La norme l'a deja contraint : il se lit comme tout numero (`NumeroContrat::lire`).
pub fn numero_declare(o: &Objet) -> Resultat<Option<NumeroContrat>> {
    let Some(pieces) = o.pieces() else { return Ok(None) };
    let declare = pieces.paies.values().rev().filter(|p| p.declare_par == Declaration::Dsn).map(|p| p.contrat.as_str()).find(|n| !n.trim().is_empty());
    declare.map(NumeroContrat::lire).transpose()
}

/// Une DSN de l'ancien logiciel a-t-elle declare le numero du contrat de `salarie` commence le `debut`
/// (`numero_declare`, lu sur l'engagement repris qui le porte) ?
pub fn numero_declare_du_contrat(racine: &Path, salarie: &str, debut: &str) -> Resultat<bool> {
    for i in tous(racine)?.iter().filter(|i| i.carte().salarie.as_deref() == Some(salarie) && i.carte().debut.as_deref() == Some(debut)) {
        if numero_declare(&i.objet)?.is_some() {
            return Ok(true);
        }
    }
    Ok(false)
}

impl Importe {
    /// LES JOURS DU CONTRAT ENTIER (S21.G00.40.013 d'un contrat d'usage : `Engagement::quotite`) d'un contrat qui continue
    /// ici, `propres` etant ceux que la saisie lui donne : les jours que les bulletins importes impriment, plus les siens ;
    /// a defaut d'un bulletin pour chaque mois verse, la quotite que la DSN deposee a declaree pour ce contrat (elle le
    /// dit pour le contrat entier, `norme-dsn/texte/bloc-S21.G00.40.txt`). `None` quand aucune piece ne les donne.
    pub fn jours_du_contrat_continue(&self, propres: usize) -> Resultat<Option<i64>> {
        let jours = |p: &PaieVersee| -> Option<i64> {
            let n: i64 = p.dates.iter().map(|d| d.fin.moins(&d.debut) + 1).sum();
            (n > 0).then_some(n)
        };
        let lus: Option<Vec<i64>> = self.paies().values().map(jours).collect();
        if let Some(l) = lus.filter(|l| !l.is_empty()) {
            return Ok(Some(l.iter().sum::<i64>() + propres as i64));
        }
        let Some(dernier) = self.paies().values().last() else { return Ok(None) };
        match dernier.quotite.filter(|q| *q > Decimal::ZERO && q.fract().is_zero()) {
            Some(q) => i64::try_from(q).map(Some).map_err(|_| format!("quotité déclarée hors de portée : {}", q)),
            None => Ok(None),
        }
    }
}

/// Le refus d'un geste sur un engagement importe : il se lit, il ne se modifie pas.
pub fn refus(qui: &str, debut: &str) -> String {
    let jour = match socle::date::Jour::du_texte(debut) {
        Some(j) => j.jj_mm_aaaa(),
        None => debut.to_string(),
    };
    format!(
        "L'engagement de {} du {} est importé : il se lit tel que l'ancien logiciel l'a déclaré, il ne se modifie pas, ne se \
         recalcule pas et ne se supprime pas. Rien n'est écrit.",
        qui, jour
    )
}

// ------------------------------------------------------------------ l'ecriture

/// UN ENGAGEMENT IMPORTE A ECRIRE : l'engagement lui-meme (son salarie, ses bornes, son numero, ce qui a ete dit de lui :
/// la carte de son premier mois), et ses pieces.
#[derive(Debug, Clone)]
pub struct Neuf {
    pub carte: Carte,
    pub pieces: Pieces,
}

/// LE CONTRAT DE PERMANENT QUI CONTINUE ICI : sa nature est dite et payee au mois, il a ete verse le dernier mois repris
/// (`dernier`), et il ne finit pas avant le mois qui suit.
fn continue_ici(e: &Neuf, dernier: &str) -> Resultat<bool> {
    let mensuel = e.carte.nature.as_deref().filter(|n| !n.is_empty()).map(|n| droit::natures::nature(n).map(|x| x.mensuelle())).transpose()?.is_some_and(|m| m);
    let suite = socle::mois::Mois::lire(dernier).map(|m| m.suivant().to_string())?;
    let premier_jour_suivant = format!("{}-01", suite);
    Ok(mensuel && e.pieces.paies.contains_key(dernier) && e.carte.fin.as_deref().is_none_or(|f| f.is_empty() || f >= premier_jour_suivant.as_str()))
}

/// ECRIT LES ENGAGEMENTS IMPORTES, tous : chacun dans le fichier de son mois d'entree -- le mois ou il commence (son
/// `debut`, sinon le premier mois ou il a ete verse), ou, pour le contrat de permanent qui continue ici (`continue_ici`),
/// le mois qui suit le dernier mois repris : son premier mois neuf. Ils vont apres les engagements de la saisie. Ceux qui
/// y etaient s'en vont ; rien d'autre ne change. LE MEME CONTRAT DEJA ECRIT (meme salarie, un mois verse en commun) garde
/// ce qui a ete dit de lui et ce que la saisie lui a ecrit ; seules ses pieces se remplacent. Rend les fichiers ecrits.
pub fn ecrire_tous(racine: &Path, importes: Vec<Neuf>) -> Resultat<Vec<PathBuf>> {
    let existants = tous(racine)?;
    let dernier = importes.iter().flat_map(|e| e.pieces.paies.keys().cloned()).max();
    let mut repris = vec![false; existants.len()];
    let mut par_mois: BTreeMap<String, Vec<stock::Ecrit>> = BTreeMap::new();
    for mut e in importes {
        let salarie = &e.carte.salarie;
        let meme = (0..existants.len()).find(|&k| {
            let x = &existants[k];
            !repris[k] && x.carte().salarie == *salarie && x.paies().keys().any(|m| e.pieces.paies.contains_key(m))
        });
        let mut ecrit = stock::Ecrit { saisie: stock::Saisie { carte: e.carte.clone(), ..Default::default() }, ..Default::default() };
        if let Some(k) = meme {
            repris[k] = true;
            let x = &existants[k].objet.brut;
            if e.carte.nature.as_deref().is_none_or(str::is_empty) && x.saisie.carte.nature.as_deref().is_some_and(|n| !n.is_empty()) {
                ecrit = (**x).clone();
                e.carte = ecrit.saisie.carte.clone();
            }
        }
        let mois = match (&dernier, e.carte.debut.as_deref().and_then(|d| d.get(..7))) {
            (Some(d), _) if continue_ici(&e, d)? => socle::mois::Mois::lire(d).map(|m| m.suivant().to_string())?,
            (_, Some(premier)) => premier.to_string(),
            _ => e.pieces.paies.keys().next().cloned().ok_or("un engagement importe sans debut ni paie")?,
        };
        ecrit.importe = Some(e.pieces);
        par_mois.entry(mois).or_default().push(ecrit);
    }
    let mut touches: Vec<String> = existants.iter().map(|i| i.objet.fichier.clone()).collect();
    touches.extend(par_mois.keys().cloned());
    touches.sort();
    touches.dedup();
    let mut ecrits = Vec::new();
    for f in touches {
        let existe = stock::lire_fichier(racine, &f)?;
        // Un mois que l'import ne touche que pour en retirer n'a rien de neuf ; un fichier qui n'existe pas se cree.
        let neufs: Vec<stock::Ecrit> = par_mois.remove(&f).into_iter().flatten().collect();
        let mut contenu = match existe.clone() {
            Some(c) => c,
            None => stock::FichierDuMois { mois: socle::mois::Mois::lire(&f)?, taux_cachet: None, engagements: Vec::new() },
        };
        contenu.engagements.retain(|e| e.importe.is_none());
        contenu.engagements.extend(neufs);
        if existe.as_ref() == Some(&contenu) {
            continue;
        }
        if contenu.engagements.is_empty() && contenu.taux_cachet.is_none() {
            if existe.is_some() {
                ecrits.push(stock::retirer(racine, &f)?);
            }
            continue;
        }
        ecrits.push(stock::poser(racine, &contenu)?);
    }
    Ok(ecrits)
}

/// CE QUE L'IMPORT COMPLETE DE LUI-MEME : ce que les pieces ne disaient pas et que la personne dit apres l'import (la
/// nature d'un contrat de permanent, son numero). `completer` recoit chaque engagement importe (sa carte, a changer, et
/// ses pieces) et dit s'il l'a changee ; ceux-la se reecrivent. Rend combien ont change.
pub fn completer(racine: &Path, mut completer: impl FnMut(&mut Carte, &Pieces) -> bool) -> Resultat<usize> {
    let mut tous: Vec<Neuf> = tous(racine)?.into_iter().map(|i| Neuf { carte: i.carte().clone(), pieces: i.pieces().clone() }).collect();
    let mut changes = 0usize;
    for e in tous.iter_mut() {
        if completer(&mut e.carte, &e.pieces) {
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
fn remuneration_lue(e: &crate::Engagement, lignes: &[LigneImprimee], ctx: &Contexte) -> Resultat<Vec<crate::calcul::LigneBrut>> {
    let mut out = Vec::new();
    for l in lignes {
        let montant = tu_vaut_zero(l.montant);
        let rattachee = l.en_ligne_de_paie().filter(|_| l.quantite.is_some() && l.taux.is_some());
        if let Some(ligne) = rattachee {
            let mut seule = e.clone();
            seule.lignes = vec![ligne];
            for mut b in crate::calcul::lignes_brut(&seule, ctx)? {
                b.montant = montant;
                out.push(b);
            }
            continue;
        }
        out.push(crate::calcul::LigneBrut {
            code: String::new(),
            libelle: l.libelle.clone(),
            unite: String::new(),
            quantite: Decimal::ZERO,
            taux: Decimal::ZERO,
            montant,
            heures_annexes: Decimal::ZERO,
            soumis: l.soumis,
            imposable: l.soumis,
            conges_spectacles: l.soumis,
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
