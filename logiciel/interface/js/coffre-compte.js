/* Les mots de passe et le compte Google (Configuration > Mots de passe). */
"use strict";

/* ------------------------------------------------------- les mots de passe */
/* Configuration → Mots de passe. Trois choses, et une règle.
 *
 * Le mot de passe DU LOGICIEL : celui de la fenêtre et de chaque dépôt. On
 * le pose, on le change (l'actuel est exigé), on le retire. Les mots de
 * passe DES ORGANISMES (net-entreprises, Urssaf) vont au coffre : on les
 * pose, on les vérifie (UNE tentative, jamais de boucle : l'organisme bloque
 * le compte au bout de quelques échecs), on les oublie. La boîte courriel
 * s'y pose aussi, avec son adresse : c'est un mot de passe
 * d'application, la seule boîte qui envoie et qui lit. Et le COMPTE GOOGLE,
 * qui ne porte que le Drive : « Lier le Drive » ouvre la
 * page de consentement dans le navigateur du système et attend qu'on y
 * accepte ; « Oublier » efface le jeton local.
 *
 * La règle : la page ne reçoit JAMAIS une valeur posée — seulement qu'elle
 * l'est, et depuis quand. Et si un mot de passe du logiciel est posé, il
 * est demandé pour écrire dans le coffre comme pour lier ou oublier le
 * compte Google, comme pour un dépôt : c'est `operations` qui l'exige, pas
 * ce fichier. */
/** Le compte qu'ouvre un mot de passe : sans lui, un mot de passe
 *  juste sur un identifiant faux échoue pareil, et rien ne le montrait.
 *  net-entreprises : le déclarant de la fiche Entreprise, qui se change
 *  là-bas ; l'Urssaf (DPAE) : un autre compte, qui se saisit sur la ligne. */
function compteDuService(s) {
  const id = s.identifiant || {};
  if (id.texte) return `<span class="compte">${echapper(id.texte)}</span>`;
  if (!id.manque || !id.manque.length) return "";
  return id.ici
    ? `<span class="compte a-completer">${echapper(id.manque_dit)}</span>`
    : `<span class="compte a-completer">${echapper(id.incomplet_dit || "")} —
        <button class="lien" data-aller-fiche="entreprise">${echapper(id.ou || "fiche Entreprise")}</button></span>`;
}

/** CE QUI EST PARTI CHEZ L'ÉDITEUR. Le journal des incidents,
 *  regroupé : ce qui est parti, ce qui
 *  attend, et pour chaque groupe le contenu EXACT qui quitte le poste. */
async function dessinerCoffre(cible) {
  // LA COMPAGNIE FICTIVE : le même écran, tout grisé —
  // on voit ce qui se pose et où, sans rien pouvoir y écrire. Rien ne part,
  // il n'y a donc rien à poser ; la page ne branche aucun geste.
  const fictive = enFictive();
  const c = await api("coffre-etat");
  const exige = c.mot_de_passe_pose;
  // L'ADMINISTRATEUR : ce que le moteur dit de cette compagnie pour lui (rattachée, ou à rattacher). Une compagnie
  // fictive n'en a pas.
  const adm = fictive ? {} : await api("administrateur-etat");
  // UNE LIGNE PAR SECRET, comme les mois de l'Historique : ce
  // qu'il sert à faire, où il en est, et les gestes. Les cases de saisie ne
  // s'ouvrent que sur un clic sur la ligne (ou « Poser… ») -- cinq formulaires ouverts
  // d'office faisaient croire qu'il y avait cinq choses à remplir.
  const etatService = (s) => `${pastilleDeLEtat(s.pastille)}${!fictive && !s.environnement && s.pose && s.compte ? ` <span class="discret">${echapper(s.compte)}</span>` : ""}`;
  const k = c.ecran || {};
  const l = k.logiciel || {};
  const a = k.administrateur;
  cible.innerHTML = `
    <p class="intro">${echapper(k.intro)}</p>
    ${encart(k.encart)}
    <div class="rangs">
      <div class="rang">
        <div class="rang-infos"><b class="rang-nom">${echapper(l.nom)}</b>
          <span class="discret">${echapper(l.dit)}</span></div>
        <div class="rang-etat">${pastilleDeLEtat(c.pastille_mot_de_passe)}</div>
        <div class="rang-actions">
          ${exige ? "" : `<button class="secondaire k-ouvrir">Poser…</button>`}</div>
        <form class="rang-forme" id="k-logiciel" onsubmit="return false" autocomplete="off" hidden>
          ${exige ? `<label>Mot de passe actuel
            <input type="password" id="k-actuel" autocomplete="current-password"></label>` : ""}
          <label>Nouveau <span class="discret">— ${echapper(l.nouveau)}</span>
            <input type="password" id="k-nouveau" autocomplete="new-password"></label>
          <label>Le retaper <input type="password" id="k-confirmation" autocomplete="new-password"></label>
          <div class="barre-boutons">
            <button class="primaire" id="k-changer">${echapper(c.bouton_mot_de_passe)}</button>
            ${exige && !adm.rattachee ? `<button class="lien" id="k-retirer">retirer</button>` : ""}
          </div>
        </form>
      </div>
      ${a && adm.rattachee ? `
      <div class="rang" id="k-admin">
        <div class="rang-infos"><b class="rang-nom">L'administrateur</b>
          <span class="discret">${echapper(a.dit)}</span></div>
        <div class="rang-etat">${pastilleDeLEtat(adm.pastille)}</div>
        <div class="rang-actions"><button class="lien k-ouvrir">Détacher…</button></div>
        <form class="rang-forme" onsubmit="return false" autocomplete="off" hidden>
          <p class="note">${echapper(a.note)}</p>
          <label>Mot de passe administrateur <input type="password" id="k-adm-actuel" autocomplete="current-password"></label>
          <label>Nouveau mot de passe <input type="password" id="k-adm-nouveau" autocomplete="new-password"></label>
          <label>Le retaper <input type="password" id="k-adm-confirmation" autocomplete="new-password"></label>
          <div class="barre-boutons"><button class="primaire" id="k-detacher">Détacher</button></div>
        </form>
      </div>` : a ? `
      <div class="rang" id="k-admin">
        <div class="rang-infos"><b class="rang-nom">L'administrateur</b>
          <span class="discret">${echapper(a.dit)}</span></div>
        <div class="rang-etat">${pastilleDeLEtat(adm.pastille)}</div>
        <div class="rang-actions"><button class="secondaire k-ouvrir">Rattacher…</button></div>
        <form class="rang-forme" onsubmit="return false" autocomplete="off" hidden>
          <p class="note">${echapper(a.note)}</p>
          ${exige ? `<label>Mot de passe actuel <input type="password" id="k-adm-propre" autocomplete="current-password"></label>` : ""}
          <label>Mot de passe administrateur <input type="password" id="k-adm-mdp" autocomplete="current-password"></label>
          <div class="barre-boutons"><button class="primaire" id="k-rattacher">Rattacher</button></div>
        </form>
      </div>` : ""}
      ${c.services.map((s) => `
      <div class="rang" data-service="${echapper(s.service)}">
        <div class="rang-infos"><b class="rang-nom">${echapper(s.libelle)}</b>
          <span class="discret">${echapper(s.sert_a)}</span>${fictive ? "" : compteDuService(s)}</div>
        <div class="rang-etat">${etatService(s)}</div>
        <div class="rang-actions">
          ${!fictive && (s.pose || s.environnement) ? `<button class="lien k-verifier"
            title="${echapper(k.verifier)}">Vérifier</button>` : ""}
          ${s.pose ? `<button class="lien k-oublier">Oublier</button>` : ""}
          ${s.pose ? "" : `<button class="secondaire k-ouvrir">Poser…</button>`}
        </div>
        <div class="k-verdict discret"></div>
        <form class="rang-forme" onsubmit="return false" autocomplete="off"
          ${s.identifiant?.ici && s.identifiant.manque.length ? "" : "hidden"}>
          ${s.identifiant?.ici ? `<label>Prénom du compte Urssaf
            <input type="text" class="k-prenom" autocomplete="off" value="${echapper(s.identifiant.prenom)}"></label>
          <label>Nom du compte Urssaf
            <input type="text" class="k-nom" autocomplete="off" value="${echapper(s.identifiant.nom)}"></label>` : ""}
          ${s.avec_compte ? `<label>Adresse de la boîte
            <input type="email" class="k-compte" autocomplete="off" value="${echapper(s.compte || "")}"></label>` : ""}
          <label>Mot de passe
            <input type="password" class="k-valeur" autocomplete="off"></label>
          <div class="barre-boutons"><button class="primaire k-poser">${echapper(s.bouton)}</button></div>
          ${s.identifiant?.ici ? `<p class="note">${echapper(k.urssaf)}</p>` : ""}
          ${s.avec_compte ? `<p class="note">${echapper(k.courriel)}</p>` : ""}
        </form>
      </div>`).join("")}
      <div class="rang" id="k-google">
        <div class="rang-infos"><b class="rang-nom">Compte Google</b>
          <span class="discret">${echapper(k.google)}</span>${VEILLE?.dernier?.sauvegarde_souci
            ? `<span class="compte">${echapper(VEILLE.dernier.sauvegarde_souci)}</span>`
            : VEILLE?.dernier?.sauvegarde
            ? `<span class="compte">${echapper(VEILLE.dernier.sauvegarde)}</span>` : ""}</div>
        <div class="rang-etat k-google-etat"><span class="discret">lecture…</span></div>
        <div class="rang-actions">
          <button class="lien" id="k-google-oublier" hidden>Oublier</button>
          <button class="secondaire" id="k-google-lier" disabled>Lier le Drive</button>
        </div>
        <div class="k-verdict discret"></div>
      </div>
      <div class="rang" id="k-diagnostic">
        <div class="rang-infos"><b class="rang-nom">Signalement des dysfonctionnements</b>
          <span class="discret">${echapper(k.diagnostic)}</span>
          <span class="compte k-diag-compte"></span></div>
        <div class="rang-etat k-diag-etat"><span class="discret">lecture…</span></div>
        <div class="rang-actions">
          <button class="lien" id="k-diag-couper" hidden>Désactiver</button>
          <button class="secondaire" id="k-diag-voir">Ce qui est parti</button>
        </div>
      </div>
    </div>`;

  if (fictive) {
    // Les lignes ouvertes, pour voir leurs champs ; tout est grisé, sauf les
    // liens vers la fiche Entreprise, qui ne font que naviguer.
    cible.querySelectorAll(".rang-forme").forEach((f) => { f.hidden = false; });
    cible.querySelectorAll("input, button:not([data-aller-fiche])").forEach((x) => {
      x.disabled = true;
    });
    cible.querySelector(".k-google-etat").innerHTML = pastilleDeLEtat(c.fictive.google);
    cible.querySelectorAll("[data-aller-fiche]").forEach((b) => {
      b.onclick = () => document.querySelector(
        `.sous-onglets button[data-fiche="${b.dataset.allerFiche}"]`).click();
    });
    return;
  }

  dessinerDiagnostic(cible.querySelector("#k-diagnostic"));

  // Venu du bandeau : la ligne de ce qui manque, ouverte et montrée.
  if (OUVRIR_SERVICE) {
    const rang = OUVRIR_SERVICE === "google" ? cible.querySelector("#k-google")
      : cible.querySelector(`[data-service="${OUVRIR_SERVICE}"]`);
    OUVRIR_SERVICE = null;
    if (rang) {
      rang.classList.add("montre");
      rang.scrollIntoView({ block: "center" });
      const forme = rang.querySelector(".rang-forme");
      if (forme) { forme.hidden = false; forme.querySelector("input").focus(); }
    }
  }
  cible.querySelectorAll("[data-aller-fiche]").forEach((b) => {
    b.onclick = () => document.querySelector(
      `.sous-onglets button[data-fiche="${b.dataset.allerFiche}"]`).click();
  });
  // LA LIGNE S'OUVRE D'UN CLIC SUR ELLE.
  // Elle ouvre ses cases et met le curseur dans la première ; un second clic
  // la referme. « Poser… » (rien n'est posé : il y a à faire) fait de même.
  const basculer = (rang) => {
    const forme = rang.querySelector(".rang-forme");
    forme.hidden = !forme.hidden;
    if (!forme.hidden) forme.querySelector("input").focus();
  };
  cible.querySelectorAll(".rang").forEach((rang) => {
    if (rang.querySelector(".rang-forme")) rendreOuvrable(rang, () => basculer(rang));
  });
  cible.querySelectorAll(".k-ouvrir").forEach((b) => { b.onclick = () => basculer(b.closest(".rang")); });

  // La preuve, si elle est exigée : le popup. Null = annulé, on ne fait rien.
  const preuve = async (quoi) => {
    if (!exige) return { motdepasse: null };
    const m = await demanderMotDePasse(quoi);
    return m ? { motdepasse: m } : null;
  };
  // L'ACTUEL EST UN CHAMP, PAS UN POPUP, et seulement ici. La regle du popup vaut pour un acte qui ENGAGE : deposer,
  // envoyer, signer, poser un secret d'organisme -- et elle ne bouge pas, le
  // popup reste pour tout cela. Changer le mot de passe du logiciel n'est pas
  // de cette famille : l'ancien n'autorise pas l'acte, il est ce que l'acte
  // remplace. Le taper au-dessus du nouveau, c'est la forme que tout le monde
  // connait, et elle se lit d'un coup d'oeil.
  const actuelSaisi = () => {
    if (!exige) return { motdepasse: null };
    const champ = document.getElementById("k-actuel");
    return { motdepasse: champ ? champ.value : "" };
  };
  const ok = (texte) => { dire(texte); dessinerCoffre(cible); };
  const raté = (e) => dire(e.message, "erreur");
  document.getElementById("k-changer").onclick = async () => {
    const nouveau = document.getElementById("k-nouveau").value;
    const confirmation = document.getElementById("k-confirmation").value;
    const p = actuelSaisi();
    if (!p) return;
    // UNE COMPAGNIE DE L'ADMINISTRATEUR : le moteur remplace l'actuel dans toutes ses compagnies d'un coup (il prépare
    // tout, puis remplace ; si une seule refuse, aucune ne change).
    try {
      const r = await api("mot-de-passe-changer", { nouveau, confirmation, ...p });
      ok(r.phrase);
    } catch (e) { raté(e); }
  };
  const detacher = document.getElementById("k-detacher");
  if (detacher) detacher.onclick = async () => {
    try {
      const r = await api("administrateur-detacher", { administrateur: document.getElementById("k-adm-actuel").value,
        nouveau: document.getElementById("k-adm-nouveau").value, confirmation: document.getElementById("k-adm-confirmation").value });
      ok(r.phrase);
    } catch (e) { raté(e); }
  };
  const rattacher = document.getElementById("k-rattacher");
  if (rattacher) rattacher.onclick = async () => {
    const propre = document.getElementById("k-adm-propre");
    try {
      const r = await api("administrateur-rattacher", { administrateur: document.getElementById("k-adm-mdp").value,
        motdepasse: propre ? propre.value : "" });
      ok(r.phrase);
    } catch (e) { raté(e); }
  };
  const retirer = document.getElementById("k-retirer");
  if (retirer) retirer.onclick = async () => {
    if (!confirm(l.retirer)) return;
    const p = actuelSaisi();
    if (!p) return;
    try {
      const r = await api("mot-de-passe-retirer", p);
      ok(r.phrase);
    } catch (e) { raté(e); }
  };
  for (const carte of cible.querySelectorAll("[data-service]")) {
    const service = carte.dataset.service, verdict = carte.querySelector(".k-verdict");
    carte.querySelector(".k-poser").onclick = async () => {
      const nom = carte.querySelector(".k-nom"), prenom = carte.querySelector(".k-prenom");
      const compte = carte.querySelector(".k-compte");
      const p = await preuve("Poser");
      if (!p) return;
      // Le moteur dit ce qui change, pose, essaie une fois la connexion et dit ce qu'il en est (`phrase`, `couleur`).
      try {
        const etat = await api("coffre-poser", { service, valeur: carte.querySelector(".k-valeur").value,
          compte: compte ? compte.value : null, ...(nom ? { nom: nom.value, prenom: prenom.value } : {}), ...p });
        await dessinerCoffre(cible);
        dire(etat.phrase, etat.couleur);
      } catch (e) { raté(e); }
    };
    const verifier = carte.querySelector(".k-verifier");
    if (verifier) verifier.onclick = async (ev) => {
      ev.target.disabled = true; verdict.textContent = "Connexion…";
      try {
        const v = await api("coffre-verifier", { service });
        verdict.innerHTML = `${pastilleDeLEtat(v.pastille)} ${v.ok ? echapper(v.message)
          : `${echapper(v.message)}${v.conseil ? "<br>" + echapper(v.conseil) : ""}`}`;
      } catch (e) { verdict.textContent = e.message; }
      ev.target.disabled = false;
    };
    const oublierService = carte.querySelector(".k-oublier");
    if (oublierService) oublierService.onclick = async () => {
      if (!confirm((c.services.find((s) => s.service === service) || {}).oublier)) return;
      const p = await preuve("Oublier");
      if (!p) return;
      try {
        const r = await api("coffre-oublier", { service, ...p });
        ok(r.phrase);
      } catch (e) { raté(e); }
    };
  }
  dessinerCompteGoogle(cible, preuve, ok, raté, k);
}

/* Le compte Google, dans la même grille. L'état fait un vrai
 * rafraîchissement du jeton côté serveur (google-etat), donc il arrive
 * après le reste de la page — la carte se remplit quand il est là, et dit
 * « sans réseau » plutôt que de casser la page.
 *
 * « Lier le Drive » : l'appel reste suspendu le temps du consentement dans
 * le navigateur du système (jusqu'à cinq minutes, google.DELAI_AUTORISATION).
 * La carte le dit, et le bouton reste grisé jusqu'au retour. */
async function dessinerCompteGoogle(cible, preuve, ok, raté, k) {
  const carte = cible.querySelector("#k-google");
  if (!carte) return;
  const etat = carte.querySelector(".k-google-etat");
  const verdict = carte.querySelector(".k-verdict");
  const lier = carte.querySelector("#k-google-lier");
  const oublier = carte.querySelector("#k-google-oublier");
  let g = null;
  try { g = await api("google-etat"); }
  catch (e) { etat.innerHTML = `<span class="discret">${echapper(e.message)}</span>`; }
  if (!carte.isConnected) return;       // la page a été redessinée entre-temps
  if (g) {
    etat.innerHTML = `${pastilleDeLEtat(g.pastille)}${g.compte_dit ? ` <span class="discret">${echapper(g.compte_dit)}</span>` : ""}`;
    lier.textContent = g.bouton;
  }
  lier.disabled = false;
  oublier.disabled = !(g && g.compte);
  oublier.hidden = oublier.disabled;

  lier.onclick = async () => {
    const p = await preuve("Lier le Drive");
    if (!p) return;
    lier.disabled = oublier.disabled = true;
    verdict.textContent = k.google_ouvert;
    try {
      const r = await api("google-autoriser", p);
      ok(r.phrase);
    } catch (e) { verdict.textContent = ""; lier.disabled = false; oublier.disabled = !(g && g.compte); raté(e); }
  };
  oublier.onclick = async () => {
    if (!confirm(k.google_oublier)) return;
    const p = await preuve("Oublier le compte Google");
    if (!p) return;
    try {
      const r = await api("google-oublier", p);
      ok(r.phrase);
    } catch (e) { raté(e); }
  };
}
