/* La compagnie fictive : les notes de l'envoi simule, les signatures jouees, les courriels expliques. */
"use strict";

/* LA COMPAGNIE FICTIVE (le moteur : `simulation` de la route `etat`, interface/compagnies.rs) : même interface, mêmes étapes,
   mais ce qui part est joué — dépôts, retours, courriels, signatures. Le
   moteur le dit dans `etat` ; la page ne fait que ne pas demander de mot de
   passe, et le dire au moment de l'acte. */
const enFictive = () => !!(ETAT && ETAT.simulation);
const noteFictive = () => encart(ETAT && ETAT.note_fictive);

/* ---------------------------------------- LA COMPAGNIE FICTIVE QUI EXPLIQUE
   La démo est informative : à chaque courriel joué, une petite fenêtre montre ce que le
   salarié reçoit — le vrai courriel, écrit par le moteur dans
   courriels-simules/ — et le circuit en trois bulles : ce qui part, ce que
   son clic prépare, ce qui revient. Rien n'y est inventé par la page : les
   messages sont ceux du moteur, la réponse est celle que le bouton du
   courriel prépare. */
async function repereCourrielsFictifs() {
  if (!enFictive()) return null;
  try { return (await api("courriels-fictifs", { apres: "￿" })).dernier; }
  catch (e) { return null; }
}

/* Le retour d'une signature jouée, le moteur le dit (`contresigne` de `courriels-fictifs`). Le circuit de chaque genre (ce que reçoit le destinataire,
   ses étapes et ce qu'elles disent) vient du moteur : `circuits` de `courriels-fictifs`. */

/* CE QU'UNE COMPAGNIE FICTIVE NE FAIT PAS, DIT AU LIEU D'ÊTRE TU : le moteur le rend (`coulisses` de la route `etat`). */
function fenetreExplicationFictive(quoi, apres = "") {
  const e = (ETAT && ETAT[quoi]) || { titre: "", sections: [] };
  ouvrirBoite(`
    <div class="fictive-fenetre">
      <h3>Compagnie fictive — ${echapper(e.titre)}</h3>
      ${apres ? `<p class="intro">${echapper(apres)}</p>` : ""}
      ${e.sections.map(([t, x]) => `<h4>${echapper(t)}</h4><p>${echapper(x.replace(/\s+/g, " "))}</p>`).join("")}
      <div class="actions"><button class="primaire" onclick="fermerBoite()">Compris</button></div>
    </div>`);
}

let ATTENTE_FICTIVE = null;
async function fenetreCourrielsFictifs(genre, repere) {
  if (!enFictive() || repere === null) return;
  let partis = [], retours = [], etape = 0, fiche = null, lu;
  try {
    lu = await api("courriels-fictifs", { apres: repere || "" });
    partis = lu.courriels;
  } catch (e) { return; }
  const c = lu.circuits[genre];
  if (!partis.length) return;
  let premier = partis[0];

  const apercu = (m) => `
    <div class="fictive-courriel">
      <div class="fictive-entete"><span>De</span> ${echapper(m.de)}<br>
        <span>À</span> ${echapper(m.a)}<br><span>Objet</span> <b>${echapper(m.sujet)}</b>
        ${m.pieces.length ? `<br><span>Pièce jointe</span> ${m.pieces.map((x, i) => x.pages
          ? `<button class="lien fictive-piece" data-courriel="${echapper(m.nom)}" data-rang="${i}"
              data-pages="${x.pages}" title="L'ouvrir ici">📎 ${echapper(x.nom)}</button>`
          : `📎 ${echapper(x.nom)}`).join(" ")}` : ""}</div>
      <div class="fictive-pdf" hidden></div>
      <iframe class="fictive-corps" sandbox title="Le courriel"></iframe>
    </div>`;
  // LA PIÈCE JOINTE S'OUVRE ICI : le PDF affiché par le lecteur du
  // navigateur — le contrat tel que le salarié l'ouvre, et signé à l'étape 3.
  const brancherPieces = () => document.querySelectorAll(".fictive-piece").forEach((b) => {
    b.onclick = () => {
      const zone = b.closest(".fictive-courriel").querySelector(".fictive-pdf");
      const corps = b.closest(".fictive-courriel").querySelector(".fictive-corps");
      if (!zone.hidden && zone.dataset.rang === b.dataset.rang) {
        zone.hidden = true; corps.hidden = false; return;
      }
      zone.dataset.rang = b.dataset.rang;
      const adresse = `/fictive/page?c=${encodeURIComponent(b.dataset.courriel)}&p=${
        b.dataset.rang}&jeton=${encodeURIComponent(JETON)}`;
      zone.innerHTML = `<button class="lien fictive-retour">← revenir au courriel</button>
        <a class="lien" href="${adresse}" target="_blank" rel="noopener">ouvrir le PDF</a>
        <iframe class="fictive-pdf-cadre" title="${echapper(b.textContent.trim())}" src="${adresse}#page=1"></iframe>`;
      zone.hidden = false; corps.hidden = true;
      zone.querySelector(".fictive-retour").onclick = () => { zone.hidden = true; corps.hidden = false; };
    };
  });
  const reponse = (b) => `
    <div class="fictive-courriel fictive-reponse">
      <div class="fictive-entete"><span>De</span> ${echapper(premier.a)}<br>
        <span>À</span> ${echapper(premier.de)}<br><span>Objet</span> <b>${echapper(b.sujet)}</b></div>
      <pre class="fictive-texte">${echapper(b.corps)}</pre>
    </div>`;

  const dessiner = () => {
    const b = document.getElementById("boite");
    if (!b || !b.querySelector(".fictive-fenetre")) return;
    b.querySelectorAll(".fictive-bulle").forEach((x, i) => {
      x.classList.toggle("actif", i === etape);
      x.classList.toggle("fait", i < etape || (i === 2 && (retours.length || fiche)));
    });
    let corps = "";
    if (etape === 0) corps = apercu(premier);
    else if (etape === 1) corps = premier.bouton ? reponse(premier.bouton)
      : `<p class="discret">${echapper(lu.circuits.sans_bouton)}</p>`;
    else if (retours.length) corps = apercu(retours[0]);
    else if (fiche) corps = `<div class="encart bien"><b>Fiche créée — ${echapper(fiche)}</b>
      ${echapper(c.creee)}</div>`;
    else corps = `<p class="discret">⏳ ${echapper(c.attente || "")}</p>`;
    b.querySelector(".fictive-etape").innerHTML = `<p class="note">${echapper(c.notes[etape])}</p>${corps}`;
    const f = b.querySelector(".fictive-corps");
    const m = etape === 0 ? premier : retours[0];
    if (f && m) f.srcdoc = m.html || `<pre style="font-family:sans-serif;white-space:pre-wrap">${echapper(m.texte)}</pre>`;
    brancherPieces();
  };

  ouvrirBoite(`
    <div class="fictive-fenetre">
      <h3>Compagnie fictive — ${echapper(c.titre)}</h3>
      <p class="intro">${echapper(lu.intro)}</p>
      ${partis.length > 1 ? `<div class="fictive-destinataires">${partis.map((m, i) => `
        <button class="${i === 0 ? "primaire" : "secondaire"} fictive-destinataire" data-rang="${i}"
          title="${echapper(m.sujet)}">${echapper(m.a)}</button>`).join("")}</div>` : ""}
      ${c.bulles.length > 1 ? `<ol class="fictive-bulles">${c.bulles.map((t, i) => `
        <li><button class="fictive-bulle" data-etape="${i}"><span>${i + 1}</span>${echapper(t)}</button></li>`).join("")}</ol>` : ""}
      <div class="fictive-etape"></div>
      <div class="actions">
        ${c.bulles.length > 1 ? `<button class="secondaire" id="fictive-suivant">Étape suivante</button>` : ""}
        <button class="primaire" onclick="fermerBoite()">Compris</button>
      </div>
    </div>`, () => { clearTimeout(ATTENTE_FICTIVE); });
  document.querySelectorAll(".fictive-bulle").forEach((x) => {
    x.onclick = () => { etape = +x.dataset.etape; dessiner(); };
  });
  // UN COURRIEL PAR SALARIÉ, ET ILS DIFFÈRENT : le permanent reçoit son
  // bulletin, l'intermittent aussi son certificat Congés Spectacles et son
  // AER. On choisit celui qu'on regarde.
  document.querySelectorAll(".fictive-destinataire").forEach((x) => {
    x.onclick = () => {
      premier = partis[+x.dataset.rang];
      document.querySelectorAll(".fictive-destinataire").forEach((y) => {
        y.classList.toggle("primaire", y === x);
        y.classList.toggle("secondaire", y !== x);
      });
      etape = 0;
      dessiner();
      if (genre === "signature") {
        api("courriels-fictifs", { apres: partis[partis.length - 1].nom }).then((r) => {
          retours = r.courriels.filter((m) => m.contresigne && m.a === premier.a);
        }).catch(() => {});
      }
    };
  });
  const suivant = document.getElementById("fictive-suivant");
  if (suivant) suivant.onclick = () => { etape = (etape + 1) % c.bulles.length; dessiner(); };
  dessiner();

  // L'étape 3 arrive d'elle-même : on la guette tant que la fenêtre est là.
  if (c.bulles.length < 3) return;
  const depuis = Date.now();
  const guetter = async () => {
    if (!document.querySelector(".fictive-fenetre") || Date.now() - depuis > 90000) return;
    try {
      if (genre === "signature") {
        const r = await api("courriels-fictifs", { apres: partis[partis.length - 1].nom });
        retours = r.courriels.filter((m) => m.contresigne && m.a === premier.a);
      } else if (Date.now() - depuis > 21000) {
        const r = await api("fiches-relever", {});
        ETAT.configuration = await api("configuration", {}); peindreEtatGlobal();
        if (r.recues.length) fiche = r.recues.map((x) => x.nom).join(", ");
        // Déjà relevée par la veille : plus rien n'attend, la fiche est là.
        else if (!r.attendues) fiche = premier.a;
      }
    } catch (e) { /* on réessaie */ }
    if (retours.length || fiche) {
      if (etape === 2) dessiner();
      else document.querySelectorAll(".fictive-bulle")[2].classList.add("fait");
      return;
    }
    ATTENTE_FICTIVE = setTimeout(guetter, 3000);
  };
  ATTENTE_FICTIVE = setTimeout(guetter, 3000);
}

