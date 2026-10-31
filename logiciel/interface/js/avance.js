/* L'AFFICHAGE AUTOMATIQUE, sous « Avancé » de chaque place.
 *
 * Une route sans écran dessiné porte, dans sa fiche, la place où elle se montre (`ecran`, `affichage: "auto"`) : la page
 * l'affiche ici depuis sa fiche, comme `moteur routes` la montre au LLM. Le geste en bouton, les champs de la charge
 * avec leur sens, la phrase rendue (ou ses lignes, `phrases`), le refus et ce qu'il faut corriger. La page ne compose
 * rien : chaque mot vient de la fiche ou de la réponse. Une place sans route automatique ne montre rien. */
"use strict";

/** La valeur d'un champ tel qu'on l'a tapé : `vrai`/`faux`, un nombre, une liste ou un objet JSON se lisent comme tels ;
 *  le reste est un texte ; vide, le champ ne part pas. */
function valeurTapee(t) {
  const v = t.trim();
  if (!v) return undefined;
  if (v === "vrai") return true;
  if (v === "faux") return false;
  if (/^[[{\d"-]/.test(v) || v === "true" || v === "false") { try { return JSON.parse(v); } catch (e) { /* un texte */ } }
  return v;
}

/** Ce que la réponse dit à lire : sa phrase, puis ses lignes. */
function phrasesDe(r) {
  const p = [];
  if (r && r.phrase) p.push(r.phrase);
  if (r && Array.isArray(r.phrases)) p.push(...r.phrases);
  return p.length ? p : ["Fait."];
}

/** Dessine dans `cible` (porteuse de `data-auto`, le nom d'une place) les routes automatiques de cette place. */
function dessinerAvance(cible) {
  if (!cible) return;
  const place = cible.dataset.auto || "";
  const routes = Object.values(CATALOGUE).filter((f) => f.affichage === "auto" && f.ecran === place);
  const enveloppe = cible.closest(".avance-auto") || cible;
  enveloppe.hidden = !routes.length;
  cible.innerHTML = routes.map((f, i) => `
    <div class="carte avance-route" data-i="${i}">
      <p class="discret">${echapper(f.nom)}</p>
      ${f.charge.map((c, j) => `<label class="champ-boite">${echapper(c.sens)}
        <input type="text" data-j="${j}" placeholder="${echapper(c.nom)}"></label>`).join("")}
      <p><button class="secondaire" data-faire>${echapper(f.geste)}</button></p>
      <div data-rendu></div>
    </div>`).join("");
  cible.querySelectorAll(".avance-route").forEach((bloc) => {
    const f = routes[+bloc.dataset.i];
    bloc.querySelector("[data-faire]").onclick = async () => {
      const charge = {};
      bloc.querySelectorAll("input[data-j]").forEach((x) => {
        const v = valeurTapee(x.value);
        if (v !== undefined) charge[f.charge[+x.dataset.j].nom] = v;
      });
      const rendu = bloc.querySelector("[data-rendu]");
      try {
        const r = await api(f.nom, charge);
        rendu.innerHTML = phrasesDe(r).map((p) => `<p>${echapper(p)}</p>`).join("");
      } catch (e) {
        rendu.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`
          + f.refus.map((x) => `<p class="note">${echapper(x.classe)} : ${echapper(x.corriger)}</p>`).join("");
      }
    };
  });
}

/** Les places dessinées une fois pour toutes (`data-auto` dans la page) : à l'ouverture, le catalogue lu. */
function dessinerLesAvances() {
  document.querySelectorAll("[data-auto]").forEach(dessinerAvance);
}
