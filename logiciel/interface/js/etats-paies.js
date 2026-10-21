/* L'ecran par paie. */
"use strict";

/* =======================================================================
   L'ÉCRAN PAR PAIE
   =======================================================================
   Demandé en regardant l'écran :

     « pour les contrats à signer, il pourrait y avoir plusieurs cases dans
       chaque paie, pour voir en un coup d'œil si la DPAE est faite, si le
       contrat est envoyé… »
     « et une case pour sélectionner, pour pouvoir en sélectionner plusieurs
       pour envoyer plusieurs contrats d'un coup, ou plusieurs DPAE d'un coup »

   Le moteur sait répondre depuis le 11/09 au soir (`operations.etat_par_paie`,
   `produire_dpae(paies=[…])`) ; ces deux fonctions n'avaient pas d'écran, et
   une fonction sans écran est une fonction que personne n'utilise.

   TROIS RÈGLES, ET LA PREMIÈRE EST TOUT LE SUJET :

   1. CHAQUE PASTILLE DIT À QUELLE MAILLE ELLE EST CONNUE. Les quatre actes ne
      se déclarent pas au même grain : une DPAE déclare une embauche, un
      contrat se signe par engagement, les documents partent par salarié, et
      une DSN est mensuelle — il n'existe pas de DSN d'une paie. Écrire « DSN
      déposée » sur un bloc de paie sans dire qu'on parle du mois promettrait
      une précision que le journal n'a pas. D'où le « du mois », le « de
      <nom> », sous la pastille — et la maille vient du moteur, pas d'ici.
   2. L'ÉTAT EST CELUI DE LA SAISIE ENREGISTRÉE, pas de ce qui est à l'écran.
      `etat_par_paie` lit `paies/AAAA-MM.json`. Un engagement qu'on vient
      d'ajouter ou de changer n'a donc pas d'état, et le bloc le DIT au lieu
      de montrer celui d'à côté. Une pastille verte sur une paie modifiée
      serait le pire mensonge possible ici.
   3. RIEN NE PART D'UN CLIC. Les deux actions groupées ouvrent les mêmes
      récapitulatifs que la colonne de droite, avec le même mot de passe du
      logiciel. Elles ne font que RESTREINDRE ce qui part à ce qui est coché.

   Ce qui n'est pas là, et qu'il ne faut pas croire oublié : le bouton
   « valider » par paie, et le bloc qui devient vert. « Valider » est un état
   de la SAISIE, pas un acte — rien ne part — et le moteur ne sait pas encore
   l'écrire. Le dessiner d'abord aurait fait une case à cocher qui ne se
   souvient de rien. Voir ETAT.md §19.12.
*/
let ETATS_PAIES = null;      // "clé|début|rang" → les quatre actes ; null = pas lu

/* LES JUMEAUX (09/10/2026) : deux contrats du même salarié qui commencent le
   même jour ne se confondent pas. Le moteur les identifie par leur numéro
   quand il est posé, sinon par leur RANG parmi ceux du même salarié et du
   même jour, dans l'ordre du fichier (`jumeaux.rs`) — et le fichier suit
   l'ordre des blocs à l'écran (`saisieCourante`). Chaque ligne
   d'`etat-par-paie` porte ce rang ; la page le rend à `valider-paie`. */
const clePaie = (salarie, debut, rang) => salarie + "|" + debut + "|" + (rang || 0);

/** Les (salarié, premier jour) d'un bloc, sans leur rang.
 *  Le moteur trie les dates et prend la première (`Engagement.debut`) : on
 *  fait pareil, sans quoi une paie saisie « 12 3 » ne se retrouverait pas. */
function identitesDuBloc(div) {
  // Les jours sont sur les lignes de paie depuis le 12/09/2026 : le premier
  // jour de la paie est le plus petit de TOUTES ses lignes -- sauf si le
  // contrat dit son début (un contrat en cours depuis octobre s'identifie
  // par le 12/10, pas par son premier cachet de novembre).
  const debut = debutDuBloc(div);
  if (!debut) return [];
  return salariesDuBloc(div).map((salarie) => ({ salarie, debut }));
}

/** Les paies d'un bloc, telles qu'elles sont SAISIES à l'écran : (salarié,
 *  premier jour), le numéro du contrat s'il est figé, et le rang du bloc
 *  parmi ceux du même salarié et du même jour qui le précèdent. */
function paiesDuBloc(div) {
  const blocs = [...document.querySelectorAll(".engagement")];
  const avant = blocs.slice(0, Math.max(0, blocs.indexOf(div))).flatMap(identitesDuBloc);
  const numero = div.dataset.numero || "";
  return identitesDuBloc(div).map((p) => ({ ...p, numero,
    rang: avant.filter((a) => a.salarie === p.salarie && a.debut === p.debut).length }));
}

/** La ligne d'`etat-par-paie` d'une paie de l'écran : par son numéro s'il est
 *  posé, sinon par son rang — jamais une ligne qui porte un autre numéro que
 *  le sien (la règle de `valider-paie`). */
function etatDePaie(p) {
  if (!ETATS_PAIES) return undefined;
  if (p.numero) {
    const n = Object.values(ETATS_PAIES).find((l) => l.salarie === p.salarie
      && l.debut === p.debut && l.numero === p.numero);
    if (n) return n;
  }
  const l = ETATS_PAIES[clePaie(p.salarie, p.debut, p.rang)];
  if (l && p.numero && l.numero && l.numero !== p.numero) return undefined;
  return l;
}

/** QUEL contrat `valider-paie` vise : son numéro s'il en a un, et son rang. */
const contratDe = (l) => ({ salarie: l.salarie, debut: l.debut,
                            numero: l.numero || "", rang: l.rang || 0 });

/* LES TROIS ACTES D'UNE PAIE. Ils étaient quatre — la DSN en faisait partie,
   avec un « du mois » sous la pastille pour prévenir du contresens. La demande : « la DSN, elle ne doit pas être liée à la sélection, elle
   fait obligatoirement tout le mois, pas comme les DPAE ». Une mention sous
   une pastille ne rattrape pas une pastille mal placée : posée dans un bloc de
   paie, à côté d'une case à cocher qui sert à restreindre ce qui part, elle se
   lisait comme « la DSN de cette paie-ci ». Il n'existe pas de DSN d'une paie.
   Elle vit désormais dans la colonne du mois, et là seulement. */
const ACTES_PAIE = [
  { cle: "dpae", titre: "DPAE" },
  { cle: "signature", titre: "Contrat" },
  { cle: "documents", titre: "Documents" },
];

/** « de Favart » : ce que la pastille couvre AU JUSTE, quand ce n'est pas
 *  cette paie-ci. Reste utile pour les documents, qui partent par salarié. */
function ditLaMaille(maille, ligne) {
  if (maille === "mois") return "du mois";
  if (maille === "salarie") {
    return "de " + String(ligne.nom || "ce salarié").split(" ")[0];
  }
  return "";
}

/* TROIS COULEURS, ET UN PETIT CARRÉ. La demande, devant les
   anciennes vignettes : « j'imaginais ça en petit, juste un petit carré qui
   est gris, qui passe au vert ou orange si problème ou pas encore arrivé — ça
   prend trop de place comme ça ».

   Elles prenaient en effet quatre encadrés sur toute la largeur du bloc, alors
   qu'on ne leur demande qu'un coup d'œil. Une ligne, trois carrés, trois
   libellés. Le détail — la date, la maille, l'état du retour — passe dans
   l'infobulle : il se lit quand on le cherche, et il ne se lit que là.

     gris     rien n'est parti ;
     orange   parti, mais l'accusé n'est pas arrivé (ou a refusé) ;
     vert     parti, et attesté.

   L'orange n'est pas décoratif : « envoyé » et « accepté » sont deux choses,
   et les confondre fait tenir pour faite une déclaration en attente. Pour les
   contrats et les documents, il n'existe pas d'accusé — un envoi est un envoi,
   et le vert est alors la vérité entière. */
function couleurPastille(cle, e) {
  // Le contrat a son accusé à lui : le papier signé qui revient, et c'est un
  // humain qui le dit d'un clic (contrat-recu). « Envoyé on passe à orange,
  // signé on passe au vert ». Un contrat signé sur
  // place, jamais envoyé, se marque reçu quand même : vert.
  if (cle === "signature") {
    // Un CDI en cours (30/09/2026) : signé à l'embauche, rien à envoyer ni à
    // comparer chaque mois — la pastille dit « CDI, signé en <mois> ».
    if (e && e.cdi_en_cours) return "vert";
    // La paie a changé depuis que le contrat est parti : il n'est plus le
    // bon, signé ou non. Rouge, comme une DPAE refusée : il faut le refaire.
    // Signé, mais il ne dit plus ce que dit la paie : orange, un avenant
    // est à faire (30/09/2026). Rouge reste pour un contrat pas signé qui a
    // changé : c'est lui qu'il faut renvoyer.
    if (e && e.caduc && e.recu) return "orange";
    if (e && e.caduc) return "rouge";
    // Un avenant parti, pas encore revenu signé : le contrat est signé, sa
    // modification non (30/09/2026).
    if (e && e.avenant_attendu) return "orange";
    if (e && e.recu) return "vert";
    return (e && e.fait) ? "orange" : "gris";
  }
  if (!e || !e.fait) return "gris";
  // Des documents partis, puis une paie qui a changé (ticket #240) : ils ne
  // sont plus les bons, le moteur le dit (`caduc`) — à renvoyer.
  if (cle === "documents" && e.caduc) return "orange";
  if (cle !== "dpae") return "vert";          // pas d'accusé pour ces envois-là
  // La DPAE, elle, reçoit un certificat de conformité — et le moteur le dit
  // PAR PAIE (`passee`), sur le dépôt qui porte cette embauche-là. Refusée :
  // rouge, et c'est la seule pastille qui demande de REFAIRE quelque chose.
  if (e.refusee) return "rouge";
  return e.passee ? "vert" : "orange";
}

function infobulle(cle, titre, e, ligne) {
  // Un contrat en cours depuis un mois précédent : sa DPAE et sa signature
  // sont celles de ce mois-là. La pastille les montre (le journal les
  // connaît par salarié et début de contrat), et dit d'où elles viennent.
  // Les documents, eux, partent chaque mois : la mention ne les concerne pas.
  const enCours = cle !== "documents" && ligne && ligne.contrat && !ligne.contrat.premier_mois
    ? " (contrat en cours depuis " + moisEnClair(ligne.contrat.depuis) + ")" : "";
  if (cle === "signature") {
    if (e && e.cdi_en_cours) {
      return titre + " — " + libelleCdiEnCours(e) + " — en cours depuis "
        + moisEnClair(e.cdi_en_cours.depuis) + ", sans terme : rien à envoyer ce mois-ci";
    }
    const envoi = (e && e.fait) ? "envoyé le " + quand(e.fait) : "pas envoyé";
    if (e && e.caduc && e.recu) {
      return titre + " — " + signeLe(e.recu, e.recu_par) + enCours + " — "
        + (e.pourquoi || "la paie a changé depuis") + " : cliquer pour faire l'avenant";
    }
    if (e && e.caduc) {
      return titre + " — " + envoi + enCours + " — " + (e.pourquoi || "la paie a changé depuis")
        + " : ce contrat n'est plus le bon, à renvoyer au salarié";
    }
    if (e && e.avenant_attendu) {
      return titre + " — " + envoi + enCours + " — avenant n° " + e.avenant.numero
        + " envoyé le " + quand(e.avenant.quand) + ", pas encore revenu signé";
    }
    if (e && e.recu) {
      return titre + " — " + envoi + enCours + " — " + signeLe(e.recu, e.recu_par)
        + (e.avenant && e.avenant.signe ? " — avenant n° " + e.avenant.numero
           + " signé le " + quand(e.avenant.signe) : "");
    }
    return titre + " — " + envoi + enCours
      + (e && e.fait ? " — pas encore revenu signé" : "");
  }
  if (!e || !e.fait) return titre + " — rien n'est parti" + enCours;
  const maille = ditLaMaille(e.maille, ligne);
  if (cle === "documents" && e.caduc) {
    return titre + " — envoyés le " + quand(e.fait) + (maille ? " (" + maille + ")" : "")
      + " — à renvoyer : " + e.pourquoi;
  }
  const couleur = couleurPastille(cle, e);
  if (couleur === "rouge") {
    return titre + " — envoyé le " + quand(e.fait) + " — REFUSÉ : "
      + ((e.anomalies || []).join(" ; ") || "motif non lu")
      + " — à corriger et à renvoyer";
  }
  return titre + " — envoyé le " + quand(e.fait) + enCours
    + (maille ? " (" + maille + ")" : "")
    + (couleur === "orange" ? " — accusé pas encore arrivé, on attend"
       : cle === "dpae" ? " — passée, rien à faire" + dossierUrssaf(e) : "");
}

/** La référence sous laquelle l'Urssaf a enregistré la DPAE (certificat de
 *  conformité, R_DOSSIER), quand elle est arrivée ; ou « déjà existante »
 *  quand l'Urssaf n'a rien créé parce qu'une DPAE identique était là. */
function dossierUrssaf(e) {
  if (!e) return "";
  if (e.deja_existante) return " (déjà existante à l'Urssaf, rien de créé)";
  return e.dossier ? " (dossier " + e.dossier + ")" : "";
}

/** « CDI, signé en septembre 2026 » : ce que la pastille Contrat écrit d'un
 *  CDI en cours à la place de « Contrat » (le moteur donne le mois). */
function libelleCdiEnCours(e) {
  return "CDI, signé en " + moisEnClair(e.cdi_en_cours.signe_en);
}

/** Le bandeau d'un bloc : une ligne, un carré par acte. */
function bandeauEtats(ligne) {
  return `<div class="etats-actes">` + ACTES_PAIE.map((a) => {
    const e = ligne.etats[a.cle] || {};
    // DPAE et Contrat se cliquent : on voit CE QUI EST PARTI, les fichiers
    // eux-mêmes (29/09/2026), et c'est là qu'on dit « reçu signé ».
    const clic = a.cle !== "documents"
      ? ` cliquable" data-acte="${a.cle}"`
      : `"`;
    return `<span class="etat-acte ${couleurPastille(a.cle, e)}${clic}
        title="${echapper(infobulle(a.cle, a.titre, e, ligne)
          + (a.cle !== "documents" ? " — cliquer pour voir ce qui est parti" : ""))}">
      <i class="carre"></i>${echapper(a.cle === "signature" && e.cdi_en_cours
        ? libelleCdiEnCours(e) : a.titre)}${e.recu && !e.caduc ? " ✓" : ""}</span>`;
  }).join("") + `</div>`;
}

/** Le clic sur la pastille DPAE ou Contrat : VOIR CE QUI EST PARTI
 *  (l'utilisateur, 29/09/2026 : « on voit le journal, pas ce qui est parti »).
 *  Les fichiers eux-mêmes — la DPAE déposée, le contrat envoyé, le contrat
 *  revenu signé — chacun s'ouvre dans le lecteur du poste. La mention
 *  « reçu signé » se pose ici, réversible d'un second clic ; le journal
 *  garde les deux. */
function brancherRecu(div) {
  div.querySelectorAll(".etat-acte.cliquable").forEach((s) => {
    s.onclick = (ev) => {
      ev.preventDefault();
      const ligne = (s.closest(".engagement") || div)._ligne;
      if (!ligne) return;
      // Un contrat signé qui ne dit plus ce que dit la paie : le clic
      // propose tout de suite l'avenant (30/09/2026, « simple »).
      const e = (ligne.etats || {})[s.dataset.acte] || {};
      if (s.dataset.acte === "signature" && e.caduc && e.recu) {
        boiteAvenant(document.getElementById("mois").value, ligne.salarie, ligne.debut, ligne.numero);
        return;
      }
      boitePieces(ligne, s.dataset.acte);
    };
  });
}

async function boitePieces(ligne, acte) {
  const mois = document.getElementById("mois").value;
  let r;
  try {
    r = await api("pieces-parties", { salarie: ligne.salarie, debut: ligne.debut });
  } catch (e) { dire(e.message, "erreur"); return; }
  const e = ligne.etats[acte] || {};
  const titre = acte === "dpae" ? "DPAE" : "Contrat";
  const pieces = (acte === "dpae" ? r.dpae : r.contrat) || [];
  const liste = pieces.length
    ? `<ul class="liste-pieces">${pieces.map((p, i) => `<li>
        <span><b>${echapper(p.quoi)}</b> — ${echapper(quand(p.quand))}
          <br><span class="discret">${echapper(p.fichier)}${p.existe ? ""
            : " — n'est plus dans le dossier du mois"}</span></span>
        <button class="secondaire p-ouvrir" data-i="${i}" ${p.existe ? "" : "disabled"}>Ouvrir</button>
      </li>`).join("")}</ul>`
    : `<p class="discret">${acte === "dpae"
        ? "Aucune DPAE déposée pour cette embauche."
        : e.recu ? "Le contrat a été noté signé à la main : aucun fichier n'est passé par le logiciel."
        : "Aucun contrat n'est parti à la signature pour cet engagement."}</p>`;
  const signature = acte === "signature";
  ouvrirBoite(`
    <h3>${titre} — ${echapper(ligne.nom)}</h3>
    <p class="discret">${echapper(pourQuoi(ligne))},
      ${echapper(periodeEnClair(ligne.debut, ligne.fin))}</p>
    <p>${echapper(infobulle(acte, titre, e, ligne))}</p>
    ${e.caduc ? `<div class="encart attention"><b>Ce contrat n'est plus le bon</b>
      ${echapper(e.pourquoi ? e.pourquoi.charAt(0).toUpperCase() + e.pourquoi.slice(1) + "."
                            : "La paie a changé depuis qu'il est parti.")}
      ${e.recu ? "Il est signé : il se modifie par un avenant, que le salarié signe à son tour."
               : "Le salarié doit recevoir le contrat à jour, et le signer à nouveau."}</div>` : ""}
    ${liste}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Fermer</button>
      ${signature && (e.fait || e.recu) ? `<button class="secondaire" id="p-recu">${e.recu
        ? "Retirer la mention « signé »" : "Noter le contrat signé reçu"}</button>` : ""}
      ${signature && e.fait ? `<button class="${e.caduc && e.recu ? "primaire" : "secondaire"}" id="p-avenant"
        title="Modifier le contrat parti : l'ancien texte et le nouveau, article par article">Avenant…</button>` : ""}
      ${signature && (e.caduc || !e.fait) && !(e.caduc && e.recu) && !(MOIS_FERME && MOIS_FORCE !== mois) ? `<button class="primaire" id="p-signer">${e.caduc
        ? "Renvoyer le contrat à jour…" : "Envoyer à la signature…"}</button>` : ""}
    </div>`);
  const avenantB = document.getElementById("p-avenant");
  if (avenantB) avenantB.onclick = () => { fermerBoite(); boiteAvenant(mois, ligne.salarie, ligne.debut, ligne.numero); };
  document.querySelectorAll(".p-ouvrir").forEach((b) => {
    b.onclick = async () => {
      try {
        await api("ouvrir-piece", { salarie: ligne.salarie, debut: ligne.debut,
                                    fichier: pieces[+b.dataset.i].fichier });
      } catch (err) { dire(err.message, "erreur"); }
    };
  });
  const recu = document.getElementById("p-recu");
  if (recu) recu.onclick = async () => {
    const oui = !e.recu;
    try {
      await api("contrat-recu", { mois, salarie: ligne.salarie, debut: ligne.debut, numero: ligne.numero || "", recu: oui });
      fermerBoite();
      dire(oui ? "Contrat signé reçu — noté au journal."
               : "Mention retirée : le contrat signé n'est pas là.");
      await dessinerEtatsDesPaies(mois);
      await rafraichirActions();
    } catch (err) { dire(err.message, "erreur"); }
  };
  const signer = document.getElementById("p-signer");
  if (signer) signer.onclick = () => { fermerBoite(); boiteSignature(mois, [ligne.salarie]); };
}

/** Peint les états sur les blocs. Rappelée après chaque frappe : changer le
 *  salarié ou les jours change la paie que le bloc désigne. */
function peindreEtats() {
  let refaire = false;
  document.querySelectorAll(".engagement").forEach((div) => {
    const cible = div.querySelector(".etats");
    if (!cible) return;
    const ps = paiesDuBloc(div);
    const lignes = ETATS_PAIES
      ? ps.map(etatDePaie).filter(Boolean)
      : [];
    avisDpaeDeplacee(div, ps, lignes.length === 1 ? lignes[0] : null);
    const connue = ps.length > 0 && lignes.length === ps.length;
    div.classList.toggle("enregistree", connue);
    if (!ETATS_PAIES) { cible.innerHTML = ""; return; }
    if (!connue) {
      fermerEdition(div, false);
      if (!salariesDuBloc(div).length) {
        // Sans salarié, ce bloc n'est pas une paie : il n'est même pas
        // enregistré. Le dire, plutôt que de laisser un bloc muet qu'on
        // ne peut ni valider ni faire partir.
        cible.innerHTML = `<p class="etats-absent">Choisir qui est engagé.</p>
           <div class="validation"><span class="boutons">${BOUTON_RETIRER}</span></div>`;
        brancherRetirer(div);
        return;
      }
      // Le cas courant et normal : une paie qu'on vient d'ajouter ou de
      // changer. Pas de pastille grise — une pastille, même grise, se lit
      // comme un état connu. Le message ne dit plus « Enregistrer la saisie
      // d'abord » : le bouton n'existe plus, l'enregistrement se fait tout
      // seul une seconde après la frappe, et il n'y a donc rien à faire —
      // seulement à attendre que l'écran se rattrape.
      // Plus de message ici (30/09/2026) : « Pas encore dans la saisie
      // enregistrée… » s'affichait en orange sur chaque carte à l'ouverture,
      // le temps d'un aller-retour, et se lisait comme un souci.
      cible.innerHTML = `<div class="validation"><span class="boutons">${BOUTON_RETIRER}</span></div>`;
      brancherRetirer(div);
      return;
    }
    if (lignes.length === 1) {
      const ligne = lignes[0];
      // Un contrat sur plusieurs mois : son chevron est au bord de la case (#127, `poserChevrons`).
      div._ligne = ligne;
      const dp = ligne.etats.dpae || {};
      if (dp.fait && !dp.refusee) {
        div._dpaeFaite = { salarie: ligne.salarie, debut: ligne.debut,
                           nom: ligne.nom, quand: dp.fait };
      }
      cible.innerHTML = `<span class="etats-qui">${echapper(ligne.nom)}
          <span class="discret">— ${echapper(pourQuoi(ligne))}</span></span>`
        + bandeauEtats(ligne) + bandeauValidation(ligne);
      brancherValidation(div, ligne);
      brancherRecu(div);
      fermerEdition(div, !!ligne.valide);
      return;
    }
    // PLUSIEURS PAIES DANS UN BLOC : pas encore validées, sinon le bloc
    // aurait été démultiplié. Si l'une l'a été ailleurs (un autre onglet),
    // on démultiplie maintenant et on repeint.
    if (lignes.some((l) => l.valide)) { dedoubler(div); refaire = true; return; }
    cible.innerHTML = lignes.map((l) =>
      `<span class="etats-qui">${echapper(l.nom)}
         <span class="discret">— ${echapper(pourQuoi(l))}</span></span>` + bandeauEtats(l))
      .join("")
      + `<div class="validation">
           <span class="discret">${lignes.length} salariés dans cet engagement</span>
           <span class="boutons">
             ${BOUTON_RETIRER}
             <button class="secondaire v-valider-tout">Valider pour les ${lignes.length}</button>
           </span>
         </div>`;
    cible.querySelector(".v-valider-tout").onclick = (ev) => {
      ev.preventDefault();
      validerPlusieurs(div, lignes);
    };
    brancherRetirer(div);
    brancherRecu(div);
    fermerEdition(div, false);
  });
  if (refaire) return peindreEtats();
}

/** Valide chaque paie du bloc, PUIS le démultiplie : « il faut le bouton
 *  valider, et une fois validé ça se démultiplie ».
 *  Chaque bloc né ici se repeint validé, fermé, avec son propre
 *  « Modifier… ». Si une validation échoue, on s'arrête là : ce qui est
 *  validé l'est, le reste se voit au repeint. */
async function validerPlusieurs(div, lignes) {
  const mois = document.getElementById("mois").value;
  let faites = 0;
  try {
    for (const l of lignes) {
      await api("valider-paie", { mois, ...contratDe(l),
                                  valide: true, forcer: MOIS_FORCE === mois });
      faites += 1;
    }
    dire(`${faites} engagements validés.`);
  } catch (e) {
    dire(e.message + (faites ? ` (${faites} validé(s) avant)` : ""),
         "erreur");
  }
  if (faites) dedoubler(div);
  await dessinerEtatsDesPaies(mois);
}

/** Lit `etat-par-paie` et repeint. */
async function dessinerEtatsDesPaies(mois) {
  ETATS_PAIES = null;
  if (!mois) { peindreEtats(); return; }
  try {
    const r = await api("etat-par-paie", { mois });
    ETATS_PAIES = {};
    r.paies.forEach((l) => { ETATS_PAIES[clePaie(l.salarie, l.debut, l.rang)] = l; });
  } catch (e) {
    // Mois jamais enregistré, spectacle manquant, saisie en cours : il n'y a
    // rien à peindre, et rien d'alarmant à dire. La saisie, elle, marche.
    ETATS_PAIES = {};
  }
  peindreEtats();
}

/* ------------------------------------------------ valider une paie
   « On valide, c'est vert, normalement on n'y revient pas, édition bloquée
   temporairement avec un bouton pour modifier avec avertissement » —
   11 et 12/09/2026.

   Trois choses, et la troisième est celle qui compte.

   1. VERT, et sur le bloc entier. Une pastille de plus se serait perdue au
      milieu des quatre autres ; c'est le bloc qui change d'état, pas un
      détail dedans.
   2. ÉDITION FERMÉE. Les champs passent en lecture seule pour de vrai
      (`disabled`), pas seulement grisés : une paie validée qu'on peut encore
      modifier par distraction n'est pas validée.
   3. L'AVERTISSEMENT DIT CE QUE L'ÉDITEUR A DIT. Pas « êtes-vous sûr ? », qui
      ne renseigne personne — mais la conséquence exacte qu'il a lui-même
      nommée : modifier une paie validée peut obliger à refaire la DPAE. Et
      quand la DPAE est DÉJÀ partie pour cette paie, l'écran le sait
      (`etat_par_paie`) et le dit avec sa date. C'est la différence entre un
      avertissement qu'on lit et un avertissement qu'on clique. */

/** Ferme ou rouvre les champs d'un bloc. Le bouton de validation, lui, reste
 *  actif — sinon on ne pourrait plus rouvrir. */
function fermerEdition(div, ferme) {
  // Une carte fermée qui doit s'ouvrir se construit ici, une fois (ouvrirCarte).
  if (!ferme) ouvrirCarte(div);
  div.classList.toggle("validee", ferme);
  peindreCarte(div);
  div.querySelectorAll("input, select, button").forEach((c) => {
    if (c.classList.contains("v-basculer") || c.classList.contains("retirer-paie")) return;
    if (c.classList.contains("verrou")) return;    // un contrat en cours
    if (c.closest(".e-resume")) return;            // la ligne de la paie validée
    if (c.closest(".e-imprevus")) return;          // ses imprévus : la reprise, le maintien se disent après la paie
    if (c.classList.contains("chevron-pluri")) return;  // le chevron du contrat sur plusieurs mois (#127)
    c.disabled = ferme;
  });
  // Ses imprévus se redisent : sur une paie validée, un jour payé pendant
  // l'arrêt ne se retire plus d'un clic (js/imprevus.js).
  peindreImprevusDesCartes();
}

/* RETIRER UNE PAIE : un vrai bouton, en bas à droite, à côté de « Valider »
   — et non plus une petite croix à côté du métier (l'éditeur, 15/09/2026 :
   « plus clair, une case comme valider la paie, avec un avertissement »).
   L'avertissement n'est là que quand il y a quelque chose à perdre : une
   paie validée, un contrat envoyé à la signature ou revenu signé, une DPAE
   partie. Sinon le bloc s'en va, et l'enregistrement suit. */
const BOUTON_RETIRER = `<button class="lien retirer-paie" title="retirer cet engagement de la saisie">Retirer cet engagement</button>`;

/** Le bandeau de validation d'un bloc, sous les quatre pastilles. */
function bandeauValidation(ligne) {
  if (ligne.valide) {
    return `<div class="validation faite">
      <span><b>Engagement validé</b> le ${echapper(quand(ligne.valide))}</span>
      <span class="boutons">
        ${BOUTON_RETIRER}
        <button class="modifier v-basculer" data-valide="0">${CRAYON}Modifier…</button>
      </span>
    </div>`;
  }
  return `<div class="validation">
    <span class="boutons">
      ${BOUTON_RETIRER}
      <button class="primaire v-basculer" data-valide="1">Valider l'engagement</button>
    </span>
  </div>`;
}

/** Branche le bouton d'un bloc, une fois le bandeau posé. */
function brancherValidation(div, ligne) {
  const b = div.querySelector(".v-basculer");
  if (b) b.onclick = (ev) => {
    ev.preventDefault();
    if (b.dataset.valide === "1") return basculerValidation(ligne, true);
    boiteRouvrir(ligne);
  };
  brancherRetirer(div);
}

/** Le bouton « Retirer cette paie » d'un bloc, quel que soit son état. */
function brancherRetirer(div) {
  const b = div.querySelector(".retirer-paie");
  if (!b) return;
  b.onclick = (ev) => {
    ev.preventDefault();
    const lignes = paiesDuBloc(div)
      .map(etatDePaie)
      .filter(Boolean);
    const risques = [];
    lignes.forEach((l) => {
      const qui = l.nom;
      if (l.valide) risques.push(`${qui} : paie validée le ${quand(l.valide)}`);
      const s = l.etats.signature || {};
      if (s.recu) risques.push(`${qui} : contrat ${signeLe(s.recu, s.recu_par)}`);
      else if (s.fait) risques.push(`${qui} : contrat envoyé à la signature le ${quand(s.fait)}`);
      const d = l.etats.dpae || {};
      if (d.fait && !d.refusee) risques.push(`${qui} : DPAE déposée le ${quand(d.fait)}`);
    });
    const retirer = () => { div.remove(); rafraichirApercu(); dire("Engagement retiré."); };
    if (!risques.length) return retirer();
    ouvrirBoite(`
      <h3>Retirer cet engagement ?</h3>
      <div class="encart attention">
        <b>Quelque chose est déjà parti ou acté pour cet engagement</b>
        <ul>${risques.map((r) => `<li>${echapper(r)}</li>`).join("")}</ul>
      </div>
      <p class="note">Retirer l'engagement ne défait rien de tout cela : une DPAE
        déposée reste déposée à l'Urssaf, un contrat envoyé reste envoyé. Si
        l'embauche n'a pas lieu, c'est une annulation de DPAE qu'il faut faire,
        pas un retrait à l'écran. Le journal garde ce qui est parti.</p>
      <div class="actions">
        <button class="secondaire" onclick="fermerBoite()">Garder la paie</button>
        <button class="primaire" id="r-retirer">Retirer quand même</button>
      </div>`);
    document.getElementById("r-retirer").onclick = () => { fermerBoite(); retirer(); };
  };
}

async function basculerValidation(ligne, valide) {
  const mois = document.getElementById("mois").value;
  try {
    await api("valider-paie", { mois, ...contratDe(ligne), valide,
                                forcer: MOIS_FORCE === mois });
    dire(valide ? "Engagement validé." : "Engagement rouvert.");
    await dessinerEtatsDesPaies(mois);
  } catch (e) { dire(e.message, "erreur"); }
}

/* LES PAIES VALIDÉES ENSEMBLE SE ROUVRENT ENSEMBLE (24/09/2026). « Valider
 * ces 2 paies » les valide d'un geste, au même instant, puis l'écran les
 * sépare en un bloc par salarié. Les rouvrir demandait autant de boîtes que
 * de salariés, pour la même paie. Sœurs : même spectacle, mêmes bornes,
 * validées à la même seconde. Un même salarié n'est qu'une fois dans un
 * bloc : son jumeau (un autre contrat du même jour, #139) n'est jamais une
 * sœur, même validé à la même seconde. */
function paiesSoeurs(ligne) {
  const toutes = Object.values(ETATS_PAIES || {});
  const soeurs = toutes.filter((l) => l.valide && l.valide === ligne.valide
    && l.spectacle === ligne.spectacle && l.debut === ligne.debut && l.fin === ligne.fin
    && (l.salarie !== ligne.salarie || l.rang === ligne.rang));
  return soeurs.length ? soeurs : [ligne];
}

async function rouvrirPaies(lignes) {
  const mois = document.getElementById("mois").value;
  try {
    for (const l of lignes) {
      await api("valider-paie", { mois, ...contratDe(l), valide: false,
                                  forcer: MOIS_FORCE === mois });
    }
    dire(lignes.length > 1 ? `Ces ${lignes.length} engagements sont rouverts.`
                           : "Engagement rouvert.");
  } catch (e) { dire(e.message, "erreur"); }
  await dessinerEtatsDesPaies(mois);
}

/* Rouvrir demande un mot, pas un clic — QUAND IL Y A QUELQUE CHOSE À DIRE.
 * Depuis le 24/09/2026, une paie dont rien n'est parti se rouvre
 * directement. Depuis le 30/09/2026, la boîte ne parle plus de la DPAE
 * (l'utilisateur : « seulement quand on change les DATES ») : changer le cachet ou
 * les heures ne touche pas la DPAE, c'est LE CONTRAT qui devient caduc, et
 * qu'il faut renvoyer au salarié. La boîte n'est donc là que pour un contrat
 * parti ; la DPAE se dit au moment où l'on change vraiment le premier jour
 * ou le salarié (`avisDpaeDeplacee`). */
function boiteRouvrir(ligne) {
  const lignes = paiesSoeurs(ligne);
  const partis = lignes.filter((l) => l.etats && l.etats.signature
    && (l.etats.signature.fait || l.etats.signature.recu));
  if (!partis.length) return rouvrirPaies(lignes);
  const noms = lignes.map((l) => l.nom).join(", ");
  const etat = (l) => {
    const s = l.etats.signature;
    return s.recu ? signeLe(s.recu, s.recu_par) : "envoyé le " + quand(s.fait);
  };
  ouvrirBoite(`
    <h3>Modifier ${lignes.length > 1 ? `ces ${lignes.length} engagements` : "cet engagement"}</h3>
    <p class="discret">${echapper(noms)} — ${echapper(ligne.spectacle)},
      du ${echapper(jjmmaaaa(ligne.debut))} au ${echapper(jjmmaaaa(ligne.fin))}
      · validée${lignes.length > 1 ? "s" : ""} le ${echapper(quand(ligne.valide))}</p>
    <div class="encart attention">
      <b>${partis.length > 1 ? "Les contrats sont déjà partis" : "Le contrat est déjà parti"}</b>
      ${partis.map((l) => `${lignes.length > 1 ? echapper(l.nom) + " : c" : "C"}ontrat ${echapper(etat(l))}.`).join(" ")}
      Il porte les dates, les heures, les cachets et la rémunération : si
      l'un d'eux change, ce contrat n'est plus le bon. Sa pastille passera au
      rouge, et il faudra renvoyer le contrat à jour au salarié.
    </div>
    <p class="note">Modifier n'annule rien et ne défait rien : ça remet les
      champs en écriture. On revalide ensuite.</p>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="v-rouvrir">Modifier ${lignes.length > 1 ? "les paies" : "la paie"}</button>
    </div>`);
  document.getElementById("v-rouvrir").onclick = async () => {
    fermerBoite();
    await rouvrirPaies(lignes);
  };
}

/* LA DPAE NE SE DIT QUE SUR CE QU'ELLE DÉCLARE (30/09/2026). L'utilisateur :
 * l'avertissement doit correspondre aux vraies attentes de la DPAE, rien
 * d'inventé. Ce qu'elle déclare, c'est R1221-1 du code du travail : le
 * salarié, la date et l'heure d'embauche, la nature et la durée du contrat
 * (sa date de fin, pour un CDD) — ni la rémunération, ni les heures, ni les
 * cachets. L'encart ne paraît donc que si l'une de ces données s'écarte de
 * la DPAE déposée : premier jour ou salarié (le bloc retient l'embauche
 * déclarée la dernière fois qu'il l'a lue), ou fin du contrat
 * (`fin_declaree`, lue au journal du dépôt). Et il dit ce que dit l'Urssaf,
 * page « DPAE » d'urssaf.fr, mise à jour le 15/07/2026 : les informations
 * relatives au contrat de travail se modifient dans l'espace en ligne
 * (Compte › Embauche › Modifier une DPAE) ; pour toute autre modification,
 * une nouvelle DPAE, ou les services de l'Urssaf (3957). Rien de plus : ni
 * délai, ni annulation, qu'aucune source officielle lue ne pose. */
const URSSAF_DPAE = "https://www.urssaf.fr/accueil/employeur/embaucher-gerer-salaries/embaucher/declaration-prealable-embauche.html";

function avisDpaeDeplacee(div, ps, ligne) {
  const cible = div.querySelector(".etats");
  let avis = div.querySelector(".avis-dpae");
  const d = div._dpaeFaite;
  const ecarts = [];
  if (d && !ps.some((p) => p.salarie === d.salarie && p.debut === d.debut)) {
    ecarts.push(ps.some((p) => p.salarie === d.salarie)
      ? `premier jour : la DPAE déclare le ${jjmmaaaa(d.debut)}`
      : `salarié : la DPAE déclare ${d.nom}`);
  }
  const dp = ligne && ligne.etats && ligne.etats.dpae;
  if (dp && dp.fait && !dp.refusee && dp.fin_declaree && dp.fin_declaree !== ligne.fin) {
    ecarts.push(`fin du contrat : la DPAE déclare le ${jjmmaaaa(dp.fin_declaree)},
      la paie dit le ${jjmmaaaa(ligne.fin)}`);
  }
  if (!ecarts.length) { if (avis) avis.remove(); return; }
  if (!avis) {
    avis = document.createElement("div");
    avis.className = "encart attention avis-dpae";
    cible.parentNode.insertBefore(avis, cible);
  }
  const quandDpae = d ? d.quand : dp.fait;
  avis.innerHTML = `<b>La paie ne correspond plus à la DPAE déposée le ${echapper(quand(quandDpae))}</b>
    <ul>${ecarts.map((x) => `<li>${echapper(x)}</li>`).join("")}</ul>
    Selon l'Urssaf, les informations relatives au contrat de travail se
    modifient dans son espace en ligne (Compte › Embauche › Modifier une
    DPAE) ; pour toute autre modification, on dépose une nouvelle DPAE ou on
    contacte l'Urssaf (3957).
    <a href="${URSSAF_DPAE}" target="_blank" rel="noopener">La page de l'Urssaf</a>`;
}

/* PLUS DE CASE DE SÉLECTION NI DE BARRE D'ACTIONS GROUPÉES (15/09/2026) :
 * « on pourrait enlever la petite coche de sélection, tout se fait
 * directement quand on clique sur le bouton déposer DPAE ou demander les
 * signatures ». Les deux boîtes de la colonne de droite listent chaque
 * contrat du mois avec sa case, cochée d'avance sur ce qui reste à faire :
 * c'est là que l'on choisit, et rien ne part sans y être passé. */

/* LA BOÎTE DPAE, comme celle des signatures (l'éditeur, 15/09/2026 : « qu'il
 * y ait la liste déroulante et que soient déjà sélectionnés les contrats qui
 * n'ont pas de DPAE »). Chaque contrat du mois enregistré, une case ; cochée
 * d'avance si aucune DPAE n'est partie, ou si elle a été refusée. Un contrat
 * en cours depuis un mois précédent n'a pas de DPAE ce mois-ci : il se voit,
 * grisé, avec le mois où elle a été faite.
 *
 * DEUX TEMPS, ET C'EST VOULU : on ÉCRIT le fichier des embauches cochées,
 * puis on ouvre le récapitulatif de dépôt habituel. Écrire n'envoie rien et
 * ne consomme aucun compteur ; le dépôt reste le même geste, avec la même
 * relecture ligne à ligne et le même mot de passe. Le fichier prend un nom
 * horodaté, à côté du fichier complet du mois, jamais par-dessus. */
async function boiteDpae(mois) {
  if (!mois) { dire("Choisir un mois."); return; }
  // LA SAISIE À L'ÉCRAN D'ABORD (ticket #105, 02/10/2026). Cette boîte lit la
  // saisie ENREGISTRÉE : une paie tapée il y a moins d'une seconde, ou qu'un
  // spectacle manquant empêche d'enregistrer, n'y figurerait pas -- et sa
  // DPAE ne partirait pas, sans que rien ne le dise.
  let pasEnregistre = "";
  const ecran = document.getElementById("mois");
  if (ecran && ecran.value === mois && typeof enregistrerSaisie === "function") {
    clearTimeout(MINUTEUR_SAISIE);
    await enregistrerSaisie(true);
    if (JSON.stringify(saisieCourante()) !== DERNIERE_ECRITE) {
      pasEnregistre = document.getElementById("note-saisie").textContent
        || "La saisie à l'écran n'est pas enregistrée.";
    }
  }
  let r;
  try {
    r = await api("etat-par-paie", { mois });
  } catch (e) {
    dire("Rien d'enregistré pour " + moisEnClair(mois) + " : saisir une paie d'abord.");
    return;
  }
  const lignes = r.paies || [];
  if (!lignes.length) { dire("Aucune paie enregistrée en " + moisEnClair(mois) + "."); return; }
  const embauche = (l) => !l.contrat || l.contrat.premier_mois;
  // Déjà partie ET pas refusée : elle ne repart pas. Une refusée, si — c'est
  // même la seule raison de redéposer.
  const aFaire = (l) => embauche(l) && (!l.etats.dpae.fait || l.etats.dpae.refusee);

  ouvrirBoite(`
    <h3>Envoyer les DPAE</h3>
    ${moisEnGros(mois)}
    ${pasEnregistre ? `<div class="encart attention"><b>Une partie de l'écran
      n'est pas enregistrée</b> ${echapper(pasEnregistre)} Les paies qui ne
      figurent pas ci-dessous n'auront pas leur DPAE : compléter la saisie,
      puis rouvrir cette fenêtre.</div>` : ""}
    <p class="note">Cochées d'avance : les embauches sans DPAE, ou dont la
      DPAE a été refusée. Une DPAE déjà passée ne se redéclare pas : l'Urssaf
      recevrait deux déclarations pour un seul contrat.</p>
    <div class="liste-choix">
      ${lignes.map((l, i) => {
        const e = l.etats.dpae;
        const etat = !embauche(l)
          ? `<br><span class="pastille">contrat en cours depuis
               ${echapper(moisEnClair(l.contrat.depuis))} — sa DPAE a été faite ce mois-là</span>`
          : !e.fait ? `<br><span class="pastille alerte">à déclarer</span>`
          : e.refusee
          ? `<br><span class="pastille erreur">refusée le ${echapper(quand(e.fait))}
               — ${echapper((e.anomalies || []).join(" ; ") || "motif non lu")}
               — à redéclarer</span>`
          : e.passee
          ? `<br><span class="pastille bien">déclarée le ${echapper(quand(e.fait))}
               — passée${echapper(dossierUrssaf(e))}</span>`
          : `<br><span class="pastille alerte">déclarée le ${echapper(quand(e.fait))}
               — retour pas encore arrivé : on attend, on ne renvoie pas</span>`;
        return `<label class="choix ${embauche(l) ? "" : "grise"}">
          <input type="checkbox" class="d-qui" data-i="${i}" ${aFaire(l) ? "checked" : ""}
                 ${embauche(l) ? "" : "disabled"}>
          <span><b>${echapper(l.nom)}</b> — ${echapper(l.spectacle)}, embauche le
            ${echapper(jjmmaaaa(l.debut))}${l.numero
              ? ` <span class="discret">(contrat n° ${echapper(l.numero)})</span>` : ""}
            ${etat}
            ${e.date_changee ? `<br><span class="note">${echapper(e.date_changee.phrase)}</span>` : ""}</span></label>`;
      }).join("")}
    </div>
    <p class="note">Heure d'embauche déclarée : 08h00. Toutes les embauches
      cochées partent ensemble, dans un seul dépôt. D'abord le test, qui ne
      déclare rien ; une fois le test revenu conforme, l'envoi réel.</p>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Fermer</button>
      <button class="primaire" id="d-ecrire">Envoyer le test…</button>
    </div>`);

  const bouton = document.getElementById("d-ecrire");
  const majBouton = () => {
    const n = document.querySelectorAll(".d-qui:checked").length;
    bouton.disabled = !n;
    bouton.textContent = n ? `Envoyer le test (${n} embauche${n > 1 ? "s" : ""})…` : "Rien de coché";
  };
  document.querySelectorAll(".d-qui").forEach((c) => { c.onchange = majBouton; });
  majBouton();
  bouton.onclick = async () => {
    const choisies = [...document.querySelectorAll(".d-qui:checked")]
      .map((c) => lignes[+c.dataset.i]);
    if (!choisies.length) return;
    const rendre = occupe(bouton, "Fabrication du fichier en cours…");
    try {
      const w = await api("dpae", {
        mois, paies: choisies.map((l) => ({ salarie: l.salarie, debut: l.debut })),
      });
      fermerBoite();
      // Le dossier porte le fichier à déposer et son récapitulatif ; c'est le
      // premier qui se dépose.
      const f = (w.fichiers || []).find(
        (x) => /\.txt$/i.test(x.nom) && !/recapitulatif/i.test(x.nom));
      dire("Fichier écrit : " + (f ? f.nom : w.dossier));
      await rafraichirActions();
      if (f) boiteDepot("dpae", f.chemin, { testerDabord: true });
    } catch (e) {
      rendre();
      dire(e.message, "erreur");
    }
  };
}

