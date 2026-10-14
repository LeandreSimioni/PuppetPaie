/* Les boites : ouvrir, fermer, le voile, mettre de cote. */
"use strict";

/* ---------------------------------------------------------- la boite */
/*  `auFermer` est appele QUOI QU'IL ARRIVE, y compris quand on clique a cote
 *  de la boite. Sans lui, une boite qui repond par une promesse resterait en
 *  attente pour toujours : le clic a cote passerait pour un « annuler » a
 *  l'ecran, et l'enregistrement qui attendait sa reponse ne viendrait jamais. */
let FERMETURE_BOITE = null;
/* LE CLIC À CÔTÉ ferme la boîte, sauf celle qui pose ici un autre geste
 * (ticket #102 : dans la boîte de signalement, « cliqué à côté ça doit faire
 * comme réduire » -- le message tapé ne doit pas se perdre). */
let CLIC_DEHORS = null;
function ouvrirBoite(html, auFermer) {
  document.getElementById("boite").innerHTML = html;
  document.getElementById("voile").classList.remove("cachee");
  FERMETURE_BOITE = auFermer || null;
  CLIC_DEHORS = null;
  ajouterToutCocher(document.getElementById("boite"));
}

/* METTRE DE CÔTÉ LA BOÎTE OUVERTE, le temps d'en ouvrir une autre par-dessus
 * (ticket #103 : « j'étais dans la dpae, j'ai cliqué sur signaler un
 * problème et ça a quitté la dpae »). Ses nœuds sortent de la page tels
 * quels -- cases cochées, champs tapés, gestes attachés --, et reviennent
 * quand la boîte du dessus se ferme. Rend la fonction qui les remet, ou
 * null s'il n'y avait rien d'ouvert. */
function mettreDeCote() {
  const voile = document.getElementById("voile"), boite = document.getElementById("boite");
  if (voile.classList.contains("cachee") || !boite.childNodes.length) return null;
  const noeuds = [...boite.childNodes];
  const fermeture = FERMETURE_BOITE, clic = CLIC_DEHORS;
  noeuds.forEach((n) => n.remove());
  FERMETURE_BOITE = null;
  return () => {
    document.getElementById("boite").innerHTML = "";
    document.getElementById("boite").append(...noeuds);
    voile.classList.remove("cachee");
    FERMETURE_BOITE = fermeture;
    CLIC_DEHORS = clic;
  };
}

/* TOUT COCHER, D'UN CLIC (l'éditeur, 30/09/2026 : « quand on clique sur
 * DPAE ou sur contrat, un truc tout sélectionné, plutôt que de cliquer à
 * la main » — renvoyer tous les contrats, par exemple). Chaque liste de
 * choix de plus d'une case reçoit en tête une case « Tout cocher ». Elle
 * passe par l'événement « change » de chaque case : ce que la boîte en
 * déduit (le libellé du bouton, un avenant) suit comme au clic. Une case
 * grisée (une DPAE déjà passée) n'est pas touchée. */
function ajouterToutCocher(racine) {
  racine.querySelectorAll(".liste-choix").forEach((liste) => {
    const cases = () => [...liste.querySelectorAll('.choix > input[type="checkbox"]:not(:disabled)')];
    if (cases().length < 2) return;
    const tete = document.createElement("label");
    tete.className = "coche tout-cocher";
    tete.innerHTML = `<input type="checkbox"> <b>Tout cocher</b>`;
    const maitre = tete.querySelector("input");
    const suivre = () => {
      const c = cases(), n = c.filter((x) => x.checked).length;
      maitre.checked = n === c.length;
      maitre.indeterminate = n > 0 && n < c.length;
    };
    maitre.onchange = () => {
      cases().forEach((x) => {
        if (x.checked !== maitre.checked) {
          x.checked = maitre.checked;
          x.dispatchEvent(new Event("change", { bubbles: true }));
        }
      });
      suivre();
    };
    liste.addEventListener("change", (e) => { if (e.target !== maitre) suivre(); });
    liste.before(tete);
    suivre();
  });
}
function fermerBoite() {
  document.getElementById("voile").classList.add("cachee");
  const f = FERMETURE_BOITE;
  FERMETURE_BOITE = null;
  if (f) f();
}
document.getElementById("voile").onclick = (e) => {
  if (e.target.id !== "voile") return;
  if (CLIC_DEHORS) CLIC_DEHORS();
  else fermerBoite();
};

/* LE MOT DE PASSE DU LOGICIEL : UN POPUP, TOUJOURS LE MÊME (17/09/2026).
   Chaque acte qui engage — dépôt réel, envoi, signature, écrire dans le
   coffre — passe par ici : on clique le bouton de l'acte, le popup demande
   le mot de passe, Entrée, et l'acte part. Plus de champ posé dans chaque
   écran (l'éditeur : « la logique c'est un popup, toujours, où je tape le
   mot de passe, pas des champs cachés »). Le popup vit dans index.html
   (`#voile-mdp`), au-dessus de la boîte courante, et rend une promesse :
   le mot de passe, ou null si on annule (Échap, Annuler, clic à côté).

   PLUS RIEN POUR CHROME. Le 15/09 on avait tout fait pour que Chrome garde
   le mot de passe (utilisateur figé, autocomplete, `memoriserMotDePasse`) ;
   la page vit dans la fenêtre unique, et vivra dans Tauri : « il n'y a plus
   Chrome, et il n'y aura pas Chrome ». Le mot de passe se tape, à chaque
   fois, et c'est le but. */
function demanderMotDePasse(pourquoi) {
  // LA COMPAGNIE FICTIVE (routes_accueil::simulation_active) : rien ne part, rien à prouver.
  // L'acte suit son cours comme en vrai, sans le popup.
  if (enFictive()) return Promise.resolve("fictive");
  return new Promise((resoudre) => {
    const voile = document.getElementById("voile-mdp");
    const forme = document.getElementById("forme-mdp");
    const champ = document.getElementById("mdp-valeur");
    document.getElementById("mdp-pourquoi").textContent = pourquoi ? "Pour " + pourquoi + "." : "";
    champ.value = "";
    voile.classList.remove("cachee");
    const finir = (valeur) => {
      voile.classList.add("cachee");
      forme.onsubmit = voile.onclick = champ.onkeydown = null;
      document.getElementById("mdp-annuler").onclick = null;
      resoudre(valeur);
    };
    forme.onsubmit = (e) => {
      e.preventDefault();
      if (!champ.value) { champ.focus(); return; }
      finir(champ.value);
    };
    document.getElementById("mdp-annuler").onclick = () => finir(null);
    voile.onclick = (e) => { if (e.target === voile) finir(null); };
    champ.onkeydown = (e) => { if (e.key === "Escape") finir(null); };
    setTimeout(() => champ.focus(), 30);
  });
}

