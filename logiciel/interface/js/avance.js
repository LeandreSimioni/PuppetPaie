/* L'AFFICHAGE AUTOMATIQUE, sous « Avancé » de chaque place.
 *
 * Une route sans écran dessiné porte, dans sa fiche, la place où elle se montre (`ecran`, `affichage: "auto"`) : la page
 * l'affiche ici depuis sa fiche, comme `moteur routes` la montre au LLM. Le geste en bouton, les champs de la charge
 * avec leur sens, la phrase rendue (ou ses lignes, `phrases`), le refus et ce qu'il faut corriger. La page ne compose
 * rien : chaque mot vient de la fiche ou de la réponse. La forme d'un champ se lit dans l'exemple de la fiche : une liste
 * se tape élément par élément, un oui/non se coche, un mois se choisit au sélecteur de mois ; le reste se tape. Une place
 * sans route automatique ne montre rien. */
"use strict";

/** La forme d'un champ, d'après la valeur que l'exemple de la fiche lui donne (un champ que l'exemple ne donne pas et
 *  dont le sens dit « vrai : … » est un oui/non). */
function formeDuChamp(f, c) {
  const ex = f.exemple ? f.exemple[c.nom] : undefined;
  if (Array.isArray(ex)) return "liste";
  if (typeof ex === "boolean" || (ex === undefined && /^vrai\b/.test(c.sens))) return "case";
  if (typeof ex === "number") return "nombre";
  if (c.nom === "mois" || (typeof ex === "string" && /^\d{4}-\d{2}$/.test(ex))) return "mois";
  return "texte";
}

function saisieDuChamp(forme, j) {
  if (forme === "case") return `<input type="checkbox" data-j="${j}">`;
  if (forme === "mois") return `<input type="month" data-j="${j}">`;
  if (forme === "nombre") return `<input type="number" data-j="${j}">`;
  if (forme === "liste") return `<span data-liste="${j}"><input type="text" data-element></span> <button type="button" class="lien" data-plus="${j}">+</button>`;
  return `<input type="text" data-j="${j}">`;
}

/** La valeur d'un champ, selon sa forme ; vide, le champ ne part pas. */
function valeurDuChamp(bloc, forme, j) {
  if (forme === "liste") {
    const v = [...bloc.querySelectorAll(`[data-liste="${j}"] input`)].map((x) => x.value.trim()).filter(Boolean);
    return v.length ? v : undefined;
  }
  const x = bloc.querySelector(`[data-j="${j}"]`);
  if (forme === "case") return x.checked ? true : undefined;
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
      ${f.charge.map((c, j) => `<label class="champ-boite">${echapper(c.sens)}
        ${saisieDuChamp(formeDuChamp(f, c), j)}</label>`).join("")}
      <p><button class="secondaire" data-faire>${echapper(f.geste)}</button></p>
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
        const v = valeurDuChamp(bloc, formeDuChamp(f, c), j);
        if (v !== undefined) charge[c.nom] = v;
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
