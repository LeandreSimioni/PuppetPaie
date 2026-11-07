//! Le contrat ecrit, tel qu'il s'ecrit : le contrat d'engagement d'un spectacle, le CDD et le CDI d'un permanent, et
//! leurs avenants.
//!
//! Le texte du contrat n'est PAS dans ce module : il vit dans le modele de sa nature (`contrat_ecrit.modele` de
//! `contrats-base.json` : `contrat-cddu.modele`, `contrat-cdd.modele`, `contrat-cdi.modele`), un fichier de donnees, que la
//! compagnie remplace par le sien dans son dossier `gabarits/`. Ce module envoie a la fabrique le TEXTE du modele (lu
//! maintenant : `fabrique.figer`) et les VALEURS de la paie (`avenant::valeurs_de_la_paie`) ; c'est elle qui le met en
//! page. Un modele qui tairait une mention que la loi exige se refuse (`contrat_ecrit::refuser_les_mentions_absentes`).

use std::path::Path;

use fabrique_pdf::forme::{self, Description, Document, Style};

use crate::avenant::valeurs_de_la_paie;
use crate::bulletin::{identite_de, FORMAT, LOGO_LARGEUR, LOGO_MARGE_DROITE, LOGO_MARGE_HAUT};
use droit::contexte::EntrepriseDuDocument;
use engagement::calcul::Paie;
use crate::conges_spectacles::chemin_texte;
use droit::contexte::Contexte;
use socle::echec::Refus;
use crate::marqueurs::{pour_la_fabrique, Marqueur, ModeleLu, Valeurs};

type R<T> = Result<T, Refus>;


/// Une description de documents modeles : le format, le style, le logo de la compagnie, et ses documents.
pub fn enveloppe(style: Style, ent: &EntrepriseDuDocument, documents: Vec<Document>) -> Description {
    Description {
        format: FORMAT,
        page: None,
        police: None,
        style: Some(style),
        logo: identite_de(&ent.gabarits, "logo").map(|logo| forme::Logo {
            chemin: chemin_texte(&logo),
            largeur: LOGO_LARGEUR,
            marge_haut: Some(LOGO_MARGE_HAUT),
            marge_droite: Some(LOGO_MARGE_DROITE),
        }),
        titre: None,
        documents,
        entete_page: None,
        pied_page: None,
    }
}

/// Un document modele : le texte du modele `lu` et ses valeurs, rien d'autre.
pub fn document_modele(lu: &ModeleLu, valeurs: &Valeurs) -> forme::Modele {
    forme::Modele {
        chemin: None,
        texte: Some(lu.texte.clone()),
        valeurs: pour_la_fabrique(valeurs),
        remplace: None,
        signature: None,
        signature_employeur: None,
        modifications: None,
    }
}

/// Les valeurs que le modele lit dans cette paie. Les montants s'ecrivent comme
/// le modele les imprime (`ECHELLE_GARDEE`).
pub fn valeurs_de_paie(ctx: &Contexte, ent: &EntrepriseDuDocument, p: &Paie) -> R<Valeurs> {
    socle::num::ECHELLE_GARDEE.with(|f| f.set(true));
    let r = (|| -> R<Valeurs> {
        // Ecrite ici, sous `ECHELLE_GARDEE` : ses montants gardent leur echelle.
        let canon = socle::ecriture::en_valeur(&p.canon(ctx)?);
        Ok(valeurs_de_la_paie(ctx, ent, &canon, None)?)
    })();
    socle::num::ECHELLE_GARDEE.with(|f| f.set(false));
    r
}

/// Les contrats qui en annulent un autre : `{(salarie, premier jour, numero du contrat): (numero remplace, date)}`.
pub type Remplacements = [((String, String, String), (String, String))];

/// `contrat.description` puis `fabrique.figer` : la description de ces contrats, un document par
/// paie, le modele en texte. `remplacements` : ces contrats-la annulent et remplacent celui qui est parti avant
/// eux, et le disent en tete.
pub fn description(ctx: &Contexte, paies: &[Paie], ent: &EntrepriseDuDocument, remplacements: &Remplacements) -> R<Description> {
    let signature = identite_de(&ent.gabarits, "signature");
    let mut documents = Vec::new();
    for p in paies {
        let nat = p.engagement.nat()?;
        let modele = socle::fichiers::modele_de(&crate::contrat_ecrit::nom_du_modele(&nat)?, Some(&ent.gabarits), &ctx.gabarits());
        let lu = ModeleLu::du_fichier(&modele)?;
        let v = valeurs_de_paie(ctx, ent, p)?;
        if v.values().any(|t| t.contains("VALEUR-SANS-TEXTE")) {
            return Err(Refus::from("description illisible : une valeur du modele de contrat n'est pas un texte"));
        }
        let mut un = document_modele(&lu, &v);
        let cle = (
            p.salarie.id.to_string(),
            p.engagement.debut.iso(),
            p.engagement.numero.as_deref().unwrap_or_default().to_string(),
        );
        if let Some((_, (numero, date))) = remplacements.iter().find(|(k, _)| *k == cle) {
            un.remplace = Some(forme::Remplace { numero: numero.clone(), date: date.clone() });
        }
        un.signature = signature.as_deref().map(chemin_texte);
        refuser_ce_que_le_modele_tairait(&modele, &lu, &v)?;
        crate::contrat_ecrit::refuser_les_mentions_absentes(&modele, &lu, &nat, &v)
            .map_err(|m| Refus { classe: Some("ModeleIncomplet"), message: m })?;
        documents.push(Document::Modele(un));
    }
    Ok(enveloppe(Style::Contrat, ent, documents))
}

/// Les valeurs qu'un modele de contrat DOIT citer des qu'elles ne sont pas vides. `remuneration.detail` ne cite que
/// ce qui fait le brut ; ce qui est verse en plus (les defraiements) n'est que dans `remuneration.hors_brut`.
const A_CITER: [Marqueur; 1] = [Marqueur::RemunerationHorsBrut];

/// REGLE 1 : un gabarit de compagnie qui n'a pas le marqueur d'une valeur a citer ne fait pas un contrat qui la tait
/// (un marqueur INCONNU, la lecture du modele le refuse deja ; c'est l'inverse : une valeur sans sa place). Une valeur
/// vide passe : le modele qui l'omet dit alors tout ce qu'il y a a dire.
///
/// L'inverse aussi, pour le taux du cachet (REGLE 1) : un contrat qui ne paie aucun cachet, et dont ni
/// l'engagement ni le mois n'ecrivent de taux, n'en a pas ; le moteur prenait 130 EUR, un chiffre qu'aucun texte ne fixe.
/// Un modele qui cite `{{remuneration.taux_cachet}}` ne fait pas ce contrat-la (la fabrique dirait « marqueur
/// inconnu », sans dire pourquoi).
fn refuser_ce_que_le_modele_tairait(modele: &Path, lu: &ModeleLu, valeurs: &Valeurs) -> R<()> {
    const TAUX: Marqueur = Marqueur::RemunerationTauxCachet;
    let qui = valeurs.get(&Marqueur::SalarieNomComplet).map_or("ce salarie", String::as_str);
    let modele_nom = || modele.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    if valeurs.get(&TAUX).is_none() && lu.cite(TAUX) {
        return Err(Refus {
            classe: Some("ModeleIncomplet"),
            message: socle::echec::avec(
                format!(
                    "Le modèle de contrat « {} » cite le taux du cachet, et le contrat de {} n'a aucun taux de cachet : il ne paie aucun cachet, et ni l'engagement ni le mois n'en écrivent. Le moteur n'en invente pas. Écrire la rémunération de ce modèle par le détail de la rémunération, qui la dit quelle que soit sa forme (le modèle livré le fait), ou porter le taux sur une ligne de cachet de l'engagement. Rien n'est écrit.",
                    modele_nom(),
                    qui,
                ),
                serde_json::json!({"chemin": modele.to_string_lossy(), "marqueur": TAUX.to_string(), "a_ecrire": Marqueur::RemunerationDetail.to_string()}),
            ),
        });
    }
    for cle in A_CITER {
        let valeur = valeurs.get(&cle).map_or("", String::as_str);
        if !valeur.trim().is_empty() && !lu.cite(cle) {
            return Err(Refus {
                classe: Some("ModeleIncomplet"),
                message: socle::echec::avec(
                    format!(
                        "Le modèle de contrat « {} » ne cite pas ce que le contrat de {} lui verse hors brut ({}) : le contrat le tairait. Ajouter son marqueur à l'article « Rémunération » de ce modèle (le modèle livré l'y place juste après la période). Rien n'est écrit.",
                        modele_nom(),
                        qui,
                        valeur.trim().trim_start_matches(',').trim()
                    ),
                    serde_json::json!({"chemin": modele.to_string_lossy(), "marqueur": cle.to_string(), "apres": Marqueur::RemunerationPeriode.to_string()}),
                ),
            });
        }
    }
    Ok(())
}

/// Ce que le logiciel depose dans la case de l'artiste, quatre lignes.
pub fn mention_signature(nom: &str, quand: &str, adresse: &str, code: &str) -> String {
    format!("Signé électroniquement le {}\npar {}\n{}\ncode de signature {}", quand, nom, adresse, code)
}

/// Le contrat de `paies` (un par page), ecrit a `chemin`. Le contrat parti se refait ainsi, depuis la version de
/// l'engagement qui est partie ; son exemplaire contresigne est ce PDF-la, plus la mention de l'artiste
/// (`fabrique_pdf::marquage::contresigner`).
pub fn ecrire_contrat(ctx: &Contexte, paies: &[Paie], ent: &EntrepriseDuDocument, chemin: &Path, remplacements: &Remplacements) -> R<()> {
    crate::fabrique::ecrire_pdf(&description(ctx, paies, ent, remplacements)?, chemin).map_err(Refus::from)
}


/// Le modele livre de l'avenant du contrat ecrit de cette nature (`contrat_ecrit.avenant` de `contrats-base.json`).
fn nom_de_l_avenant(nat: &droit::natures::Nature) -> R<String> {
    nat.modele_avenant().ok_or_else(|| Refus::from(format!("La nature {} n'a pas d'avenant écrit.", nat.libelle)))
}

/// `avenant.description` puis `fabrique.figer` : le modele de l'avenant (`lu`) en texte, les valeurs du contrat et les
/// siennes (`extra`), et les articles modifies avec l'ancien texte et le nouveau. Un avenant au contrat signe ailleurs
/// (`ailleurs`) ne cite aucun article du modele de contrat : les refus qui gardent ces articles ne le regardent pas.
fn description_avenant(ctx: &Contexte, p: &Paie, ent: &EntrepriseDuDocument, lu: &ModeleLu, modifs: &[forme::Modification], extra: Valeurs, ailleurs: bool) -> R<Description> {
    let nat = p.engagement.nat()?;
    let mut v = valeurs_de_paie(ctx, ent, p)?;
    // Les memes refus que le contrat : ses articles modifies sont ceux du modele de CONTRAT de la compagnie,
    // et un modele qui n'a pas le marqueur d'une valeur a citer ferait un avenant qui la tait.
    if v.values().any(|t| t.contains("VALEUR-SANS-TEXTE")) {
        return Err(Refus::from("description illisible : une valeur du modele de contrat n'est pas un texte"));
    }
    v.extend(extra);
    if !ailleurs {
        let contrat = socle::fichiers::modele_de(&crate::contrat_ecrit::nom_du_modele(&nat)?, Some(&ent.gabarits), &ctx.gabarits());
        let du_contrat = ModeleLu::du_fichier(&contrat)?;
        refuser_ce_que_le_modele_tairait(&contrat, &du_contrat, &v)?;
        crate::contrat_ecrit::refuser_les_mentions_absentes(&contrat, &du_contrat, &nat, &v).map_err(|m| Refus { classe: Some("ModeleIncomplet"), message: m })?;
    }
    let mut un = document_modele(lu, &v);
    un.modifications = Some(modifs.to_vec());
    un.signature = identite_de(&ent.gabarits, "signature").as_deref().map(chemin_texte);
    Ok(enveloppe(Style::Avenant, ent, vec![Document::Modele(un)]))
}

/// Le contrat que l'avenant modifie, comme il le nomme (`{{avenant.reference}}`) : « n° FA1.26.13 du 12/10/2026 », ou,
/// signe hors du logiciel (`ailleurs`), « commencé le 01/03/2022 » -- le numero que le logiciel lui a donne n'est pas sur
/// le papier signe. `date_contrat` en JJ/MM/AAAA.
fn reference_du_contrat(p: &Paie, date_contrat: &str, ailleurs: bool) -> String {
    match p.engagement.numero.as_deref() {
        Some(n) if !ailleurs => format!("n° {} du {}", n, date_contrat),
        _ if ailleurs => format!("commencé le {}", date_contrat),
        _ => format!("du {}", date_contrat),
    }
}

/// L'avenant en PDF, a l'identite de la compagnie, la signature de l'employeur posee comme sur
/// le contrat. Les dates en JJ/MM/AAAA. `modifs` : ce que
/// `avenant-apercu` rend ; `ailleurs` : le contrat a ete signe hors du logiciel, ses modifications se disent en clair
/// (`avenant::changements_en_clair`), par le paragraphe `{{modification.phrase}}` du bloc MODIFICATION:.
#[allow(clippy::too_many_arguments)]
pub fn ecrire_avenant(
    ctx: &Contexte,
    p: &Paie,
    ent: &EntrepriseDuDocument,
    chemin: &Path,
    modifs: &[forme::Modification],
    numero: i64,
    date_contrat: &str,
    effet: &str,
    date: &str,
    ailleurs: bool,
) -> R<()> {
    if modifs.is_empty() {
        return Err(Refus { classe: Some("ValueError"), message: "Rien n'a changé : pas d'avenant à écrire.".into() });
    }
    let nom = nom_de_l_avenant(&*p.engagement.nat()?)?;
    let fichier = socle::fichiers::modele_de(&nom, Some(&ent.gabarits), &ctx.gabarits());
    let lu = ModeleLu::du_fichier(&fichier)?;
    let m = lu.decoupe(&fichier.to_string_lossy()).map_err(|r| Refus { classe: Some("ValueError"), message: r })?;
    if m.modification.is_empty() {
        return Err(Refus { classe: Some("ValueError"), message: format!("{} n'a pas de bloc MODIFICATION:.", nom) });
    }
    if ailleurs && !m.modification.iter().any(|b| b.contains(Marqueur::ModificationPhrase.nom())) {
        return Err(Refus {
            classe: Some("ModeleIncomplet"),
            message: socle::echec::avec(
                format!(
                    "Le modèle d'avenant {} ne dit pas une modification en clair : un contrat signé hors du logiciel n'a pas d'article à citer. Ajoutez à son bloc de modification un paragraphe qui porte la phrase de la modification (Configuration › Modèles de contrat). Rien n'est parti.",
                    nom
                ),
                serde_json::json!({"marqueur": Marqueur::ModificationPhrase.to_string()}),
            ),
        });
    }
    let extra = Valeurs::from([
        (Marqueur::AvenantNumero, numero.to_string()),
        (Marqueur::AvenantDateContrat, date_contrat.to_string()),
        (Marqueur::AvenantReference, reference_du_contrat(p, date_contrat, ailleurs)),
        (Marqueur::AvenantEffet, effet.to_string()),
        (Marqueur::AvenantDate, date.to_string()),
    ]);
    let d = description_avenant(ctx, p, ent, &lu, modifs, extra, ailleurs)?;
    crate::fabrique::ecrire_pdf(&d, chemin).map_err(Refus::from)
}
