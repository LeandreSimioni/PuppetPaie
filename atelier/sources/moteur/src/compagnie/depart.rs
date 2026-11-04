//! CE QUI PART SE FABRIQUE A SON DEPART, depuis l'engagement valide (`engagement::etats::validees_du_mois`) :
//!
//! - la DSN du mois, quand on la depose (`fabriquer_dsn`, que la route `depot-apercu` appelle) : des qu'un engagement du mois est
//!   valide, sans attendre ni DPAE ni contrat. Le moteur dit si c'est un depot initial ou un annule-et-remplace ; apres la
//!   veille de l'echeance, un mois deja declare ne se redepose plus (`AnnuleRemplaceHorsDelai`, cahier technique DSN
//!   §1.4.2) : sa correction part dans la DSN d'un mois suivant (`produire_regularisations::regularisations_pour`) ;
//! - le bulletin et le certificat Conges Spectacles, quand les documents partent au salarie (`fabriquer_documents`, la
//!   route `envoyer-documents`). Le numero du bulletin (`N/AAAA`, suite de l'annee) et le premier N CE du certificat
//!   s'ecrivent alors dans la paie de l'engagement (`DocumentsFabriques::engager`) et se relisent ensuite. Une paie partie
//!   puis refaite part sous un numero nouveau, comme un bulletin rectificatif de celui qui est parti, et l'original reste
//!   a cote ; quand la DSN du mois ne se refait plus, le rectificatif porte l'ecart avec le bulletin parti. Chaque paie a un
//!   seul bulletin et un seul paiement : la DSN ne fabrique aucun bulletin.
//!
//! L'AER ne se fabrique pas : France Travail la renvoie apres la DSN, et elle part avec les documents quand elle est la.
//! Un numero de contrat s'ecrit dans l'engagement a sa validation (`engagement::numeros`) ; tout ce qui part le lit la.

use socle::mois::Mois;
use crate::routes::dossier_salarie::cle_de;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use documents::conges_spectacles::{self, PaieCs};
use declarations::produire_regularisations::{self as regularisations, Reg};
use crate::routes::etapes::Moteur;
use droit::contexte::Contexte;
use engagement::calcul::Paie;
use engagement::Avis;
use socle::echec::erreur;
use socle::echec::Echec;
use socle::echec::R;
use socle::ecriture::ecrire_json;
use socle::fichiers::vrai;
use documents::ecrire_documents::Reserves;
use socle::valeur::valeur_en_texte;

fn info(texte: impl Into<String>) -> Avis {
    Avis { info: true, texte: texte.into() }
}

fn avertissement(texte: impl Into<String>) -> Avis {
    Avis { info: false, texte: texte.into() }
}

fn os(chemin: &Path, e: std::io::Error) -> Echec {
    erreur("OSError", socle::echec::du_disque(chemin, &e))
}

fn valeur(m: String) -> Echec {
    erreur("ValueError", m)
}

fn prenom_de(sal: &droit::fiche::ChampsSalarie) -> String {
    match sal.prenom_affichage.texte() {
        "" => sal.prenom.texte().to_string(),
        p => p.to_string(),
    }
}

fn annee_de(mois: &str) -> R<i64> {
    mois.get(..4).and_then(|a| a.parse().ok()).ok_or_else(|| valeur(format!("nombre entier illisible : {}", socle::fichiers::repr(mois))))
}

// ------------------------------------------------------------- le journal des pieces ecrites

/// Le journal des pieces ecrites, sous la racine de la compagnie.
const DSN_ECRITES: &str = "journal/dsn-ecrites.json";

/// LE JOURNAL DES PIECES ECRITES (`journal/dsn-ecrites.json`) : chaque piece que le moteur ecrit -- une DSN, une
/// DPAE, un export compta -- y note QUAND, au temps du moteur (`temps.rs`, par `socle::paris::maintenant`). C'est ce que
/// l'ecran dit (« ecrit le ... ») : la date d'un fichier n'est pas celle de son ecriture (une fictive livree est
/// decompressee chez le visiteur, un banc simule le temps). `cle` : `<mois>/<fichier>`, ou `exercices/<fichier>` ;
/// `empreinte_saisie` : la paie que la piece declare -- pour une DSN, celle de ses paies validees ; pour l'export compta
/// de l'annee, celles de ses saisies (`exercice_ecrit::empreintes_des_saisies`) ; `versions_portees` : pour une DSN
/// mensuelle, la version de chaque engagement d'un mois passe dont elle porte la correction
/// (`produire_regularisations::versions_portees`), que la ligne du journal de son depot emporte (`versions_portees_de`).
pub(crate) fn noter_piece_ecrite(racine: &Path, cle: &str, empreinte_saisie: Option<Value>, versions_portees: Option<Vec<Value>>) -> R<()> {
    let p = racine.join(DSN_ECRITES);
    let mut d = Value::Object(journal_des_ecritures(racine));
    let mut note = json!({"quand": socle::paris::maintenant()});
    if let Some(e) = empreinte_saisie {
        note["empreinte_saisie"] = e;
    }
    if let Some(v) = versions_portees {
        note["versions_portees"] = Value::Array(v);
    }
    d[cle] = note;
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| os(parent, e))?;
    }
    ecrire_json(&p, &d, 2).map_err(|m| erreur("OSError", m))
}

/// UNE PIECE PARTIE NE SE RECRIT JAMAIS (regle 4 de `atelier/AGENTS.md` : ce qui est depose ne se regenere pas) : le
/// fichier `cible` dont le contenu est celui d'un depot reel accepte, qu'aucun retour n'a refuse (`journal/depots.jsonl`,
/// par son empreinte), ne s'ecrit pas par-dessus ; `Some(le jour de son depot)`. Chaque ecriture d'une declaration sous
/// un nom qui ne depend pas de son contenu (un signalement, la DPAE du mois, la DSN mensuelle) le lit avant d'ecrire.
pub(crate) fn partie_le(racine: &Path, cible: &Path) -> R<Option<String>> {
    if !cible.is_file() {
        return Ok(None);
    }
    let empreinte = envois::journal::empreinte_fichier(cible)?;
    let lignes = envois::journal::lu(racine)?.lignes;
    Ok(lignes
        .iter()
        .filter(|l| l.get("mode").and_then(Value::as_str) == Some(envois::journal::REEL) && l.get("accepte").is_some_and(vrai))
        .filter(|l| l.get("empreinte").and_then(Value::as_str) == Some(empreinte.as_str()))
        .find(|l| !envois::etats::refuse_par_un_retour(l).unwrap_or(false))
        .map(|l| l.get("quand").map(valeur_en_texte).unwrap_or_default().chars().take(10).collect()))
}

/// `partie_le`, en refus : `cible` est parti, rien n'est ecrit.
pub(crate) fn refuser_si_partie(racine: &Path, cible: &Path) -> R<()> {
    match partie_le(racine, cible)? {
        None => Ok(()),
        Some(le) => Err(erreur(
            "ErreurOperation",
            format!(
                "{} est parti le {} : ce qui est parti ne se réécrit pas. Rien n'est écrit.",
                socle::fichiers::nom_du(cible),
                socle::date::Jour::lire(&le).map(|j| j.jj_mm_aaaa()).unwrap_or(le)
            ),
        )),
    }
}

/// La cle d'une piece au journal des ecritures : `<son dossier>/<son nom>` (`2026-11/DPAE_Novembre2026.txt`,
/// `exercices/Compta_2026.csv`).
pub(crate) fn cle_de_piece(fichier: &Path) -> String {
    let nom = |p: Option<&Path>| p.and_then(Path::file_name).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    format!("{}/{}", nom(fichier.parent()), nom(Some(fichier)))
}

/// Le journal des pieces ecrites : `{"<mois>/<fichier>": {"quand", "empreinte_saisie"}}`. Un journal absent ou illisible
/// est vide.
pub(crate) fn journal_des_ecritures(racine: &Path) -> Map<String, Value> {
    std::fs::read(racine.join(DSN_ECRITES))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

/// LA DSN DU MOIS EST-ELLE ECRITE ? Une DSN mensuelle du mois notee au journal des ecritures, et toujours la ou le moteur
/// l'a ecrite (`sorties/<mois>/`). Un fichier DSN que le moteur n'a pas note (pose a la main) ne compte pas. C'est le
/// critere de l'accueil (`crate::routes::ouverture::bilan_des_mois`), des rappels (`crate::routes::rappels::dsn_deposable`) et de
/// l'etape de depot (`routes::etapes`).
pub(crate) fn mois_ecrit(racine: &Path, mois: &str) -> bool {
    let prefixe = format!("{}/", mois);
    let sorties = racine.join("sorties");
    journal_des_ecritures(racine)
        .keys()
        .any(|k| k.strip_prefix(&prefixe).is_some_and(crate::routes::etapes::est_une_dsn_mensuelle) && sorties.join(k).is_file())
}

/// QUAND LE MOTEUR A ECRIT CETTE PIECE (`AAAA-MM-JJTHH:MM`), lu au journal des ecritures (`noter_piece_ecrite`) ; `null`
/// s'il ne le dit pas. Jamais la date du fichier.
pub(crate) fn ecrit_le(racine: &Path, fichier: &Path) -> Value {
    journal_des_ecritures(racine)
        .get(&cle_de_piece(fichier))
        .and_then(|n| n.get("quand"))
        .and_then(Value::as_str)
        .map_or(Value::Null, |q| json!(q.chars().take(16).collect::<String>()))
}

/// CE QUE CETTE DSN MENSUELLE CORRIGE, tel que sa fabrication l'a note (`noter_piece_ecrite`) ; `None` si le moteur ne l'a
/// pas ecrite.
pub(crate) fn versions_portees_de(racine: &Path, fichier: &Path) -> Option<Vec<Value>> {
    journal_des_ecritures(racine).get(&cle_de_piece(fichier)).and_then(|n| n.get("versions_portees")).and_then(Value::as_array).cloned()
}

// ------------------------------------------------------------- la DSN, a son depot

/// Ce que doit etre la prochaine DSN REELLE de ce mois : `(type, ordre, remplace)`. Normale d'ordre 1 si rien n'est
/// passe ; sinon annule-et-remplace au rang suivant -- et un refus si son delai est passe.
fn forme_de_la_dsn(ctx: &Contexte, m: &mut Moteur, le_mois: Mois, nic: &Value) -> R<(String, i64, bool)> {
    let faits = m.depots_reels(le_mois, nic)?;
    let dernier = m.dernier_ordre(le_mois, &faits)?;
    // CE QUE LA PLATE-FORME A VU EN REEL ce mois civil, lu en silence dans les bilans archives
    // (`ordre_signalements::dernier_ordre_des_mensuelles`) : le rang se compte « au cours du mois courant » (cahier
    // technique DSN, p. 136), toutes declarations du SIREN confondues, d'ou qu'elles soient parties (un autre poste).
    let constitution: String = ctx.aujourdhui.iso().chars().take(7).collect();
    let vues = envois::ordre_signalements::dernier_ordre_des_mensuelles(&ctx.racine, &envois::ordre_signalements::siren_de(ctx), false, &constitution, &le_mois.to_string())?;
    let dernier = match dernier {
        Some(d) => d.max(vues.ordre),
        // Une mensuelle du mois acceptee en reel que le journal ignore : la suivante la remplace.
        None if vues.du_mois_acceptee => vues.ordre.max(1),
        None => return Ok(("01".into(), vues.ordre + 1, false)),
    };
    m.refuser_si_hors_delai(le_mois)?;
    Ok(("03".into(), dernier + 1, true))
}

/// La declaration qu'un annule-et-remplace remplace (la derniere DSN mensuelle partie du mois, `dsn_parties`)
/// portait-elle un versement du prelevement a la source (S21.G00.20.002 = DGFIP_PAS) ? `None` : aucune n'est partie.
fn remplacee_versait_pas_de(ctx: &Contexte, m: &mut Moteur, mois: &str, nic: &Value) -> R<Option<bool>> {
    Ok(dsn_parties(ctx, m, mois, nic)?.last().map(verse_a_la_dgfip))
}

/// La DSN porte-t-elle un versement du prelevement a la source ?
fn verse_a_la_dgfip(f: &declarations::dsn::Fabriquee) -> bool {
    f.ordres.iter().any(|o| o.reference.as_deref() == Some(declarations::dsn::DGFIP_PAS))
}

/// Le jour de constitution (S20.G00.05.007, `JJMMAAAA`) d'une DSN deja ecrite.
fn constitution_de(chemin: &Path) -> Option<socle::date::Jour> {
    let octets = std::fs::read(chemin).ok()?;
    let t = envois::depot::lire(&octets).champ("constitution").to_string();
    let (j, m, a) = (t.get(0..2)?, t.get(2..4)?, t.get(4..8)?);
    socle::date::Jour::lire(&format!("{}-{}-{}", a, m, j)).ok()
}

// ------------------------------------------------------------- ce qu'une DSN porte

/// La forme d'une DSN mensuelle : son en-tete, l'etablissement qu'elle declare, et le jour de sa constitution.
struct Forme {
    type_dsn: String,
    ordre: i64,
    /// Le NIC de l'etablissement ; `None` : la compagnie n'en a qu'un.
    etablissement: Option<String>,
    constitution: socle::date::Jour,
    remplacee_versait_pas: Option<bool>,
}

impl Forme {
    fn options(&self) -> declarations::dsn::Options {
        declarations::dsn::Options {
            date_constitution: Some(self.constitution),
            envoi_test: false,
            type_declaration: self.type_dsn.clone(),
            ordre: self.ordre,
            etablissement: self.etablissement.clone(),
            remplacee_versait_pas: self.remplacee_versait_pas,
            ..Default::default()
        }
    }
}
/// LA DSN DE `mois` TELLE QU'ELLE PARTIRAIT, sous la forme `forme` : depuis les paies ecrites des engagements valides du
/// mois de son etablissement, avec ce que `fabriquer_dsn` lui donne (les corrections a porter, les arrets du mois).
fn fabriquer_du_present(ctx: &Contexte, lu: &Lu, mois: &str, forme: &Forme) -> R<declarations::dsn::Fabriquee> {
    lu.present(ctx, mois, forme)?.fabriquer(ctx, mois, forme)
}

/// CE QUE LA DSN DE `mois` LIT, tous etablissements confondus : ce qui est parti, les paies ecrites des engagements
/// valides du mois, les corrections d'autres mois a porter, les arrets du mois. Une fabrication le lit une fois (ses
/// avis, la DSN de chaque etablissement, le fichier deja ecrit qu'elle compare) : rien ne s'ecrit entre deux lectures.
struct Lu {
    departs: engagement::departs::Departs,
    paies: Vec<Paie>,
    regs: Vec<Reg>,
    arrets: Vec<Value>,
}

impl Lu {
    fn du_mois(ctx: &Contexte, mois: &str) -> R<Lu> {
        let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
        let (_data, paies) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, mois, None, &departs)?;
        let regs = regularisations::regularisations_pour(ctx, mois, &departs)?;
        let arrets = crate::compagnie::marques_depot::arrets_pour(ctx, mois, &departs)?;
        Ok(Lu { departs, paies, regs, arrets })
    }

    /// Ce que la DSN de l'etablissement de `forme` en declare.
    fn present(&self, ctx: &Contexte, mois: &str, forme: &Forme) -> R<Present> {
        let fiche = ctx.entreprise_du_mois(mois).map_err(valeur)?;
        let nic = forme.etablissement.clone();
        let du_nic = |sal: &droit::fiche::SalarieDuContrat| -> R<bool> {
            match &nic {
                Some(n) => Ok(droit::bareme::etablissement_de(&fiche, sal).map_err(valeur)? == *n),
                None => Ok(true),
            }
        };
        let mut paies: Vec<Paie> = Vec::new();
        for p in &self.paies {
            if du_nic(&p.salarie)? {
                paies.push(p.clone());
            }
        }
        let mut regs_e: Vec<&Reg> = Vec::new();
        for r in &self.regs {
            if du_nic(&r.apres.salarie)? {
                regs_e.push(r);
            }
        }
        let regs_dsn = regularisations::pour_la_dsn(&regs_e);
        let mut arrets: Vec<declarations::dsn::Arret> = Vec::new();
        for a in &self.arrets {
            let sal = engagement::imprevus::fiche_du_salarie(ctx, a).map_err(valeur)?;
            if du_nic(&sal)? {
                arrets.push(declarations::dsn::Arret { v: a.clone() });
            }
        }
        Ok(Present { paies, regs: regs_dsn, arrets, departs: self.departs.clone() })
    }
}

/// CE QU'UNE DSN DE `mois` DECLARERAIT AUJOURD'HUI pour l'etablissement de `forme` : les paies ecrites de ses engagements
/// valides, les corrections d'autres mois a porter, les arrets du mois.
struct Present {
    paies: Vec<Paie>,
    regs: Vec<declarations::dsn::Regularisation>,
    arrets: Vec<declarations::dsn::Arret>,
    departs: engagement::departs::Departs,
}

impl Present {
    /// Ni paie ni correction : rien a declarer (un mois tenu avant ce logiciel, un mois sans engagement).
    fn vide(&self) -> bool {
        self.paies.is_empty() && self.regs.is_empty()
    }

    fn fabriquer(&self, ctx: &Contexte, mois: &str, forme: &Forme) -> R<declarations::dsn::Fabriquee> {
        declarations::dsn::fabriquer(ctx, &self.paies, mois, &self.regs, &self.arrets, &forme.options(), &self.departs).map_err(valeur)
    }
}

/// LA DSN QUE PORTE LE FICHIER `chemin` (mois principal `mois`, etablissement `nic` ; le principal sans lui), fabriquee
/// de nouveau depuis les paies des engagements qu'elle couvre (`fabriquer_du_present`), sous la forme que son en-tete
/// dit, au jour ou elle a ete constituee. `None` : le fichier ne dit plus ce que disent les paies, il se refait.
pub(crate) fn refabriquer(ctx: &Contexte, chemin: &Path, mois: &str, nic: Option<&str>) -> R<Option<declarations::dsn::Fabriquee>> {
    refabriquer_sur(ctx, None, chemin, mois, nic)
}

/// `refabriquer`, sur ce que la fabrication en cours a deja lu (`lu`), ou sur ce qui se lit maintenant.
fn refabriquer_sur(ctx: &Contexte, lu: Option<&Lu>, chemin: &Path, mois: &str, nic: Option<&str>) -> R<Option<declarations::dsn::Fabriquee>> {
    let Ok(octets) = std::fs::read(chemin) else { return Ok(None) };
    let Some(constitution) = constitution_de(chemin) else { return Ok(None) };
    let entete = envois::depot::lire(&octets);
    let type_dsn = entete.champ("type_declaration").to_string();
    let Ok(ordre) = entete.champ("ordre").trim().parse::<i64>() else { return Ok(None) };
    let plusieurs = !ctx.entreprise_du_mois(mois).map_err(valeur)?.secondaires().is_empty();
    let nic = nic.filter(|n| !n.is_empty()).map(str::to_string).unwrap_or_else(|| ctx.entreprise.nic_principal());
    let nic_v = if plusieurs { Value::String(nic.clone()) } else { Value::Null };
    let remplacee_versait_pas = if type_dsn == "03" || type_dsn == "05" {
        let mut m = Moteur::nouveau(ctx);
        remplacee_versait_pas_de(ctx, &mut m, mois, &nic_v)?
    } else {
        None
    };
    let forme = Forme { type_dsn, ordre, etablissement: plusieurs.then_some(nic), constitution, remplacee_versait_pas };
    let f = match lu {
        Some(lu) => fabriquer_du_present(ctx, lu, mois, &forme)?,
        None => fabriquer_du_present(ctx, &Lu::du_mois(ctx, mois)?, mois, &forme)?,
    };
    Ok((declarations::dsn::en_latin1(&f.texte).ok().as_deref() == Some(octets.as_slice())).then_some(f))
}

/// L'engagement de `mois` du salarie `cle` au contrat `numero`, actif ou non.
fn engagement_de(ctx: &Contexte, mois: &str, cle: &str, numero: &str) -> R<engagement::stock::Objet> {
    engagement::stock::objets_du_mois(&ctx.racine, mois)?
        .into_iter()
        .chain(engagement::stock::non_actifs_du_mois(&ctx.racine, mois)?)
        .find(|o| {
            let b = &*o.brut;
            engagement::cle_de_l_engagement(ctx, &b).as_deref() == Some(cle) && b.get("numero").map(valeur_en_texte).as_deref() == Some(numero)
        })
        .ok_or_else(|| valeur(format!("{} : aucun engagement de {} ne porte le contrat {} qu'une DSN partie a declare.", cle, socle::mois::Mois::en_clair(mois), numero)))
}

/// La paie de `mois` que la version `version` de l'engagement `o` (salarie `cle`, contrat `numero`) a ecrite
/// (`engagement::etats::paie_de_la_version_n`).
fn paie_de_la_version(ctx: &Contexte, o: &engagement::stock::Objet, cle: &str, numero: &str, mois: &str, version: u64) -> R<Paie> {
    engagement::etats::paie_de_la_version_n(ctx, o, version, mois)?
        .ok_or_else(|| valeur(format!("{} : la version {} du contrat {} n'a pas de paie de {}.", cle, version, numero, socle::mois::Mois::en_clair(mois))))
}

/// Le jour ou la version `n` de l'engagement `o` a ete validee (`le` de la presente, `valide_le` d'une passee).
fn valide_le(o: &engagement::stock::Objet, n: u64) -> Option<socle::date::Jour> {
    let passees = o.brut.get("versions").and_then(Value::as_array).into_iter().flatten();
    let v = o.brut.get("valide").into_iter().chain(passees).find(|v| v.get("version").and_then(Value::as_u64) == Some(n))?;
    let le = v.get("valide_le").or_else(|| v.get("le")).and_then(Value::as_str)?;
    socle::date::Jour::lire(le.get(..10)?).ok()
}

/// LA DSN MENSUELLE PARTIE que dit la ligne `lignes[rang]` du journal, fabriquee de nouveau par la fonction qui l'a
/// fabriquee (`dsn::fabriquer`) depuis les versions qu'elle a emportees (`versions` de sa ligne) : les paies de son mois,
/// et les corrections des mois passes qu'elle portait -- leur paie d'apres, la version emportee ; ce qu'elles annulaient,
/// le dernier etat declare par ce qui etait parti avant elle (`versement_declare::dernier_declare` sur les lignes qui la
/// precedent) ; les arrets du mois de son etablissement, qu'elle reporte. `remplacee_versait_pas` : sur un
/// annule-et-remplace, si la DSN remplacee versait a la DGFiP.
fn dsn_partie(ctx: &Contexte, m: &mut Moteur, lignes: &[Value], rang: usize, remplacee_versait_pas: Option<bool>) -> R<declarations::dsn::Fabriquee> {
    let l = &lignes[rang];
    let mois = valeur_en_texte(l.get("mois").unwrap_or(&Value::Null));
    let detail = l.get("detail").cloned().unwrap_or(Value::Null);
    let champ = |k: &str| detail.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    let type_dsn = Some(champ("type")).filter(|t| !t.is_empty()).unwrap_or_else(|| "01".into());
    let plusieurs = !ctx.entreprise_du_mois(&mois).map_err(valeur)?.secondaires().is_empty();
    let nic = Some(champ("etablissement")).filter(|n| !n.is_empty()).unwrap_or_else(|| ctx.entreprise.nic_principal());
    // Constituee le jour ou son fichier a ete ecrit (le journal des ecritures), sinon le jour de son depot.
    let ecrite = journal_des_ecritures(&ctx.racine)
        .get(&format!("{}/{}", mois, valeur_en_texte(l.get("fichier").unwrap_or(&Value::Null))))
        .and_then(|n| n.get("quand"))
        .map(valeur_en_texte);
    let quand = ecrite.unwrap_or_else(|| valeur_en_texte(l.get("quand").unwrap_or(&Value::Null)));
    let constitution = socle::date::Jour::lire(quand.get(..10).unwrap_or_default()).map_err(valeur)?;
    let forme = Forme { type_dsn, ordre: m.ordre_de_la_ligne(socle::mois::Mois::lire(&mois)?, l)?, etablissement: plusieurs.then_some(nic), constitution, remplacee_versait_pas };
    let avant = envois::journal::departs_des_lignes(&lignes[..rang]);
    let ctx_m = ctx.du_mois(&mois).map_err(valeur)?;
    let mut paies: Vec<Paie> = Vec::new();
    let mut regs: Vec<declarations::dsn::Regularisation> = Vec::new();
    for v in l.get("versions").and_then(Value::as_array).into_iter().flatten() {
        let texte = |k: &str| v.get(k).map(valeur_en_texte).unwrap_or_default();
        let (cle, numero) = (texte("salarie"), texte("numero"));
        let version = v.get("version").and_then(Value::as_u64).ok_or_else(|| valeur(format!("journal : une version illisible dans la DSN de {}", mois)))?;
        let corrige = Some(texte("mois")).filter(|c| !c.is_empty() && *c != mois);
        let Some(c) = corrige else {
            let o = engagement_de(ctx, &mois, &cle, &numero)?;
            paies.push(paie_de_la_version(&ctx_m, &o, &cle, &numero, &mois, version)?);
            continue;
        };
        let ctx_c = ctx.du_mois(&c).map_err(valeur)?;
        let o = engagement_de(ctx, &c, &cle, &numero)?;
        let dernier = declarations::versement_declare::dernier_declare(&ctx_c, &o, &avant, &c, &mois).map_err(valeur)?;
        // La version d'un non actif : la correction annulait ce qui avait ete declare (`Paie::annulee`), le jour ou il
        // a cesse d'etre actif. Sinon, le jour ou la version qu'elle emporte a ete validee.
        let (apres, decidee_le) = match (engagement::etats::NonActif::de(o.clone()).filter(|n| n.version() == version), &dernier) {
            (Some(n), Some(d)) => (d.avant.annulee(), n.le().get(..10).and_then(|t| socle::date::Jour::lire(t).ok())),
            _ => (paie_de_la_version(&ctx_c, &o, &cle, &numero, &c, version)?, valide_le(&o, version)),
        };
        regs.push(match dernier {
            Some(d) => declarations::dsn::Regularisation {
                mois: c,
                avant: d.avant,
                taux_pas_origine: d.versement.taux_pas.clone(),
                declare: d.versement,
                apres,
                net_deja_verse: d.net_deja_verse,
                decidee_le,
            },
            // Un contrat ajoute apres la limite : rien n'etait declare ni verse pour lui.
            None => declarations::dsn::Regularisation {
                mois: c.clone(),
                avant: apres.clone(),
                declare: declarations::versement_declare::VersementDeclare {
                    fichier: String::new(),
                    net_imposable: socle::num::Decimal::ZERO,
                    net_verse: socle::num::Decimal::ZERO,
                    net_social: socle::num::Decimal::ZERO,
                    rappels_net_social: socle::num::Decimal::ZERO,
                    frais: socle::num::Decimal::ZERO,
                    brut: socle::num::Decimal::ZERO,
                    lignes: Vec::new(),
                    affiliations: Vec::new(),
                    taux_pas: None,
                    debut_contrat: None,
                },
                apres,
                taux_pas_origine: None,
                net_deja_verse: socle::num::Decimal::ZERO,
                decidee_le,
            },
        });
    }
    let mut arrets: Vec<declarations::dsn::Arret> = Vec::new();
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    for a in crate::compagnie::marques_depot::arrets_pour(ctx, &mois, &departs)? {
        if let Some(n) = &forme.etablissement {
            let sal = engagement::imprevus::fiche_du_salarie(ctx, &a).map_err(valeur)?;
            if droit::bareme::etablissement_de(&ctx.entreprise, &sal).map_err(valeur)? != *n {
                continue;
            }
        }
        arrets.push(declarations::dsn::Arret { v: a });
    }
    declarations::dsn::fabriquer(ctx, &paies, &mois, &regs, &arrets, &forme.options(), &departs).map_err(valeur)
}

/// LES DSN MENSUELLES PARTIES de `mois` pour l'etablissement `nic` (`Null` : le principal), reelles et acceptees, dans
/// l'ordre de leur rang, chacune fabriquee de nouveau telle qu'elle est partie (`dsn_partie`) ; un annule-et-remplace
/// sait ainsi si celle qu'il remplacait versait a la DGFiP.
fn dsn_parties(ctx: &Contexte, m: &mut Moteur, mois: &str, nic: &Value) -> R<Vec<declarations::dsn::Fabriquee>> {
    let lignes = m.lignes()?;
    let mut faits: Vec<(i64, usize)> = Vec::new();
    for l in m.depots_reels(socle::mois::Mois::lire(mois)?, nic)? {
        let Some(rang) = lignes.iter().position(|x| *x == l) else { continue };
        faits.push((m.ordre_de_la_ligne(socle::mois::Mois::lire(mois)?, &l)?, rang));
    }
    faits.sort();
    let mut out: Vec<declarations::dsn::Fabriquee> = Vec::new();
    for (_, rang) in faits {
        let versait = out.last().map(verse_a_la_dgfip);
        out.push(dsn_partie(ctx, m, &lignes, rang, versait)?);
    }
    Ok(out)
}

/// CE QU'UNE DSN DE `mois` FAIT VERSER, pour l'etablissement `nic` (le principal sans lui) : la derniere DSN mensuelle
/// partie du mois, telle qu'elle est partie (ses paies et les corrections d'autres mois qu'elle portait, `dsn_parties`) ;
/// si aucune n'est partie, celle qui partirait aujourd'hui (`fabriquer_du_present`). Rend aussi si elle est partie.
/// `None` : rien n'est parti et rien n'est a declarer pour cet etablissement (ni paie validee, ni correction).
pub(crate) fn dsn_du_mois(ctx: &Contexte, mois: &str, nic: Option<&str>) -> R<Option<(declarations::dsn::Fabriquee, bool)>> {
    let mut m = Moteur::nouveau(ctx);
    let plusieurs = !ctx.entreprise_du_mois(mois).map_err(valeur)?.secondaires().is_empty();
    let nic = nic.filter(|n| !n.is_empty()).map(str::to_string).unwrap_or_else(|| ctx.entreprise.nic_principal());
    let nic_v = if plusieurs { Value::String(nic.clone()) } else { Value::Null };
    if let Some(f) = dsn_parties(ctx, &mut m, mois, &nic_v)?.pop() {
        return Ok(Some((f, true)));
    }
    let forme = Forme { type_dsn: "01".into(), ordre: 1, etablissement: plusieurs.then_some(nic), constitution: ctx.aujourdhui, remplacee_versait_pas: None };
    let present = Lu::du_mois(ctx, mois)?.present(ctx, mois, &forme)?;
    if present.vide() {
        return Ok(None);
    }
    Ok(Some((present.fabriquer(ctx, mois, &forme)?, false)))
}

/// La DSN du mois, telle que le depot la prend.
pub(crate) struct DsnFabriquee {
    /// Une DSN par etablissement, le principal d'abord.
    pub(crate) fichiers: Vec<PathBuf>,
    pub(crate) avertissements: Vec<Avis>,
}

impl DsnFabriquee {
    /// Ce que la fabrique dit avant le depot : les avertissements, et les informations.
    pub(crate) fn avis(&self) -> (Vec<String>, Vec<String>) {
        let de = |info: bool| self.avertissements.iter().filter(|a| a.info == info).map(|a| a.texte.clone()).collect::<Vec<_>>();
        (de(false), de(true))
    }
}

/// LA DSN DE `mois` NE S'ECRIT NI NE PART AVANT SON PREMIER JOUR DE DEPOT : « le dépôt initial de la DSN "normale" peut
/// être émis jusqu'à 1 mois à l'avance » (cahier technique DSN 2026.1, § 1.4.2,
/// `atelier/reference/norme-dsn/texte/chapitre-1-la-norme.txt` ; `Echeance::premier_depot`). Avant ce jour, le refus dit
/// le jour et l'echeance. Seule `fabriquer_dsn` ecrit une mensuelle : c'est elle qui refuse.
fn pas_avant(ctx: &Contexte, mois: &str) -> R<()> {
    let echeance = declarations::echeance_dsn::de_la_compagnie(ctx, mois).map_err(|m| erreur("ValueError", m))?;
    let premier = echeance.premier_depot().map_err(|m| erreur("ValueError", m))?;
    if premier.ordinal() <= ctx.aujourdhui.ordinal() {
        return Ok(());
    }
    Err(erreur(
        "ErreurOperation",
        format!(
            "La DSN de {} se dépose à partir du {} : la norme admet un dépôt au plus un mois avant l'échéance (cahier technique DSN 2026.1, § 1.4.2). Son échéance : {}. Rien n'est parti.",
            socle::mois::Mois::en_clair(mois),
            premier.jj_mm_aaaa(),
            echeance.phrase()
        ),
    ))
}

/// LA DSN DU MOIS, A SON DEPOT : une par etablissement, depuis les paies ecrites des engagements valides du mois, avec
/// les corrections des mois deja declares qu'elle porte, les arrets qu'elle reporte, les suspensions, l'OETH d'avril et
/// la CSA de mars. Un mois deja declare repart en annule-et-remplace, sous un nom a lui (`_AR<ordre>`) : le fichier
/// depose reste tel qu'il est parti. Un fichier deja ecrit qui dit exactement la meme chose est repris tel quel : le test
/// qu'il a passe vaut pour lui.
///
/// Refus : un mois de l'ancien logiciel, sa DSN importee ou non (`import::mois::refuser_la_dsn_de_l_ancien_logiciel`) ;
/// un mois dont aucun engagement n'est valide alors qu'il en a (rien ne part d'un engagement en saisie) ; un mois
/// deja declare dont l'annule-et-remplace n'est plus admis.
pub(crate) fn fabriquer_dsn(ctx: &Contexte, le_mois: Mois, qui: &str) -> R<DsnFabriquee> {
    let mois = &le_mois.to_string();
    crate::import::mois::refuser_la_dsn_de_l_ancien_logiciel(ctx, mois)?;
    let _verrou = documents::ecrire_documents::prendre_le_verrou(ctx, qui)?;
    crate::routes::mois::refuser_si_clos(ctx, mois)?;
    pas_avant(ctx, mois)?;
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let mut avertissements: Vec<Avis> = Vec::new();

    // RIEN NE PART D'UN ENGAGEMENT EN SAISIE : la DSN declare les engagements valides du mois.
    let valides = engagement::etats::valides_du_mois(ctx, mois).map_err(Echec::from)?;
    let en_saisie = valides.iter().filter(|v| !**v).count();
    if en_saisie > 0 && en_saisie == valides.len() {
        return Err(erreur(
            "EngagementEnSaisie",
            format!(
                "{} : aucun engagement n'est valide. Rien ne part d'un engagement en saisie : le valider d'abord (« Valider l'engagement »). Rien n'est ecrit.",
                socle::mois::Mois::en_clair(mois)
            ),
        ));
    }
    if en_saisie > 0 {
        avertissements.push(avertissement(format!(
            "{} engagement(s) de {} encore en saisie : la DSN ne les declare pas. Une fois valides, la DSN se refait.",
            en_saisie,
            socle::mois::Mois::en_clair(mois)
        )));
    }
    let lu = Lu::du_mois(ctx, mois)?;
    let paies = &lu.paies;
    avertissements.extend(sans_taux_recu(ctx, paies.iter())?.into_iter().map(avertissement));
    avertissements.extend(crate::routes::mois::archives_employees(&paies)?.into_iter().map(avertissement));
    for p in paies {
        avertissements.extend(p.avertissements.iter().cloned());
    }
    for p in paies {
        if p.engagement.sans_paie() {
            avertissements.push(info(format!(
                "{} {} : contrat {} en cours, aucun cachet en {} -- la DSN porte le contrat avec un versement a zero.",
                p.salarie.nom.texte().to_string(),
                prenom_de(&p.salarie),
                p.engagement.numero.as_deref().unwrap_or("None"),
                socle::mois::Mois::en_clair(mois)
            )));
        }
    }

    let fiche = ctx.entreprise_du_mois(mois).map_err(valeur)?;
    let principal = fiche.nic_principal();
    let secondaires: Vec<String> = fiche.secondaires().iter().map(|e| e.nic.texte().to_string()).collect();
    let plusieurs = !secondaires.is_empty();
    let mut m = Moteur::nouveau(ctx);
    // Un mois deja declare dont l'annule-et-remplace n'est plus admis : refuse AVANT d'ecrire quoi que ce soit.
    let nics: Vec<Value> = if plusieurs { std::iter::once(principal.clone()).chain(secondaires.iter().cloned()).map(Value::String).collect() } else { vec![Value::Null] };
    for nic in &nics {
        forme_de_la_dsn(ctx, &mut m, le_mois, nic)?;
    }
    let etiq = le_mois.etiquette();
    let sortie = ctx.racine.join("sorties").join(mois);

    // LES CORRECTIONS DES MOIS DEJA DECLARES : la DSN de ce mois les porte, datees de leur mois d'origine.
    let regs = &lu.regs;
    // La DSN DECLARE, ELLE NE FABRIQUE AUCUN BULLETIN : le bulletin d'une paie corrigee part avec les documents de son
    // mois (`fabriquer_documents` : le rectificatif d'un bulletin parti, ou le bulletin d'une paie qui n'est jamais
    // partie). Une correction a zero (la paie d'apres est `sans_paie`) n'a pas de bulletin : le net deja verse se retient.
    for (r, ligne) in regs.iter().zip(regularisations::resume_regularisations(&regs)?) {
        let suite = if r.apres.engagement.sans_paie() {
            " La paie est retirée : le net déjà versé se retient, et le salarié doit en être prévenu par écrit."
        } else {
            ""
        };
        avertissements.push(info(format!("Régularisation portée par la DSN : {}.{}", ligne, suite)));
    }
    if paies.is_empty() && regs.is_empty() {
        avertissements.push(info(format!(
            "Aucun salarié en {} : la DSN part quand même, sans individu (envoi néant) -- une DSN est due chaque mois.",
            socle::mois::Mois::en_clair(mois)
        )));
    }
    // LA DECLARATION OETH D'AVRIL : elle ajoute au bordereau Urssaf une contribution (CTP 730 ou 740).
    if let Some(doeth) = declarations::oeth::declaration(ctx, mois).map_err(valeur)? {
        avertissements.push(info(format!("{} Le bordereau Urssaf du mois en porte {}.", doeth.resume(), socle::texte::euros(doeth.a_verser()))));
    }
    // LA CONTRIBUTION SUPPLEMENTAIRE A L'APPRENTISSAGE DE MARS : le bloc 82 (079 ou 080) et le CTP 998 (`csa`).
    if let Some(c) = declarations::csa::declaration(ctx, mois, &departs).map_err(valeur)? {
        avertissements.push(info(c.resume()));
    }
    // LES ARRETS DE TRAVAIL : la DSN mensuelle REPORTE ceux que `Arret::a_reporter` y met, signales ou non.
    let arrets = &lu.arrets;
    let nomenclature = crate::routes::imprevus::nomenclatures(ctx)?;
    let jour = |v: &Value| -> R<String> { documents::bulletin::jj_mm_aaaa(v).map_err(valeur) };
    let mut non_signales = 0;
    for a in arrets {
        let resume = crate::routes::imprevus::resume_arret(ctx, &nomenclature, a)?;
        let reprise = a.get("reprise").filter(|r| vrai(r));
        let fin = match reprise {
            Some(r) => r.clone(),
            None => a.get("fin_previsionnelle").cloned().unwrap_or(Value::Null),
        };
        avertissements.push(info(format!(
            "Arret de travail reporte dans la DSN : {}, {}, du {} au {}{}.",
            valeur_en_texte(a.get("salarie").unwrap_or(&Value::Null)),
            resume.motif_libelle,
            engagement::imprevus::dernier_jour_declare(a).map_err(valeur)?.jj_mm_aaaa(),
            jour(&fin)?,
            if reprise.is_some() { " (reprise)" } else { " (fin previsionnelle)" }
        )));
        if !resume.signale {
            non_signales += 1;
        }
    }
    // LES SUSPENSIONS (bloc 65) : chacune se declare entiere dans la DSN de chaque mois qu'elle traverse.
    for s in engagement::imprevus::du_mois(&ctx.racine, mois, Some(engagement::imprevus::SUSPENSION)).map_err(crate::routes::imprevus::refus_magasin)? {
        avertissements.push(info(format!(
            "Suspension declaree dans la DSN : {}, motif {}, du {} au {}.",
            valeur_en_texte(&s["salarie"]),
            valeur_en_texte(&s["motif"]),
            jour(&s["debut"])?,
            jour(&s["fin"])?
        )));
    }
    if non_signales > 0 {
        avertissements.push(avertissement(format!(
            "{} arret(s) reporte(s) dans la DSN mais JAMAIS SIGNALE(S) : le signalement d'arret est du dans les cinq jours, la DSN du mois ne le remplace pas (Arrets de travail › Signaler).",
            non_signales
        )));
    }

    // UNE DSN PAR ETABLISSEMENT : chacun avec ses salaries, ses corrections et ses arrets ; celle du principal garde
    // son nom de toujours, les autres portent leur NIC. Un etablissement sans salarie ce mois-ci depose quand meme la
    // sienne, sans individu : une DSN est due chaque mois, par SIRET.
    let mut a_ecrire: Vec<(PathBuf, String, Vec<Value>)> = Vec::new();
    let mut fichiers: Vec<PathBuf> = Vec::new();
    for nic in &nics {
        let n = nic.as_str().map(str::to_string);
        let mut nom = match &n {
            Some(x) if *x != principal => format!("DSN_{}_{}.dsn", etiq, x),
            _ => format!("DSN_{}.dsn", etiq),
        };
        let (type_dsn, ordre, remplace) = forme_de_la_dsn(ctx, &mut m, le_mois, nic)?;
        let mut remplacee_versait_pas = None;
        if remplace {
            remplacee_versait_pas = remplacee_versait_pas_de(ctx, &mut m, mois, nic)?;
            nom = format!("{}_AR{}.dsn", &nom[..nom.len() - ".dsn".len()], ordre);
            avertissements.push(info(format!(
                "La DSN de {} est deja partie : celle-ci est un ANNULE-ET-REMPLACE integral (ordre {}). Elle remplace la precedente, ordres de telepaiement compris, et s'envoie au plus tard le {} a minuit.",
                socle::mois::Mois::en_clair(mois),
                ordre,
                m.limite(le_mois)?.jj_mm_aaaa()
            )));
        }
        let f_dsn = sortie.join(&nom);
        // LE FICHIER DEJA ECRIT QUI DIT LA MEME CHOSE se reprend (`refabriquer`) : meme declaration, au jour ou il a ete
        // constitue.
        if refabriquer_sur(ctx, Some(&lu), &f_dsn, mois, n.as_deref())?.is_none() {
            // Ce que la DSN de cet etablissement corrige des mois passes : elle le dit a son depot.
            let portees = declarations::produire_regularisations::versions_portees(ctx, &lu.regs, n.as_deref().unwrap_or(&principal))?;
            let forme = Forme { type_dsn, ordre, etablissement: n, constitution: ctx.aujourdhui, remplacee_versait_pas };
            a_ecrire.push((f_dsn.clone(), fabriquer_du_present(ctx, &lu, mois, &forme)?.texte, portees));
        }
        fichiers.push(f_dsn);
    }
    for (f, _, _) in &a_ecrire {
        refuser_si_partie(&ctx.racine, f)?;
    }
    if !a_ecrire.is_empty() {
        std::fs::create_dir_all(&sortie).map_err(|e| os(&sortie, e))?;
        for (f, texte, _) in &a_ecrire {
            declarations::dsn::ecrire(texte, f).map_err(|m| erreur("OSError", m))?;
        }
        // La paie que chaque DSN declare : l'ecran dira si elle change avant l'envoi.
        let empreinte = m.empreinte_saisie_avec(le_mois, || Ok(Moteur::portees(&lu.regs)))?;
        for (f, _, portees) in a_ecrire {
            noter_piece_ecrite(&ctx.racine, &cle_de_piece(&f), Some(json!(empreinte)), Some(portees))?;
        }
    }
    Ok(DsnFabriquee { fichiers, avertissements })
}

// ------------------------------------------------------------- les documents, a leur envoi

/// Les documents d'un mois, fabriques pour ceux a qui ils partent : leurs numeros sont reserves, pas encore ecrits.
pub(crate) struct DocumentsFabriques {
    _verrou: socle::verrou::Verrou,
    reserves: Reserves,
    /// `(cle du salarie, numero du contrat, numero de bulletin, premier et dernier N CE)` : ce qui s'ecrit dans les paies au
    /// depart.
    a_poser: Vec<(String, String, Option<String>, Option<(i64, i64)>)>,
    pub(crate) avertissements: Vec<String>,
}

impl DocumentsFabriques {
    /// LES DOCUMENTS SONT PARTIS : leurs numeros s'ecrivent dans les paies des engagements, et les compteurs passent
    /// au-dessus.
    pub(crate) fn engager(self, ctx: &Contexte, mois: &str) -> R<()> {
        self.reserves.valider(ctx)?;
        for (cle, numero, bulletin, ce) in &self.a_poser {
            engagement::etats::poser_numeros_des_documents(ctx, mois, cle, numero, bulletin.as_deref(), ce.map(|c| c.0)).map_err(Echec::from)?;
        }
        Ok(())
    }

    /// UN ENVOI COUPE EN ROUTE : seuls les salaries `servis` ont recu leurs documents, eux seuls ont leurs numeros. Les
    /// compteurs passent au-dessus du plus grand numero servi ; ceux des autres se rendent, ils les reprendront a leur
    /// depart.
    pub(crate) fn engager_servis(self, ctx: &Contexte, mois: &str, servis: &[&str]) -> R<()> {
        let mut bornes = Reserves::default();
        let (mut bulletin_max, mut ce_max): (Option<(i64, i64)>, Option<i64>) = (None, None);
        let an = annee_de(mois)?;
        for (cle, numero, bulletin, ce) in self.a_poser.iter().filter(|a| servis.contains(&a.0.as_str())) {
            if let Some(n) = bulletin.as_deref().and_then(|b| b.split('/').next()).and_then(|n| n.trim().parse::<i64>().ok()) {
                bulletin_max = Some((an, bulletin_max.map_or(n, |(_, m)| m.max(n))));
            }
            if let Some((_, dernier)) = ce {
                ce_max = Some(ce_max.map_or(*dernier, |m| m.max(*dernier)));
            }
            engagement::etats::poser_numeros_des_documents(ctx, mois, cle, numero, bulletin.as_deref(), ce.map(|c| c.0)).map_err(Echec::from)?;
        }
        if let Some((an, n)) = bulletin_max {
            bornes.enregistrer(an, "bulletin", n);
        }
        if let Some(n) = ce_max {
            bornes.enregistrer(an, "certificat_cs", n);
        }
        bornes.valider(ctx)
    }
}

/// Le numero d'un bulletin dans un nom de fichier : `12/2026` -> `12-2026`.
/// L'ECART D'UNE PAIE REFAITE AVEC LE BULLETIN PARTI (`parti`, ce qu'il a paye), verse -- ou retenu -- avec la paie de
/// `mois_versement` : la difference de net, et le prelevement a la source sur la difference de net imposable
/// (`engagement::etats::prelevement_de_la_correction`, au taux du bulletin parti pour un trop-verse : BOFiP,
/// BOI-IR-PAS-30-10-50, § 1 et § 90, `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-50-extrait-regularisation.md`).
fn ecart_avec_le_bulletin_parti(ctx: &Contexte, apres: &Paie, parti: &Paie, mois_versement: &str) -> R<Value> {
    let taux_parti = parti.pas.as_ref().map(|p| valeur_en_texte(&p.taux)).filter(|t| !t.is_empty());
    let d_net = apres.net_a_payer - parti.net_a_payer;
    let pas_c = engagement::etats::prelevement_de_la_correction(ctx, apres, parti.net_imposable, taux_parti.as_deref(), mois_versement).map_err(valeur)?;
    let (indu, prelevement) = (pas_c.indu(), pas_c.prelevement);
    let taux = match (indu, taux_parti) {
        (false, _) => pas_c.versement.taux.clone(),
        (true, Some(t)) => json!(t),
        (true, None) => Value::Null,
    };
    let a_verser = match prelevement {
        Some(p) => d_net - p,
        None => d_net,
    };
    Ok(json!({
        "net_avant": parti.net_a_payer.to_string(),
        "net_apres": apres.net_a_payer.to_string(),
        "difference_net": d_net.to_string(),
        "difference_net_imposable": pas_c.difference.to_string(),
        "indu": indu,
        "taux": taux,
        "prelevement": prelevement.map_or(Value::Null, |p| json!(p.to_string())),
        "a_verser": a_verser.to_string(),
        "mois_versement": socle::mois::Mois::en_clair(mois_versement),
    }))
}

fn numero_en_nom(n: &str) -> String {
    n.replace('/', "-")
}

/// LES DOCUMENTS DU MOIS, FABRIQUES A LEUR ENVOI, pour les salaries `cles` : depuis les paies ecrites des engagements
/// valides (chaque contrat sous le numero que sa validation lui a ecrit), un `Bulletin_<cle>_<Mois>.pdf` (tous ses
/// bulletins du mois) et un `CertificatCS_<cle>_<Mois>.pdf` par salarie,
/// dans `sorties/<mois>/envoi/`. Un bulletin parti garde son numero ; un bulletin qui part pour la premiere fois prend le
/// suivant de l'annee ; une paie partie puis refaite part sous un nouveau numero, imprimee comme le rectificatif de celle
/// qui est partie, et le bulletin d'origine se garde a cote (`sorties/<mois>/Bulletin_<cle>_<Mois>_<numero>.pdf`). Les N CE
/// suivent la sequence continue des certificats. L'archive du mois (`Bulletins_<Mois>.pdf`, `CertificatsCS_<Mois>.pdf` et
/// son recapitulatif) porte tous les documents numerotes du mois.
/// CE QUE LES BULLETINS EXIGENT DES FICHES (`exige_par` de `champs-base.json`, la liste que « Pour demarrer » lit) :
/// l'entreprise, et le spectacle de chaque paie dont la nature declare un numero d'objet (S21.G00.40.050, la marque des
/// annexes VIII et X) ; tout ce qui manque, d'un coup, avant que rien ne s'ecrive.
fn exiges_des_bulletins<'a>(ent: &droit::fiche::ChampsEntreprise, paies: impl Iterator<Item = &'a Paie>) -> R<()> {
    let mut fiches: Vec<(&str, String, &dyn droit::fiche::ParCase)> = vec![("entreprise", "la fiche entreprise".to_string(), ent)];
    let mut spectacles: Vec<&Value> = Vec::new();
    for p in paies {
        if !p.engagement.nat().map_err(valeur)?.ecrit("S21.G00.40.050") || spectacles.contains(&&p.spectacle) {
            continue;
        }
        spectacles.push(&p.spectacle);
        let titre = p.spectacle.get("titre").filter(|t| vrai(t)).map(valeur_en_texte).unwrap_or_default();
        fiches.push(("spectacle", format!("Spectacles, la fiche de {}", titre), &p.spectacle));
    }
    match declarations::champs::refus_des_manques("bulletin", &fiches, &[])? {
        Some(m) => Err(erreur("ErreurOperation", m)),
        None => Ok(()),
    }
}

pub(crate) fn fabriquer_documents(ctx: &Contexte, mois: &str, cles: &[String], qui: &str) -> R<DocumentsFabriques> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let validees = engagement::etats::validees_du_mois(ctx, mois, &departs).map_err(Echec::from)?;
    let ent = ctx.pour_un_modele(&ctx.entreprise_du_mois(mois).map_err(valeur)?);
    if let Some((_, paies)) = &validees {
        exiges_des_bulletins(&ent, paies.iter().filter(|p| cles.contains(&cle_de(p)) && !p.engagement.sans_paie()))?;
    }
    let verrou = documents::ecrire_documents::prendre_le_verrou(ctx, qui)?;
    let mut faits = DocumentsFabriques { _verrou: verrou, reserves: Reserves::default(), a_poser: Vec::new(), avertissements: Vec::new() };
    let Some((_data, paies)) = validees else { return Ok(faits) };
    faits.avertissements.extend(sans_taux_recu(ctx, paies.iter().filter(|p| cles.contains(&cle_de(p))))?);
    let ecrits = engagement::etats::numeros_des_documents(ctx, mois, &departs).map_err(Echec::from)?;
    let an = annee_de(mois)?;
    let etiq = socle::mois::Mois::lire(mois).map(socle::mois::Mois::etiquette).map_err(valeur)?;
    let sortie = ctx.racine.join("sorties").join(mois);
    let dossier = sortie.join("envoi");
    std::fs::create_dir_all(&dossier).map_err(|e| os(&dossier, e))?;
    let part = |p: &Paie| cles.contains(&cle_de(p));
    let mut a_poser: HashMap<usize, (Option<String>, Option<(i64, i64)>)> = HashMap::new();

    // LES BULLETINS : un numero par paie qui a des cachets, ecrit dans la paie a son depart.
    // Le numero de chaque bulletin, a cote de sa paie : celui que la paie ecrite porte, sinon celui que ce depart reserve.
    // Les numeros neufs se donnent dans l'ordre ou les documents partent (`cles`), salarie par salarie : ceux d'un salarie
    // se suivent, et un envoi coupe en route laisse aux servis les premiers, sans trou.
    let mut numeros: Vec<Option<String>> = vec![None; paies.len()];
    let mut numerotees: Vec<bool> = vec![false; paies.len()];
    for i in 0..paies.len() {
        if paies[i].engagement.sans_paie() {
            continue;
        }
        if let Some(b) = ecrits.get(i).and_then(|e| e.bulletin.clone()) {
            numeros[i] = Some(b);
            numerotees[i] = true;
        }
    }
    for cle in cles {
        for i in (0..paies.len()).filter(|i| cle_de(&paies[*i]) == *cle) {
            if paies[i].engagement.sans_paie() || numerotees[i] {
                continue;
            }
            let n = faits.reserves.prochain(ctx, "bulletin", an)?;
            faits.reserves.enregistrer(an, "bulletin", n);
            let numero = format!("{}/{}", n, an);
            numeros[i] = Some(numero.clone());
            a_poser.entry(i).or_default().0 = Some(numero);
            numerotees[i] = true;
        }
    }
    // LE RECTIFICATIF D'UNE PAIE PARTIE PUIS REFAITE. Tant que la DSN du mois se refait, il remplace le bulletin parti ;
    // quand elle ne se refait plus (`se_corrige_ailleurs`), il porte l'ecart avec ce bulletin, verse avec la paie du mois
    // dont la DSN declare la correction (`ecart_avec_le_bulletin_parti`).
    let clos = regularisations::se_corrige_ailleurs(ctx, &departs, mois)?;
    let valides: Vec<engagement::etats::Valide> = engagement::etats::du_mois(ctx, mois, &departs).map_err(Echec::from)?.iter().filter_map(|e| e.valide().cloned()).collect();
    let mut rectificatifs: HashMap<usize, Value> = HashMap::new();
    for i in 0..paies.len() {
        let Some(r) = ecrits.get(i).and_then(|e| e.rectifie.clone()) else { continue };
        let parti = match (clos, valides.get(i)) {
            (true, Some(v)) => engagement::etats::paie_partie(ctx, v.objet(), &departs, mois).map_err(Echec::from)?.map(|p| (p, v)),
            _ => None,
        };
        let rectif = match parti {
            Some((parti, v)) => {
                let porteur = crate::regularisation::dsn_qui_porte_la_correction(ctx, v, &departs, mois)?;
                let mut e = ecart_avec_le_bulletin_parti(ctx, &paies[i], &parti, &porteur)?;
                e["rectifie"] = json!(r);
                e["mois_origine"] = json!(socle::mois::Mois::en_clair(mois));
                e
            }
            None => json!({"remplace": true, "rectifie": r, "mois_origine": socle::mois::Mois::en_clair(mois)}),
        };
        rectificatifs.insert(i, rectif);
    }

    // LES CERTIFICATS CONGES SPECTACLES : un par contrat qui finit ce mois-ci, numerotes dans la sequence continue, dans
    // l'ordre ou les documents partent, salarie par salarie, comme les bulletins.
    let (mut pour_cs, avert_cs) = crate::routes::dossier_salarie::certificats_dus(ctx, &paies)?;
    faits.avertissements.extend(avert_cs);
    pour_cs.sort_by_key(|(cle, _, _)| cles.iter().position(|c| c == cle).unwrap_or(usize::MAX));
    let mut prochain_ce: Option<i64> = None;
    let mut certificats: Vec<(usize, &PaieCs, i64)> = Vec::new();
    for (cle, debut, pc) in &pour_cs {
        let Some(pc) = pc else { continue };
        let Some(i) = (0..paies.len()).find(|i| cle_de(&paies[*i]) == *cle && paies[*i].engagement.debut.iso() == *debut) else { continue };
        let n = match ecrits.get(i).and_then(|e| e.premier_ce) {
            Some(n) => n,
            None if part(&paies[i]) => {
                let n = match prochain_ce {
                    Some(n) => n,
                    None => documents::ecrire_documents::prochain_continu(ctx, "certificat_cs", 1)?,
                };
                let combien = conges_spectacles::decouper(&pc.engagement)?.len() as i64;
                prochain_ce = Some(n + combien);
                faits.reserves.enregistrer(an, "certificat_cs", n + combien - 1);
                a_poser.entry(i).or_default().1 = Some((n, n + combien - 1));
                if ctx.salarie(cle).map_err(valeur)?.nir.texte().trim().is_empty() {
                    faits.avertissements.push(format!(
                        "{} {} : certificat Conges Spectacles emis SANS numero de securite sociale -- Audiens ne pourra pas le rattacher.",
                        paies[i].salarie.nom.texte().to_string(),
                        prenom_de(&paies[i].salarie)
                    ));
                }
                n
            }
            None => continue,
        };
        certificats.push((i, pc, n));
    }

    // LE BULLETIN D'ORIGINE RESTE : une paie refaite part sous un nouveau numero, et ce qui est parti se garde.
    for (i, p) in paies.iter().enumerate().filter(|(i, p)| part(p) && numerotees[*i]) {
        if let (Some(r), true) = (ecrits[i].rectifie.as_deref(), a_poser.get(&i).is_some_and(|x| x.0.is_some())) {
            let cle = cle_de(p);
            let piece = dossier.join(envois::envoi::nom_fichier("Bulletin", &cle, &etiq));
            let garde = sortie.join(format!("Bulletin_{}_{}_{}.pdf", cle, etiq, numero_en_nom(r)));
            if piece.exists() && !garde.exists() {
                std::fs::copy(&piece, &garde).map_err(|e| os(&garde, e))?;
            }
        }
    }
    let impression = Impression { paies: &paies, numeros: &numeros, rectificatifs: &rectificatifs, certificats: &certificats };
    imprimer(ctx, mois, &ent, &impression, cles, false)?;
    let mut poses: Vec<(usize, (Option<String>, Option<(i64, i64)>))> = a_poser.into_iter().collect();
    poses.sort_by_key(|(i, _)| *i);
    faits.a_poser = poses.into_iter().map(|(i, (b, ce))| (cle_de(&paies[i]), paies[i].engagement.numero.as_deref().unwrap_or_default().to_string(), b, ce)).collect();
    Ok(faits)
}

/// Ce qu'un envoi de documents imprime : les paies du mois, au meme rang le numero de bulletin de chacune (`None` : elle
/// n'en a pas), le rectificatif de celles qui en sont un, et les certificats Conges Spectacles numerotes (rang de la paie,
/// certificat, premier N CE).
struct Impression<'a> {
    paies: &'a [Paie],
    numeros: &'a [Option<String>],
    rectificatifs: &'a HashMap<usize, Value>,
    certificats: &'a [(usize, &'a PaieCs, i64)],
}

/// LES DOCUMENTS D'UN ENVOI, ECRITS : un `Bulletin_<cle>_<Mois>.pdf` (tous ses bulletins du mois) et un
/// `CertificatCS_<cle>_<Mois>.pdf` par salarie de `cles`, dans `sorties/<mois>/envoi/`, et l'archive du mois
/// (`Bulletins_<Mois>.pdf`, `CertificatsCS_<Mois>.pdf` et son recapitulatif), qui porte tous les documents numerotes.
/// `absents` : seuls les fichiers qui manquent s'ecrivent (une piece partie refaite pour se lire).
fn imprimer(ctx: &Contexte, mois: &str, ent: &droit::contexte::EntrepriseDuDocument, imp: &Impression, cles: &[String], absents: bool) -> R<()> {
    let etiq = socle::mois::Mois::lire(mois).map(socle::mois::Mois::etiquette).map_err(valeur)?;
    let sortie = ctx.racine.join("sorties").join(mois);
    let dossier = sortie.join("envoi");
    std::fs::create_dir_all(&dossier).map_err(|e| os(&dossier, e))?;
    let a_ecrire = |f: &Path| !(absents && f.exists());
    let paies = imp.paies;
    let numerotee = |i: usize| imp.numeros.get(i).is_some_and(Option::is_some);
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;

    // UN JEU DE DOCUMENTS PAR SALARIE QUI LES RECOIT, et l'archive du mois : tous ses documents numerotes.
    let mut destinataires: Vec<String> = Vec::new();
    for p in paies.iter().filter(|p| cles.contains(&cle_de(p))) {
        if !destinataires.contains(&cle_de(p)) {
            destinataires.push(cle_de(p));
        }
    }
    let siennes = |cle: &str| -> Vec<usize> { (0..paies.len()).filter(|i| cle_de(&paies[*i]) == cle && numerotee(*i)).collect() };
    let piece_de = |cle: &str| dossier.join(envois::envoi::nom_fichier("Bulletin", cle, &etiq));
    let archivees: Vec<usize> = (0..paies.len()).filter(|i| numerotee(*i)).collect();
    let archive = sortie.join(format!("Bulletins_{}.pdf", etiq));
    let archive_a_ecrire = !archivees.is_empty() && a_ecrire(&archive);

    // Ce que les bulletins cumulent se lit une fois, pour tous les salaries qu'ils impriment.
    let mut imprimees: Vec<usize> = destinataires.iter().filter(|c| a_ecrire(&piece_de(c))).flat_map(|c| siennes(c)).collect();
    if archive_a_ecrire {
        imprimees.extend(&archivees);
    }
    imprimees.sort_unstable();
    imprimees.dedup();
    let leurs_paies: Vec<Paie> = imprimees.iter().map(|i| paies[*i].clone()).collect();
    let passe = documents::bulletin::passe_des_bulletins(ctx, &leurs_paies, mois, &departs).map_err(valeur)?;
    let bulletins = |rangs: &[usize], toutes: Option<&[Paie]>| -> R<Value> {
        let imprimees: Vec<Paie> = rangs.iter().map(|i| paies[*i].clone()).collect();
        let rectifs: HashMap<usize, Value> = rangs.iter().enumerate().filter_map(|(k, i)| imp.rectificatifs.get(i).map(|r| (k, r.clone()))).collect();
        let leurs_numeros: Vec<Option<String>> = rangs.iter().map(|i| imp.numeros[*i].clone()).collect();
        documents::bulletin::jeux_et_description(ctx, &imprimees, &leurs_numeros, ent, mois, toutes, false, &rectifs, &departs, &passe).map_err(valeur)
    };

    for cle in &destinataires {
        let siennes = siennes(cle);
        let piece = piece_de(cle);
        if !siennes.is_empty() && a_ecrire(&piece) {
            documents::fabrique::ecrire_pdf(&bulletins(&siennes, Some(paies))?, &piece)?;
        }
        let ses_certificats: Vec<(&PaieCs, i64)> = imp.certificats.iter().filter(|(i, _, _)| cle_de(&paies[*i]) == *cle).map(|(_, pc, n)| (*pc, *n)).collect();
        let piece = dossier.join(envois::envoi::nom_fichier("CertificatCS", cle, &etiq));
        if !ses_certificats.is_empty() && a_ecrire(&piece) {
            let description = conges_spectacles::description_numerotee(&ses_certificats, ent).map_err(Echec::from)?;
            documents::fabrique::ecrire_pdf(&description, &piece)?;
        }
    }

    if archive_a_ecrire {
        documents::fabrique::ecrire_pdf(&bulletins(&archivees, None)?, &archive)?;
    }
    if !imp.certificats.is_empty() {
        let mut paires: Vec<(&PaieCs, i64)> = imp.certificats.iter().map(|(_, pc, n)| (*pc, *n)).collect();
        paires.sort_by_key(|(_, n)| *n);
        let archive = sortie.join(format!("CertificatsCS_{}.pdf", etiq));
        if a_ecrire(&archive) {
            let description = conges_spectacles::description_numerotee(&paires, ent).map_err(Echec::from)?;
            documents::fabrique::ecrire_pdf(&description, &archive)?;
        }
        let recap = sortie.join(format!("CertificatsCS_{}_recapitulatif.txt", etiq));
        if a_ecrire(&recap) {
            let texte = conges_spectacles::recapitulatif(&paires, ent).map_err(Echec::from)?;
            std::fs::write(&recap, texte).map_err(|e| os(&recap, e))?;
        }
    }
    Ok(())
}

// ------------------------------------------------------------- une piece partie, refaite pour se lire

/// UNE PIECE PARTIE, pour la lire : son fichier dans `sorties/<mois>/` ou son `envoi/` ; s'il manque, refait a
/// l'identique depuis ce que le journal dit d'elle -- la ligne qui l'a fait partir, les versions des engagements qu'elle
/// a emportees, le jour ou elle a ete fabriquee -- et remis a sa place. Regle 4 : refaite pour se lire, jamais pour
/// repartir ; ce qui se redepose se fabrique a son depart (`fabriquer_dsn`, `fabriquer_documents`).
///
/// Se refont : la DSN mensuelle (`dsn_partie`, et l'empreinte que le journal a notee a son depot le verifie), les
/// bulletins, rectificatifs compris, et les certificats Conges Spectacles d'un envoi de documents, avec l'archive du mois.
/// Ce qui vient d'un organisme (l'AER, un bilan, un compte rendu) ne se refait pas : il se lit dans `retours/`.
/// Refus : un nom qui n'est pas celui d'une piece partie de `mois`, une piece qui ne se refait pas, une piece refaite
/// qui ne dit pas ce qui est parti.
pub(crate) fn piece_partie(ctx: &Contexte, mois: &str, nom: &str) -> R<PathBuf> {
    let refus = |m: String| erreur("ErreurOperation", m);
    if [mois, nom].iter().any(|t| t.is_empty() || t.contains(['/', '\\']) || t.contains("..")) {
        return Err(refus(format!("{} n'est pas une piece de {}.", nom, mois)));
    }
    if let Some(f) = envois::etats::chemin_piece(&ctx.racine, &json!(mois), &json!(nom)) {
        return Ok(f);
    }
    let mut m = Moteur::nouveau(ctx);
    let lignes = m.lignes()?;
    let texte = |l: &Value, k: &str| l.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    let du_mois = |l: &Value| texte(l, "mois") == mois && vrai(l.get("accepte").unwrap_or(&Value::Null));
    let etiq = socle::mois::Mois::lire(mois).map(socle::mois::Mois::etiquette).map_err(valeur)?;
    let sortie = ctx.racine.join("sorties").join(mois);

    // LA DSN MENSUELLE : la derniere ligne qui l'a deposee, le reel devant son test.
    let deposee = |reel: bool| lignes.iter().rposition(|l| texte(l, "quoi") == "dsn" && texte(l, "fichier") == nom && du_mois(l) && (texte(l, "mode") == "reel") == reel);
    if crate::routes::etapes::est_une_dsn_mensuelle(nom) {
        let Some(rang) = deposee(true).or_else(|| deposee(false)) else {
            return Err(refus(format!("{} : le journal ne dit pas qu'elle est partie.", nom)));
        };
        refaire_la_dsn(ctx, &mut m, &lignes, rang, &sortie.join(nom))?;
        return Ok(sortie.join(nom));
    }

    // LES DOCUMENTS D'UN ENVOI AUX SALARIES, et l'archive du mois.
    let des_documents = nom.starts_with("Bulletin_")
        || nom.starts_with("CertificatCS_")
        || [format!("Bulletins_{}.pdf", etiq), format!("CertificatsCS_{}.pdf", etiq), format!("CertificatsCS_{}_recapitulatif.txt", etiq)].contains(&nom.to_string());
    if des_documents {
        let envois: Vec<usize> = (0..lignes.len()).filter(|r| texte(&lignes[*r], "quoi") == "documents" && du_mois(&lignes[*r])).collect();
        // LE BULLETIN D'ORIGINE GARDE A COTE DE SON RECTIFICATIF (`Bulletin_<cle>_<Mois>_<N-AAAA>.pdf`, ou `N/AAAA` est le
        // numero rectifie) : la piece du salarie telle que l'a ecrite le dernier envoi ou son bulletin portait ce numero.
        let garde = nom.strip_prefix("Bulletin_").and_then(|r| r.strip_suffix(".pdf")).and_then(|r| r.rsplit_once('_')).and_then(|(tete, n)| {
            let cle = tete.strip_suffix(&format!("_{}", etiq))?;
            n.split_once('-').filter(|(a, b)| a.chars().all(|c| c.is_ascii_digit()) && b.len() == 4).map(|(a, b)| (cle.to_string(), format!("{}/{}", a, b)))
        });
        if let Some((cle, numero)) = garde {
            let piece = sortie.join("envoi").join(envois::envoi::nom_fichier("Bulletin", &cle, &etiq));
            for rang in envois.iter().rev() {
                if !refaire_les_documents(ctx, mois, &lignes, *rang, false)?.iter().any(|(c, n)| *c == cle && n.as_deref() == Some(numero.as_str())) {
                    continue;
                }
                // Cet envoi ne refait que cette piece : ce qu'il ecrirait d'autre n'est pas le dernier etat parti.
                let fichiers = || -> Vec<PathBuf> {
                    [sortie.clone(), sortie.join("envoi")].iter().flat_map(|d| std::fs::read_dir(d).into_iter().flatten().flatten().map(|e| e.path())).filter(|p| p.is_file()).collect()
                };
                let avant = fichiers();
                let de_cote = piece.with_extension("pdf.presente");
                let presente = piece.exists();
                if presente {
                    std::fs::rename(&piece, &de_cote).map_err(|e| os(&piece, e))?;
                }
                let fait = refaire_les_documents(ctx, mois, &lignes, *rang, true).and_then(|_| std::fs::rename(&piece, sortie.join(nom)).map_err(|e| os(&piece, e)));
                for f in fichiers().into_iter().filter(|f| !avant.contains(f) && *f != sortie.join(nom)) {
                    let _ = std::fs::remove_file(f);
                }
                if presente {
                    std::fs::rename(&de_cote, &piece).map_err(|e| os(&de_cote, e))?;
                }
                fait?;
                return Ok(sortie.join(nom));
            }
            return Err(refus(format!("{} : aucun envoi de documents de {} n'a fait partir le bulletin n° {}.", nom, socle::mois::Mois::en_clair(mois), numero)));
        }
        // Du dernier envoi au premier : le fichier est celui que le dernier envoi qui le porte a ecrit.
        for rang in envois.into_iter().rev() {
            refaire_les_documents(ctx, mois, &lignes, rang, true)?;
        }
        return envois::etats::chemin_piece(&ctx.racine, &json!(mois), &json!(nom))
            .ok_or_else(|| refus(format!("{} : aucun envoi de documents de {} ne l'a fait partir.", nom, socle::mois::Mois::en_clair(mois))));
    }
    // LA DPAE : son enregistrement, son `.zip` et son recapitulatif, d'apres la derniere ligne qui l'a deposee.
    if nom.starts_with("DPAE_") {
        let base = nom.strip_suffix("_recapitulatif.txt").map(|b| format!("{}.txt", b)).or_else(|| nom.strip_suffix(".zip").map(String::from)).unwrap_or_else(|| nom.to_string());
        let deposee = |reel: bool| lignes.iter().rposition(|l| texte(l, "quoi") == "dpae" && texte(l, "fichier") == base && du_mois(l) && (texte(l, "mode") == "reel") == reel);
        let Some(rang) = deposee(true).or_else(|| deposee(false)) else {
            return Err(refus(format!("{} : le journal ne dit pas qu'elle est partie.", nom)));
        };
        refaire_la_dpae(ctx, mois, &lignes[rang], &sortie.join(&base))?;
        return Ok(sortie.join(nom));
    }
    // LES SIGNALEMENTS (amorcage, arret, reprise, fin de contrat) : leur ligne porte le jour de l'evenement, rangee dans
    // le mois de ce jour.
    let du_jour = |l: &Value| texte(l, "mois").get(..7) == Some(mois) && vrai(l.get("accepte").unwrap_or(&Value::Null));
    let signale = |reel: bool| lignes.iter().rposition(|l| texte(l, "quoi") == "dsn" && texte(l, "fichier") == nom && du_jour(l) && (texte(l, "mode") == "reel") == reel);
    if nom.ends_with(".dsn") {
        if let Some(rang) = signale(true).or_else(|| signale(false)) {
            refaire_le_signalement(ctx, mois, &lignes[rang], &sortie.join(nom))?;
            return Ok(sortie.join(nom));
        }
    }
    // LE CONTRAT ET L'AVENANT : la derniere ligne qui les a fait partir.
    if nom.starts_with("Contrat") || nom.starts_with("Avenant") {
        // L'EXEMPLAIRE CONTRESIGNE : le contrat parti, plus la mention que sa signature a notee (`contresigner`).
        let signe = lignes.iter().rev().filter(|l| texte(l, "mois") == mois).flat_map(|l| envois::etats::revenus(l).unwrap_or_default()).find(|r| texte(r, "fichier_signe").split(", ").any(|f| f == nom));
        if let Some(r) = signe {
            let (Some(envoye), mention) = (nom.strip_suffix(envois::releve_signatures::SUFFIXE_CONTRESIGNE), texte(&r, "mention")) else {
                return Err(refus(format!("{} : la signature notée ne dit pas quel contrat parti elle signe.", nom)));
            };
            if mention.is_empty() {
                return Err(refus(format!("{} : la signature notée ne dit pas la mention de l'artiste ; il ne se refait pas.", nom)));
            }
            let pdf = sortie.join("envoi").join(nom);
            contresigner(ctx, &lignes, &texte(&r, "code"), &sortie.join("envoi").join(format!("{}.pdf", envoye)), &pdf, &mention)?;
            return Ok(pdf);
        }
        let porte = |l: &Value| l["detail"]["contrats"].as_array().is_some_and(|cs| cs.iter().any(|c| c["fichier"] == json!(nom)));
        if let Some(rang) = lignes.iter().rposition(|l| texte(l, "quoi") == "contrat-signature" && du_mois(l) && porte(l)) {
            let pdf = sortie.join("envoi").join(nom);
            // Parti par courriel, il a son empreinte : le refait la redonne, ou il ne s'ecrit pas ; imprime, ses clauses le gardent.
            let empreinte = lignes[rang]["detail"]["contrats"].as_array().into_iter().flatten().find(|c| c["fichier"] == json!(nom)).and_then(|c| c.get("empreinte")).and_then(Value::as_str);
            match empreinte {
                Some(e) => std::fs::write(&pdf, contrat_parti(ctx, &lignes, rang, &pdf, e)?).map_err(|e| os(&pdf, e))?,
                None => refaire_le_contrat(ctx, &lignes, rang, &pdf, &pdf)?,
            }
            return Ok(pdf);
        }
    }
    if nom.starts_with("Amorcage") && nom.ends_with("_recapitulatif.txt") {
        return Err(refus(format!(
            "{} est la note qui accompagnait les signalements d'amorçage au moment de les déposer : elle n'est pas partie et ne se refait pas ; les signalements partis s'ouvrent, eux.",
            nom
        )));
    }
    // L'AER vient de France Travail : elle ne se refait pas, elle se relit dans `retours/` (l'archive du retour 46) et se
    // range de nouveau comme a sa releve (`routes::documents::ranger_aer_detail`), signee au jour de cette releve.
    if nom.starts_with("AER_") {
        let mut archives: Vec<PathBuf> = std::fs::read_dir(ctx.racine.join(envois::retours::RETOURS))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && socle::fichiers::nom_du(p).contains("_nature46_"))
            .collect();
        archives.sort();
        for a in archives {
            let n = socle::fichiers::nom_du(&a);
            let quand = match (n.get(..4), n.get(4..6), n.get(6..8), n.get(9..11), n.get(11..13), n.get(13..15)) {
                (Some(an), Some(mo), Some(j), Some(h), Some(mi), Some(s)) => format!("{}-{}-{}T{}:{}:{}", an, mo, j, h, mi, s),
                _ => continue,
            };
            let Some((jour, instant)) = instant_de(&quand) else { continue };
            let ctx_j = contexte_du_jour(ctx, jour)?;
            socle::temps::pendant(instant, || crate::routes::documents::ranger_aer_detail(&ctx_j, Some(mois), Some(std::slice::from_ref(&a)), false))?;
            if let Some(f) = envois::etats::chemin_piece(&ctx.racine, &json!(mois), &json!(nom)) {
                return Ok(f);
            }
        }
        return Err(refus(format!(
            "{} vient de France Travail : elle ne se refait pas, elle se relit dans retours/, où son retour (nature 46) manque ; le fichier a été déplacé ou effacé.",
            nom
        )));
    }
    Err(refus(format!("{} n'est plus dans sorties/{} : le journal dit qu'il est parti, le fichier a été déplacé ou effacé.", nom, mois)))
}

/// Le contexte de la compagnie au jour `jour`, ce jour-la etant aujourd'hui : celui de la route qui a fabrique une piece.
fn contexte_du_jour(ctx: &Contexte, jour: socle::date::Jour) -> R<Contexte> {
    let mut c = Contexte::nouveau(&ctx.racine, Some(jour)).map_err(valeur)?;
    c.aujourdhui = jour;
    Ok(c)
}

/// L'instant `AAAA-MM-JJTHH:MM[:SS]` (heure de Paris), en secondes UTC, et son jour.
fn instant_de(quand: &str) -> Option<(socle::date::Jour, f64)> {
    let (jour, heure) = quand.split_once('T')?;
    let jour = socle::date::Jour::lire(jour).ok()?;
    let mut hms = heure.split(':').map(|x| x.parse::<i64>().ok());
    let (h, m, s) = (hms.next()??, hms.next().flatten().unwrap_or(0), hms.next().flatten().unwrap_or(0));
    Some((jour, socle::temps::instant_de(jour, h) + (m * 60 + s) as f64))
}

/// LA DSN MENSUELLE QUE DIT LA LIGNE `lignes[rang]`, refaite (`dsn_partie`) au jour ou son fichier a ete ecrit (le journal
/// des ecritures), et ecrite a `cible` si ses octets ont l'empreinte que le journal a notee a son depot.
fn refaire_la_dsn(ctx: &Contexte, m: &mut Moteur, lignes: &[Value], rang: usize, cible: &Path) -> R<()> {
    let l = &lignes[rang];
    let mois = valeur_en_texte(l.get("mois").unwrap_or(&Value::Null));
    let nic = l.pointer("/detail/etablissement").filter(|v| vrai(v)).map(valeur_en_texte);
    let plusieurs = !ctx.entreprise_du_mois(&mois).map_err(valeur)?.secondaires().is_empty();
    let nic_v = match (plusieurs, nic) {
        (true, Some(n)) => Value::String(n),
        (true, None) => Value::String(ctx.entreprise.nic_principal()),
        (false, _) => Value::Null,
    };
    // Un annule-et-remplace : la DSN qu'il remplacait versait-elle a la DGFiP ?
    let mut avant: Vec<(i64, usize)> = Vec::new();
    for x in m.depots_reels(socle::mois::Mois::lire(&mois)?, &nic_v)? {
        if let Some(r) = lignes.iter().position(|y| *y == x).filter(|r| *r < rang) {
            avant.push((m.ordre_de_la_ligne(socle::mois::Mois::lire(&mois)?, &x)?, r));
        }
    }
    avant.sort();
    let type_dsn = l.pointer("/detail/type").map(valeur_en_texte).unwrap_or_default();
    let versait = match avant.last() {
        Some((_, r)) if type_dsn == "03" || type_dsn == "05" => Some(verse_a_la_dgfip(&dsn_partie(ctx, m, lignes, *r, None)?)),
        _ => None,
    };
    let f = dsn_partie(ctx, m, lignes, rang, versait)?;
    let octets = declarations::dsn::en_latin1(&f.texte).map_err(valeur)?;
    let attendue = valeur_en_texte(l.get("empreinte").unwrap_or(&Value::Null));
    if envois::journal::empreinte(&octets) != attendue {
        return Err(erreur(
            "ErreurOperation",
            format!("{} ne se refait pas à l'identique de ce qui est parti : le fichier a été déplacé ou effacé.", socle::fichiers::nom_du(cible)),
        ));
    }
    if let Some(parent) = cible.parent() {
        std::fs::create_dir_all(parent).map_err(|e| os(parent, e))?;
    }
    std::fs::write(cible, &octets).map_err(|e| os(cible, e))
}

/// QUAND LA PIECE `nom` DE `mois` A ETE ECRITE : le journal des ecritures, sinon l'instant de la ligne `l` qui l'a
/// fait partir ; `(jour, instant, AAAA-MM-JJTHH:MM:SS)`.
fn ecrite_a(ctx: &Contexte, mois: &str, nom: &str, l: &Value) -> R<(socle::date::Jour, f64, String)> {
    let quand = journal_des_ecritures(&ctx.racine)
        .get(&format!("{}/{}", mois, nom))
        .and_then(|n| n.get("quand"))
        .map(valeur_en_texte)
        .unwrap_or_else(|| valeur_en_texte(l.get("quand").unwrap_or(&Value::Null)));
    let (jour, instant) = instant_de(&quand).ok_or_else(|| valeur(format!("journal : {} sans date d'ecriture ({})", nom, quand)))?;
    let iso = socle::paris::iso_secondes(instant);
    Ok((jour, instant, iso))
}

/// Les paies que la ligne `l` a emportees (`versions`), chacune dans la version sur laquelle elle est partie, vues en
/// `mois`.
fn paies_emportees(ctx: &Contexte, mois: &str, l: &Value) -> R<Vec<Paie>> {
    let ctx_m = ctx.du_mois(mois).map_err(valeur)?;
    let mut out = Vec::new();
    for v in l.get("versions").and_then(Value::as_array).into_iter().flatten() {
        let texte = |k: &str| v.get(k).map(valeur_en_texte).unwrap_or_default();
        let (cle, numero) = (texte("salarie"), texte("numero"));
        let version = v.get("version").and_then(Value::as_u64).ok_or_else(|| valeur(format!("journal : une version illisible ({})", cle)))?;
        let o = engagement_de(ctx, mois, &cle, &numero)?;
        out.push(paie_de_la_version(&ctx_m, &o, &cle, &numero, mois, version)?);
    }
    Ok(out)
}

/// Les fichiers `ecrits` se gardent s'ils ont l'empreinte que la ligne `l` a notee au depot du premier ; sinon ils
/// s'effacent et le refus le dit : une piece refaite qui ne dit pas ce qui est parti ne se montre pas.
fn garder_si_identique(l: &Value, ecrits: &[PathBuf]) -> R<()> {
    let attendue = valeur_en_texte(l.get("empreinte").unwrap_or(&Value::Null));
    let premier = &ecrits[0];
    let ok = std::fs::read(premier).ok().is_some_and(|o| envois::journal::empreinte(&o) == attendue);
    if ok {
        return Ok(());
    }
    for f in ecrits {
        let _ = std::fs::remove_file(f);
    }
    Err(erreur(
        "ErreurOperation",
        format!("{} ne se refait pas à l'identique de ce qui est parti : le fichier a été déplacé ou effacé.", socle::fichiers::nom_du(premier)),
    ))
}

/// LA DPAE QUE DIT LA LIGNE `l` (`cible` : son enregistrement), refaite par la fonction qui l'a ecrite
/// (`routes::depots::ecrire_la_dpae`) : les embauches qu'elle a emportees, dans leur version, horodatee de l'instant ou
/// elle a ete ecrite, au jour de cet instant ; l'heure d'embauche que la route prend quand on ne lui en donne pas.
/// Avec son `.zip` et son recapitulatif ; verifiee par l'empreinte que le journal a notee a son depot.
fn refaire_la_dpae(ctx: &Contexte, mois: &str, l: &Value, cible: &Path) -> R<()> {
    let nom = socle::fichiers::nom_du(cible);
    let (jour, instant, quand) = ecrite_a(ctx, mois, &nom, l)?;
    let ctx_j = contexte_du_jour(ctx, jour)?;
    let paies = paies_emportees(&ctx_j, mois, l)?;
    let retenues: Vec<&Paie> = paies.iter().collect();
    let base = nom.strip_suffix(".txt").unwrap_or(&nom).to_string();
    let recap = cible.with_file_name(format!("{}_recapitulatif.txt", base));
    let zip = cible.with_file_name(format!("{}.zip", nom));
    socle::temps::pendant(instant, || crate::routes::depots::ecrire_la_dpae(&ctx_j, &retenues, cible, &recap, &json!(declarations::dpae::HEURE_EMBAUCHE), &quand))?;
    garder_si_identique(l, &[cible.to_path_buf(), zip, recap])
}

/// LE SIGNALEMENT QUE DIT LA LIGNE `l` (amorcage 08, arret 04, reprise 05, fin de contrat 07), refait a `cible` par ce qui
/// l'a ecrit, sous le numero d'ordre que sa ligne porte, constitue le jour ou il a ete ecrit, a l'instant de son ecriture ;
/// verifie par l'empreinte que le journal a notee a son depot. Un annule-et-remplace ne se refait pas : sa ligne ne dit pas
/// ce qu'il remplacait.
fn refaire_le_signalement(ctx: &Contexte, mois: &str, l: &Value, cible: &Path) -> R<()> {
    let nom = socle::fichiers::nom_du(cible);
    let detail = l.get("detail").cloned().unwrap_or(Value::Null);
    let champ = |k: &str| detail.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    let (nature, type_dsn) = (champ("nature"), champ("type"));
    let ordre: i64 = champ("ordre").trim().parse().map_err(|_| valeur(format!("journal : {} sans numero d'ordre", nom)))?;
    if !type_dsn.is_empty() && type_dsn != "01" {
        return Err(erreur("ErreurOperation", format!("{} annulait et remplaçait un signalement : il ne se refait pas ; le fichier a été déplacé ou effacé.", nom)));
    }
    let (jour, instant, _) = ecrite_a(ctx, mois, &nom, l)?;
    let ctx_j = contexte_du_jour(ctx, jour)?;
    let premier = l.pointer("/versions/0/salarie").map(valeur_en_texte).unwrap_or_default();
    let evenement = valeur_en_texte(l.get("mois").unwrap_or(&Value::Null));
    let refaire = || -> R<()> {
        match nature.as_str() {
            declarations::amorcage::NATURE => {
                let motif = nom.strip_prefix("Amorcage").and_then(|r| r.get(..2)).unwrap_or_default().to_string();
                let paies = paies_emportees(&ctx_j, mois, l)?;
                let paie = paies.first().ok_or_else(|| valeur(format!("journal : {} n'emporte aucun engagement", nom)))?;
                let options = declarations::amorcage::Options {
                    motif,
                    type_declaration: declarations::amorcage::NORMALE.to_string(),
                    ordre,
                    remplace: None,
                    envoi_test: false,
                    date_constitution: Some(jour),
                };
                let texte = declarations::amorcage::generer(&ctx_j, paie, &options, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?).map_err(|e| valeur(e.message().to_string()))?;
                declarations::dsn::ecrire(&texte, cible).map_err(valeur)
            }
            declarations::fctu::NATURE => crate::routes::imprevus::signaler_fin(&ctx_j, &json!({"mois": mois, "salarie": premier, "ordre": ordre, "le": jour.iso()}), true).map(|_| ()),
            n => {
                // L'arret tel qu'il etait a l'ecriture : celui de ses etats d'alors que l'empreinte reconnait.
                let dernier = crate::routes::imprevus::dernier_jour_saisi(&ctx_j, &premier, &evenement)?;
                let charge = json!({"salarie": premier, "dernier_jour": dernier.iso(), "nature": n, "ordre": ordre, "le": jour.iso()});
                let mut rendu = Ok(());
                for a in crate::routes::imprevus::etats_d_alors(&ctx_j, &premier, &dernier, &crate::routes::maintenant())? {
                    crate::routes::imprevus::signaler_arret(&ctx_j, &charge, &a)?;
                    rendu = garder_si_identique(l, &[cible.to_path_buf()]);
                    if rendu.is_ok() {
                        break;
                    }
                }
                rendu
            }
        }
    };
    socle::temps::pendant(instant, refaire)?;
    garder_si_identique(l, &[cible.to_path_buf()])
}

/// L'EXEMPLAIRE CONTRESIGNE `signe` DU CONTRAT (OU DE L'AVENANT) `envoye` QU'A FAIT PARTIR LA DEMANDE DE SIGNATURE `code` :
/// le PDF parti, octet pour octet (l'empreinte que la demande a notee), plus la mention de l'artiste dans sa case, par une
/// mise a jour incrementale (`fabrique_pdf::marquage::contresigner`). On ne contresigne que ce qui est parti : le PDF se
/// lit ou il est ; s'il manque ou a change, il se refait (`refaire_le_contrat`) et doit redonner l'empreinte notee, sinon
/// refus. Le fichier `envoye` n'est jamais reecrit.
pub(crate) fn contresigner(ctx: &Contexte, lignes: &[Value], code: &str, envoye: &Path, signe: &Path, mention: &str) -> R<()> {
    let nom = socle::fichiers::nom_du(envoye);
    let refus = |m: String| erreur("ErreurOperation", m);
    let entree = |l: &Value| -> Option<Value> {
        (l.get("quoi").and_then(Value::as_str) == Some("contrat-signature"))
            .then(|| l["detail"]["contrats"].as_array().and_then(|cs| cs.iter().find(|c| c["fichier"] == json!(nom) && c["code"] == json!(code)).cloned()))
            .flatten()
    };
    let Some(rang) = lignes.iter().rposition(|l| entree(l).is_some()) else {
        return Err(refus(format!("{} : aucune demande de signature ne porte le code {}.", nom, code)));
    };
    let Some(empreinte) = entree(&lignes[rang]).and_then(|c| c.get("empreinte").and_then(Value::as_str).map(String::from)) else {
        return Err(refus(format!("{} est parti sans son empreinte : rien ne dit que c'est lui qu'on contresigne. Relancer la demande de signature.", nom)));
    };
    let octets = contrat_parti(ctx, lignes, rang, envoye, &empreinte)?;
    let contresigne = fabrique_pdf::marquage::contresigner(&octets, mention).map_err(|r| erreur("FabriqueRefus", format!("{} : {}", nom, r.0)))?;
    if let Some(parent) = signe.parent() {
        std::fs::create_dir_all(parent).map_err(|e| os(parent, e))?;
    }
    std::fs::write(signe, contresigne).map_err(|e| os(signe, e))
}

/// LES OCTETS DU CONTRAT (OU DE L'AVENANT) `envoye` TELS QU'ILS SONT PARTIS, d'empreinte `empreinte` (notee par la
/// demande de signature `lignes[rang]`) : le fichier, s'il a cette empreinte ; sinon refait a cote, par la ligne qui
/// l'a ecrit (celle-ci, ou une plus ancienne qui a fait partir les memes octets : la relance d'un avenant reprend le
/// PDF parti). Refus quand rien ne redonne l'empreinte. Le fichier `envoye` n'est pas reecrit.
fn contrat_parti(ctx: &Contexte, lignes: &[Value], rang: usize, envoye: &Path, empreinte: &str) -> R<Vec<u8>> {
    if let Ok(o) = std::fs::read(envoye) {
        if envois::journal::empreinte(&o) == empreinte {
            return Ok(o);
        }
    }
    let nom = socle::fichiers::nom_du(envoye);
    let refait = envoye.with_extension("pdf.refait");
    let a_empreinte = |l: &Value| {
        l.get("quoi").and_then(Value::as_str) == Some("contrat-signature")
            && l["detail"]["contrats"].as_array().is_some_and(|cs| cs.iter().any(|c| c["fichier"] == json!(nom) && c["empreinte"] == json!(empreinte)))
    };
    for r in (0..=rang).rev().filter(|r| a_empreinte(&lignes[*r])) {
        let fait = refaire_le_contrat(ctx, lignes, r, envoye, &refait).and_then(|_| std::fs::read(&refait).map_err(|e| os(&refait, e)));
        let _ = std::fs::remove_file(&refait);
        let o = fait?;
        if envois::journal::empreinte(&o) == empreinte {
            return Ok(o);
        }
    }
    Err(erreur("ErreurOperation", format!("{} ne se refait pas à l'identique de ce qui est parti : le fichier parti a été déplacé ou effacé.", nom)))
}

/// LE CONTRAT OU L'AVENANT `pdf` QU'A FAIT PARTIR LA LIGNE `lignes[rang]` (une demande de signature ou une impression),
/// ecrit a `vers`. Il est
/// refait par ce qui l'a ecrit (`documents::contrat::ecrire_contrat`, `ecrire_avenant`) :
/// chaque contrat dans la version de l'engagement que la ligne a emportee, au jour et a l'heure de son depart ; un contrat
/// qui en remplacait un autre le dit en tete, d'apres le contrat parti avant lui ; un avenant cite ce que sa ligne a
/// note (articles modifies, effet), date du contrat qu'il modifie. Verifie par les clauses que la ligne a notees de chaque
/// contrat : la piece refaite qui ne les redit pas s'efface et se refuse.
pub(crate) fn refaire_le_contrat(ctx: &Contexte, lignes: &[Value], rang: usize, pdf: &Path, vers: &Path) -> R<()> {
    let l = &lignes[rang];
    let nom = socle::fichiers::nom_du(pdf);
    let quand = valeur_en_texte(l.get("quand").unwrap_or(&Value::Null));
    let (jour, instant) = instant_de(&quand).ok_or_else(|| valeur(format!("journal : un depart de contrat sans date ({})", quand)))?;
    let ctx_j = contexte_du_jour(ctx, jour)?;
    let contrats: Vec<&Value> = l["detail"]["contrats"].as_array().into_iter().flatten().filter(|c| c["fichier"] == json!(nom)).collect();
    let texte = |v: &Value, k: &str| v.get(k).filter(|x| vrai(x)).map(valeur_en_texte).unwrap_or_default();
    let mut paies: Vec<Paie> = Vec::new();
    for c in &contrats {
        let (cle, numero, mois_c) = (texte(c, "salarie"), texte(c, "numero"), texte(c, "mois"));
        let version = l["versions"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|v| texte(v, "salarie") == cle && texte(v, "numero") == numero)
            .and_then(|v| v.get("version").and_then(Value::as_u64))
            .ok_or_else(|| valeur(format!("journal : le contrat {} est parti sans sa version", numero)))?;
        let o = engagement_de(&ctx_j, &mois_c, &cle, &numero)?;
        paies.push(paie_de_la_version(&ctx_j.du_mois(&mois_c).map_err(valeur)?, &o, &cle, &numero, &mois_c, version)?);
    }
    let (Some(premier), Some(p0)) = (contrats.first(), paies.first()) else {
        return Err(valeur(format!("journal : aucun contrat ne part dans {}", nom)));
    };
    let mois_c = texte(premier, "mois");
    let ent = documents::avenant::ent_du_mois(&ctx_j, &mois_c)?;
    let jj = |iso: &str| socle::date::Jour::lire(iso.get(..10).unwrap_or_default()).map(|j| j.jj_mm_aaaa()).map_err(valeur);
    let ecrire = || -> R<()> {
        if texte(premier, "acte") == "avenant" {
            let numero_av = premier.get("avenant").and_then(Value::as_i64).ok_or_else(|| valeur(format!("journal : {} sans numero d'avenant", nom)))?;
            // Le contrat qu'il modifie : parti avant lui, ou signe hors du logiciel (`ailleurs`), date de son premier jour.
            let ailleurs = vrai(&premier["ailleurs"]);
            let date_contrat = if ailleurs {
                texte(premier, "debut")
            } else {
                envois::etats::contrat_parti(&lignes[..rang], &texte(premier, "salarie"), &texte(premier, "numero"))?
                    .ok_or_else(|| valeur(format!("journal : {} modifie un contrat qui n'est pas parti avant lui", nom)))?
                    .date()
            };
            let modifs: Vec<Value> = premier["modifications"].as_array().cloned().unwrap_or_default();
            return documents::contrat::ecrire_avenant(&ctx_j, p0, &ent, vers, &modifs, numero_av, &jj(&date_contrat)?, &jj(&texte(premier, "effet"))?, &jour.jj_mm_aaaa(), ailleurs)
                .map_err(Echec::from);
        }
        let mut remplacements: Vec<((String, String, String), (String, String))> = Vec::new();
        if nom.ends_with("_remplace.pdf") {
            for c in &contrats {
                let (cle, debut) = (texte(c, "salarie"), texte(c, "debut"));
                let parti = envois::etats::contrat_parti(&lignes[..rang], &cle, &texte(c, "numero"))?
                    .ok_or_else(|| valeur(format!("journal : {} remplace un contrat qui n'est pas parti avant lui", nom)))?;
                let numero = if vrai(&parti.numero) { valeur_en_texte(&parti.numero) } else { "(sans numéro)".to_string() };
                remplacements.push(((cle, debut, texte(c, "numero")), (numero, jj(&parti.date())?)));
            }
        }
        documents::contrat::ecrire_contrat(&ctx_j, &paies, &ent, vers, &remplacements).map_err(Echec::from)
    };
    socle::temps::pendant(instant, ecrire)?;
    let pareils = contrats.iter().zip(&paies).all(|(c, p)| c.get("clauses").is_none() || crate::compagnie::signature::clauses_du_contrat(&ctx_j, p, &ent) == c["clauses"]);
    if !pareils {
        let _ = std::fs::remove_file(vers);
        return Err(erreur("ErreurOperation", format!("{} ne se refait pas à l'identique de ce qui est parti : le fichier a été déplacé ou effacé.", nom)));
    }
    Ok(())
}

/// La paie ecrite de `mois` que porte la version de l'engagement `o` dont le bulletin a le numero `numero`.
fn paie_ecrite_au_bulletin(o: &engagement::stock::Objet, mois: &str, numero: &str) -> R<Option<engagement::paie_ecrite::PaieEcrite>> {
    Ok(engagement::paies::ecrites(o, mois).map_err(Echec::from)?.into_iter().find(|p| p.bulletin.as_deref() == Some(numero)))
}

/// LES DOCUMENTS QU'A FAIT PARTIR LA LIGNE `lignes[rang]` (un envoi aux salaries de `mois`), refaits tels qu'ils sont
/// partis : chaque engagement du mois dans la version sur laquelle ses derniers documents etaient partis a ce moment-la
/// (`engagement::etats::paie_partie`), sous les numeros que cette version porte ; le rectificatif d'une paie refaite,
/// avec l'ecart au bulletin parti avant lui quand la DSN du mois ne se refaisait plus ; au jour et a l'heure de l'envoi.
/// Seuls les fichiers qui manquent s'ecrivent, et aucun si `ecrire` est faux. Rend, pour chaque paie de l'envoi, le
/// salarie et le numero de son bulletin.
fn refaire_les_documents(ctx: &Contexte, mois: &str, lignes: &[Value], rang: usize, ecrire: bool) -> R<Vec<(String, Option<String>)>> {
    let l = &lignes[rang];
    let quand = valeur_en_texte(l.get("quand").unwrap_or(&Value::Null));
    let Some((jour, instant)) = instant_de(&quand) else { return Err(valeur(format!("journal : un envoi de documents sans date ({})", quand))) };
    let cles: Vec<String> = l.pointer("/detail/salaries").and_then(Value::as_array).into_iter().flatten().filter_map(|s| s.get("cle")).map(valeur_en_texte).collect();
    let refaire = || -> R<Vec<(String, Option<String>)>> {
        let ctx = contexte_du_jour(ctx, jour)?.du_mois(mois).map_err(valeur)?;
        let jusqu_a = envois::journal::departs_des_lignes(&lignes[..=rang]);
        let avant = envois::journal::departs_des_lignes(&lignes[..rang]);
        let clos = regularisations::se_corrige_ailleurs(&ctx, &avant, mois)?;
        let mut paies: Vec<Paie> = Vec::new();
        let mut numeros: Vec<Option<String>> = Vec::new();
        let mut premiers_ce: Vec<Option<i64>> = Vec::new();
        let mut rectificatifs: HashMap<usize, Value> = HashMap::new();
        for e in engagement::etats::du_mois(&ctx, mois, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?).map_err(Echec::from)? {
            let Some(v) = e.valide() else { continue };
            let Some(p) = engagement::etats::paie_partie(&ctx, v.objet(), &jusqu_a, mois).map_err(Echec::from)? else { continue };
            let ecrite = match p.numero_bulletin.as_deref() {
                Some(n) => paie_ecrite_au_bulletin(v.objet(), mois, n)?,
                None => None,
            };
            if let Some(r) = ecrite.as_ref().and_then(|x| x.rectifie.as_deref()) {
                let parti = if clos { engagement::etats::paie_partie(&ctx, v.objet(), &avant, mois).map_err(Echec::from)? } else { None };
                let rectif = match parti {
                    Some(parti) => {
                        let porteur = crate::regularisation::dsn_qui_porte_la_correction(&ctx, v, &avant, mois)?;
                        let mut e = ecart_avec_le_bulletin_parti(&ctx, &p, &parti, &porteur)?;
                        e["rectifie"] = json!(r);
                        e["mois_origine"] = json!(socle::mois::Mois::en_clair(mois));
                        e
                    }
                    None => json!({"remplace": true, "rectifie": r, "mois_origine": socle::mois::Mois::en_clair(mois)}),
                };
                rectificatifs.insert(paies.len(), rectif);
            }
            numeros.push(if p.engagement.sans_paie() { None } else { p.numero_bulletin.clone() });
            premiers_ce.push(ecrite.as_ref().and_then(|x| x.ce));
            paies.push(p);
        }
        let (mut pour_cs, _) = crate::routes::dossier_salarie::certificats_dus(&ctx, &paies)?;
        pour_cs.sort_by_key(|(cle, _, _)| cles.iter().position(|c| c == cle).unwrap_or(usize::MAX));
        let mut certificats: Vec<(usize, &PaieCs, i64)> = Vec::new();
        for (cle, debut, pc) in &pour_cs {
            let Some(pc) = pc else { continue };
            let Some(i) = (0..paies.len()).find(|i| cle_de(&paies[*i]) == *cle && paies[*i].engagement.debut.iso() == *debut) else { continue };
            if let Some(n) = premiers_ce[i] {
                certificats.push((i, pc, n));
            }
        }
        let ent = ctx.pour_un_modele(&ctx.entreprise_du_mois(mois).map_err(valeur)?);
        let impression = Impression { paies: &paies, numeros: &numeros, rectificatifs: &rectificatifs, certificats: &certificats };
        if ecrire {
            imprimer(&ctx, mois, &ent, &impression, &cles, true)?;
        }
        Ok(paies.iter().zip(&numeros).map(|(p, n)| (cle_de(p), n.clone())).collect())
    };
    socle::temps::pendant(instant, refaire)
}

/// La route `piece-partie` : `{"mois", "fichier"}`. Le chemin de la piece partie, refaite si son fichier manque
/// (`piece_partie`), relatif a la compagnie : `{"chemin": "sorties/<mois>/[envoi/]<fichier>"}`.
pub(crate) fn piece_partie_route(ctx: &Contexte, charge: &Value) -> R<Value> {
    let champ = |k: &str| charge.get(k).and_then(Value::as_str).map(String::from).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k)));
    let f = piece_partie(ctx, &champ("mois")?, &champ("fichier")?)?;
    let relatif = f.strip_prefix(&ctx.racine).unwrap_or(&f).to_string_lossy().replace('\\', "/");
    Ok(json!({"chemin": relatif}))
}

/// LE TAUX DE PAS DES PAIES QUI PARTENT : une phrase par salarie dont une paie n'a pas de taux transmis par la DGFiP qui
/// vaille le jour de son paiement. Elle prend alors le taux non personnalise de la grille : « Lorsque le débiteur ne dispose
/// pas d'un taux calculé par l'administration fiscale [...] il est appliqué un taux proportionnel » (CGI, art. 204 H, III,
/// 1, `atelier/reference/sources/code-general-des-impots/xml/204_H.xml` l. 126). Se dit au depot de la DSN et a la
/// production des bulletins, pour ces paies-la seulement.
fn sans_taux_recu<'a>(ctx: &Contexte, paies: impl Iterator<Item = &'a Paie>) -> R<Vec<String>> {
    let mut lignes: Vec<String> = Vec::new();
    // La paie dit le taux qu'elle a pris (`Paie::pas`, ecrit a la validation) : un autre type que « 01 » (taux transmis
    // par la DGFiP, S21.G00.50.007) est la grille.
    for p in paies.filter(|p| p.pas.as_ref().is_some_and(|v| v.type_taux.as_str() != Some("01"))) {
        let paye_le = p.engagement.date_du_versement().map_err(Echec::from)?;
        let pourquoi = droit::pas::avertissements(ctx, Some(&[cle_de(p)]), Some(paye_le), Some(&p.engagement.mois)).map_err(Echec::from)?;
        let demande = crate::compagnie::amorcage_demande::en_cours(ctx, &cle_de(p))?
            .map(|d| format!(" ; taux demandé le {}, réponse attendue avant le {}", d.quand.jj_mm_aaaa(), d.avant.jj_mm_aaaa()))
            .unwrap_or_default();
        // UN TAUX ARRIVE APRES QUE LA PAIE EST PARTIE (ses documents), avant son paiement : rien ne dit qu'aucun taux ne
        // valait. La paie garde la grille, sans regularisation : « Aucune régularisation ne doit être effectuée par le
        // débiteur en raison de la réception pour la première fois d'un taux personnalisé alors qu'il a, jusque-là,
        // régulièrement appliqué les grilles de taux par défaut. Le taux personnalisé ne s'applique que pour l'avenir »
        // (BOI-IR-PAS-30-10-50, § 20, `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-50-20190227.html`).
        let l = match pourquoi.first() {
            Some(x) => format!("{}{} : sa paie prend le taux non personnalisé (CGI, art. 204 H).", x, demande),
            None => format!(
                "{} : le taux transmis par la DGFiP est arrivé après le départ de sa paie, qui garde le taux non personnalisé ; il vaut pour les suivantes (BOI-IR-PAS-30-10-50, § 20).",
                ctx.nomme(&cle_de(p))
            ),
        };
        if !lignes.contains(&l) {
            lignes.push(l);
        }
    }
    Ok(lignes)
}
