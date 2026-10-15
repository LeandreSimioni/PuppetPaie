// LE CADRE DE LA FENETRE (ticket #195, 15/10/2026). « Pas de barre noire, mais de la meme couleur que le logiciel en
// haut » : la fenetre n'a plus de bordure systeme (`decorations(false)`, main.rs), et ce script, que Tauri pose dans
// chaque page de la fenetre avant les siennes (accueil, mot de passe, logiciel), fait ce que faisait la barre de titre.
// Il n'existe que dans la fenetre : dans un navigateur (`paie interface`), rien de ceci n'est la, et la page le sait
// (`window.fenetrePuppetPaie` absent).
//
//   - `window.fenetrePuppetPaie` : reduire, agrandir (ou restaurer), fermer, `agrandie()` -- ce que les boutons d'une
//     page appellent (la page du logiciel a les siens dans son bandeau, `js/demarrage.js`) ;
//   - TIRER : un appui dans une zone marquee `.fenetre-tire` (le bandeau du logiciel) ou `.pywebview-drag-region` (le
//     corps des pages de l'accueil, ecrites pour la fenetre d'avant) deplace la fenetre des que la souris a bouge de
//     quelques points -- un clic sans mouvement reste un clic (une tuile de l'accueil se clique) ; un double-clic
//     agrandit ou restaure. Ce qui se clique (bouton, lien, champ, ce qui montre la main) ne tire pas ;
//   - LES TROIS BOUTONS, en haut a droite, sur une page qui n'a pas les siens (`.fenetre-boutons`) : l'accueil et le
//     mot de passe, servis par le moteur.
// Les bords se tirent pour redimensionner : c'est Tauri qui le fait pour une fenetre sans bordure. Agrandie, elle
// s'arrete a la barre des taches (Windows : tao borne une fenetre sans bordure agrandie a la zone de travail).
// Ce que le script peut demander a la fenetre est borne par `capabilities/fenetre.json`.
(function () {
  "use strict";
  if (window.top !== window || window.fenetrePuppetPaie) return;
  const geste = (nom) => {
    const t = window.__TAURI_INTERNALS__;
    return t ? t.invoke("plugin:window|" + nom).catch(() => false) : Promise.resolve(false);
  };
  const fenetre = Object.freeze({
    reduire: () => geste("minimize"),
    agrandir: () => geste("toggle_maximize"),
    fermer: () => geste("close"),
    agrandie: () => geste("is_maximized"),
  });
  Object.defineProperty(window, "fenetrePuppetPaie", { value: fenetre });

  const TIRE = ".fenetre-tire, .pywebview-drag-region";
  const CLIQUABLE = "button, a, input, select, textarea, label, summary, option, [contenteditable], [role=button]";
  const tirable = (t) => t instanceof Element && !!t.closest(TIRE) && !t.closest(CLIQUABLE)
    && getComputedStyle(t).cursor !== "pointer";

  let appui = null;
  document.addEventListener("mousedown", (e) => {
    appui = e.button === 0 && e.detail < 2 && tirable(e.target) ? { x: e.screenX, y: e.screenY } : null;
  });
  document.addEventListener("mousemove", (e) => {
    if (!appui) return;
    if (!(e.buttons & 1)) { appui = null; return; }
    if (Math.abs(e.screenX - appui.x) + Math.abs(e.screenY - appui.y) < 4) return;
    appui = null;
    geste("start_dragging");
  });
  document.addEventListener("mouseup", () => { appui = null; });
  document.addEventListener("dblclick", (e) => {
    if (e.button === 0 && tirable(e.target)) fenetre.agrandir();
  });

  function boutons() {
    if (!document.body || document.querySelector(".fenetre-boutons") || document.getElementById("fenetre-cadre")) return;
    const hote = document.createElement("div");
    hote.id = "fenetre-cadre";
    hote.style.cssText = "position:fixed;top:0;right:0;z-index:2147483647";
    // Les couleurs de la page (ses variables passent l'ombre), et celles de la charte si elle n'en a pas.
    const ombre = hote.attachShadow({ mode: "closed" });
    ombre.innerHTML = `<style>
      div { display: flex; gap: 2px; padding: 4px 6px; }
      button { width: 34px; height: 26px; margin: 0; padding: 0; border: 0; border-radius: 4px; cursor: default;
               background: transparent; color: var(--discret, #7A8794); font: 13px/1 Arial, sans-serif;
               display: inline-flex; align-items: center; justify-content: center; }
      button:hover { background: var(--bord, #DDD8D2); color: var(--texte, #2C3E50); }
      button.fermer:hover { background: var(--accent, #B22222); color: #fff; }
    </style><div>
      <button data-geste="reduire" title="Réduire">&#x2014;</button>
      <button data-geste="agrandir" title="Agrandir / restaurer">&#x25A1;</button>
      <button data-geste="fermer" class="fermer" title="Fermer PuppetPaie">&#x2715;</button>
    </div>`;
    ombre.addEventListener("click", (e) => {
      const b = e.target.closest("button");
      if (b) fenetre[b.dataset.geste]();
    });
    // L'appui sur un bouton ne tire pas la fenetre : il n'atteint pas le corps, que l'accueil marque tout entier.
    hote.addEventListener("mousedown", (e) => e.stopPropagation());
    document.body.appendChild(hote);
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", boutons);
  else boutons();
})();
