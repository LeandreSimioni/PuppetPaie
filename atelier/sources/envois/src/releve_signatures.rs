//! LA RELEVE DES REPONSES « JE SIGNE », cote boite : ce que la releve rend (`ReleveSignatures`, `SignatureRecue`), la
//! reponse a un code attendu cherchee dans la boite et triee strictement (`chercher_reponse`), et le contrat contresigne
//! renvoye au salarie, note au journal (`renvoyer_contresigne`). Ce qui se passe entre les deux (la preuve archivee, le
//! contrat contresigne, la ligne `contrat-recu`) est fabrique et note par `moteur`.
//!
//! LE TRI STRICT. La boite cherche le code partout (objet, corps, pieces jointes) ; on trie ensuite :
//! - notre propre envoi est ecarte par son identifiant (le `Message-ID` note avec la demande, relu dans chaque message), et
//!   par sa phrase (« bouton « Je signe » ») -- pas par son expediteur : en test le salarie, c'est nous ;
//! - le code doit figurer dans la partie ECRITE de la reponse, avant la citation du message d'origine
//!   (`courriel_lecture::texte_propre`) ;
//! - elle doit venir de l'adresse a laquelle le contrat a ete envoye : une reponse venue d'ailleurs est signalee, pas
//!   retenue.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::courriel::Courriel;
use crate::courriel_lecture as lecture;
use crate::envoi::{self, Message};
use crate::journal::{self, Note};
use socle::echec::R;
use socle::paris::maintenant;
use socle::valeur::valeur_en_texte;

const SUJET_CONTRESIGNE: &str = "Ton contrat signé - {spectacle}";
const SUJET_CONTRESIGNE_PLURIEL: &str = "Tes contrats signés - {mois} {annee}";
const CORPS_CONTRESIGNE: &str = "Bonjour {prenom},\n\nNous avons bien reçu ta signature le {quand}. Ci-joint {quoi}, signé des deux parties : c'est l'exemplaire à conserver.\n\nBien à toi,\n{signataire}\n{raison_sociale}\n";

// ------------------------------------------------------------------ ce que la releve rend

/// Une signature trouvee, notee, contresignee (si elle a pu l'etre).
#[derive(Debug, Clone)]
pub struct SignatureRecue {
    pub mois: String,
    pub salarie: String,
    pub nom: String,
    pub code: String,
    pub adresse: String,
    /// ISO, la date du message.
    pub quand: String,
    /// `[{salarie, debut, fin, numero, spectacle, fichier, avenant, mois}]`, tels que le journal les donne.
    pub contrats: Vec<Value>,
    pub fichier_signe: String,
    pub preuve: String,
    pub renvoye: bool,
}

impl SignatureRecue {
    pub fn en_json(&self) -> Value {
        json!({
            "mois": self.mois, "salarie": self.salarie, "nom": self.nom, "code": self.code, "adresse": self.adresse,
            "quand": self.quand, "contrats": self.contrats, "fichier_signe": self.fichier_signe, "preuve": self.preuve,
            "renvoye": self.renvoye,
        })
    }
}

/// Combien de signatures attendaient, celles qui sont arrivees, ce qui a empeche le reste.
#[derive(Debug, Clone)]
pub struct ReleveSignatures {
    pub quand: String,
    pub attendues: usize,
    pub recues: Vec<SignatureRecue>,
    pub incidents: Vec<String>,
}

impl ReleveSignatures {
    pub fn nouvelle() -> ReleveSignatures {
        ReleveSignatures { quand: maintenant(), attendues: 0, recues: Vec::new(), incidents: Vec::new() }
    }

    /// La phrase de la veille, vide quand rien n'etait attendu ni arrive.
    pub fn resume(&self) -> String {
        if self.attendues == 0 && self.recues.is_empty() {
            return String::new();
        }
        if self.recues.is_empty() {
            return format!("{} signature(s) attendue(s), aucune arrivée", self.attendues);
        }
        let noms: Vec<&str> = self.recues.iter().map(|r| r.nom.as_str()).collect();
        format!("{} signature(s) reçue(s) : {}", self.recues.len(), noms.join(", "))
    }

    /// Ce que l'ouverture met dans son bilan, avec le resume.
    pub fn en_json(&self) -> Value {
        json!({
            "quand": self.quand,
            "attendues": self.attendues,
            "recues": self.recues.iter().map(SignatureRecue::en_json).collect::<Vec<_>>(),
            "incidents": self.incidents,
            "resume": self.resume(),
        })
    }
}

fn texte_de(d: &Value, cle: &str) -> String {
    d.get(cle).map(valeur_en_texte).unwrap_or_default()
}

/// `a["envoyes"]` : les `Message-ID` de NOS envois pour cette demande.
pub fn identifiants_de(a: &Value) -> Vec<String> {
    a.get("envoyes").and_then(Value::as_array).map(|l| l.iter().map(valeur_en_texte).collect()).unwrap_or_default()
}

// ------------------------------------------------------------------ la reponse

/// La reponse retenue a un code : son identifiant dans la boite, le message tel qu'il est arrive, son auteur, sa date.
pub struct Reponse {
    pub identifiant: String,
    pub brut: Vec<u8>,
    pub de: String,
    pub quand: String,
}

/// LA REPONSE A LA DEMANDE `a` (une signature attendue, `etats::signatures_attendues`), triee strictement. `expediteur` :
/// la boite de la compagnie. Ce qui ecarte une reponse se dit dans `incidents`. `Err` : la boite ne repond pas (une erreur
/// de transport ne se corrige pas en essayant le code suivant).
pub fn chercher_reponse(client: &mut dyn Courriel, a: &Value, expediteur: &str, incidents: &mut Vec<String>) -> Result<Option<Reponse>, ()> {
    let code = texte_de(a, "code");
    let a_nom = texte_de(a, "nom");
    let a_adresse = texte_de(a, "adresse");
    let envoyes = identifiants_de(a);
    let identifiants = match client.rechercher(&format!("\"{}\"", code), 10) {
        Ok(i) => i,
        Err(e) => {
            incidents.push(format!("Recherche de {} : {}", code, e));
            return Err(());
        }
    };
    for ident in identifiants {
        if envoyes.contains(&ident) {
            continue; // notre propre demande
        }
        let brut = match client.message_brut(&ident) {
            Ok(b) => b,
            Err(e) => {
                incidents.push(format!("Lecture du message {} : {}", ident, e));
                continue;
            }
        };
        // Un bloc d'en-tetes rompu se recoupe, et le message se lit quand meme.
        let lu = lecture::recouper_les_entetes(&brut);
        let Ok(m) = mailparse::parse_mail(&lu) else { continue };
        if envoyes.contains(&lecture::identifiant_de(&m)) {
            continue; // notre propre demande, par son en-tete
        }
        let texte = lecture::texte_propre(&m);
        if !texte.to_uppercase().contains(code.as_str()) {
            continue; // le code n'est que dans la citation
        }
        if texte.contains("bouton « Je signe »") {
            continue; // notre demande elle-meme (envoyee avant que son identifiant soit note)
        }
        let de = lecture::adresse_de(&m);
        if de == expediteur && de != a_adresse {
            continue; // un autre de nos envois (le contresigne)
        }
        if de != a_adresse {
            incidents.push(format!("{} : une reponse portant {} vient de {} et non de {} ; elle n'est pas retenue.", a_nom, code, de, a_adresse));
            continue;
        }
        let quand = lecture::date_de(&m, &maintenant());
        return Ok(Some(Reponse { identifiant: ident, brut: brut.clone(), de, quand }));
    }
    Ok(None)
}

// ------------------------------------------------------------------ le contresigne

/// Ce qui part avec le contrat contresigne.
pub struct Contresigne<'a> {
    /// La demande (`etats::signatures_attendues`).
    pub a: &'a Value,
    /// Les contrats qu'elle porte, chacun avec son `avenant` quand c'en est un.
    pub contrats: &'a [Value],
    pub est_avenant: bool,
    /// L'adresse de la reponse, et sa date lisible.
    pub de: &'a str,
    pub quand_lisible: &'a str,
    /// Les PDF contresignes.
    pub signes: &'a [PathBuf],
    pub signataire: &'a str,
    pub raison: &'a str,
    pub expediteur: &'a str,
    /// L'annee et le nom du mois de la demande, pour l'objet.
    pub an: i32,
    pub nom_mois: &'a str,
}

/// LE CONTRAT CONTRESIGNE, RENVOYE AU SALARIE, puis sa ligne `contrat-contresigne` au journal. `qui` : la ligne.
pub fn renvoyer_contresigne(racine: &Path, client: &mut dyn Courriel, c: &Contresigne, qui: &str) -> R<()> {
    let a = c.a;
    let a_mois = texte_de(a, "mois");
    let nb = c.contrats.len();
    let mut spectacles: Vec<String> = Vec::new();
    for x in c.contrats {
        let s = texte_de(x, "spectacle");
        if !spectacles.contains(&s) {
            spectacles.push(s);
        }
    }
    let avenant_de = |x: &Value| match x.get("avenant").filter(|v| socle::fichiers::vrai(v)) {
        Some(v) => valeur_en_texte(v),
        None => valeur_en_texte(a.get("avenant").unwrap_or(&Value::Null)),
    };
    let numeros: Vec<String> = c.contrats.iter().map(avenant_de).collect();
    let (sujet, quoi) = if c.est_avenant {
        (
            format!("{} - {}", if nb == 1 { "Ton avenant signé" } else { "Tes avenants signés" }, spectacles[0]),
            if nb == 1 { format!("l'avenant n° {} à ton contrat", numeros[0]) } else { format!("tes {} avenants (un document chacun)", nb) },
        )
    } else {
        (
            if nb == 1 || spectacles.len() == 1 {
                envoi::formater(SUJET_CONTRESIGNE, &[("spectacle", spectacles[0].clone())])
            } else {
                envoi::formater(SUJET_CONTRESIGNE_PLURIEL, &[("mois", c.nom_mois.to_string()), ("annee", c.an.to_string())])
            },
            if nb == 1 { "ton contrat d'engagement".to_string() } else { format!("tes {} contrats d'engagement (un par page)", nb) },
        )
    };
    let message = Message {
        cle: texte_de(a, "salarie"),
        nom: texte_de(a, "nom"),
        destinataire: c.de.to_string(),
        sujet,
        corps: envoi::formater(
            CORPS_CONTRESIGNE,
            &[
                ("prenom", texte_de(a, "prenom")),
                ("quand", c.quand_lisible.to_string()),
                ("quoi", quoi),
                ("signataire", c.signataire.to_string()),
                ("raison_sociale", c.raison.to_string()),
            ],
        ),
        pieces: c.signes.to_vec(),
        html: String::new(),
    };
    envoi::envoyer(&[message], c.expediteur, client, None, None).map_err(|e| socle::echec::Echec::from(e.to_string()))?;
    let noms_signes: Vec<String> = c.signes.iter().map(|p| socle::fichiers::nom_du(p)).collect();
    let detail = json!({"salarie": texte_de(a, "salarie"), "nom": texte_de(a, "nom"), "code": texte_de(a, "code"), "contrats": c.contrats, "fichiers": noms_signes});
    let premier = c.signes[0].to_string_lossy().into_owned();
    let note = Note {
        quoi: "contrat-contresigne",
        mois: Some(&a_mois),
        fichier: Some(&premier),
        mode: journal::REEL,
        qui,
        accepte: true,
        destinataire: c.de,
        flux: "",
        reponse: "contrat contresigne renvoye au salarie",
        trace: None,
        detail: Some(&detail),
        versions: None,
        quand: "",
        origine: "",
    };
    journal::noter(racine, &note, &maintenant()).map(|_| ()).map_err(socle::echec::Echec::from)
}
