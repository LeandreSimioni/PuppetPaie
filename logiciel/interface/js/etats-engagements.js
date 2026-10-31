/* L'écran de l'engagement : la carte en saisie, la ligne validée et ses trois carrés, le non actif.

   L'état est celui des engagements enregistrés (`etat-par-engagement`) : un engagement qu'on vient d'ajouter ou de
   changer n'en a pas encore, et sa carte n'en montre pas. Les carrés, leurs phrases et leurs gestes viennent du
   moteur ; un clic ouvre l'envoi que le moteur nomme, rien ne part d'un clic sans sa boîte. */
"use strict";

let ETATS_ENGAGEMENTS = null;      // les lignes d'`etat-par-engagement`, par identité d'engagement ; null = pas lu

/** La ligne d'`etat-par-engagement` d'une carte : celle de son identité (`data-engagement`), que le moteur rend avec
 *  chaque ligne. Une carte que le moteur n'a pas encore écrite n'a pas d'identité, donc pas d'état. */
function etatDeLaCarte(div) {
  if (!ETATS_ENGAGEMENTS || !div.dataset.engagement) return undefined;
  return ETATS_ENGAGEMENTS.get(div.dataset.engagement);
}

/* LES TROIS CARRÉS DE LA LIGNE VALIDÉE : DPAE, Contrat, Documents. Le moteur
   rend chacun (`etat-par-engagement`, `carres`) : son nom, sa couleur, ce qui
   reste en un mot, sa phrase et le geste qu'ouvre un clic. La page les
   dessine ; un carré qui a un geste est un bouton, sauf sur la carte d'un
   engagement validé, qui s'ouvre toute entière sur son détail. */

/** Le bandeau d'une ligne : un carré par acte. */
function bandeauEtats(ligne, gestes = true) {
  return `<div class="etats-actes">` + (ligne.carres || []).map((c, i) => {
    const clic = gestes && !!c.action;
    const classe = c.coupee ? "en-attente" : (c.couleur || "gris");
    return `<span class="etat-acte ${classe}${clic ? " cliquable" : ""}"
        ${clic ? `data-carre="${i}" tabindex="0" role="button"` : ""}
        title="${echapper(c.phrase + (clic ? " — " + c.action.libelle : ""))}">
      <i class="carre"></i>${echapper(c.nom)}${c.reste ? ` <span class="reste">${echapper(c.reste)}</span>` : ""}</span>`;
  }).join("") + `</div>`;
}

/** Le clic sur un carré : le geste que le moteur lui donne. Le contrat ou l'avenant parti
 *  et pas revenu se note signé d'un clic, et le message permet de revenir. */
function brancherRecu(div) {
  div.querySelectorAll(".etat-acte.cliquable").forEach((s) => {
    const agir = (ev) => {
      ev.preventDefault();
      ev.stopPropagation();
      const ligne = (s.closest(".engagement") || div)._ligne;
      const c = ligne && (ligne.carres || [])[+s.dataset.carre];
      if (!c || !c.action) return;
      const mois = document.getElementById("mois").value;
      if (c.action.geste === "contrat_recu") return noterSigne(mois, ligne, c.action.recu, c.action.avenant);
      ouvrirGeste(c.action, mois);
    };
    s.onclick = agir;
    s.onkeydown = (ev) => { if (ev.key === "Enter" || ev.key === " ") agir(ev); };
  });
}

/** « Reçu signé » (`contrat-recu`) du contrat, ou de l'avenant que le geste
 *  nomme (`avenant`), ou son annulation. Le message est la phrase du moteur ;
 *  une note posée porte « Annuler », qui rappelle la charge que le moteur
 *  rend pour la défaire (`annuler`). */
async function noterSigne(mois, ligne, recu, avenant) {
  await noterRecu(mois, { mois, salarie: ligne.salarie, debut: ligne.debut,
                          numero: ligne.numero || "", avenant: avenant ?? null, recu });
}

async function noterRecu(mois, charge) {
  let r;
  try {
    r = await api("contrat-recu", charge);
  } catch (e) { dire(e.message, "erreur"); return; }
  await dessinerEtatsDesEngagements(mois);
  await rafraichirActions();
  dire(r.phrase);
  if (!charge.recu || !r.annuler) return;
  const el = document.getElementById("message");
  const annuler = document.createElement("button");
  annuler.className = "lien message-annuler";
  annuler.textContent = "Annuler";
  annuler.onclick = () => { el.classList.add("cachee"); noterRecu(mois, r.annuler.charge); };
  el.append(" — ", annuler);
}

/** Les pièces parties d'un engagement pour un acte (`dpae`, `contrat-signature` : la clé de son carré). */
async function boitePieces(ligne, acte) {
  const mois = document.getElementById("mois").value;
  let r, lu;
  try {
    r = await api("pieces-parties", { salarie: ligne.salarie, debut: ligne.debut, numero: ligne.numero });
    lu = await api("etat-par-engagement", { mois });
  } catch (e) { dire(e.message, "erreur"); return; }
  // Le moteur rend, avec le carré (`carres[].pieces`), ce que le détail montre : la phrase sans pièce, l'encart d'un
  // contrat qui n'est plus le bon, et les gestes.
  const sienne = (lu.engagements || []).find((l) => l.salarie === ligne.salarie && l.debut === ligne.debut) || ligne;
  const carre = (sienne.carres || []).find((c) => c.cle === acte) || {};
  const montre = carre.pieces || { gestes: [] };
  const titre = acte === "dpae" ? "DPAE" : "Contrat";
  const pieces = (acte === "dpae" ? r.dpae : r.contrat) || [];
  const liste = pieces.length
    ? `<ul class="liste-pieces">${pieces.map((p, i) => `<li>
        <span><b>${echapper(p.quoi)}</b> — ${echapper(quand(p.quand))}
          <br><span class="discret">${echapper(p.fichier)}${p.existe ? ""
            : " — n'est plus dans le dossier du mois"}</span></span>
        <button class="secondaire p-ouvrir" data-i="${i}" ${p.existe ? "" : "disabled"}>Ouvrir</button>
      </li>`).join("")}</ul>`
    : `<p class="discret">${echapper(montre.vide || "")}</p>`;
  ouvrirBoite(`
    <h3>${titre} — ${echapper(ligne.nom)}</h3>
    <p class="discret">${echapper(pourQuoi(ligne))},
      ${echapper(periodeEnClair(ligne.debut, ligne.fin))}</p>
    ${carre.phrase ? `<p>${echapper(carre.phrase)}</p>` : ""}
    ${montre.encart ? `<div class="encart attention"><b>${echapper(montre.encart.titre)}</b>
      ${echapper(montre.encart.texte)}</div>` : ""}
    ${liste}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Fermer</button>
      ${montre.gestes.map((g, i) => `<button class="${g.principal ? "primaire" : "secondaire"} p-geste" data-i="${i}"
        ${g.titre ? `title="${echapper(g.titre)}"` : ""}>${echapper(g.libelle)}</button>`).join("")}
    </div>`);
  document.querySelectorAll(".p-geste").forEach((b) => {
    const a = montre.gestes[+b.dataset.i].action;
    b.onclick = () => {
      fermerBoite();
      if (a.geste === "contrat_recu") noterSigne(mois, ligne, a.recu);
      else if (a.geste === "avenant") boiteAvenant(mois, ligne.salarie, ligne.debut, ligne.numero);
      else if (a.geste === "faire_signer") boiteSignature(mois, [ligne.salarie]);
    };
  });
  document.querySelectorAll(".p-ouvrir").forEach((b) => {
    b.onclick = async () => {
      try {
        const o = await api("ouvrir-piece", { salarie: ligne.salarie, debut: ligne.debut,
                                              fichier: pieces[+b.dataset.i].fichier });
        if (o && o.phrase) dire(o.phrase);
      } catch (err) { dire(err.message, "erreur"); }
    };
  });
}

/** Pose les états d'une carte, seulement s'ils ont changé. Le champ qu'on quitte en cliquant « Valider » se
 *  relit (`change`) et repeint la carte entre l'appui et le relâché du bouton : refait, le bouton n'aurait pas
 *  reçu le clic. */
function poserEtats(cible, html) {
  if (cible._etatsPeints === html) return;
  cible.innerHTML = html;
  cible._etatsPeints = html;
}

/** Peint les états sur les blocs. Rappelée après chaque frappe : changer le
 *  salarié ou les jours change l'engagement que le bloc désigne. */
function peindreEtats() {
  document.querySelectorAll(".engagement").forEach((div) => {
    const cible = div.querySelector(".etats");
    if (!cible) return;
    // UNE CARTE OUVERTE POUR ÊTRE MODIFIÉE désigne l'engagement validé qu'elle a ouvert, quoi qu'on y tape (son
    // salarié, ses dates) : son bandeau reste celui de la modification jusqu'à « Modifier l'engagement » ou « Annuler ».
    if (div.classList.contains("en-modification") && div._ligne) {
      const ligne = etatDeLaCarte(div) || div._ligne;
      div._ligne = ligne;
      poserEtats(cible, `<span class="etats-qui">${echapper(ligne.nom)}
          <span class="discret">— ${echapper(pourQuoi(ligne))}</span></span>`
        + bandeauEtats(ligne) + bandeauValidation(ligne));
      brancherValidation(div, ligne);
      brancherRecu(div);
      return;
    }
    const ligne = etatDeLaCarte(div);
    const connue = !!ligne;
    div.classList.toggle("enregistree", connue);
    if (!ETATS_ENGAGEMENTS) { poserEtats(cible, ""); return; }
    if (!connue) {
      fermerEdition(div, false);
      if (!salarieDuBloc(div)) {
        // Sans salarié, ce bloc n'est pas un engagement : il n'est même pas
        // enregistré. Le dire, plutôt que de laisser un bloc muet qu'on
        // ne peut ni valider ni faire partir.
        poserEtats(cible, `<p class="etats-absent">Choisir qui est engagé.</p>
           <div class="validation"><span class="boutons">${BOUTON_RETIRER}</span></div>`);
        brancherRetirer(div);
        return;
      }
      // Un engagement qu'on vient d'ajouter ou de changer : pas de pastille (une pastille, même grise, se lit comme un
      // état connu), mais le geste « Valider l'engagement » tout de suite : `valider-engagement` écrit la carte et la
      // valide, ou dit le fait qui lui manque sans rien écrire.
      poserEtats(cible, bandeauValidation({}));
      brancherValidation(div, {});
      return;
    }
    // Un contrat sur plusieurs mois : son chevron est au bord de la case.
    div._ligne = ligne;
    poserEtats(cible, `<span class="etats-qui">${echapper(ligne.nom)}
        <span class="discret">— ${echapper(pourQuoi(ligne))}</span></span>`
      + bandeauEtats(ligne) + bandeauValidation(ligne));
    brancherValidation(div, ligne);
    brancherRecu(div);
    fermerEdition(div, !!ligne.valide);
  });
}

/** Lit `etat-par-engagement` et repeint. Pendant la lecture, les cartes gardent les états qu'elles montrent : une
 *  frappe qui repeint entre-temps ne leur retire pas leurs boutons. */
async function dessinerEtatsDesEngagements(mois) {
  if (!mois) { ETATS_ENGAGEMENTS = null; peindreEtats(); return; }
  try {
    const r = await api("etat-par-engagement", { mois });
    ETATS_ENGAGEMENTS = new Map();
    r.engagements.forEach((l) => { ETATS_ENGAGEMENTS.set(JSON.stringify(l.engagement), l); });
    peindreNonActifs(r.non_actifs);
  } catch (e) {
    // Mois jamais enregistré, spectacle manquant, saisie en cours : il n'y a
    // rien à peindre, et rien d'alarmant à dire. La saisie, elle, marche.
    // On ne sait rien des engagements : les cartes restent comme elles sont.
    // Un état vide les ouvrait TOUTES en formulaire, validées comprises, et le
    // refus du moteur (dit dans « Les étapes ») se lisait « tout s'est ouvert ».
    ETATS_ENGAGEMENTS = null;
  }
  peindreEtats();
}

/* ------------------------------------------------ valider un engagement
   Validé, l'engagement est vert, sur le bloc entier, et ses champs sont en lecture seule pour de vrai (`disabled`).
   Un clic sur sa carte l'ouvre, toutes ses cases modifiables ; ce qui est parti et ce que le changement entraîne, le
   moteur le dit à « Modifier l'engagement » (`modifierLaCarte`). */

/** Ferme ou ouvre les champs d'un bloc. Le bouton de validation, lui, reste
 *  actif — sinon on ne pourrait plus rouvrir. */
function fermerEdition(div, ferme) {
  // Une carte fermée qui doit s'ouvrir se construit ici, une fois (ouvrirCarte).
  if (!ferme) ouvrirCarte(div);
  div.classList.toggle("validee", ferme);
  peindreCarte(div);
  div.querySelectorAll("input, select, button").forEach((c) => {
    if (c.classList.contains("v-basculer") || c.classList.contains("retirer-engagement")) return;
    if (c.classList.contains("verrou")) return;    // un contrat en cours
    if (c.closest(".e-resume")) return;            // la ligne de l'engagement validé
    if (c.closest(".e-imprevus")) return;          // ses imprévus : la reprise, le maintien se disent après la paie
    if (c.classList.contains("chevron-pluri")) return;  // le chevron du contrat sur plusieurs mois
    c.disabled = ferme;
  });
  // Ses imprévus se redisent : sur un engagement validé, un jour payé pendant
  // l'arrêt ne se retire plus d'un clic (js/imprevus.js).
  peindreImprevusDesCartes();
}

/* RETIRER UN ENGAGEMENT : un vrai bouton, en bas à droite, à côté de « Valider ».
   L'avertissement n'est là que quand il y a quelque chose à perdre : un
   engagement validé, un contrat envoyé à la signature ou revenu signé, une DPAE
   partie. Sinon le bloc s'en va, et l'enregistrement suit. */
const BOUTON_RETIRER = `<button class="lien retirer-engagement" title="retirer cet engagement de la saisie">Retirer cet engagement</button>`;

/** Le bandeau de validation d'un bloc, sous les quatre pastilles. Un engagement validé ne le montre qu'ouvert pour être
 *  modifié : ses pièces, ses versions, et « Annuler » à côté de « Modifier l'engagement ». */
function bandeauValidation(ligne) {
  if (ligne.valide) {
    return `${recalculDe(ligne)}<div class="validation faite">
      <span><b>Engagement validé</b> le ${echapper(quand(ligne.valide))}
        ${(ligne.carres || []).filter((c) => c.lien).map((c) =>
          ` · <button class="lien v-pieces" data-acte="${echapper(c.cle)}">${echapper(c.lien)}</button>`).join("")}
        ${ligne.versions ? ` · ${versionsDe(ligne)}` : ""}</span>
      <span class="boutons">
        ${BOUTON_RETIRER}
        <button class="secondaire v-annuler">Annuler</button>
        <button class="primaire v-modifier">Modifier l'engagement</button>
      </span>
    </div>`;
  }
  return `<div class="validation">
    <span class="boutons">
      ${BOUTON_RETIRER}
      <button class="primaire v-basculer" data-valide="1">Valider l'engagement</button>
    </span>
  </div>`;
}

/** Branche le bouton d'un bloc, une fois le bandeau posé. */
function brancherValidation(div, ligne) {
  const b = div.querySelector(".v-basculer");
  if (b) b.onclick = (ev) => {
    ev.preventDefault();
    validerLaCarte(div);
  };
  const m = div.querySelector(".v-modifier");
  if (m) m.onclick = (ev) => { ev.preventDefault(); modifierLaCarte(div); };
  const a = div.querySelector(".v-annuler");
  if (a) a.onclick = (ev) => { ev.preventDefault(); annulerLaModification(div); };
  div.querySelectorAll(".v-pieces").forEach((p) => {
    p.onclick = (ev) => { ev.preventDefault(); boitePieces(ligne, p.dataset.acte); };
  });
  const frise = div.querySelector(".etats .carte-versions");
  if (frise) frise.onclick = (ev) => { ev.preventDefault(); boiteFrise(ligne); };
  brancherRetirer(div);
}

/** « Valider l'engagement », UN GESTE : la carte part au moteur telle qu'elle est (`valider-engagement` l'écrit, puis la
 *  valide), sans attendre l'enregistrement automatique. Il lui manque un fait : le moteur dit lequel, et n'écrit rien
 *  (l'enregistrement automatique garde la carte en saisie). */
async function validerLaCarte(div) {
  clearTimeout(MINUTEUR_SAISIE);
  const charge = chargeDeLaCarte(div, document.getElementById("mois").value);
  if (!charge) { dire("Choisir qui est engagé.", "erreur"); return; }
  // Une écriture automatique de cette carte déjà en route se finit d'abord : son identité en vient.
  await ENREGISTREMENT;
  if (div.dataset.engagement) charge.engagement = JSON.parse(div.dataset.engagement);
  try {
    const r = await api("valider-engagement", charge);
    div.dataset.engagement = JSON.stringify(r.engagement);
    div._ecrite = JSON.stringify({ ...charge, engagement: r.engagement });
    dire(r.phrase, r.couleur || undefined);
  } catch (e) { dire(e.message, "erreur"); }
  await rafraichirActions();
}

/** Le bouton « Retirer cet engagement » d'une carte, quel que soit son état. Le moteur décide (`retirer-engagement`,
 *  la carte désignée par son identité) : rien de parti, l'engagement s'efface ; un envoi parti, il avertit sans rien
 *  écrire (`avertissement`), et la page propose de le rendre non actif (`confirme`). Une carte jamais écrite
 *  n'a rien dans le moteur : elle s'efface de l'écran. Ce qui attend s'écrit
 *  d'abord, puis le mois se relit. */
function brancherRetirer(div) {
  const b = div.querySelector(".retirer-engagement");
  if (!b) return;
  b.onclick = async (ev) => {
    ev.preventDefault();
    const mois = document.getElementById("mois").value;
    clearTimeout(MINUTEUR_SAISIE);
    await enregistrerSaisie(true);
    if (!div.dataset.engagement) { div.remove(); rafraichirApercu(); dire("Engagement retiré."); return; }
    const charge = { mois, engagement: JSON.parse(div.dataset.engagement) };
    try {
      let r = await api("retirer-engagement", charge);
      if (r.avertissement) {
        if (!await boiteNonActif(r.avertissement)) return;
        r = await api("retirer-engagement", { ...charge, confirme: true });
      }
      dire(r.phrase);
      div.remove();
      await chargerMois(mois, true);
    } catch (e) { dire(e.message, "erreur"); }
  };
}

/** La phrase du moteur, et le choix : rendre l'engagement non actif, ou le garder. */
function boiteNonActif(phrase) {
  return new Promise((resoudre) => {
    ouvrirBoite(`
      <h3>Retirer cet engagement ?</h3>
      <div class="encart attention">${echapper(phrase)}</div>
      <div class="actions">
        <button class="secondaire" id="r-garder">Garder l'engagement</button>
        <button class="primaire" id="r-non-actif">Le rendre non actif</button>
      </div>`, () => resoudre(false));
    document.getElementById("r-garder").onclick = () => { fermerBoite(); resoudre(false); };
    document.getElementById("r-non-actif").onclick = () => { fermerBoite(); resoudre(true); };
  });
}

/** Les engagements non actifs du mois (`etat-par-engagement`, `non_actifs`) : une
 *  carte qui se lit, qui ne s'ouvre pas, et ce que leur désactivation rend à faire. */
function peindreNonActifs(liste) {
  const cible = document.getElementById("engagements");
  if (!cible) return;
  cible.querySelectorAll(".engagement-non-actif").forEach((d) => d.remove());
  (liste || []).forEach((n) => {
    const d = document.createElement("div");
    d.className = "engagement-non-actif";
    d.innerHTML = `<div class="na-ligne">
        <span><b>${echapper(n.nom)}</b>
          <span class="discret">· contrat ${echapper(n.numero || "sans numéro")},
            ${echapper(periodeEnClair(n.debut, n.fin))}</span></span>
        <span class="mot-etat" title="non actif depuis le ${echapper(quand(n.non_actif))}">non actif</span>
      </div>
      ${n.a_faire?.length ? `<div class="etats-actes">${n.a_faire.map((x) =>
        `<span class="etat-acte orange"><i class="carre"></i>${echapper(x)}</span>`).join("")}</div>` : ""}`;
    cible.appendChild(d);
  });
}


/* UN CLIC SUR LA CARTE D'UN ENGAGEMENT VALIDÉ L'OUVRE, toutes ses cases modifiables. Rien ne s'écrit tant qu'on n'a
 * pas cliqué « Modifier l'engagement » : la carte reste ouverte avec ce qu'on y a tapé, « Annuler » la referme telle
 * qu'elle est enregistrée. */
function ouvrirEnModification(div) {
  if (!div._ligne || !div._ligne.valide) return;
  div.classList.add("en-modification");
  fermerEdition(div, false);
  peindreEtats();
}

/** Referme la carte telle qu'elle est enregistrée : le mois se relit, ce qu'on y a tapé s'efface. */
async function refermerLaCarte(div) {
  const mois = document.getElementById("mois").value;
  div.classList.remove("en-modification");
  clearTimeout(MINUTEUR_SAISIE);
  await enregistrerSaisie(true);
  await chargerMois(mois, true);
}

async function annulerLaModification(div) {
  await refermerLaCarte(div);
}

/* « MODIFIER L'ENGAGEMENT » : la page envoie ce que la carte ouverte montre ; le moteur dit si rien n'a changé
 * (la carte se referme), écrit ce qui a changé, ou rend son avertissement quand ce qui est parti le demande. La page
 * le montre tel quel ; « Annuler » revient à la carte avec ce qu'on y a tapé, la confirmation renvoie la demande. */
async function modifierLaCarte(div) {
  const mois = document.getElementById("mois").value;
  let saisie;
  LIRE_LES_MODIFICATIONS = true;
  try { saisie = engagementDuBloc(div, mois) || {}; } finally { LIRE_LES_MODIFICATIONS = false; }
  const charge = { mois, engagement: JSON.parse(div.dataset.engagement), saisie };
  try {
    let r = await api("modifier-engagement", charge);
    while (r && r.avertissement) {
      const reponse = await boiteAvertissement(r.avertissement);
      if (!reponse) return;
      r = await api("modifier-engagement", { ...charge, confirme: true, a_partir_du: reponse.a_partir_du,
                                             a_partir_du_jour: reponse.a_partir_du_jour });
    }
    if (r && r.change) dire(r.phrase, r.couleur || undefined);
    await refermerLaCarte(div);
    await rafraichirActions();
  } catch (e) { dire(e.message, "erreur"); }
}

/** L'avertissement du moteur : chaque engagement nommé une fois, une ligne par ce que le changement entraîne, et la
 *  question « à partir de quel mois ? » quand elle se pose, avec le jour, le 1er tant qu'on n'en dit pas d'autre.
 *  Rend la réponse (`{a_partir_du, a_partir_du_jour}`) à la confirmation, `null` à « Annuler ». */
function boiteAvertissement(a) {
  const q = a.a_partir_du;
  return new Promise((repondre) => {
    ouvrirBoite(`<h3>Modifier l'engagement</h3>
      ${(a.engagements || []).map((e) => `<div class="encart attention"><b>${echapper(e.qui)}</b>
        ${e.consequences.length ? `<ul>${e.consequences.map((c) => `<li>${echapper(c)}</li>`).join("")}</ul>` : ""}</div>`).join("")}
      ${q ? `<p>${echapper(q.phrase)}</p>
        ${(q.si || []).length ? `<ul class="discret">${q.si.map((s) => `<li>${echapper(s)}</li>`).join("")}</ul>` : ""}
        <label>À partir de <select id="av-mois">${q.choix.map((c) =>
          `<option value="${echapper(c.mois)}">${echapper(c.libelle)}${c.partis ? " (déjà parti)" : ""}</option>`).join("")}</select></label>
        <label>le <input id="av-jour" class="jour-du-mois" type="number" min="1" max="31" step="1" value="1"></label>` : ""}
      <div class="actions">
        <button class="secondaire" id="av-non">Annuler</button>
        <button class="primaire" id="av-oui">Modifier l'engagement</button>
      </div>`, () => repondre(null));
    document.getElementById("av-non").onclick = () => fermerBoite();
    document.getElementById("av-oui").onclick = () => {
      const choix = document.getElementById("av-mois");
      const jour = document.getElementById("av-jour");
      repondre({ a_partir_du: choix ? choix.value : null, a_partir_du_jour: jour ? jour.value : null });
      fermerBoite();
    };
  });
}

/* PLUS DE CASE DE SÉLECTION NI DE BARRE D'ACTIONS GROUPÉES :
 * « on pourrait enlever la petite coche de sélection, tout se fait
 * directement quand on clique sur le bouton déposer DPAE ou demander les
 * signatures ». Les deux boîtes de la colonne de droite listent chaque
 * contrat du mois avec sa case, cochée d'avance sur ce qui reste à faire :
 * c'est là que l'on choisit, et rien ne part sans y être passé. */

/* LA BOÎTE DPAE, comme celle des signatures : une liste déroulante.
 * Chaque contrat du mois enregistré, une case ; cochée
 * d'avance si aucune DPAE n'est partie, ou si elle a été refusée. Un contrat
 * en cours depuis un mois précédent n'a pas de DPAE ce mois-ci : il se voit,
 * grisé, avec le mois où elle a été faite.
 *
 * LES EMBAUCHES COCHÉES SE DÉSIGNENT : le récapitulatif de dépôt habituel
 * (`boiteDepot`) écrit leur DPAE, ou reprend celle qui dit la même chose,
 * à côté du fichier complet du mois, jamais par-dessus. Écrire n'envoie rien ;
 * le dépôt reste le même geste, avec la même relecture et le même mot de passe. */
async function boiteDpae(mois) {
  if (!mois) { dire("Choisir un mois."); return; }
  let r;
  try {
    r = await api("etat-par-engagement", { mois });
  } catch (e) {
    // Un mois sans engagement rend une liste vide (plus bas) ; un refus est un refus, et il se dit tel quel.
    dire(e.message, "erreur");
    return;
  }
  const lignes = r.engagements || [];
  if (!lignes.length) { dire("Aucun engagement enregistré en " + moisEnClair(mois) + "."); return; }
  // Le moteur dit, par engagement (`dpae_choix`), s'il se coche, s'il est coché
  // d'avance, son état en mots et sa couleur.

  ouvrirBoite(`
    <h3>Envoyer les DPAE</h3>
    ${moisEnGros(mois)}
    <p class="note">Cochées d'avance : les embauches sans DPAE, ou dont la
      DPAE a été refusée. Une DPAE déjà passée ne se redéclare pas : l'Urssaf
      recevrait deux déclarations pour un seul contrat.</p>
    <div class="liste-choix">
      ${lignes.map((l, i) => {
        const e = l.etats.dpae;
        const c = l.dpae_choix || {};
        const etat = `<br>${pastilleDeLEtat({ couleur: c.couleur, mots: c.phrase })}`;
        return `<label class="choix ${c.cochable ? "" : "grise"}">
          <input type="checkbox" class="d-qui" data-i="${i}" ${c.cochee ? "checked" : ""}
                 ${c.cochable ? "" : "disabled"}>
          <span><b>${echapper(l.nom)}</b> — ${l.spectacle ? `${echapper(l.spectacle)}, ` : ""}embauche le
            ${echapper(jjmmaaaa(l.debut))}${l.numero
              ? ` <span class="discret">(contrat n° ${echapper(l.numero)})</span>` : ""}
            ${etat}
            ${e.date_changee ? `<br><span class="note">${echapper(e.date_changee.phrase)}</span>` : ""}</span></label>`;
      }).join("")}
    </div>
    <p class="note">Heure d'embauche déclarée : 08h00. Toutes les embauches
      cochées partent ensemble, dans un seul dépôt. D'abord le test, qui ne
      déclare rien ; une fois le test revenu conforme, ${enFictive() ? "l'envoi, simulé" : "l'envoi réel"}.</p>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Fermer</button>
      <button class="primaire" id="d-ecrire">Envoyer le test…</button>
    </div>`);

  const bouton = document.getElementById("d-ecrire");
  const majBouton = () => {
    const n = document.querySelectorAll(".d-qui:checked").length;
    bouton.disabled = !n;
    bouton.textContent = n ? `Envoyer le test (${n} embauche${n > 1 ? "s" : ""})…` : "Rien de coché";
  };
  document.querySelectorAll(".d-qui").forEach((c) => { c.onchange = majBouton; });
  majBouton();
  bouton.onclick = async () => {
    const choisies = [...document.querySelectorAll(".d-qui:checked")]
      .map((c) => lignes[+c.dataset.i]);
    if (!choisies.length) return;
    const rendre = occupe(bouton, "Fabrication du fichier en cours…");
    await boiteDepot({ quoi: "dpae", mois,
                       engagements: choisies.map((l) => ({ salarie: l.salarie, numero: l.numero })) },
                     { testerDabord: true });
    rendre();
  };
}

