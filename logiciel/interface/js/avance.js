/* L'AFFICHAGE AUTOMATIQUE, sous « Avancé » de chaque place.
 *
 * Une route sans écran dessiné porte, dans sa fiche, la place où elle se montre (`ecran`, `affichage: "auto"`) et son
 * bouton (`bouton`) : la page l'affiche ici depuis sa fiche. Chaque champ de la charge dit ce que l'écran en montre
 * (`libelle`) et sa forme (`forme` : une liste se tape élément par élément, un oui/non se coche, un nombre, un mois ou une
 * date se choisissent ; le reste se tape). Puis la phrase rendue (ou ses lignes, `phrases`), ou le refus. La page ne
 * compose ni ne décide rien : chaque mot et chaque forme viennent de la fiche ou de la réponse. Une place sans route
 * automatique ne montre rien. */
"use strict";

const SAISIES = {
  oui_non: (j) => `<input type="checkbox" data-j="${j}">`,
  mois: (j) => `<input type="month" data-j="${j}">`,
  date: (j) => `<input type="date" data-j="${j}">`,
  nombre: (j) => `<input type="number" data-j="${j}">`,
  liste: (j) => `<span data-liste="${j}"><input type="text" data-element></span> <button type="button" class="lien" data-plus="${j}">+</button>`,
};

function saisieDuChamp(forme, j) {
  return (SAISIES[forme] || ((k) => `<input type="text" data-j="${k}">`))(j);
}

/** La valeur d'un champ, selon sa forme ; vide, le champ ne part pas. */
function valeurDuChamp(bloc, forme, j) {
  if (forme === "liste") {
    const v = [...bloc.querySelectorAll(`[data-liste="${j}"] input`)].map((x) => x.value.trim()).filter(Boolean);
    return v.length ? v : undefined;
  }
  const x = bloc.querySelector(`[data-j="${j}"]`);
  if (forme === "oui_non") return x.checked ? true : undefined;
  const v = x.value.trim();
  if (!v) return undefined;
  return forme === "nombre" ? Number(v) : v;
}

/** Ce que la réponse dit à lire : sa phrase, puis ses lignes (chaque route automatique rend sa phrase). */
function phrasesDe(r) {
  const p = [];
  if (r && r.phrase) p.push(r.phrase);
  if (r && Array.isArray(r.phrases)) p.push(...r.phrases);
  return p;
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
      ${f.charge.map((c, j) => `<label class="champ-boite">${echapper(c.libelle)}
        ${saisieDuChamp(c.forme, j)}</label>`).join("")}
      <p><button class="secondaire" data-faire>${echapper(f.bouton)}</button></p>
      <div data-rendu></div>
    </div>`).join("");
  cible.querySelectorAll(".avance-route").forEach((bloc) => {
    const f = routes[+bloc.dataset.i];
    bloc.querySelectorAll("[data-plus]").forEach((b) => {
      b.onclick = () => {
        const x = document.createElement("input");
        x.type = "text";
        bloc.querySelector(`[data-liste="${b.dataset.plus}"]`).appendChild(x);
        x.focus();
      };
    });
    bloc.querySelector("[data-faire]").onclick = async () => {
      const charge = {};
      f.charge.forEach((c, j) => {
        const v = valeurDuChamp(bloc, c.forme, j);
        if (v !== undefined) charge[c.nom] = v;
      });
      const rendu = bloc.querySelector("[data-rendu]");
      try {
        const r = await api(f.nom, charge);
        rendu.innerHTML = phrasesDe(r).map((p) => `<p>${echapper(p)}</p>`).join("");
      } catch (e) {
        rendu.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`;
      }
    };
  });
}

/** Les places dessinées une fois pour toutes (`data-auto` dans la page) : à l'ouverture, le catalogue lu. */
function dessinerLesAvances() {
  document.querySelectorAll("[data-auto]").forEach(dessinerAvance);
}
