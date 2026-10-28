//! Un envoi coupe en route : ce qui est parti se lit, il ne se renvoie pas (regle 4). L'erreur nomme ceux qui sont deja
//! partis, et les listes ne portent que ceux-la.

use std::cell::RefCell;
use std::rc::Rc;

use envois::courriel::{Courriel, ErreurCourriel};
use envois::envoi::{self, Message};

/// Une boite en memoire dont l'envoi de rang `refuse` (a partir de 0) echoue.
struct Boite {
    recus: Rc<RefCell<Vec<Vec<u8>>>>,
    refuse: usize,
}

impl Courriel for Boite {
    fn compte(&self) -> &str {
        "boite@exemple.invalid"
    }
    fn envoyer(&mut self, m: &[u8]) -> Result<String, ErreurCourriel> {
        let n = self.recus.borrow().len();
        if n == self.refuse {
            return Err(ErreurCourriel("le serveur refuse ce message".into()));
        }
        self.recus.borrow_mut().push(m.to_vec());
        Ok(format!("<faux-{}@exemple.invalid>", n))
    }
    fn rechercher(&mut self, _: &str, _: usize) -> Result<Vec<String>, ErreurCourriel> {
        Ok(Vec::new())
    }
    fn message_brut(&mut self, _: &str) -> Result<Vec<u8>, ErreurCourriel> {
        Ok(Vec::new())
    }
    fn fermer(&mut self) {}
}

fn message(dest: &str) -> Message {
    Message {
        cle: dest.split('@').next().unwrap().into(),
        nom: "X".into(),
        destinataire: dest.into(),
        sujet: format!("Sujet pour {}", dest),
        corps: "Bonjour,\n".into(),
        ..Default::default()
    }
}

#[test]
fn envoyer_l_echec_au_troisieme_dit_qui_est_deja_parti() {
    let recus = Rc::new(RefCell::new(Vec::new()));
    let mut c = Boite { recus: recus.clone(), refuse: 2 };
    let messages = [message("a@exemple.fr"), message("b@exemple.fr"), message("c@exemple.fr"), message("d@exemple.fr")];
    let (mut ids, mut copies) = (Vec::new(), Vec::new());
    let e = envoi::envoyer(&messages, "compagnie@gmail.com", &mut c, Some(&mut ids), Some(&mut copies)).unwrap_err();
    assert!(e.contains("a@exemple.fr") && e.contains("b@exemple.fr") && !e.contains("d@exemple.fr"), "{}", e);
    // Le quatrieme n'a pas ete tente ; les listes ne portent que ce qui est reellement parti.
    assert_eq!(recus.borrow().len(), 2);
    assert_eq!((ids.len(), copies.len()), (2, 2));
}
