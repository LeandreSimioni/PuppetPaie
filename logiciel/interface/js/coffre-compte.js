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
 * (17/09/2026) s'y pose aussi, avec son adresse : c'est un mot de passe
 * d'application, la seule boîte qui envoie et qui lit. Et le COMPTE GOOGLE
 * (18/09/2026), qui ne porte que le Drive : « Lier le Drive » ouvre la
 * page de consentement dans le navigateur du système et attend qu'on y
 * accepte ; « Oublier » efface le jeton local.
 *
 * La règle : la page ne reçoit JAMAIS une valeur posée — seulement qu'elle
 * l'est, et depuis quand. Et si un mot de passe du logiciel est posé, il
 * est demandé pour écrire dans le coffre comme pour lier ou oublier le
 * compte Google, comme pour un dépôt : c'est `operations` qui l'exige, pas
 * ce fichier. */
/** Le compte qu'ouvre un mot de passe (§94) : sans lui, un mot de passe
 *  juste sur un identifiant faux échoue pareil, et rien ne le montrait.
 *  net-entreprises : le déclarant de la fiche Entreprise, qui se change
 *  là-bas ; l'Urssaf (DPAE) : un autre compte, qui se saisit sur la ligne. */
function compteDuService(s) {
  const id = s.identifiant || {};
  if (id.texte) return `<span class="compte">${echapper(id.texte)}</span>`;
  if (!id.manque || !id.manque.length) return "";
  const manque = id.manque.map((m) => ({ nom: "le nom", prenom: "le prénom", siret: "le SIRET" }[m] || m)).join(", ");
  return id.ici
    ? `<span class="compte a-completer">le compte Urssaf n'est pas dit (${echapper(manque)}) : la DPAE ne partira pas</span>`
    : `<span class="compte a-completer">identifiant incomplet (${echapper(manque)}) —
        <button class="lien" data-aller-fiche="entreprise">${echapper(id.ou || "fiche Entreprise")}</button></span>`;
}

/** CE QUI EST PARTI CHEZ L'ÉDITEUR (30/09/2026). Le journal des incidents,
 *  regroupé (`operations_filet.diagnostic_etat`) : ce qui est parti, ce qui
 *  attend, et pour chaque groupe le contenu EXACT qui quitte le poste. */
async function dessinerCoffre(cible) {
  // LA COMPAGNIE FICTIVE (27/09/2026, l'éditeur) : le même écran, tout grisé —
  // on voit ce qui se pose et où, sans rien pouvoir y écrire. Rien ne part,
  // il n'y a donc rien à poser ; la page ne branche aucun geste.
  const fictive = enFictive();
  const c = await api("coffre-etat");
  const exige = c.mot_de_passe_pose;
  // UNE LIGNE PAR SECRET (23/09/2026), comme les mois de l'Historique : ce
  // qu'il sert à faire, où il en est, et les gestes. Les cases de saisie ne
  // s'ouvrent que sur « Poser… » ou « Modifier… » -- cinq formulaires ouverts
  // d'office faisaient croire qu'il y avait cinq choses à remplir.
  const etatService = (s) => fictive ? `<span class="pastille">simulé</span>`
    : s.environnement
    ? `<span class="pastille alerte" title="Il passe devant le coffre.">fourni par l'environnement</span>`
    : s.pose ? `<span class="pastille bien" title="${echapper(s.pose_le)}">posé le ${echapper(jjmmaaaa(String(s.pose_le || "").slice(0, 10)))}</span>${
        s.compte ? ` <span class="discret">${echapper(s.compte)}</span>` : ""}`
    : `<span class="pastille alerte">absent</span>`;
  cible.innerHTML = `
    <p class="intro">Les mots de passe dont le logiciel a besoin pour déposer, relever et
      envoyer. La page ne voit jamais un mot de passe, seulement s'il est posé.</p>
    ${fictive ? `<div class="encart info"><b>Compagnie fictive — les champs sont grisés</b>
      Les dépôts de DPAE et de DSN, leurs retours, les courriels et les
      signatures sont simulés : aucun mot de passe n'est demandé, et rien ne
      quitte le poste. Dans une compagnie réelle, c'est ici que se posent les
      accès à net-entreprises, à l'Urssaf et à la boîte courriel — et que
      s'active le filet de sécurité DPAE, un courriel de rappel si une DPAE
      n'est pas partie la veille d'une embauche.</div>`
    : c.cle_poste ? "" : `<div class="encart"><b>Cet ordinateur n'a pas encore sa clé du coffre</b>
      Rien n'est perdu : elle se refait toute seule la prochaine fois que le mot de passe du
      logiciel est tapé (ou posé, ci-dessous). Elle vit à part des données, pour qu'une
      sauvegarde n'emporte jamais de quoi ouvrir le coffre.</div>`}
    <div class="rangs">
      <div class="rang">
        <div class="rang-infos"><b class="rang-nom">Le mot de passe du logiciel</b>
          <span class="discret">${exige || fictive
            ? "demandé à l'ouverture et à chaque dépôt réel, envoi ou signature"
            : "rien ne peut partir en réel tant qu'il n'est pas posé"}</span></div>
        <div class="rang-etat">${fictive ? `<span class="pastille">simulé</span>`
          : exige ? `<span class="pastille bien">posé</span>`
          : `<span class="pastille alerte">aucun</span>`}</div>
        <div class="rang-actions">
          ${exige ? `<button class="modifier k-ouvrir">${CRAYON}Modifier…</button>`
            : `<button class="secondaire k-ouvrir">Poser…</button>`}</div>
        <form class="rang-forme" id="k-logiciel" onsubmit="return false" autocomplete="off" hidden>
          ${exige ? `<label>Mot de passe actuel
            <input type="password" id="k-actuel" autocomplete="current-password"></label>` : ""}
          <label>Nouveau <span class="discret">— six caractères au moins</span>
            <input type="password" id="k-nouveau" autocomplete="new-password"></label>
          <label>Le retaper <input type="password" id="k-confirmation" autocomplete="new-password"></label>
          <div class="barre-boutons">
            <button class="primaire" id="k-changer">${exige ? "Enregistrer" : "Poser"}</button>
            ${exige ? `<button class="lien" id="k-retirer">retirer le mot de passe</button>` : ""}
          </div>
        </form>
      </div>
      ${c.services.map((s) => `
      <div class="rang" data-service="${echapper(s.service)}">
        <div class="rang-infos"><b class="rang-nom">${echapper(s.libelle)}</b>
          <span class="discret">${echapper(s.sert_a)}</span>${fictive ? "" : compteDuService(s)}</div>
        <div class="rang-etat">${etatService(s)}</div>
        <div class="rang-actions">
          ${!fictive && (s.pose || s.environnement) ? `<button class="lien k-verifier"
            title="Une tentative de connexion chez l'organisme, une seule">Vérifier</button>` : ""}
          ${s.pose ? `<button class="lien k-oublier">Oublier</button>` : ""}
          ${s.pose ? `<button class="modifier k-ouvrir">${CRAYON}Modifier…</button>`
            : `<button class="secondaire k-ouvrir">Poser…</button>`}
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
          <label>Mot de passe${s.identifiant?.ici && s.pose
              ? ` <span class="discret">— vide : on garde celui qui est posé</span>` : ""}
            <input type="password" class="k-valeur" autocomplete="off"></label>
          <div class="barre-boutons"><button class="primaire k-poser">${s.identifiant?.ici ? "Enregistrer" : "Poser"}</button></div>
          ${s.identifiant?.ici ? `<p class="note">Le compte ouvert sur urssaf.fr pour la DPAE (service
            « DPAE Service plus ») : ce n'est pas celui de net-entreprises. Les accents sont
            retirés, la DPAE les refuse.</p>` : ""}
          ${s.avec_compte ? `<p class="note">Pour Gmail : validation en deux étapes activée,
            puis myaccount.google.com › Sécurité › Mots de passe des applications. C'est ce
            mot de passe-là, pas celui du compte.</p>` : ""}
        </form>
      </div>`).join("")}
      <div class="rang" id="k-google">
        <div class="rang-infos"><b class="rang-nom">Compte Google</b>
          <span class="discret">la sauvegarde du dossier privé et l'archivage des mois,
            dans son Drive — rien d'autre</span>${VEILLE?.dernier?.sauvegarde_souci
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
      <div class="rang" id="k-filet">
        <div class="rang-infos"><b class="rang-nom">Filet de sécurité DPAE</b>
          <span class="discret">un courriel, et un seul, si une DPAE n'est pas partie la
            veille d'une embauche — jamais rien quand elle est partie</span>
          <span class="compte k-filet-compte"></span></div>
        <div class="rang-etat k-filet-etat"><span class="discret">lecture…</span></div>
        <div class="rang-actions">
          <button class="lien" id="k-filet-arreter" hidden>Désactiver</button>
          <button class="secondaire k-ouvrir" id="k-filet-ouvrir">Activer…</button>
        </div>
        <div class="k-verdict discret"></div>
        <form class="rang-forme" onsubmit="return false" autocomplete="off" hidden>
          <label>Adresse qui recevra le rappel
            <input type="email" id="k-filet-adresse" autocomplete="off"></label>
          <div class="barre-boutons"><button class="primaire" id="k-filet-activer">Activer</button></div>
          <p class="note">PuppetPaie tourne sur cet ordinateur : éteint, il ne peut prévenir
            personne. Le filet confie donc un réveil au serveur de PuppetPaie, pour la veille
            de chaque embauche dont le contrat est parti, et le désarme dès que la DPAE part.
            S'il n'a pas été désarmé, il sonne : un courriel à cette adresse. Le serveur ne
            reçoit ni nom, ni salarié, ni contrat — seulement cette adresse, un code tiré au
            hasard et le jour de l'embauche, effacés dès que le réveil sonne ou se désarme.
            Un lien de confirmation part d'abord à l'adresse : sans lui, rien ne sera envoyé.
            La DPAE reste obligatoire avant la première heure de travail ; le filet ne la
            fait pas, il rappelle qu'elle manque.</p>
        </form>
      </div>
      <div class="rang" id="k-diagnostic">
        <div class="rang-infos"><b class="rang-nom">Signalement des dysfonctionnements</b>
          <span class="discret">ce qui casse part chez l'éditeur, sans nom, sans NIR, sans
            montant — des codes et des messages qui nomment un champ</span>
          <span class="compte k-diag-compte"></span></div>
        <div class="rang-etat k-diag-etat"><span class="discret">lecture…</span></div>
        <div class="rang-actions">
          <button class="lien" id="k-diag-couper" hidden>Désactiver</button>
          <button class="secondaire" id="k-diag-voir">Voir ce qui est parti…</button>
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
    cible.querySelector(".k-filet-etat").innerHTML = `<span class="pastille">désactivé</span>`;
    cible.querySelector(".k-google-etat").innerHTML = `<span class="pastille">non lié</span>`;
    cible.querySelectorAll("[data-aller-fiche]").forEach((b) => {
      b.onclick = () => document.querySelector(
        `.sous-onglets button[data-fiche="${b.dataset.allerFiche}"]`).click();
    });
    return;
  }

  dessinerFilet(cible.querySelector("#k-filet"), exige);
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
  // « Poser… » / « Modifier… » ouvre la ligne, et met le curseur dans la
  // première case ; un second clic la referme.
  cible.querySelectorAll(".k-ouvrir").forEach((b) => {
    b.onclick = () => {
      const forme = b.closest(".rang").querySelector(".rang-forme");
      forme.hidden = !forme.hidden;
      if (!forme.hidden) forme.querySelector("input").focus();
    };
  });

  // La preuve, si elle est exigée : le popup. Null = annulé, on ne fait rien.
  const preuve = async (quoi) => {
    if (!exige) return { motdepasse: null };
    const m = await demanderMotDePasse(quoi);
    return m ? { motdepasse: m } : null;
  };
  // L'ACTUEL EST UN CHAMP, PAS UN POPUP, et seulement ici (21/09/2026,
  // l'editeur). La regle du 17/09 vaut pour un acte qui ENGAGE : deposer,
  // envoyer, signer, poser un secret d'organisme -- et elle ne bouge pas, le
  // popup reste pour tout cela. Changer le mot de passe du logiciel n'est pas
  // de cette famille : l'ancien n'autorise pas l'acte, il est ce que l'acte
  // remplace. Le taper au-dessus du nouveau, c'est la forme que tout le monde
  // connait, et elle se lit d'un coup d'oeil.
  const actuelSaisi = (quoi) => {
    if (!exige) return { motdepasse: null };
    const champ = document.getElementById("k-actuel");
    const v = champ ? champ.value : "";
    if (!v) {
      dire("Tapez le mot de passe actuel pour " + quoi + ".");
      if (champ) champ.focus();
      return null;
    }
    return { motdepasse: v };
  };
  const ok = (texte) => { dire(texte); dessinerCoffre(cible); };
  const raté = (e) => dire(e.message, "erreur");

  document.getElementById("k-changer").onclick = async () => {
    const nouveau = document.getElementById("k-nouveau").value;
    const confirmation = document.getElementById("k-confirmation").value;
    if (!nouveau) { dire("Tapez le nouveau mot de passe."); return; }
    const p = actuelSaisi("changer");
    if (!p) return;
    try {
      await api("mot-de-passe-changer", { nouveau, confirmation, ...p });
      ok(exige ? "Mot de passe du logiciel changé." : "Mot de passe du logiciel posé : il sera demandé à la prochaine ouverture.");
    } catch (e) { raté(e); }
  };
  const retirer = document.getElementById("k-retirer");
  if (retirer) retirer.onclick = async () => {
    if (!confirm("Retirer le mot de passe du logiciel ? La fenêtre s'ouvrira sans rien demander et le coffre ne sera plus protégé.")) return;
    const p = actuelSaisi("retirer");
    if (!p) return;
    try {
      await api("mot-de-passe-retirer", p);
      ok("Mot de passe du logiciel retiré.");
    } catch (e) { raté(e); }
  };
  for (const carte of cible.querySelectorAll("[data-service]")) {
    const service = carte.dataset.service, verdict = carte.querySelector(".k-verdict");
    carte.querySelector(".k-poser").onclick = async () => {
      const valeur = carte.querySelector(".k-valeur").value;
      const nom = carte.querySelector(".k-nom"), prenom = carte.querySelector(".k-prenom");
      const s = c.services.find((x) => x.service === service);
      const compteChange = nom && (nom.value !== s.identifiant.nom
        || prenom.value !== s.identifiant.prenom);
      if (!valeur && !compteChange) {
        dire(nom ? "Rien n'a changé : ni le compte, ni le mot de passe."
          : "Tapez le mot de passe " + service + " à poser.", "erreur");
        return;
      }
      if (!valeur && !s.pose && !s.environnement) {
        dire("Le compte est à enregistrer avec son mot de passe : tapez-le aussi.");
        return;
      }
      const compte = carte.querySelector(".k-compte");
      const p = await preuve(valeur ? "poser le mot de passe " + service
        : "changer le compte qui dépose les DPAE");
      if (!p) return;
      let etat;
      try {
        if (compteChange) {
          etat = await api("coffre-compte-dpae", { nom: nom.value, prenom: prenom.value, ...p });
        }
        if (valeur) {
          etat = await api("coffre-poser", { service, valeur, compte: compte ? compte.value : null, ...p });
        }
      } catch (e) { raté(e); return; }
      // PUIS UNE TENTATIVE, UNE SEULE (§94) : le message d'après disait « le
      // vérifier maintenant évite de le découvrir faux au dépôt ». Autant le
      // faire. Pas si l'identifiant est incomplet : l'échec serait certain,
      // et chaque échec rapproche du blocage du compte.
      const apres = (etat?.services || []).find((x) => x.service === service);
      const quoi = compteChange && !valeur ? "Compte " : "Mot de passe ";
      if (apres?.identifiant?.manque?.length) {
        ok(quoi + service + " enregistré. Identifiant incomplet : pas de vérification.");
        return;
      }
      dire(quoi + service + " enregistré. Vérification chez l'organisme…");
      try {
        const v = await api("coffre-verifier", { service });
        await dessinerCoffre(cible);
        dire(v.ok ? quoi + service + " enregistré, et vérifié : la connexion passe."
          : quoi + service + " enregistré, mais la connexion est REFUSÉE : " + v.message
            + (v.conseil ? " — " + v.conseil : ""), v.ok ? undefined : "erreur");
      } catch (e) {
        await dessinerCoffre(cible);
        dire(quoi + service + " enregistré ; la vérification n'a pas pu se faire : " + e.message, "erreur");
      }
    };
    const verifier = carte.querySelector(".k-verifier");
    if (verifier) verifier.onclick = async (ev) => {
      ev.target.disabled = true; verdict.textContent = "Connexion…";
      try {
        const v = await api("coffre-verifier", { service });
        verdict.innerHTML = v.ok ? `<span class="pastille bien">OK</span> ${echapper(v.message || "session ouverte chez l'organisme.")}`
          : `<span class="pastille erreur">refusé</span> ${echapper(v.message)}${v.conseil ? "<br>" + echapper(v.conseil) : ""}`;
      } catch (e) { verdict.textContent = e.message; }
      ev.target.disabled = false;
    };
    const oublierService = carte.querySelector(".k-oublier");
    if (oublierService) oublierService.onclick = async () => {
      if (!confirm("Oublier le mot de passe " + service + " ? Les retours et les dépôts ne seront plus possibles jusqu'à ce qu'il soit reposé.")) return;
      const p = await preuve("oublier le mot de passe " + service);
      if (!p) return;
      try {
        await api("coffre-oublier", { service, ...p });
        ok("Mot de passe " + service + " oublié.");
      } catch (e) { raté(e); }
    };
  }
  dessinerCompteGoogle(cible, preuve, ok, raté);
}

/* Le compte Google, dans la même grille. L'état fait un vrai
 * rafraîchissement du jeton côté serveur (google-etat), donc il arrive
 * après le reste de la page — la carte se remplit quand il est là, et dit
 * « sans réseau » plutôt que de casser la page.
 *
 * « Lier le Drive » : l'appel reste suspendu le temps du consentement dans
 * le navigateur du système (jusqu'à cinq minutes, google.DELAI_AUTORISATION).
 * La carte le dit, et le bouton reste grisé jusqu'au retour. */
async function dessinerCompteGoogle(cible, preuve, ok, raté) {
  const carte = cible.querySelector("#k-google");
  if (!carte) return;
  const etat = carte.querySelector(".k-google-etat");
  const verdict = carte.querySelector(".k-verdict");
  const lier = carte.querySelector("#k-google-lier");
  const oublier = carte.querySelector("#k-google-oublier");
  let g = null;
  try { g = await api("google-etat"); }
  catch (e) { etat.innerHTML = `<span class="pastille alerte">état inconnu</span> ${echapper(e.message)}`; }
  if (!carte.isConnected) return;       // la page a été redessinée entre-temps
  if (g) {
    etat.innerHTML = !g.compte
      ? `<span class="pastille alerte" title="La sauvegarde ne peut pas partir tant qu'un Drive n'est pas lié.">aucun Drive lié</span>`
      : g.verifie
        ? `<span class="pastille bien" title="L'autorisation tient${g.accordees?.length ? " (" + g.accordees.map(echapper).join(", ") + ")" : ""}.">lié</span> <span class="discret">${echapper(g.compte)}</span>`
        : `<span class="pastille alerte">lié, mais</span> <span class="discret">${echapper(g.compte)} : ${echapper(g.probleme || "l'autorisation ne répond pas")}</span>`;
    lier.textContent = g.compte ? "Relier le Drive" : "Lier le Drive";
  }
  lier.disabled = false;
  oublier.disabled = !(g && g.compte);
  oublier.hidden = oublier.disabled;

  lier.onclick = async () => {
    const p = await preuve("lier le compte Google");
    if (!p) return;
    lier.disabled = oublier.disabled = true;
    verdict.innerHTML = `<b>La fenêtre Google est ouverte dans votre navigateur.</b>
      Choisissez le compte, acceptez, puis revenez ici — cette ligne se met à jour
      toute seule. Si Google dit que l'application n'est pas validée : Paramètres
      avancés › Accéder à PuppetPaie.`;
    try {
      const r = await api("google-autoriser", p);
      ok("Drive lié : " + r.compte + ". La prochaine ouverture sauvegarde.");
    } catch (e) { verdict.textContent = ""; lier.disabled = false; oublier.disabled = !(g && g.compte); raté(e); }
  };
  oublier.onclick = async () => {
    if (!confirm("Oublier le compte Google ? Le jeton local est effacé : plus de sauvegarde ni d'archivage jusqu'à ce qu'un Drive soit relié. L'accès accordé côté Google, lui, se retire sur myaccount.google.com › Sécurité › Vos connexions à des applications tierces.")) return;
    const p = await preuve("oublier le compte Google");
    if (!p) return;
    try {
      await api("google-oublier", p);
      ok("Compte Google oublié. Pensez à retirer l'accès sur myaccount.google.com.");
    } catch (e) { raté(e); }
  };
}
