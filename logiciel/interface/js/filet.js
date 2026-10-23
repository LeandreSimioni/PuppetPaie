/* Le diagnostic, « Un probleme, une idee ? », les captures, le filet. */
"use strict";

async function dessinerDiagnostic(rang) {
  if (!rang) return;
  const etat = rang.querySelector(".k-diag-etat"), compte = rang.querySelector(".k-diag-compte");
  let d;
  try { d = await api("diagnostic-etat"); } catch (e) { etat.textContent = e.message; return; }
  // L'ENVOI SE COUPE (Leandre, 08/10/2026) : avec un avertissement, et il se
  // rallume d'un clic. Coupé, rien ne part ; le journal reste sur le poste.
  const couper = rang.querySelector("#k-diag-couper");
  couper.hidden = false;
  couper.textContent = d.envoi === false ? "Réactiver" : "Désactiver";
  couper.onclick = async () => {
    if (d.envoi !== false) {
      const oui = await new Promise((resoudre) => {
        let repondu = false;
        const repondre = (v) => { if (!repondu) { repondu = true; resoudre(v); } };
        ouvrirBoite(`<h3>Désactiver le signalement des dysfonctionnements ?</h3>
          <p>Si vous le désactivez, l'éditeur ne verra plus les problèmes de votre
            logiciel, et risque de ne pas pouvoir les corriger avant qu'ils vous
            arrivent — ou qu'ils arrivent à d'autres compagnies. Il ne saura plus
            non plus combien de paies PuppetPaie fait chez vous : le logiciel est
            gratuit, c'est sa seule façon de savoir s'il sert.</p>
          <p class="note">Ce qui part : les dysfonctionnements (des codes et des
            messages qui nomment un champ), et une fois par an, à partir du
            16 janvier, le nombre de paies faites l'année précédente. Rien d'autre :
            ni nom, ni NIR, ni montant. Les incidents resteront notés sur cet
            ordinateur, et vous pourrez réactiver l'envoi à tout moment.</p>
          <p>Voulez-vous vraiment continuer ?</p>
          <div class="actions">
            <button class="secondaire" id="dg-garder">Non, garder l'envoi</button>
            <button class="primaire" id="dg-couper">Oui, désactiver</button>
          </div>`, () => repondre(false));
        document.getElementById("dg-garder").onclick = () => { repondre(false); fermerBoite(); };
        document.getElementById("dg-couper").onclick = () => { repondre(true); fermerBoite(); };
      });
      if (!oui) return;
    }
    try {
      await api("diagnostic-regler", { actif: d.envoi === false });
      dire(d.envoi === false ? "Le signalement des dysfonctionnements est réactivé."
        : "Le signalement des dysfonctionnements est désactivé : rien ne part plus chez l'éditeur.");
    } catch (e) { dire(e.message, "erreur"); return; }
    dessinerDiagnostic(rang);
  };
  etat.innerHTML = d.envoi === false ? `<span class="pastille alerte">désactivé</span>`
    : !d.total ? `<span class="pastille bien">rien à signaler</span>`
    : d.en_attente ? `<span class="pastille alerte">${d.en_attente} en attente</span>`
    : `<span class="pastille bien">tout est parti</span>`;
  compte.textContent = d.envoi === false ? "désactivé : les incidents restent sur cet ordinateur, rien ne part"
    : !d.serveur ? "aucun serveur de l'éditeur connu : rien ne part"
    : d.dernier_envoi ? `${d.partis} incident(s) parti(s), le dernier le ${quand(d.dernier_envoi)}`
    : d.total ? "rien n'est encore parti" : "";
  // Le nombre de paies de l'année (ticket #246) : le dernier parti, à la suite.
  const annee = d.envoi !== false && (d.paies_annuelles || [])[0];
  if (annee) {
    compte.textContent += `${compte.textContent ? " · " : ""}nombre de paies de ${annee.annee} parti le ${quand(annee.parti_le)}`;
  }
  let signales = [];
  try { signales = (await api("signalements")).signalements; } catch (e) { /* la liste est un plus */ }
  rang.querySelector("#k-diag-voir").onclick = () => boiteDiagnostic(d, rang, signales);
}

function boiteDiagnostic(d, rang, signales = []) {
  const NOMS = { logiciel: "plantage", refus: "refus du moteur", dpae: "DPAE rejetée",
                 dsn: "DSN rejetée", paies: "nombre de paies de l'année" };
  ouvrirBoite(`
    <h3>Ce qui est parti chez l'éditeur</h3>
    <p class="note">Quand quelque chose casse, PuppetPaie le note et l'envoie au serveur
      de l'éditeur, pour qu'il le corrige. Une fois par an, à partir du 16 janvier, il
      envoie aussi le nombre de paies faites l'année précédente : le logiciel est
      gratuit, c'est la seule façon pour l'éditeur de savoir s'il sert. Ce qui part est
      affiché ici tel quel :
      ni nom, ni NIR, ni montant ; le poste est désigné par un numéro tiré au hasard.
      ${d.serveur ? `Serveur : <b>${echapper(d.serveur)}</b>.` : "<b>Aucun serveur connu : rien ne part.</b>"}</p>
    ${!d.groupes.length ? `<p>Rien n'a été noté.</p>` : `<div class="liste-choix">
      ${d.groupes.map((g, i) => `
        <div class="choix"><span>
          <b>${echapper(NOMS[g.quoi] || g.quoi)}</b>
          ${g.combien > 1 ? `<span class="discret">× ${g.combien}</span>` : ""}
          — ${echapper(g.resume || g.genre || "")}
          <br><span class="discret">${echapper(g.ou || "")} · dernier le ${echapper(quand(g.derniere))}</span>
          <br>${g.en_attente
            ? `<span class="pastille alerte">${g.en_attente} en attente</span>`
            : `<span class="pastille bien">parti le ${echapper(quand(g.parti_le))}</span>`}
          <button class="lien d-contenu" data-i="${i}">voir le contenu exact</button>
          <pre class="d-pre" data-i="${i}" hidden></pre></span>
        </div>`).join("")}</div>`}
    ${listeSignalements(signales)}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Fermer</button>
      ${d.en_attente && d.serveur ? `<button class="primaire" id="d-envoyer">Envoyer maintenant</button>` : ""}
    </div>`);
  document.querySelectorAll(".d-contenu").forEach((b) => {
    b.onclick = () => {
      const pre = document.querySelector(`.d-pre[data-i="${b.dataset.i}"]`);
      pre.textContent = JSON.stringify(d.groupes[+b.dataset.i].contenu, null, 2);
      pre.hidden = !pre.hidden;
    };
  });
  const envoyer = document.getElementById("d-envoyer");
  if (envoyer) envoyer.onclick = async () => {
    const rendre = occupe(envoyer, "Envoi en cours…");
    try {
      const r = await api("diagnostic-envoyer");
      fermerBoite();
      dire(r.bilan.erreur ? "Envoi impossible : " + r.bilan.erreur
                          : r.bilan.envoyes + " incident(s) envoyé(s).",
           r.bilan.erreur ? "erreur" : "envoye");
      dessinerDiagnostic(rang);
    } catch (e) { rendre(); dire(e.message, "erreur"); }
  };
}

/* SIGNALER UN PROBLÈME. Le bouton du bandeau, sur tous les écrans.
 *
 * PAS SEULEMENT UN PROBLÈME (l'éditeur, 01/10/2026 : « ils peuvent aussi
 * émettre une envie, une idée ») : le bouton s'appelle « Un problème, une
 * idée ? ».
 *
 * L'UTILISATEUR ÉCRIT, JOINT SES CAPTURES ET ENVOIE (ticket #98, 01/10/2026 :
 * « les gens veulent que l'on règle leur problème »). Cinq captures au plus,
 * collées ou choisies. « Réduire » range la boîte dans une pastille, le temps
 * de prendre une capture de l'écran qu'elle cachait, sans rien perdre.
 * « Voir ce qui part » reste là pour qui veut regarder, mais n'est plus une
 * étape : l'aperçu est le corps exact de l'envoi
 * (`operations_filet.signalement_apercu`), la page ne décide de rien. */
const CAPTURE_MAX = 1536 * 1024;           // 1,5 Mo, comme le moteur
const CAPTURES_MAX = 5;
let SIGNALEMENT_REDUIT = null;             // rouvre la boîte rangée dans sa pastille

function ecranCourant() {
  const b = document.querySelector(".onglets button.actif");
  const nom = b ? b.textContent.trim() : "";
  const mois = document.getElementById("mois");
  return nom === "Paie" && mois && mois.value ? `${nom} > ${mois.value}` : nom;
}

function boiteSignalement() {
  if (SIGNALEMENT_REDUIT) return SIGNALEMENT_REDUIT();
  // Déjà ouverte. Fermée, elle reste dans le nid de la boîte sous le voile
  // caché : ce n'est pas « ouverte » (02/10/2026 : « quand je clique sur
  // signaler il ne se rouvre pas »).
  if (document.getElementById("sg-boite")
      && !document.getElementById("voile").classList.contains("cachee")) return;
  const ecran = ecranCourant();
  const captures = [];                      // { type, donnees (data URL), octets }
  const enCours = new Set();                // les captures d'écran qui se dessinent encore (promesses)
  chargerHtml2canvas().catch(() => {});     // l'outil de capture se charge dès l'ouverture, pas au clic (#253)
  // La boîte déjà ouverte (une DPAE, un dépôt) attend dessous, et revient
  // quand celle-ci se ferme ou se réduit.
  let dessous = mettreDeCote();
  const rendreDessous = () => { const d = dessous; dessous = null; if (d) d(); };
  ouvrirBoite(`<div id="sg-boite">
    <h3>Un problème, une idée ?</h3>
    <label class="champ-boite">Ce qui ne va pas, ce qui vous manque, ce que vous aimeriez
      <textarea id="sg-texte" rows="6" maxlength="5000"></textarea></label>
    <div class="sg-vignettes" id="sg-vignettes"></div>
    <p class="discret" id="sg-ajout">Captures d'écran (${CAPTURES_MAX} au plus) : collez-les
      (Ctrl+V), <label class="lien">choisissez des images<input type="file" id="sg-fichier"
      accept="image/png,image/jpeg" multiple hidden></label>, ou
      <button type="button" class="lien" id="sg-capturer"
        title="Photographier l'écran de PuppetPaie tel qu'il est derrière cette boîte">capturez
        l'écran</button>.</p>
    <p class="encart erreur" id="sg-erreur" hidden></p>
    <p id="sg-resultat" hidden></p>
    <div class="actions">
      <button type="button" class="lien sg-voir" id="sg-voir" disabled>voir ce qui part</button>
      <button class="secondaire" id="sg-fermer">Fermer</button>
      <button class="secondaire" id="sg-reduire"
        title="Ranger la boîte le temps de faire une capture d'écran">Réduire</button>
      <button class="primaire" id="sg-envoyer" disabled>Envoyer</button>
    </div>
    <pre class="d-pre" id="sg-pre" hidden></pre>
    <div id="sg-messages"></div></div>`);
  // Les éléments se tiennent par la boîte, pas par le document : réduite,
  // elle est hors de la page, et ses gestes en cours doivent la trouver.
  const boite = document.getElementById("sg-boite");
  const $ = (id) => boite.querySelector("#" + id);
  const texte = $("sg-texte"), envoyer = $("sg-envoyer");
  const montrerErreur = (m) => { $("sg-erreur").textContent = m || ""; $("sg-erreur").hidden = !m; };
  const charge = () => ({ texte: texte.value, ecran,
    captures: captures.map((c) => ({ type: c.type, donnees: c.donnees })) });
  const changement = () => {
    envoyer.disabled = !texte.value.trim();
    $("sg-voir").disabled = !texte.value.trim();
    $("sg-pre").hidden = true;              // l'aperçu montré n'est plus le bon
  };

  const dessinerVignettes = () => {
    $("sg-vignettes").innerHTML = captures.map((c, i) => `
      <span class="sg-vignette"><img src="${c.donnees}" alt="Capture ${i + 1}">
        <button type="button" class="sg-retirer" data-i="${i}" title="Retirer cette capture">×</button></span>`).join("")
      + `<span class="sg-vignette sg-en-cours">capture…</span>`.repeat(enCours.size);
    $("sg-vignettes").querySelectorAll(".sg-retirer").forEach((b) => {
      b.onclick = () => { captures.splice(+b.dataset.i, 1); dessinerVignettes(); };
    });
    $("sg-ajout").hidden = captures.length + enCours.size >= CAPTURES_MAX;
    changement();
  };
  /** Pose une image parmi les captures ; rend une promesse tenue quand elle y est (ou refusée). */
  const poser = (fichier) => new Promise((fini) => {
    montrerErreur("");
    if (captures.length >= CAPTURES_MAX) {
      return fini(montrerErreur(`${CAPTURES_MAX} captures au plus.`));
    }
    if (!/^image\/(png|jpeg)$/.test(fichier.type)) {
      return fini(montrerErreur("Une capture doit être une image PNG ou JPEG."));
    }
    if (fichier.size > CAPTURE_MAX) {
      return fini(montrerErreur("Une capture dépasse 1,5 Mo : réduisez-la, ou découpez-la."));
    }
    const lecteur = new FileReader();
    lecteur.onload = () => {
      if (captures.length >= CAPTURES_MAX) return fini(montrerErreur(`${CAPTURES_MAX} captures au plus.`));
      captures.push({ type: fichier.type, donnees: lecteur.result, octets: fichier.size });
      dessinerVignettes();
      fini();
    };
    lecteur.onerror = () => fini();
    lecteur.readAsDataURL(fichier);
  });
  $("sg-fichier").onchange = () => {
    [...$("sg-fichier").files].forEach(poser);
    $("sg-fichier").value = "";
  };
  // Sur le contenu de CETTE boite : il disparait avec elle, l'ecouteur aussi.
  boite.addEventListener("paste", (e) => {
    const images = [...(e.clipboardData ? e.clipboardData.items : [])]
      .filter((i) => i.type.startsWith("image/"));
    if (!images.length) return;
    e.preventDefault();
    images.forEach((i) => poser(i.getAsFile()));
  });
  texte.addEventListener("input", changement);

  // VOS MESSAGES, et leurs réponses : ouvrir la boîte les rend lues.
  (async () => {
    try {
      const l = (await api("signalements")).signalements;
      $("sg-messages").innerHTML = vosMessages(l);
      if (l.some((s) => s.reponse && !s.lu)) {
        await api("signalements-lus");
        dessinerPastilleReponses();
      }
    } catch (e) { /* la liste est un plus */ }
  })();

  // RÉDUIRE : la boîte sort de la page, intacte, et une pastille la rappelle.
  // Le bouton « Un problème, une idée ? » du bandeau la rappelle aussi.
  const surCetteBoite = () => {
    FERMETURE_BOITE = rendreDessous;
    // Un clic à côté range la boîte ; une fois le message parti, il la ferme.
    // Rien d'écrit ni de capturé : rien à garder, il la ferme aussi
    // (02/10/2026 : « si on n'a rien modifié, on laisse partir »).
    const vide = () => !texte.value.trim() && !captures.length;
    CLIC_DEHORS = () => ($("sg-reduire").hidden || vide() ? fermerBoite() : $("sg-reduire").click());
  };
  $("sg-fermer").onclick = () => fermerBoite();

  // CAPTURER L'ÉCRAN, PAR LE LOGICIEL (ticket #102, 02/10/2026 : « un bouton,
  // capturer l'écran actuel, une espèce d'imprime-écran »). La photo est la
  // page SANS cette boîte -- et avec la fenêtre qu'elle recouvrait (une DPAE,
  // un dépôt) --, celle de la PAGE, dessinée par html2canvas : ce qui est hors
  // de PuppetPaie (une autre fenêtre, le bureau) n'y est pas.
  //
  // LA BOÎTE NE BOUGE PLUS (#253, 23/10/2026 : « quand j'ai cliqué sur
  // capturer l'écran ça a pris 1,5 seconde alors que ça devrait être
  // instantané, en arrière-plan discret »). Elle s'effaçait de l'écran le temps
  // de la photo, puis revenait. Elle reste : c'est dans la COPIE de la page que
  // html2canvas dessine qu'on la retire et qu'on remet la fenêtre d'en dessous
  // (`photographierLaPage`). Une place « capture… » tient celle de l'image
  // pendant le dessin ; Envoyer attend qu'elle soit posée. L'image est la même
  // qu'avant : la page telle qu'on la voyait, en JPEG sous 1,5 Mo.
  $("sg-capturer").onclick = () => {
    montrerErreur("");
    if (captures.length + enCours.size >= CAPTURES_MAX) return montrerErreur(`${CAPTURES_MAX} captures au plus.`);
    // La place « capture… » d'abord : elle se peint avant que la copie de la page ne prenne la main.
    const tour = photographierLaPage(dessous && dessous.noeuds)
      .then(poser, (e) => montrerErreur("La capture n'a pas pu se faire : " + e.message))
      .finally(() => { enCours.delete(tour); dessinerVignettes(); });
    enCours.add(tour);
    dessinerVignettes();
  };
  $("sg-reduire").onclick = () => {
    const pastille = document.createElement("button");
    pastille.type = "button";
    pastille.className = "sg-pastille";
    pastille.textContent = "Message en cours"
      + (captures.length ? ` (${captures.length} capture${captures.length > 1 ? "s" : ""})` : "")
      + " — reprendre";
    SIGNALEMENT_REDUIT = () => {
      SIGNALEMENT_REDUIT = null;
      pastille.remove();
      dessous = mettreDeCote();
      ouvrirBoite("");
      document.getElementById("boite").appendChild(boite);
      surCetteBoite();
      texte.focus();
    };
    pastille.onclick = SIGNALEMENT_REDUIT;
    boite.remove();
    fermerBoite();                          // rend la boîte de dessous, s'il y en a une
    document.body.appendChild(pastille);
  };
  surCetteBoite();

  $("sg-voir").onclick = async () => {
    if (!$("sg-pre").hidden) { $("sg-pre").hidden = true; return; }
    montrerErreur("");
    try {
      await Promise.all([...enCours]);      // une capture qui se dessine encore part avec le reste
      const a = await api("signalement-apercu", charge());
      $("sg-pre").textContent = JSON.stringify(a.corps, null, 2);
      $("sg-pre").hidden = false;
    } catch (e) { montrerErreur(e.message); }
  };
  // ENVOYER REND LA MAIN TOUT DE SUITE (Léandre, 14/10/2026) : le moteur
  // enregistre le message sur l'ordinateur et le fait partir en fond ; ce qui
  // ne part pas tout de suite, la veille le renvoie. La page ne fait que
  // montrer où il en est, dans « Vos messages ».
  envoyer.onclick = async () => {
    montrerErreur("");
    const rendre = occupe(envoyer, "Envoi en cours…");
    try {
      await Promise.all([...enCours]);      // une capture qui se dessine encore part avec le reste
      const r = await api("signalement-envoyer", charge());
      $("sg-pre").hidden = true;
      $("sg-resultat").hidden = false;
      // D'ABORD MERCI (01/10/2026), puis où en est l'envoi, dit par le moteur (ticket #220 : « envoi en cours » puis
      // « message envoyé »). La réponse revient dans « Vos messages ».
      SG_ENVOI = { id: r.id, deja: !!r.deja };
      $("sg-resultat").innerHTML = departDuMessage((r.signalements || []).find((s) => s.id === r.id));
      $("sg-messages").innerHTML = vosMessages(r.signalements || []);
      ["sg-voir", "sg-reduire", "sg-ajout"].forEach((id) => { $(id).hidden = true; });
      envoyer.hidden = true;
      texte.readOnly = true;
      $("sg-vignettes").querySelectorAll(".sg-retirer").forEach((b) => { b.hidden = true; });
      suivreLesMessages(boite, Date.now());
    } catch (e) { rendre(); montrerErreur(e.message); }
  };
  texte.focus();
}
document.getElementById("signaler").onclick = boiteSignalement;
document.getElementById("signaler-voile").onclick = boiteSignalement;

/** html2canvas, chargé au premier usage seulement (servi par le moteur). */
let HTML2CANVAS = null;
function chargerHtml2canvas() {
  if (window.html2canvas) return Promise.resolve(window.html2canvas);
  if (!HTML2CANVAS) {
    HTML2CANVAS = new Promise((ok, ko) => {
      const sc = document.createElement("script");
      sc.src = "/html2canvas.min.js";
      sc.onload = () => ok(window.html2canvas);
      sc.onerror = () => { HTML2CANVAS = null; ko(new Error("outil de capture introuvable")); };
      document.head.appendChild(sc);
    });
  }
  return HTML2CANVAS;
}

/** La page telle qu'on la voit (la partie affichée), SANS la boîte du message, en JPEG sous 1,5 Mo. `dessous` : les
 *  nœuds de la boîte qu'elle recouvre (`mettreDeCote`), qui figurent sur la photo à sa place ; sans eux, le voile part.
 *  L'écran, lui, ne change pas (#253) : tout se fait dans la copie de la page que html2canvas dessine (`onclone`).
 *
 *  UN SEUL DESSIN (#253) : la page se dessine une fois, à l'échelle 1 ; une image trop lourde se réduit depuis ce dessin
 *  (mêmes échelles et qualités qu'avant), au lieu de redessiner la page. */
async function photographierLaPage(dessous) {
  const h2c = await chargerHtml2canvas();
  await new Promise((ok) => requestAnimationFrame(() => requestAnimationFrame(ok)));
  const toile = await h2c(document.body, {
    scale: 1, logging: false, backgroundColor: "#ffffff",
    x: window.scrollX, y: window.scrollY,
    width: window.innerWidth, height: window.innerHeight,
    windowWidth: document.documentElement.clientWidth,
    ignoreElements: (el) => el.id === "signaler-voile"
      || (el.classList && el.classList.contains("sg-pastille")),
    onclone: (copie) => {
      const nid = copie.getElementById("boite");
      if (!nid) return;
      nid.innerHTML = "";
      if (dessous && dessous.length) dessous.forEach((n) => nid.appendChild(copieAvecSaisie(n, copie)));
      else copie.getElementById("voile").classList.add("cachee");
    },
  });
  const essais = [[1, 0.85], [1, 0.6], [0.75, 0.6], [0.5, 0.6]];
  for (const [echelle, qualite] of essais) {
    let t = toile;
    if (echelle !== 1) {
      t = document.createElement("canvas");
      t.width = Math.round(toile.width * echelle);
      t.height = Math.round(toile.height * echelle);
      t.getContext("2d").drawImage(toile, 0, 0, t.width, t.height);
    }
    const blob = await new Promise((ok) => t.toBlob(ok, "image/jpeg", qualite));
    if (blob && blob.size <= CAPTURE_MAX) {
      const quand = new Date().toTimeString().slice(0, 8).replace(/:/g, "");
      return new File([blob], `ecran-${quand}.jpg`, { type: "image/jpeg" });
    }
  }
  throw new Error("l'image dépasse 1,5 Mo même réduite");
}

/** `noeud` copié dans le document `doc`, avec ce qu'on y a tapé ou coché : une copie de nœud reprend les attributs,
 *  pas la valeur courante d'un champ (une case cochée à la main, un texte tapé). */
function copieAvecSaisie(noeud, doc) {
  const copie = doc.importNode(noeud, true);
  if (noeud.nodeType !== 1) return copie;
  const champs = (n) => [n, ...n.querySelectorAll("input, textarea, select")].filter((c) => /^(INPUT|TEXTAREA|SELECT)$/.test(c.tagName));
  const de = champs(noeud), vers = champs(copie);
  de.forEach((c, i) => {
    const v = vers[i];
    if (!v) return;
    if (c.type === "checkbox" || c.type === "radio") v.checked = c.checked;
    else v.value = c.value;
  });
  return copie;
}

/* LES RÉPONSES DE L'ÉDITEUR (01/10/2026 : « chaque ticket a le droit à une
 * réponse »). Le moteur les relève à chaque veille (`relever_reponses`) ; une
 * réponse non lue se voit sur le bouton du bandeau, et se lit dans la boîte. */
function vosMessages(liste) {
  if (!liste.length) return "";
  return `<h4 class="sg-titre-messages">Vos messages</h4><ul class="sg-messages">${liste.map((s) => `
    <li${s.reponse && !s.lu ? ' class="nouveau"' : ""}><span class="discret">${echapper(quand(s.quand))}</span>
      ${echapper(s.debut)}${s.debut && s.debut.length >= 80 ? "…" : ""}
      ${s.reponse ? `<p class="sg-reponse">${echapper(s.reponse)}</p>` : etatDuMessage(s)}</li>`).join("")}</ul>`;
}

/** CE QUE DIT LA BOÎTE DU MESSAGE QU'ON VIENT D'ENVOYER, d'après son état dans le moteur : en cours, envoyé, ou pas
 *  encore parti (et pourquoi). Un message déjà enregistré (un second clic, ticket #229) le dit : il ne part qu'une fois. */
let SG_ENVOI = null;
function departDuMessage(s) {
  const deja = SG_ENVOI && SG_ENVOI.deja ? ` <span class="discret">Ce message était déjà enregistré : il ne part qu'une fois.</span>` : "";
  if (!s || s.etat === "en-cours") {
    return `<b>Merci, votre message est enregistré.</b> <span class="sg-envoi">Envoi en cours…</span>${deja}`;
  }
  if (s.etat === "pas-parti") {
    return `<b>Merci, votre message est enregistré.</b> <span class="sg-envoi alerte">Pas encore parti</span>
      — ${echapper(s.raison || "PuppetPaie le renverra de lui-même.")}${deja}`;
  }
  return `<span class="sg-envoi envoye">Message envoyé.</span> Merci ! La réponse de l'éditeur viendra ici,
    dans « Vos messages ».${deja}`;
}

/** Où en est un message (l'état vient du moteur, `problemes::signalements`). */
function etatDuMessage(s) {
  if (s.etat === "en-cours") return ` <span class="pastille">en cours d'envoi</span>`;
  if (s.etat === "pas-parti") {
    return ` <span class="pastille alerte">pas encore parti</span>
      <span class="discret">— ${echapper(s.raison || "")}</span>`;
  }
  return ` <span class="discret">— arrivé, en attente de réponse</span>`;
}

/** Tant qu'un message est en cours d'envoi et la boîte ouverte, la liste se
 *  relit (toutes les trois secondes, deux minutes au plus). */
function suivreLesMessages(boite, depuis) {
  setTimeout(async () => {
    if (!boite.isConnected && !SIGNALEMENT_REDUIT) return;
    if (Date.now() - depuis > 2 * 60 * 1000) return;
    let l;
    try { l = (await api("signalements")).signalements; } catch (e) { return; }
    const nid = boite.querySelector("#sg-messages");
    if (nid) nid.innerHTML = vosMessages(l);
    const res = boite.querySelector("#sg-resultat");
    if (res && SG_ENVOI) res.innerHTML = departDuMessage(l.find((s) => s.id === SG_ENVOI.id));
    if (l.some((s) => s.etat === "en-cours")) suivreLesMessages(boite, depuis);
  }, 3000);
}

/** La pastille des réponses. `lus` : les signalements déjà lus (l'ouverture d'un coup, js/demarrage.js), sinon on
 *  les demande. */
async function dessinerPastilleReponses(lus) {
  let n = 0;
  try { n = (lus || await api("signalements")).non_lues || 0; } catch (e) { return; }
  document.getElementById("signaler").innerHTML = "Un problème, une idée ?"
    + (n ? ` <span class="sg-nouvelles">· ${n} réponse${n > 1 ? "s" : ""}</span>` : "");
}

function listeSignalements(liste) {
  if (!liste.length) return "";
  return `<h4>Vos signalements</h4><ul class="sg-liste">${liste.map((s) => `
    <li><span class="discret">${echapper(quand(s.quand))}</span> <b>${echapper(s.numero)}</b>
      — ${echapper(s.debut)}${s.ticket
        ? ` <a href="${echapper(s.ticket)}" target="_blank" rel="noopener">ticket n° ${echapper(String(s.ticket_numero ?? ""))}</a>` : ""}${s.etat !== "arrive" ? etatDuMessage(s) : ""}</li>`).join("")}</ul>`;
}

/** La ligne du filet de sécurité DPAE (27/09/2026). L'état vient du moteur
 *  (`operations_filet.etat_filet`) ; la clé du compte ne vient jamais ici. */
let ATTENTE_FILET = null;      // la minuterie qui guette la confirmation

async function dessinerFilet(rang, exige) {
  if (!rang) return;
  clearTimeout(ATTENTE_FILET);
  const etat = rang.querySelector(".k-filet-etat"), compte = rang.querySelector(".k-filet-compte");
  const verdict = rang.querySelector(".k-verdict");
  const arreter = rang.querySelector("#k-filet-arreter"), ouvrir = rang.querySelector("#k-filet-ouvrir");
  let f;
  try { f = await api("filet-etat"); } catch (e) { etat.textContent = e.message; return; }
  if (f.actif && !f.confirme) {
    try { f = await api("filet-verifier"); } catch (e) { /* le serveur dira plus tard */ }
  }
  etat.innerHTML = !f.actif ? `<span class="pastille">désactivé</span>`
    : f.confirme ? `<span class="pastille bien">actif</span>`
    : `<span class="pastille alerte">adresse à confirmer</span>`;
  compte.textContent = !f.actif ? "" : f.confirme ? f.adresse
    : `un courriel est parti à ${f.adresse} : cliquez sur le lien qu'il contient —
       cette ligne passera d'elle-même à « actif »`;
  // LA CONFIRMATION SE VOIT SANS RIEN FAIRE (27/09/2026) : tant que l'adresse
  // attend son clic, on redemande au serveur toutes les dix secondes, dix
  // minutes au plus, et on s'arrête dès qu'on quitte l'écran.
  if (f.actif && !f.confirme) guetterConfirmation(rang, exige, Date.now());
  if (f.erreur) verdict.textContent = f.erreur;
  arreter.hidden = !f.actif;
  // Actif, la ligne s'ouvre d'un clic sur elle (ticket #218) ; « Activer… » reste tant qu'il y a à faire.
  ouvrir.hidden = !!f.actif;
  const champ = rang.querySelector("#k-filet-adresse");
  if (!champ.value) champ.value = f.adresse || "";
  const preuve = async (quoi) => {
    if (!exige) return { motdepasse: null };
    const m = await demanderMotDePasse(quoi);
    return m ? { motdepasse: m } : null;
  };
  rang.querySelector("#k-filet-activer").onclick = async () => {
    const adresse = champ.value.trim();
    if (!adresse) { dire("L'adresse qui recevra le rappel."); champ.focus(); return; }
    const p = await preuve("activer le filet de sécurité DPAE");
    if (!p) return;
    try {
      await api("filet-activer", { adresse, ...p });
      dire("Un courriel de confirmation est parti à " + adresse + ".");
      rang.querySelector(".rang-forme").hidden = true;
      dessinerFilet(rang, exige);
    } catch (e) { dire(e.message, "erreur"); }
  };
  arreter.onclick = async () => {
    if (!confirm("Désactiver le filet ? Plus aucun courriel ne préviendra d'une DPAE oubliée.")) return;
    const p = await preuve("désactiver le filet de sécurité DPAE");
    if (!p) return;
    try {
      await api("filet-desactiver", p);
      dire("Filet de sécurité DPAE désactivé.");
      dessinerFilet(rang, exige);
    } catch (e) { dire(e.message, "erreur"); }
  };
}

function guetterConfirmation(rang, exige, depuis) {
  ATTENTE_FILET = setTimeout(async () => {
    if (!rang.isConnected || Date.now() - depuis > 10 * 60 * 1000) return;
    let f;
    try { f = await api("filet-verifier"); } catch (e) { f = null; }
    if (f && f.confirme) {
      dire("Adresse confirmée : le filet de sécurité DPAE est actif.");
      return dessinerFilet(rang, exige);
    }
    guetterConfirmation(rang, exige, depuis);
  }, 10000);
}

