/* La demonstration : les notes de l'envoi simule, les signatures jouees, les courriels expliques. */
"use strict";

/* LA DÉMONSTRATION (le moteur : routes_accueil::simulation_active, editeur.rs) : même interface, mêmes étapes,
   mais ce qui part est joué — dépôts, retours, courriels, signatures. Le
   moteur le dit dans `etat` ; la page ne fait que ne pas demander de mot de
   passe, et le dire au moment de l'acte. */
const enDemonstration = () => !!(ETAT && ETAT.simulation);
const noteDemonstration = (vers, reponse = true) => `<p class="note"><b>Démonstration</b> —
  rien ne part vers ${echapper(vers)} : l'envoi est simulé${reponse
  ? ", et sa réponse arrive comme la vraie, quelques secondes plus tard" : ""}.</p>`;
/* LA DPAE PRÉPARÉE D'AVANCE, DITE DANS LA DÉMONSTRATION (27/09/2026) : le
   test part en silence (filet.rs) et le filet n'y existe pas — une
   ligne, là où naissent les contrats, pour que le visiteur sache l'un et
   l'autre. */
const noteDpaeAvance = () => enDemonstration() ? `<p class="note"><b>La DPAE préparée
  d'avance</b> — dès que les contrats sont écrits ou envoyés, la DPAE de chaque
  embauche se prépare et part en test, d'elle-même : le jour venu, il ne reste
  qu'à l'envoyer. Dans la version complète, une option (Configuration › Mots de
  passe) envoie en plus un courriel de rappel si elle n'est pas partie la veille
  de l'embauche.</p>` : "";

/** LA DÉMONSTRATION : les salariés fictifs signent une vingtaine de secondes
 *  après l'envoi (releve_signatures.rs, boite_des_signatures). La page relève à ce moment-là,
 *  comme la veille l'aurait fait à son passage. */
function attendreSignaturesDemo() {
  setTimeout(async () => {
    try { await api("veille-maintenant"); } catch (e) { return; }
    await rafraichirActions();
    dire("Démonstration : les contrats sont revenus signés.");
  }, 22000);
}

/* ---------------------------------------- LA DÉMONSTRATION QUI EXPLIQUE
   (27/09/2026, l'éditeur) : « il faut vraiment que ce soit une démo
   informative, qui nous renseigne aussi sur le fonctionnement de notre
   logiciel ». À chaque courriel joué, une petite fenêtre montre ce que le
   salarié reçoit — le vrai courriel, écrit par le moteur dans
   courriels-simules/ — et le circuit en trois bulles : ce qui part, ce que
   son clic prépare, ce qui revient. Rien n'y est inventé par la page : les
   messages sont ceux du moteur, la réponse est celle que le bouton du
   courriel prépare. */
async function repereCourrielsDemo() {
  if (!enDemonstration()) return null;
  try { return (await api("demonstration-courriels", { apres: "￿" })).dernier; }
  catch (e) { return null; }
}

const CIRCUITS_DEMO = {
  signature: {
    titre: "Ce que reçoit le salarié",
    bulles: ["Le contrat part", "Il appuie sur « Je signe »", "Le contrat signé lui revient"],
    notes: [
      "Un courriel par salarié, le contrat en pièce jointe et un bouton « Je signe ».",
      "Le bouton ouvre sa messagerie sur cette réponse toute écrite, avec son code unique : il n'a qu'à l'envoyer. Le logiciel la relève seul, à l'ouverture et avec la veille, et la garde comme preuve.",
      "Dès la signature relevée, le contrat contresigné part tout seul — le seul envoi sans clic du logiciel, puisque le clic est celui du salarié. La pastille du contrat passe au vert.",
    ],
    attente: "La signature du salarié fictif arrive une vingtaine de secondes après l'envoi.",
    retour: /signé/i,
  },
  fiche: {
    titre: "Ce que reçoit la personne",
    bulles: ["La demande part", "Elle appuie sur « Remplir ma fiche »", "Sa fiche se crée"],
    notes: [
      "Un courriel avec un bouton « Remplir ma fiche », et les questions recopiées dans le texte pour les messageries qui n'affichent pas le bouton.",
      "Le bouton ouvre sa messagerie sur les questions, une par ligne : elle écrit après chaque deux-points et envoie. Seul ce qu'elle sait lui est demandé — l'emploi reste à l'employeur.",
      "Sa réponse est relevée comme une signature : la fiche salarié se crée avec ce qu'elle a écrit, marquée « à compléter » pour l'emploi. Une réponse illisible laisse la case vide, et le dit.",
    ],
    attente: "La réponse de la personne fictive arrive une vingtaine de secondes après l'envoi.",
  },
  documents: {
    titre: "Ce que reçoit le salarié",
    bulles: ["Les documents partent"],
    notes: ["Un courriel par salarié, avec ses documents en pièces jointes. L'envoi est noté au journal, et un second envoi le signale avant de repartir."],
  },
  courriel: {
    titre: "Le courriel envoyé",
    bulles: ["Le courriel part"],
    notes: ["Voici le message tel qu'il serait parti. Dans la démonstration, il est écrit dans le dossier courriels-simules/ de la compagnie, et rien ne quitte le poste."],
  },
};

/* CE QUE LA DÉMONSTRATION NE FAIT PAS, DIT AU LIEU D'ÊTRE TU (27/09/2026).
   La sauvegarde et les mises à jour sont coupées dans la démo, et quatre
   salariés fictifs n'ont volontairement pas de taux de PAS : le visiteur
   doit savoir ce que fait la version complète, et pourquoi il voit ce qu'il
   voit. Les phrases disent le logiciel tel qu'il est (sauvegarde.rs,
   maj.rs, pas.rs, amorcage_demande.rs, dans atelier/sources/paie/src/), rien de plus. */
const EXPLICATIONS_DEMO = {
  coulisses: {
    titre: "Les coulisses de la démonstration",
    sections: [
      ["La sauvegarde", `Dans la version complète, à l'ouverture puis toutes les vingt minutes
        quand quelque chose a changé, le dossier de la compagnie — saisies, bulletins,
        contrats, déclarations, retours, journal — part dans son propre Google Drive.
        Seul ce qui a changé repart, et rien de ce qui est effacé sur le poste ne l'est
        dans le Drive. Les mots de passe, eux, ne quittent jamais l'ordinateur. Le
        logiciel ne voit dans le Drive que ce qu'il y a lui-même créé.`],
      ["Les mises à jour", `À l'ouverture, le logiciel regarde si l'éditeur a publié une
        nouvelle version. Il la télécharge en arrière-plan, vérifie qu'elle est bien
        signée par l'éditeur, et l'installe au démarrage suivant, sans rien demander.
        Une version qui change un calcul attend que la DSN du mois en cours soit
        déposée ; une version marquée « exigée » (une norme DSN qui change) bloque les
        dépôts réels tant qu'elle n'est pas installée.`],
      ["Dans la démonstration", `Ni l'une ni l'autre ne tourne : aucun compte Google n'est
        lié, rien ne quitte le poste, et la version reste celle de la démonstration.
        Le bandeau dit « À jour » parce que la relève des retours, elle, est jouée.`],
      ["L'horloge de la démonstration", `La compagnie vit sans vous. À chaque ouverture,
        ce qui tombait depuis votre dernier passage a été fait par un administrateur
        fictif, chaque geste à sa date limite : le 1er du mois, le mois s'écrit ; le
        jour d'une embauche, sa DPAE part et son contrat revient signé ; le 15, la DSN
        du mois précédent part, puis les documents aux salariés ; fin janvier, l'année
        passée se clôt. Avant la date limite, c'est à vous : faites la DPAE deux jours
        avant, ou la DSN le 14, et l'horloge ne la refera pas. Ajoutez ou retirez un
        contrat : à votre retour, le mois est réécrit et tout est à jour.`],
    ],
  },
  pas: {
    titre: "Les taux de prélèvement à la source",
    sections: [
      ["D'où vient le taux", `Chaque salarié a un taux, que la DGFiP transmet dans un compte
        rendu (nature 94) après chaque DSN. Il vaut jusqu'à la fin du deuxième mois qui
        suit : un intermittent qui n'a pas travaillé depuis quelques mois n'en a donc
        plus de valide.`],
      ["Sans taux valide", `La loi impose la grille par défaut de l'article 204 H du code
        général des impôts, qui prélève le plus souvent plus que le vrai taux. La paie
        sort quand même ; le bandeau le signale des semaines avant la DSN, pour qu'on
        ait le temps d'agir.`],
      ["Comment l'éviter", `Quand la DPAE d'une embauche s'écrit, le logiciel écrit aussi,
        pour chaque salarié sans taux valide, une demande de taux à la DGFiP (signalement
        d'amorçage), à déposer comme une DSN. La DGFiP répond sous cinq jours ouvrés.`],
      ["Dans la démonstration", `Ces salariés fictifs n'ont volontairement aucun taux : leurs
        bulletins et leurs DSN montrent la grille par défaut, telle qu'elle s'applique.`],
    ],
  },
};

function fenetreExplicationDemo(quoi, apres = "") {
  const e = EXPLICATIONS_DEMO[quoi];
  ouvrirBoite(`
    <div class="demo-fenetre">
      <h3>Démonstration — ${echapper(e.titre)}</h3>
      ${apres ? `<p class="intro">${echapper(apres)}</p>` : ""}
      ${e.sections.map(([t, x]) => `<h4>${echapper(t)}</h4><p>${echapper(x.replace(/\s+/g, " "))}</p>`).join("")}
      <div class="actions"><button class="primaire" onclick="fermerBoite()">Compris</button></div>
    </div>`);
}

let ATTENTE_DEMO = null;
async function fenetreCourrielsDemo(genre, repere) {
  if (!enDemonstration() || repere === null) return;
  const c = CIRCUITS_DEMO[genre];
  let partis = [], retours = [], etape = 0, fiche = null;
  try {
    partis = (await api("demonstration-courriels", { apres: repere || "" })).courriels;
  } catch (e) { return; }
  if (!partis.length) return;
  const premier = partis[0];

  const apercu = (m) => `
    <div class="demo-courriel">
      <div class="demo-entete"><span>De</span> ${echapper(m.de)}<br>
        <span>À</span> ${echapper(m.a)}<br><span>Objet</span> <b>${echapper(m.sujet)}</b>
        ${m.pieces.length ? `<br><span>Pièce jointe</span> ${m.pieces.map((x, i) => x.pages
          ? `<button class="lien demo-piece" data-courriel="${echapper(m.nom)}" data-rang="${i}"
              data-pages="${x.pages}" title="L'ouvrir ici">📎 ${echapper(x.nom)}</button>`
          : `📎 ${echapper(x.nom)}`).join(" ")}` : ""}</div>
      <div class="demo-pdf" hidden></div>
      <iframe class="demo-corps" sandbox title="Le courriel"></iframe>
    </div>`;
  // LA PIÈCE JOINTE S'OUVRE ICI : le PDF affiché par le lecteur du
  // navigateur — le contrat tel que le salarié l'ouvre, et signé à l'étape 3.
  const brancherPieces = () => document.querySelectorAll(".demo-piece").forEach((b) => {
    b.onclick = () => {
      const zone = b.closest(".demo-courriel").querySelector(".demo-pdf");
      const corps = b.closest(".demo-courriel").querySelector(".demo-corps");
      if (!zone.hidden && zone.dataset.rang === b.dataset.rang) {
        zone.hidden = true; corps.hidden = false; return;
      }
      zone.dataset.rang = b.dataset.rang;
      const adresse = `/demonstration/page?c=${encodeURIComponent(b.dataset.courriel)}&p=${
        b.dataset.rang}&jeton=${encodeURIComponent(JETON)}`;
      zone.innerHTML = `<button class="lien demo-retour">← revenir au courriel</button>
        <a class="lien" href="${adresse}" target="_blank" rel="noopener">ouvrir le PDF</a>
        <iframe class="demo-pdf-cadre" title="${echapper(b.textContent.trim())}" src="${adresse}#page=1"></iframe>`;
      zone.hidden = false; corps.hidden = true;
      zone.querySelector(".demo-retour").onclick = () => { zone.hidden = true; corps.hidden = false; };
    };
  });
  const reponse = (b) => `
    <div class="demo-courriel demo-reponse">
      <div class="demo-entete"><span>De</span> ${echapper(premier.a)}<br>
        <span>À</span> ${echapper(premier.de)}<br><span>Objet</span> <b>${echapper(b.sujet)}</b></div>
      <pre class="demo-texte">${echapper(b.corps)}</pre>
    </div>`;

  const dessiner = () => {
    const b = document.getElementById("boite");
    if (!b || !b.querySelector(".demo-fenetre")) return;
    b.querySelectorAll(".demo-bulle").forEach((x, i) => {
      x.classList.toggle("actif", i === etape);
      x.classList.toggle("fait", i < etape || (i === 2 && (retours.length || fiche)));
    });
    let corps = "";
    if (etape === 0) corps = apercu(premier);
    else if (etape === 1) corps = premier.bouton ? reponse(premier.bouton)
      : `<p class="discret">Ce courriel n'a pas de bouton.</p>`;
    else if (retours.length) corps = apercu(retours[0]);
    else if (fiche) corps = `<div class="encart bien"><b>Fiche créée : ${echapper(fiche)}</b>
      Elle est dans Configuration › Salariés, avec ce que la personne a écrit ; il reste à
      choisir son emploi.</div>`;
    else corps = `<p class="discret">⏳ ${echapper(c.attente || "")}</p>`;
    b.querySelector(".demo-etape").innerHTML = `<p class="note">${echapper(c.notes[etape])}</p>${corps}`;
    const f = b.querySelector(".demo-corps");
    const m = etape === 0 ? premier : retours[0];
    if (f && m) f.srcdoc = m.html || `<pre style="font-family:sans-serif;white-space:pre-wrap">${echapper(m.texte)}</pre>`;
    brancherPieces();
  };

  ouvrirBoite(`
    <div class="demo-fenetre">
      <h3>Démonstration — ${echapper(c.titre)}</h3>
      <p class="intro">${partis.length} courriel${partis.length > 1 ? "s" : ""} joué${partis.length > 1 ? "s" : ""}
        — rien n'a quitté le poste.${partis.length > 1 ? ` Voici celui de ${echapper(premier.a)} ; les autres
        sont sur le même modèle.` : ""}</p>
      ${c.bulles.length > 1 ? `<ol class="demo-bulles">${c.bulles.map((t, i) => `
        <li><button class="demo-bulle" data-etape="${i}"><span>${i + 1}</span>${echapper(t)}</button></li>`).join("")}</ol>` : ""}
      <div class="demo-etape"></div>
      <div class="actions">
        ${c.bulles.length > 1 ? `<button class="secondaire" id="demo-suivant">Étape suivante</button>` : ""}
        <button class="primaire" onclick="fermerBoite()">Compris</button>
      </div>
    </div>`, () => { clearTimeout(ATTENTE_DEMO); });
  document.querySelectorAll(".demo-bulle").forEach((x) => {
    x.onclick = () => { etape = +x.dataset.etape; dessiner(); };
  });
  const suivant = document.getElementById("demo-suivant");
  if (suivant) suivant.onclick = () => { etape = (etape + 1) % c.bulles.length; dessiner(); };
  dessiner();

  // L'étape 3 arrive d'elle-même : on la guette tant que la fenêtre est là.
  if (c.bulles.length < 3) return;
  const depuis = Date.now();
  const guetter = async () => {
    if (!document.querySelector(".demo-fenetre") || Date.now() - depuis > 90000) return;
    try {
      if (genre === "signature") {
        const r = await api("demonstration-courriels", { apres: partis[partis.length - 1].nom });
        retours = r.courriels.filter((m) => c.retour.test(m.sujet) && m.a === premier.a);
      } else if (Date.now() - depuis > 21000) {
        const r = await api("fiches-relever", {});
        ETAT.configuration = await api("configuration", {});
        if (r.recues.length) fiche = r.recues.map((x) => x.nom).join(", ");
        // Déjà relevée par la veille : plus rien n'attend, la fiche est là.
        else if (!r.attendues) fiche = premier.a;
      }
    } catch (e) { /* on réessaie */ }
    if (retours.length || fiche) {
      if (etape === 2) dessiner();
      else document.querySelectorAll(".demo-bulle")[2].classList.add("fait");
      return;
    }
    ATTENTE_DEMO = setTimeout(guetter, 3000);
  };
  ATTENTE_DEMO = setTimeout(guetter, 3000);
}

