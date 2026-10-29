/* L'INDICATEUR DE LA MISE A JOUR, en bas a gauche, des l'ecran d'accueil :
   « Recherche de mise a jour » / « Logiciel a jour » / « Mise a jour en
   cours » (avec la barre) / « Redemarrer pour mettre a jour ».

   Le moteur cherche la mise a jour au lancement, en fond, et tient ou il en
   est (`maj::Recherche`) ; lance sans reseau, il reprend de lui-meme, et a
   jour, il recherche toutes les 5 minutes tant qu'il tourne. Cette page le
   lui demande par le geste `maj` du lanceur (`window.pywebview.api.maj`, la
   cale du serveur : la meme sur l'accueil et dans la page d'une compagnie)
   et affiche ce qu'il rend : la phrase, la bulle, la barre, et deux phrases
   qui se lisent sans survol : `exigee` (la version prete est indispensable
   avant de declarer) et `installee` (ce que la version en place, installee
   juste avant ce lancement, change d'un calcul de paie). Elle relit quand le
   moteur le dit (`relire_ms`), puis se tait. Une lecture ne fait que lire ou
   en est le moteur.

   « Redemarrer pour mettre a jour » est un clic quand le moteur l'offre
   (`redemarrer`) : le geste `redemarrer` passe la version prete au relais et
   ferme le logiciel ; le relais l'installe et rouvre la fenetre neuve. Dans la
   page d'une compagnie, la saisie s'enregistre d'abord
   (`window.avantDeQuitter`, js/demarrage.js) : si elle ne s'enregistre pas,
   on reste. */
(function () {
  let boite = null;

  function construire() {
    boite = document.createElement("div");
    boite.className = "maj";
    boite.id = "mise-a-jour";
    boite.hidden = true;
    boite.setAttribute("aria-live", "polite");
    boite.innerHTML = '<span class="maj-installee" hidden></span>'
      + '<span class="maj-texte"></span>'
      + '<button type="button" class="maj-redemarrer" hidden></button>'
      + '<span class="maj-exigee" hidden></span>'
      + '<span class="maj-barre" hidden><span></span></span>'
      + '<span class="maj-refus"></span>';
    // La fenetre se tire par son corps (accueil) : l'indicateur n'est pas une poignee.
    boite.addEventListener("mousedown", (e) => e.stopPropagation());
    boite.querySelector(".maj-redemarrer").addEventListener("click", redemarrer);
    document.body.appendChild(boite);
  }

  function phrase(classe, t) {
    const el = boite.querySelector(classe);
    el.hidden = !t;
    el.textContent = t || "";
  }

  function montrer(e) {
    if (!e || (!e.texte && !e.installee)) { boite.hidden = true; return; }
    boite.hidden = false;
    boite.title = e.detail || "";
    phrase(".maj-installee", e.installee);
    phrase(".maj-exigee", e.exigee);
    const texte = boite.querySelector(".maj-texte");
    const bouton = boite.querySelector(".maj-redemarrer");
    texte.hidden = !!e.redemarrer || !e.texte;
    bouton.hidden = !e.redemarrer;
    texte.textContent = e.texte || "";
    bouton.textContent = e.texte || "";
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
    if (r.etat && r.etat.relire_ms) setTimeout(lire, r.etat.relire_ms);
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
    refus.textContent = (r && r.erreur) || "PuppetPaie reste ouvert sur cette version.";
  }

  /* Dans la page d'une compagnie, l'état arrive avec l'ouverture (
     js/demarrage.js : un seul appel peint la page) ; la relecture suit le
     moteur comme ailleurs. */
  window.miseAJourMontrer = (etat) => {
    if (!boite) construire();
    montrer(etat);
    if (etat && etat.relire_ms) setTimeout(lire, etat.relire_ms);
  };

  function demarrer() {
    if (!boite) construire();
    // La page d'une compagnie (son voile de demarrage) recoit l'etat avec l'ouverture.
    if (!document.getElementById("chargement")) lire();
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", demarrer);
  else demarrer();
})();
