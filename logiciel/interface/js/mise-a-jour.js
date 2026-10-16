/* L'INDICATEUR DE LA MISE A JOUR (ticket #210, Leandre, 16/10/2026 : « un
   petit truc en bas a gauche, des l'ecran d'accueil, avec une barre de
   progression : Recherche de mise a jour / Aucune mise a jour / Mise a jour
   en cours (avec la barre) / Redemarrer pour mettre a jour »).

   Le moteur cherche la mise a jour au lancement, en fond, et tient ou il en
   est (`maj::Recherche`). Cette page le lui demande par le geste `maj` du
   lanceur (`window.pywebview.api.maj`, la cale du serveur : la meme sur
   l'accueil et dans la page d'une compagnie) et affiche ce qu'il rend : la
   phrase, la bulle, la barre. Elle relit tant que le moteur dit `en_cours`,
   puis se tait : plus rien ne bouge, plus rien n'est demande.

   « Redemarrer pour mettre a jour » est un clic quand le moteur l'offre
   (`redemarrer`) : le geste `redemarrer` passe la version prete au relais et
   ferme le logiciel ; le relais l'installe et rouvre la fenetre neuve. Dans la
   page d'une compagnie, la saisie s'enregistre d'abord
   (`window.avantDeQuitter`, js/demarrage.js) : si elle ne s'enregistre pas,
   on reste. */
(function () {
  const RELIRE_MS = 700;
  let boite = null;

  function construire() {
    boite = document.createElement("div");
    boite.className = "maj";
    boite.id = "mise-a-jour";
    boite.hidden = true;
    boite.setAttribute("aria-live", "polite");
    boite.innerHTML = '<span class="maj-texte"></span>'
      + '<button type="button" class="maj-redemarrer" hidden></button>'
      + '<span class="maj-barre" hidden><span></span></span>'
      + '<span class="maj-refus"></span>';
    // La fenetre se tire par son corps (accueil) : l'indicateur n'est pas une poignee.
    boite.addEventListener("mousedown", (e) => e.stopPropagation());
    boite.querySelector(".maj-redemarrer").addEventListener("click", redemarrer);
    document.body.appendChild(boite);
  }

  function montrer(e) {
    if (!e || !e.texte) { boite.hidden = true; return; }
    boite.hidden = false;
    boite.title = e.detail || "";
    const texte = boite.querySelector(".maj-texte");
    const bouton = boite.querySelector(".maj-redemarrer");
    texte.hidden = !!e.redemarrer;
    bouton.hidden = !e.redemarrer;
    texte.textContent = e.texte;
    bouton.textContent = e.texte;
    const barre = boite.querySelector(".maj-barre");
    barre.hidden = !e.barre;
    barre.classList.toggle("sans-fin", e.barre && e.progression == null);
    barre.firstElementChild.style.width = e.progression == null ? "" : (e.progression * 100).toFixed(1) + "%";
  }

  async function lire() {
    const api = window.pywebview && window.pywebview.api;
    if (!api || typeof api.maj !== "function") return;
    const r = await api.maj();
    if (!r || !r.ok) return;
    montrer(r.etat);
    if (r.etat && r.etat.en_cours) setTimeout(lire, RELIRE_MS);
  }

  async function redemarrer() {
    const api = window.pywebview && window.pywebview.api;
    const bouton = boite.querySelector(".maj-redemarrer");
    const refus = boite.querySelector(".maj-refus");
    refus.textContent = "";
    if (typeof window.avantDeQuitter === "function" && !(await window.avantDeQuitter())) return;
    bouton.disabled = true;
    const r = await api.redemarrer();
    if (r && r.ok) {
      bouton.textContent = "Redémarrage…";
      return;
    }
    bouton.disabled = false;
    refus.textContent = (r && r.erreur) || "Le redémarrage ne s'est pas fait.";
  }

  function demarrer() {
    if (!boite) construire();
    lire();
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", demarrer);
  else demarrer();
})();
