/* Le demarrage : demarrer, finir le chargement ; la fenetre PuppetPaie, le bouton Accueil. Charge en dernier. */
"use strict";

/* =======================================================================
   DEMARRAGE
   ===================================================================== */
async function demarrer() {
  ETAT = await api("etat");

  // Une option VIDE en tete, et c'est elle le defaut. Un deroulant montre
  // toujours quelque chose : sans elle, le premier spectacle de la liste a
  // l'air choisi alors que personne ne l'a choisi -- et il porte le numero
  // d'objet qui part en DSN. Un mois porte d'ailleurs plusieurs spectacles :
  // c'est l'engagement qui dit le sien, pas le mois.
  const c = ETAT.configuration;
  const err = [c.entreprise, ...c.salaries, ...c.spectacles]
    .reduce((n, f) => n + f.anomalies.filter((a) => a.gravite === "erreur" && !aRemplir(a)).length, 0);
  const annees = Object.entries(c.compteurs).filter(([k]) => /^\d{4}$/.test(k));
  const compteurs = annees.slice(-1)[0];
  document.getElementById("etat-global").innerHTML = `
    ${err ? `<span class="pastille erreur">${err} erreur(s) de configuration</span>` : ""}
    ${compteurs ? `<div>compteurs ${echapper(compteurs[0])} :
       bulletin <b>${compteurs[1].bulletin ?? "?"}</b> ·
       AEM <b>${compteurs[1].aem ?? "?"}</b> ·
       CS <b>${compteurs[1].certificat_cs ?? "?"}</b></div>` : ""}`;

  // LE MOIS EN COURS, et rien d'autre. « Le mois doit être le mois en cours de
  // base quand j'ouvre l'app ». La version d'avant
  // ouvrait sur « le premier mois non produit », ce qui paraissait serviable
  // et ne l'était pas : une saisie d'essai laissée en octobre faisait ouvrir
  // le logiciel en octobre, en septembre. On ouvre sur aujourd'hui ; le mois
  // se change d'une case, et l'Historique liste les autres.
  // SAUF UN MOIS PAS FINI (01/10/2026) : « on doit attendre que tout soit
  // vert pour passer au mois suivant, pour éviter d'oublier la DSN ou
  // l'envoi des documents ». Le moteur rend le plus ancien mois passé dont
  // une étape n'est pas verte ; seuls les mois saisis et passés comptent,
  // une saisie d'essai en avance n'y change rien.
  let ouvrir = moisCourant();
  try { ouvrir = (await api("mois-a-ouvrir")).mois || ouvrir; } catch (e) { /* le calendrier, alors */ }
  await chargerMois(ouvrir);
  finirChargement();

  // Ce qui attend : la DSN du mois d'avant, la clôture en janvier. Une
  // boîte, tout de suite, tant que ce n'est pas fait.
  await montrerRappels();

  // Le travail de fond : on le regarde toutes les minutes. C'est une lecture
  // d'etat en memoire cote serveur, elle ne coute rien -- surtout pas un
  // appel a Google ou a net-entreprises, qui n'ont lieu que dans le fil de
  // la veille, a son rythme a lui.
  suivreVeille();
  setInterval(suivreVeille, 60000);
}

/** Le voile du demarrage (index.html, #chargement) : une roue tant que la
 *  page se peint, puis la page d'un coup. Retire aussi en cas d'echec -- un
 *  voile qui resterait cacherait le message qui dit pourquoi -- et au bout de
 *  vingt secondes quoi qu'il arrive : une lecture qui traine ne doit pas
 *  bloquer l'ecran. */
function finirChargement() {
  const v = document.getElementById("chargement");
  if (!v || v.classList.contains("fini")) return;
  v.classList.add("fini");
  setTimeout(() => v.remove(), 300);
}
setTimeout(finirChargement, 20000);

demarrer().catch((e) => {
  finirChargement();
  dire("Démarrage impossible : " + e.message, "erreur");
});

function fenetreNative() {
  const api = window.pywebview && window.pywebview.api;
  if (!api || document.body.classList.contains("fenetre") || document.body.classList.contains("accueil-possible")) return;
  // La fenetre Rust (lot J3) a sa propre barre de titre : elle ne porte que le retour aux compagnies.
  if (!api.reduire) {
    if (typeof api.accueil === "function") boutonAccueil(api);
    return;
  }
  document.body.classList.add("fenetre");
  document.getElementById("fenetre-reduire").onclick = () => api.reduire();
  document.getElementById("fenetre-agrandir").onclick = () => api.agrandir();
  document.getElementById("fenetre-fermer").onclick = () => api.fermer();
  // L'ACCUEIL DES COMPAGNIES (ticket #113). Un lanceur d'avant ce bouton
  // (une mise à jour s'applique au lanceur au démarrage suivant) n'a pas
  // `accueil` : le bouton ne se montre pas. Ce qui est à l'écran
  // s'enregistre d'abord, comme quand on change de mois ; le serveur de
  // la compagnie s'arrête ensuite, côté lanceur.
  if (typeof api.accueil === "function") boutonAccueil(api);
  // Tout le bandeau se tire (pywebview remonte du point cliqué jusqu'à
  // l'élément marqué) ; ses boutons arrêtent le mousedown avant qu'il
  // n'atteigne le corps, où pywebview l'écoute, et restent des boutons.
  const bandeau = document.querySelector(".bandeau");
  bandeau.classList.add("pywebview-drag-region");
  for (const el of bandeau.querySelectorAll("button, a, input")) {
    el.addEventListener("mousedown", (e) => e.stopPropagation());
  }
  bandeau.addEventListener("dblclick", (e) => {
    if (e.target.closest("button, a, input")) return;
    api.agrandir();
  });
  const poignee = document.getElementById("fenetre-poignee");
  poignee.addEventListener("mousedown", (e) => {
    e.preventDefault();
    const x0 = e.screenX, y0 = e.screenY, w0 = window.innerWidth, h0 = window.innerHeight;
    let attente = false;
    const bouger = (ev) => {
      if (attente) return;
      attente = true;
      api.redimensionner(Math.max(700, w0 + ev.screenX - x0), Math.max(500, h0 + ev.screenY - y0))
        .finally(() => { attente = false; });
    };
    const lacher = () => {
      window.removeEventListener("mousemove", bouger);
      window.removeEventListener("mouseup", lacher);
    };
    window.addEventListener("mousemove", bouger);
    window.addEventListener("mouseup", lacher);
  });
}
function boutonAccueil(api) {
  document.body.classList.add("accueil-possible");
  document.getElementById("fenetre-accueil").onclick = async () => {
    clearTimeout(MINUTEUR_SAISIE);
    await enregistrerSaisie(false);
    // Un enregistrement refusé : on reste, le message est à l'écran.
    // Partir perdrait ce qu'on vient de taper.
    const s = saisieCourante();
    const ecrivable = s.mois && !(MOIS_FERME && MOIS_FORCE !== s.mois);
    if (ecrivable && JSON.stringify(s) !== DERNIERE_ECRITE) return;
    api.accueil();
  };
}
window.addEventListener("pywebviewready", fenetreNative);
fenetreNative();

