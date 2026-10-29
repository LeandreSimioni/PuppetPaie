/* LES CONSEILS « LE SAVIEZ-VOUS ? » (ticket 224, 21/10/2026). Une option de
 * la fiche Entreprise, décochée par défaut : sans elle, le moteur ne rend
 * rien, et rien ne s'affiche. Tout se décide dans le moteur
 * (moteur/src/conseils.rs, la banque dans donnees/conseils.json) ; la page
 * affiche :
 *   - à l'accueil, le conseil du jour et « un autre » (`conseil-du-jour`),
 *     et ce que les contrats de la compagnie ont fait pour les droits de
 *     ses salariés (`conseils-resume`) ;
 *   - sous la carte d'un engagement, le conseil que la saisie appelle
 *     (le champ `conseil` de la vue d'une paie, route `apercu`) ;
 *   - dans la fiche d'un salarié, ce que la compagnie a fait pour lui
 *     (`conseils-salarie`).
 * Chaque conseil finit par la référence de son texte. Sous une carte, un
 * conseil tient en une phrase : son détail (les deux faces, les chiffres)
 * et sa référence se déplient derrière « pourquoi ? » (Léandre, 21/10/2026) ;
 * la phrase et le détail viennent tous deux du moteur (`texte`, `detail`). */
"use strict";

let CONSEIL_DECALAGE = 0;

/** Un conseil, en gris. `replie` : la phrase seule, le détail et la référence derrière « pourquoi ? » ; sinon la
 * référence suit le texte. */
function conseilHtml(c, replie) {
  if (!c || !c.texte) return "";
  const source = c.reference ? `<span class="conseil-source">(${echapper(c.reference)})</span>` : "";
  const suite = replie && (c.detail || source)
    ? `<details class="conseil-pourquoi"><summary>pourquoi ?</summary>${
        c.detail ? echapper(c.detail) + " " : ""}${source}</details>`
    : (source ? " " + source : "");
  return `<div class="encart conseil"><span class="conseil-titre">Le saviez-vous ?</span> ${echapper(c.texte)}${suite}</div>`;
}

/** Le conseil d'un bloc d'engagement : le premier que le moteur a posé sur l'une de ses paies (au plus un par carte),
 * replié. */
function conseilDuBloc(part) {
  const p = (part || []).find((x) => x && x.conseil);
  return p ? conseilHtml(p.conseil, true) : "";
}

/** L'accueil : le conseil du jour, et le résumé de ce que la compagnie a fait. Vide quand l'option est décochée.
 *  `lus` : les deux lectures déjà faites (`{jour, resume}`, l'ouverture d'un coup, js/demarrage.js) ; sinon on les
 *  demande. Repeindre le même contenu ne change rien à l'écran. */
async function dessinerConseils(lus) {
  const cible = document.getElementById("conseils-accueil");
  if (!cible) return;
  let jour, resume;
  try {
    [jour, resume] = lus ? [lus.jour, lus.resume] : await Promise.all([
      api("conseil-du-jour", { decalage: CONSEIL_DECALAGE }),
      api("conseils-resume"),
    ]);
  } catch (e) { cible.innerHTML = ""; cible.dataset.peint = ""; return; }
  const c = jour && jour.conseil;
  const r = (resume && resume.conseils) || [];
  if (!c && !r.length) { cible.innerHTML = ""; cible.dataset.peint = ""; return; }
  const html = `
    ${r.map((x) => conseilHtml(x)).join("")}
    ${c ? `<div class="conseil-du-jour">${conseilHtml(c)}
      <button type="button" class="lien" id="conseil-autre">un autre</button></div>` : ""}`;
  if (cible.dataset.peint === html) return;
  cible.dataset.peint = html;
  cible.innerHTML = html;
  const autre = document.getElementById("conseil-autre");
  if (autre) autre.onclick = async () => {
    CONSEIL_DECALAGE += 1;
    try {
      const j = await api("conseil-du-jour", { decalage: CONSEIL_DECALAGE });
      const bloc = cible.querySelector(".conseil-du-jour");
      if (bloc && j && j.conseil) bloc.firstElementChild.outerHTML = conseilHtml(j.conseil);
    } catch (e) { /* le conseil d'avant reste */ }
  };
}

/** La fiche d'un salarié : ce que les contrats de la compagnie ont fait pour ses droits. */
async function dessinerConseilsDuSalarie(cle) {
  const cible = document.getElementById("fiche-conseils");
  if (!cible || !cle) return;
  let r;
  try { r = await api("conseils-salarie", { salarie: cle }); }
  catch (e) { cible.innerHTML = ""; return; }
  if (CLE_ACTIVE !== cle) return;      // une autre fiche s'est ouverte entre-temps
  const liste = (r && r.conseils) || [];
  cible.innerHTML = liste.length
    ? `<h3>Ses droits, et ce que vos contrats y font</h3>${liste.map((x) => conseilHtml(x)).join("")}`
    : "";
}
