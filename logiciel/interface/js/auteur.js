/* L'AUTEUR DU LOGICIEL, a droite du pied, discret comme l'indicateur de la mise a
   jour : sur l'accueil des compagnies et dans la page d'une compagnie. La phrase,
   le nom qui porte le lien et l'adresse viennent du moteur (`auteur` des routes
   `compagnies` et `etat`, `routes::accueil::auteur`) : l'accueil les pose dans
   `#auteur-donnee`, la page d'une compagnie les passe avec l'etat
   (`window.poserAuteur`, js/demarrage.js).

   Dans un navigateur, le lien s'ouvre a cote (`target="_blank"`). Dans la
   fenetre PuppetPaie (`window.fenetrePuppetPaie`), la fenetre ne quitte jamais
   sa page : le geste `site` l'ouvre dans le navigateur du poste. */
(function () {
  "use strict";
  let ligne = null;

  function poser(a) {
    if (!a || !a.phrase) return;
    if (!ligne) {
      ligne = document.createElement("div");
      ligne.className = "auteur";
      // La fenetre se tire par son corps (accueil) : la ligne n'est pas une poignee.
      ligne.addEventListener("mousedown", (e) => e.stopPropagation());
      document.getElementById("pied").append(ligne);
    }
    ligne.textContent = "";
    const i = a.nom ? a.phrase.indexOf(a.nom) : -1;
    if (i < 0 || !a.adresse) { ligne.textContent = a.phrase; return; }
    const lien = document.createElement("a");
    lien.href = a.adresse;
    lien.target = "_blank";
    lien.rel = "noopener";
    lien.textContent = a.nom;
    lien.addEventListener("click", ouvrir);
    ligne.append(a.phrase.slice(0, i), lien, a.phrase.slice(i + a.nom.length));
  }

  function ouvrir(e) {
    const api = window.pywebview && window.pywebview.api;
    if (!window.fenetrePuppetPaie || !api || typeof api.site !== "function") return;
    e.preventDefault();
    api.site(e.currentTarget.href);
  }

  window.poserAuteur = poser;

  function demarrer() {
    const donnee = document.getElementById("auteur-donnee");
    if (!donnee) return;
    try { poser(JSON.parse(donnee.textContent)); } catch (e) { /* rien a poser */ }
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", demarrer);
  else demarrer();
})();
