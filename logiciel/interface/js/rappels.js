/* Les rappels de l'ouverture, « pour demarrer », l'echeancier. */
"use strict";

/* =======================================================================
   LES RAPPELS DE L'OUVERTURE, ET L'EXERCICE
   =======================================================================
   « Quand le mois est fini, le logiciel s'ouvre sur le mois suivant, et là
   il faut un pop-up : il est temps d'envoyer la DSN du mois précédent. Avez-
   vous bien fini les paies du mois précédent ? Et un bouton vérifier et
   envoyer. ». Puis, en janvier : clôturer l'exercice, exporter
   les chiffres pour la compta, saisir les numéros d'objet de l'année.

   La page ne décide de rien : /api/rappels rend la liste, déjà dans l'ordre,
   et chaque rappel dit où en est le mois (à saisir, à écrire, à déposer, à
   corriger). « Vérifier et envoyer » ne fait qu'ouvrir le mois et enchaîner
   les boîtes qui existent déjà — écrire le mois, puis le récapitulatif de
   dépôt avec son mot de passe. Rien ne part d'ici. « Plus tard » ferme la
   boîte ; elle reviendra à la prochaine ouverture, tant que le journal des
   dépôts ne dit pas que c'est fait. */
/* `lu(route)` : la lecture de l'ouverture (js/demarrage.js : tout arrive d'un
 * coup, et la boîte s'ouvre avec la page) — elle rend le résultat ou lève le
 * refus, comme `api`. Sans elle, on demande. */
let RAPPELS_MONTRES = null;   // ce que la boîte de l'ouverture a montré : `{liste, ech, manques}`, ou null
async function montrerRappels(lu = api) {
  let r, ech = null;
  try { r = await lu("rappels"); } catch (e) { return; }
  try { ech = await lu("echeancier"); } catch (e) { ech = null; }
  let manques = [];
  try { manques = await lireDemarrage(lu); } catch (e) { manques = []; }
  const liste = r.rappels || [];
  // L'ÉCHÉANCIER : « un pop-up au démarrage : on est le…,
  // bienvenue sur PuppetPaie, n'oublie pas de faire la DSN ». Elle s'ouvrait
  // aussi une fois par jour quand rien n'attendait : elle n'apparaît
  // que s'il y a quelque chose à faire.
  if (!liste.length && !(ech && ech.a_faire) && !manques.length) return;
  RAPPELS_MONTRES = { liste, ech, manques };
  if (dejaMontres(RAPPELS_MONTRES)) return;
  boiteRappels(liste, ech, manques, r.fermer_dit);
}

/* UNE FOIS, PUIS PLUS : la boîte s'ouvre à l'ouverture du logiciel ; la page
 * rechargée pendant qu'il tourne ne la rouvre pas tant qu'elle dirait la même
 * chose (le même jour, les mêmes choses à faire). La mémoire est celle de la
 * fenêtre (`sessionStorage`) : le logiciel rouvert la montre de nouveau. */
function dejaMontres(montres) {
  const cle = "rappels-montres:" + ((montres.ech && montres.ech.compagnie) || "");
  const empreinte = JSON.stringify(montres);
  try {
    if (sessionStorage.getItem(cle) === empreinte) return true;
    sessionStorage.setItem(cle, empreinte);
  } catch (e) { /* sans mémoire, la boîte s'ouvre */ }
  return false;
}

/* CE QUE LE MOTEUR A POSÉ DE LUI-MÊME (`pour-demarrer`, `poses`) : dit avec
 * ce qui manque, en information, avec le lien vers sa case. */
let POSES_DEMARRAGE = [];
function htmlPoses() {
  if (!POSES_DEMARRAGE.length) return "";
  return `<div class="rappel demarrage-poses">
    <h3>Posé pour vous</h3>
    <ul class="rappel-liste">${POSES_DEMARRAGE.map((p) => `
      <li>${pastilleDeLEtat(p.pastille)}
        <button type="button" class="lien" data-manque-champ="${echapper(p.champ || "")}"
          data-manque-onglet="${echapper(p.onglet || "")}"><b>${echapper(p.libelle)}</b></button>
        <div class="note">${echapper(p.texte)}</div></li>`).join("")}</ul>
  </div>`;
}

/* POUR DÉMARRER. Le moteur (`pour-demarrer`) rend d'un coup ce que la
 * compagnie peut remplir d'avance : chaque ligne, sa case (`case`), où elle
 * vit (`ou`), et pour qui (`qui`, une ligne d'un salarié ou d'un spectacle). Chaque case se
 * remplit sur place, dessinée comme dans sa fiche (`champDeFiche`) ; un seul
 * « Enregistrer » écrit tout ce qui a changé (`enregistrer-fiche`, une
 * écriture par fiche), et la liste se relit. La page ne fait que la mise en
 * page. */
let DEMARRAGE = { titre: "", note: "", manques: [] };

/* La case qu'une ligne propose, telle que sa fiche la décrit : `{nom, fiche,
 * c}`, ou null quand elle ne se remplit pas ici (un organisme, le barème, le
 * taux AT/MP, un accès : la ligne mène à sa place). Une case de structure se
 * dessine seule, dans sa structure. */
function caseDuManque(m) {
  const conf = ETAT.configuration || {};
  const nom = ["salarie", "spectacle", "entreprise"].includes(m.onglet) ? m.onglet : null;
  if (!nom || !m.case) return null;
  const fiche = nom === "entreprise" ? conf.entreprise
    : ((nom === "salarie" ? conf.salaries : conf.spectacles) || []).find((f) => f.cle === (m.salarie || m.spectacle));
  if (!fiche) return null;
  const [racine, ...reste] = m.case.split(".");
  let c = ((conf.description || {})[nom] || []).find((x) => x.cle === racine);
  if (!c || c.calcule || c.ecran === false) return null;
  if (reste.length) {
    const sous = (c.champs || []).find((s) => s.cle === reste.join("."));
    if (!sous) return null;
    c = { ...c, champs: [sous] };
  }
  // UNE VALEUR QUE LE MOTEUR PROPOSE (`propose` : le titre lu dans les bulletins de
  // l'ancien logiciel) remplit la case ; la fiche, elle, garde ce qu'elle dit.
  const affiche = m.propose ? { ...fiche, donnees: { ...fiche.donnees, [racine]: m.propose } } : fiche;
  return { nom, fiche, affiche, c, racine };
}

/* Le dessin de la case : celui de la fiche. Le métier se choisit dans la
 * liste des métiers, comme dans la fiche (`METIER_HTML`) : le code s'en lit. */
function caseDemarrageHtml(k) {
  if (k.racine === "code_emploi_ft") {
    return `<div class="champ"><label>Métier
      <select data-cle="code_emploi_ft">${optionsMetiers(k.fiche.donnees.code_emploi_ft || "", "— choisir —")}</select></label></div>`;
  }
  return champDeFiche(k.c, k.affiche, k.nom).replace(/^(\s*<div class="champ[^"]*"[^>]*?)\s+hidden(?=[\s>])/, "$1");
}

function htmlDemarrage(manques) {
  if (!manques || !manques.length) return htmlPoses();
  const n = ++DEMARRAGE_LISTES;
  // CHAQUE CHOSE MÈNE À SA PLACE : `ou` est un lien qui y conduit (la fiche,
  // l'onglet), sous la case.
  const lien = (m) => `<button type="button" class="lien" data-manque-champ="${echapper(m.champ || "")}"
        data-manque-onglet="${echapper(m.onglet || "")}" data-manque-fiche="${m.salarie ? "salarie" : m.spectacle ? "spectacle" : ""}" data-manque-cle="${echapper(m.salarie || m.spectacle || "")}">${echapper(m.ou || "")}</button>`;
  const ligne = (m) => {
    const i = manques.indexOf(m);
    const k = caseDuManque(m);
    if (!k) {
      return `<li class="demarrage-ligne">
        <b>${echapper(m.libelle)}</b> ${lien(m)}
        <div class="note">${echapper(m.pour || "")}</div>
        ${m.trouver ? `<details><summary>Où le trouver</summary><p class="note">${echapper(m.trouver)}</p></details>` : ""}
      </li>`;
    }
    return `<li class="demarrage-ligne" data-demarrage="${i}">
      <div class="demarrage-case">${caseDemarrageHtml(k)}</div>
      <div class="note">${echapper(m.pour || "")}</div>
      <div class="actions">${lien(m)}</div>
    </li>`;
  };
  const entreprise = manques.filter((m) => !m.qui);
  const qui = [...new Set(manques.filter((m) => m.qui).map((m) => m.qui))];
  const aRemplir = manques.some((m) => caseDuManque(m));
  return `
    <div class="rappel demarrage demarrage-liste" data-demarrage-liste="${n}">
      ${DEMARRAGE.titre ? `<h3>${echapper(DEMARRAGE.titre)}</h3>` : ""}
      ${DEMARRAGE.note ? `<p class="note">${echapper(DEMARRAGE.note)}</p>` : ""}
      ${entreprise.length ? `<ul class="rappel-liste">${entreprise.map(ligne).join("")}</ul>` : ""}
      ${qui.map((q) => `<h4>${echapper(q)}</h4>
        <ul class="rappel-liste">${manques.filter((m) => m.qui === q).map(ligne).join("")}</ul>`).join("")}
      <div class="anomalie" data-demarrage-dit hidden></div>
      <div class="actions">
        ${aRemplir ? `<button class="primaire" data-demarrage-enregistrer>Enregistrer</button>` : ""}
        <button class="petit" data-aller-configuration>Ouvrir la configuration</button>
      </div>
    </div>${htmlPoses()}`;
}
let DEMARRAGE_LISTES = 0;   // chaque liste peinte a son numéro : ses cases se lisent chez elle

/* Les cases dessinées d'une liste : leur aide, et la taxe sur les salaires
 * qui dit, sous ses cases, ce que le moteur en conclut. */
function brancherDemarrage(conteneur) {
  conteneur.querySelectorAll(".demarrage-liste [data-demarrage]").forEach((li) => {
    const sel = `[data-demarrage-liste="${li.closest(".demarrage-liste").dataset.demarrageListe}"] [data-demarrage="${li.dataset.demarrage}"]`;
    brancherAide(sel);
    brancherTaxeSalaires(sel);
  });
}

/* CE QUE LA LISTE PORTE ET QUI N'EST PAS ENREGISTRÉ : par fiche (l'entreprise,
 * chaque salarié, chaque spectacle), les cases remplies qui disent autre chose que la fiche.
 * Une case de structure part seule dans sa structure : le moteur garde les
 * autres cases (`enregistrer-fiche`). */
function changementsDemarrage(liste) {
  const parFiche = new Map();
  const memes = (a, b) => JSON.stringify(a ?? "") === JSON.stringify(b ?? "");
  // Une case de structure dont aucune sous-case n'est remplie (`signataire`, `banque`) est vide, comme une case seule.
  const vide = (v) => v === null || v === undefined || v === "" || (typeof v === "object" && Object.values(v).every(vide));
  const fondre = (a, b) => {
    if (!a || typeof a !== "object" || !b || typeof b !== "object") return b;
    const o = { ...a };
    Object.entries(b).forEach(([x, v]) => { o[x] = fondre(o[x], v); });
    return o;
  };
  liste.querySelectorAll("[data-demarrage]").forEach((li) => {
    const m = DEMARRAGE.manques[+li.dataset.demarrage];
    const k = m && caseDuManque(m);
    if (!k) return;
    const lu = lireFormulaire(`[data-demarrage-liste="${liste.dataset.demarrageListe}"] [data-demarrage="${li.dataset.demarrage}"] .demarrage-case`, {});
    const v = lu[k.racine];
    const avant = k.fiche.donnees[k.racine];
    if (vide(v) || (v === false && !avant) || memes(v, avant)) return;
    const cle = k.nom + "|" + (k.nom === "entreprise" ? "" : k.fiche.cle);
    const f = parFiche.get(cle) || { nom: k.nom, ...(k.nom === "entreprise" ? {} : { cle: k.fiche.cle }), donnees: {} };
    f.donnees[k.racine] = fondre(f.donnees[k.racine], v);
    parFiche.set(cle, f);
  });
  return [...parFiche.values()].map(({ donnees, ...f }) => ({ ...f, ...casesDeLaCharge(f.nom, donnees) }));
}
function demarrageModifie() {
  return [...document.querySelectorAll(".demarrage-liste")].some((l) => changementsDemarrage(l).length);
}

/* ENREGISTRER « POUR DÉMARRER » : tout ce que la liste porte, une écriture
 * par fiche (`enregistrer-fiche`), puis la liste se relit. Rend vrai quand
 * tout est écrit. */
async function enregistrerDemarrage(liste) {
  const fiches = changementsDemarrage(liste);
  const dit = liste.querySelector("[data-demarrage-dit]");
  const bouton = liste.querySelector("[data-demarrage-enregistrer]");
  if (!fiches.length) return true;
  if (bouton) bouton.disabled = true;
  dit.hidden = true;
  let phrase = "";
  try {
    for (const charge of fiches) {
      let r = await api("enregistrer-fiche", charge);
      if (r.avertissement) {
        const rendre = mettreDeCote();
        const reponse = await demanderDateDEffet(r.avertissement);
        if (rendre) rendre();
        if (!reponse) { if (bouton) bouton.disabled = false; return false; }
        r = await api("enregistrer-fiche", { ...charge, ...reponse, confirme: true });
      }
      phrase = r.phrase;
    }
  } catch (e) {
    if (bouton) bouton.disabled = false;
    const anomalies = ((e.detail || {}).anomalies || []).filter((a) => a.gravite === "erreur");
    dit.textContent = anomalies.length ? anomalies.map((a) => `${a.libelle || a.champ} : ${a.message}`).join(" ; ") : e.message;
    dit.hidden = false;
    await relireEtat();
    return false;
  }
  await relireEtat();
  dire(phrase);
  await repeindreDemarrage();
  return true;
}
document.addEventListener("click", (e) => {
  const b = e.target.closest && e.target.closest("[data-demarrage-enregistrer]");
  if (b) enregistrerDemarrage(b.closest(".demarrage-liste"));
});

/* QUITTER « POUR DÉMARRER » avec des cases remplies et pas enregistrées : la
 * même boîte que la fiche (`demanderSiEnregistrer`). Rend vrai si l'on peut
 * partir. */
async function quitterDemarrage() {
  const liste = [...document.querySelectorAll(".demarrage-liste")].find((l) => changementsDemarrage(l).length);
  if (!liste) return true;
  const choix = await demanderSiEnregistrer();
  if (choix === "rester") return false;
  if (choix === "jeter") { await repeindreDemarrage(); return true; }
  return enregistrerDemarrage(liste);
}
/* La liste relue, partout où elle est peinte ; la fiche Entreprise, si elle
 * est ouverte et qu'on n'y a rien tapé, montre ce qui vient de s'écrire. */
async function repeindreDemarrage() {
  try { await lireDemarrage(api); } catch (e) { return; }
  document.querySelectorAll(".demarrage-liste").forEach((l) => {
    const neuf = document.createElement("div");
    neuf.innerHTML = htmlDemarrage(DEMARRAGE.manques);
    const pose = l.nextElementSibling && l.nextElementSibling.classList.contains("demarrage-poses") ? l.nextElementSibling : null;
    if (pose) pose.remove();
    l.replaceWith(...neuf.childNodes);
  });
  document.querySelectorAll("[data-aller-configuration]").forEach((b) => {
    if (b.closest("#config-liste, #import-demarrage")) b.remove();
  });
  brancherDemarrage(document);
  if (FICHE_ACTIVE === "entreprise" && !FICHE_MODIFIEE
      && !document.getElementById("vue-configuration").classList.contains("cachee")) {
    dessinerFormulaire(fichesDe("entreprise")[0]);
  }
}

/* « Ouvrir la configuration », sous la liste de la boîte de l'ouverture. */
document.addEventListener("click", async (e) => {
  const b = e.target.closest && e.target.closest("[data-aller-configuration]");
  if (!b) return;
  if (!(await quitterDemarrage())) return;
  fermerBoite();
  document.querySelector('.onglets button[data-vue="configuration"]').click();
  const sous = document.querySelector('.sous-onglets button[data-fiche="entreprise"]');
  if (sous && !sous.classList.contains("actif")) sous.click();
});

/* `pour-demarrer`, lu : la liste et ce que le moteur a posé. */
async function lireDemarrage(lu) {
  const pd = await lu("pour-demarrer");
  DEMARRAGE = { titre: pd.titre || "", note: pd.note || "", manques: pd.manques || [] };
  POSES_DEMARRAGE = pd.poses || [];
  return DEMARRAGE.manques;
}

/** Aller à ce qui manque : l'onglet, puis la case (son tiroir ouvert, le
 *  curseur dedans). Le taux AT/MP n'est pas une case de la fiche
 *  (`ATMP_HTML`) ; les organismes ont leur onglet ; ce qui se dit sur la
 *  fiche d'un salarié ou d'un spectacle (`salarie`, `spectacle`, du moteur)
 *  ouvre sa fiche. */
async function allerAuManque(champ, onglet, nom, cle) {
  if (!document.getElementById("voile").classList.contains("cachee")) {
    if (!(await quitterDemarrage())) return;
    fermerBoite();
  }
  // Déjà dans la configuration : on n'y retourne pas. Recliquer l'onglet
  // redessinait la fiche et, sur une fiche en cours de saisie, ouvrait « Vous
  // avez modifié cette fiche » — suivre un manque après l'autre sans
  // enregistrer entre deux ne doit rien demander.
  if (document.getElementById("vue-configuration").classList.contains("cachee")) {
    document.querySelector('.onglets button[data-vue="configuration"]').click();
  }
  const fiche = nom || onglet || "entreprise";
  if (nom) await ouvrirFicheImportee(nom, cle);
  else {
    const sous = document.querySelector(`.sous-onglets button[data-fiche="${fiche}"]`);
    if (sous && !sous.classList.contains("actif")) sous.click();
  }
  if (fiche === "organismes" || fiche === "coffre" || !champ) return;
  const trouver = () => champ === "taux_atmp" ? document.getElementById("atmp-valeur")
    : document.querySelector(`#config-formulaire [data-cle="${CSS.escape(champ)}"]:not([type="hidden"])`)
      || document.querySelector(`#config-formulaire [data-sous-de="${CSS.escape(champ)}"]:not([type="hidden"])`);
  let essais = 0;
  const viser = () => {
    const c = trouver();
    if (!c) { if (++essais < 20) setTimeout(viser, 100); return; }
    const tiroir = c.closest("details");
    if (tiroir) tiroir.open = true;
    c.scrollIntoView({ block: "center" });
    c.focus({ preventScroll: true });
  };
  // La liste « Pour démarrer » se peint au-dessus de la fiche quand le moteur
  // a répondu : viser après elle, sinon elle repousse la case hors de la vue.
  Promise.resolve(DEMARRAGE_PEINT).then(viser, viser);
}
document.addEventListener("click", (e) => {
  const b = e.target.closest && e.target.closest("[data-manque-champ]");
  if (!b) return;
  e.preventDefault();
  allerAuManque(b.dataset.manqueChamp, b.dataset.manqueOnglet, b.dataset.manqueFiche, b.dataset.manqueCle);
});

/** La liste « Pour démarrer », en tête de Configuration › Entreprise. */
let DEMARRAGE_PEINT = null;   // la peinture en cours de la liste, pour `allerAuManque`
function peindreDemarrage(conteneur) {
  DEMARRAGE_PEINT = peindreDemarrageDans(conteneur);
  return DEMARRAGE_PEINT;
}
async function peindreDemarrageDans(conteneur) {
  let manques = [];
  try { manques = await lireDemarrage(api); } catch (e) { return; }
  conteneur.innerHTML = htmlDemarrage(manques);
  const b = conteneur.querySelector("[data-aller-configuration]");
  if (b) b.remove();
  brancherDemarrage(conteneur);
}

/* L'en-tête de la boîte : la date, la bienvenue, ce qui est à faire ce
 * mois-ci, et l'année qui vient repliée. Tout vient du moteur
 * : la page ne sait pas ce qu'est une échéance. */
function htmlEcheancier(ech) {
  if (!ech) return "";
  const pastille = (x) => pastilleDeLEtat(x.pastille);
  const avant = (x) => x.quand && x.etat !== "fait"
    ? ` <span class="discret">— ${echapper(x.quand)}</span>` : "";
  const ligne = (x) => `
    <li class="echeance ${echapper(x.etat)}">
      <div class="echeance-titre">${pastille(x)} ${echapper(x.titre)}${avant(x)}</div>
      ${x.etat === "fait" ? "" : `<div class="note">${echapper(x.detail)}</div>`}
      ${x.cochable ? `<button class="secondaire petit" data-echeance="${echapper(x.cle)}"
        data-fait="${x.etat === "fait" ? "non" : "oui"}">${echapper(x.bouton_coche)}</button>` : ""}
    </li>`;
  const parMois = {};
  (ech.a_venir || []).forEach((x) => { (parMois[x.tombe] = parMois[x.tombe] || []).push(x); });
  const annee = Object.keys(parMois).map((m) => `
    <li><b>${echapper(moisEnClair(m))}</b>
      <ul>${parMois[m].map((x) => `<li>${echapper(x.titre)}${x.etat === "fait" ? " ✓" : ""}</li>`).join("")}</ul>
    </li>`).join("");
  // CE QUI EST FAIT NE SE RAPPELLE PAS : la DSN de septembre,
  // déposée, ne s'affiche plus « fait ». Une échéance cochable reste, pour
  // pouvoir la décocher ; les autres faites se taisent.
  // UNE CHOSE SE DIT UNE FOIS : la DSN qu'un rappel réclame
  // (`rappel`, le moteur le dit) se dit dans ce rappel, sous l'en-tête, avec
  // sa date et son geste ; ici elle se dirait en double.
  const ceMois = (ech.ce_mois || []).filter((x) => (x.etat !== "fait" || x.cochable) && !x.rappel);
  const ditAilleurs = (ech.ce_mois || []).some((x) => x.rappel);
  return `
    <div class="accueil">
      <p class="accueil-date">${echapper(ech.date_dit)}</p>
      <h2 class="accueil-titre">${echapper(ech.bienvenue)}</h2>
      ${ceMois.length
        ? `<h3>Ce mois-ci</h3><ul class="echeances">${ceMois.map(ligne).join("")}</ul>`
        : ditAilleurs ? "" : `<h3>Ce mois-ci</h3><p class="discret">Rien à faire</p>`}
      ${annee ? `<details class="accueil-annee"><summary>L'année qui vient</summary>
        <ul class="echeances-annee">${annee}</ul></details>` : ""}
    </div>`;
}

function boiteRappels(liste, ech, manques = [], fermerDit = "Fermer") {
  const html = htmlDemarrage(manques) + htmlEcheancier(ech) + liste.map((r, i) => `
    <div class="rappel ${r.en_retard ? "retard" : ""}">
      <h3>${echapper(r.titre)}${r.pastille ? ` ${pastilleDeLEtat(r.pastille)}` : ""}${r.quand
        ? ` <span class="discret">— ${echapper(r.quand)}</span>` : ""}</h3>
      ${r.question ? `<p class="rappel-question">${echapper(r.question)}</p>` : ""}
      ${r.detail ? `<p class="note">${echapper(r.detail)}</p>` : ""}
      ${r.spectacles && r.spectacles.length ? `<ul class="rappel-liste">${r.spectacles.map((s) =>
        `<li>${echapper(s.titre)} <span class="discret">— ${echapper(s.numero_objet || "aucun numéro")}</span></li>`).join("")}</ul>` : ""}
      <div class="actions">${boutonRappel(r, i)}</div>
    </div>`).join("");
  ouvrirBoite(`
    <div class="boite-rappels"></div>${html}
    <div class="actions">
      <button class="secondaire" id="rappels-fermer">${echapper(fermerDit)}</button>
    </div>`, () => {
    // FERMÉE, ELLE SORT DE LA PAGE : cachée sous le voile, elle gardait
    // ses boutons `[data-manque-champ]`, périmés dès que la fiche change, et
    // qu'on retrouvait en cherchant ceux de « Pour démarrer ».
    const boite = document.getElementById("boite");
    if (boite.querySelector(".boite-rappels")) boite.innerHTML = "";
  });
  // FERMER AVEC DES CASES REMPLIES ET PAS ENREGISTRÉES demande, comme partout (`quitterDemarrage`).
  const fermer = async () => { if (await quitterDemarrage()) fermerBoite(); };
  document.getElementById("rappels-fermer").onclick = fermer;
  CLIC_DEHORS = fermer;
  liste.forEach((r, i) => {
    const b = document.getElementById("rappel-" + i);
    if (b) b.onclick = () => agirSurRappel(r);
  });
  brancherDemarrage(document.getElementById("boite"));
  document.querySelectorAll("[data-echeance]").forEach((b) => {
    b.onclick = async () => {
      try {
        await api("echeance-faite", { cle: b.dataset.echeance, fait: b.dataset.fait === "oui" });
        const ech2 = await api("echeancier");
        const r2 = await api("rappels");
        boiteRappels(r2.rappels || [], ech2, manques, r2.fermer_dit);
      } catch (e) { dire(e.message, "erreur"); }
    };
  });
}

/* CE QUI EST DÛ, ET SON GESTE : le moteur rend l'`action` de chaque rappel, une route du catalogue (sa `charge`, la
   route de l'acte qui suit, `puis`, et le mot du bouton, `libelle`) ; la boîte s'ouvre d'après la route
   (`ouvrirGeste`), comme une case du mois. */
function boutonRappel(r, i) {
  const libelle = r.action && r.action.libelle;
  // UNE ÉCHÉANCE QUE SEULE LA COMPAGNIE PEUT DIRE FAITE (`cochable`) : « C'est fait », comme dans l'échéancier.
  const coche = r.cochable ? `<button class="secondaire" data-echeance="${echapper(r.cle)}" data-fait="oui">${echapper(r.bouton_coche)}</button>` : "";
  return (libelle ? `<button class="primaire" id="rappel-${i}">${echapper(libelle)}</button>` : "") + coche;
}

async function agirSurRappel(r) {
  if (!(await quitterDemarrage())) return;
  fermerBoite();
  const mois = ((r.action || {}).charge || {}).mois;
  // Un geste qui porte sur un mois l'ouvre d'abord : la boîte s'ouvre sur lui.
  if (mois) {
    document.querySelector('.onglets button[data-vue="paie"]').click();
    await chargerMois(mois);
  }
  return ouvrirGeste(r.action, mois || r.mois, null, r);
}

/* ---- La revue des spectacles, en début d'année. Un numéro d'objet vaut
 * tant que le spectacle se joue : on
 * demande seulement lesquels continuent. Tout est coché ; décocher archive,
 * et une fiche archivée se réactive d'un clic dans Configuration. */
function boiteRevueSpectacles(r, charge) {
  const lignes = (r.spectacles || []).map((s, i) => `
    <label class="coche"><input type="checkbox" class="rv-garde" data-cle="${echapper(s.cle)}" checked>
      ${echapper(s.titre)}${s.numero_objet ? ` <span class="discret">${echapper(s.numero_objet)}</span>` : ""}</label>`).join("");
  ouvrirBoite(`
    <h3>${echapper(r.titre_boite)}</h3>
    <p>${echapper(r.question)}</p>
    ${lignes}
    <p class="note">${aideDe("revue_spectacles")}</p>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Plus tard</button>
      <button class="primaire" id="rv-valider">Enregistrer</button>
    </div>`);
  document.getElementById("rv-valider").onclick = async () => {
    const bouton = document.getElementById("rv-valider");
    bouton.disabled = true;
    const continuent = [...document.querySelectorAll(".rv-garde")]
      .filter((c) => c.checked).map((c) => c.dataset.cle);
    try {
      const res = await api("revue-spectacles", { ...charge, continuent });
      await relireEtat();
      fermerBoite();
      dire(res.phrase);
    } catch (e) {
      bouton.disabled = false;
      dire(e.message, "erreur");
    }
  };
}

/* ---- L'exercice, dans l'Historique : par année, ce qui est fait, ce qui
 * se fait. Clôturer fige l'année et écrit le zip ; l'export compta écrit le
 * CSV du journal de paie. Les deux sont locaux — rien ne part. */
async function dessinerExercices() {
  const cible = document.getElementById("exercices");
  let r;
  try { r = await api("exercices"); }
  catch (e) { cible.innerHTML = `<p class="discret">${echapper(e.message)}</p>`; return; }
  if (!r.exercices.length) { cible.innerHTML = `<p class="discret">Aucune année</p>`; return; }
  cible.innerHTML = r.exercices.map((x) => {
    const clos = x.clos;
    return `
    <div class="rang exercice">
      <span class="rang-nom">${echapper(x.annee)}</span>
      ${pastilleDeLEtat(x.pastille)}
      <span class="rang-infos">
        ${echapper(x.infos_dit)}
      </span>
      <button class="secondaire" data-export="${echapper(x.annee)}">Export compta</button>
      ${clos
        ? `<button class="secondaire" data-rouvrir="${echapper(x.annee)}">Rouvrir</button>`
        : `<button class="secondaire" data-cloturer="${echapper(x.annee)}">Clôturer…</button>`}
    </div>`;
  }).join("") + `<p class="note"><button class="lien" id="ouvrir-exercices">Dossier des archives</button></p>`;
  cible.querySelectorAll("[data-export]").forEach((b) => {
    b.onclick = () => lancerExportCompta(b.dataset.export);
  });
  cible.querySelectorAll("[data-cloturer]").forEach((b) => {
    b.onclick = () => boiteCloture(b.dataset.cloturer);
  });
  cible.querySelectorAll("[data-rouvrir]").forEach((b) => {
    b.onclick = async () => {
      try {
        const x = await api("cloture", { annee: b.dataset.rouvrir, rouvrir: true });
        dire(x.phrase);
        await dessinerExercices();
      } catch (e) { dire(e.message, "erreur"); }
    };
  });
  const o = document.getElementById("ouvrir-exercices");
  if (o) o.onclick = async () => {
    await ouvrirEtDire(r.dossier);
  };
}

