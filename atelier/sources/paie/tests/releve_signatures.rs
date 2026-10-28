//! La releve des reponses « Je signe » : le resume d'une releve vide, la lecture d'un courriel mal forme, le decodage
//! d'un corps base64 abime.

use paie::releve_signatures as rs;
use serde_json::json;

#[test]
fn le_resume_et_le_json_d_une_releve_vide_ou_sans_arrivee() {
    let vide = rs::ReleveSignatures { quand: "2026-10-07T10:00:00".into(), attendues: 0, recues: vec![], incidents: vec![] };
    assert_eq!(vide.resume(), "");
    let attente = rs::ReleveSignatures { attendues: 2, ..vide.clone() };
    assert!(attente.resume().starts_with("2 signature"), "le resume dit combien sont attendues : {}", attente.resume());
    assert_eq!(attente.en_json()["recues"], json!([]));
    assert_eq!(attente.en_json()["resume"], json!(attente.resume()));
}

// ================================================================== un message mal forme se lit quand meme

#[test]
fn un_bloc_d_entetes_rompu_se_recoupe_avant_la_ligne_rompue() {
    use paie::courriel_lecture::lisible_comme_python;
    // Sain : lu tel quel.
    let sain = b"From: a@x.invalid\nSubject: x\n\nbonjour\n";
    assert_eq!(&*lisible_comme_python(sain), &sain[..]);
    // Une ligne qui n'est pas un en-tete : les en-tetes finissent avant elle, tout le reste est le corps.
    let rompu = b"From: a@x.invalid\nGARBAGE LINE\nSubject: x\n\nCode SIG-ABCDEF\n";
    let lu = lisible_comme_python(rompu);
    let m = mailparse::parse_mail(&lu).unwrap();
    assert_eq!(mailparse::MailHeaderMap::get_first_value(m.headers.as_slice(), "From").as_deref(), Some("a@x.invalid"));
    assert!(m.get_body().unwrap().starts_with("GARBAGE LINE\nSubject: x\n\nCode SIG-ABCDEF"), "{:?}", m.get_body());
    // Rien qui ressemble a un en-tete : aucun en-tete, tout est le corps.
    let m2 = lisible_comme_python("\u{0}\u{ff} pas un courriel SIG-ABCDEF".as_bytes());
    let p = mailparse::parse_mail(&m2).unwrap();
    assert!(p.headers.is_empty() && p.get_body().unwrap().contains("SIG-ABCDEF"));
    // Une suite de ligne (espace, tabulation) et un `From ` d'mbox sont des en-tetes.
    let mbox = b"From moi Sat Oct 3\nSubject: un\n suite\n\nx";
    assert_eq!(&*lisible_comme_python(mbox), &mbox[..]);
}

#[test]
fn un_corps_base64_abime_se_decode_au_mieux_ou_reste_tel_quel() {
    use paie::simulation_courriels::decoder_b64_python as d;
    assert_eq!(d(b"Q29kZSBTSUc="), b"Code SIG");
    assert_eq!(d(b"Q29k\r\nZSBT\r\nSUc="), b"Code SIG", "les lignes se joignent");
    assert_eq!(d(b"Q29kZSBTSUc"), b"Code SIG", "bourrage manquant : complete");
    assert_eq!(d(b"Q29k!!ZSBTSUc="), b"Code SIG", "signes hors alphabet : ignores");
    // Longueur impossible (1 de plus qu'un multiple de 4) : le texte lui-meme, lignes jointes.
    assert_eq!(d(b"!!!!Code SIG-ABCDEF\n"), b"!!!!Code SIG-ABCDEF");
}

