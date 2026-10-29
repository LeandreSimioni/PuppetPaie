/* L'écran de l'engagement : la carte en saisie, la ligne validée et ses trois carrés, le non actif.

   L'état est celui des engagements enregistrés (`etat-par-engagement`) : un engagement qu'on vient d'ajouter ou de
   changer n'en a pas encore, et sa carte n'en montre pas. Les carrés, leurs phrases et leurs gestes viennent du
   moteur ; un clic ouvre l'envoi que le moteur nomme, rien ne part d'un clic sans sa boîte. */
"use strict";

let ETATS_ENGAGEMENTS = null;      // "clé|début|rang" → les quatre actes ; null = pas lu

/* LES JUMEAUX : deux contrats du même salarié qui commencent le
   même jour ne se confondent pas. Le moteur les identifie par leur numéro
   quand il est posé, sinon par leur RANG parmi ceux du même salarié et du
   même jour, dans l'ordre du fichier (`jumeaux.rs`) — et le fichier suit
   l'ordre des blocs à l'écran (`saisieCourante`). Chaque ligne
   d'`etat-par-engagement` porte ce rang ; la page le rend à `valider-engagement`. */
const cleEngagement = (salarie, debut, rang) => salarie + "|" + debut + "|" + (rang || 0);

/** Les (salarié, premier jour) d'un bloc, sans leur rang.
 *  Le moteur trie les dates et prend la première (`Engagement.debut`) : on
 *  fait pareil, sans quoi un engagement saisi « 12 3 » ne se retrouverait pas. */
function identitesDuBloc(div) {
  // Les jours sont sur les lignes de paie : le premier
  // jour de l'engagement est le plus petit de TOUTES ses lignes -- sauf si le
  // contrat dit son début (un contrat en cours depuis octobre s'identifie
  // par le 12/10, pas par son premier cachet de novembre).
  const debut = debutDuBloc(div);
  if (!debut) return [];
  return salariesDuBloc(div).map((salarie) => ({ salarie, debut }));
}

/** Les engagements d'un bloc, tels qu'ils sont SAISIS à l'écran : (salarié,
 *  premier jour), le numéro du contrat s'il est figé, et le rang du bloc
 *  parmi ceux du même salarié et du même jour qui le précèdent. */
function engagementsSaisisDuBloc(div) {
  const blocs = [...document.querySelectorAll(".engagement")];
  const avant = blocs.slice(0, Math.max(0, blocs.indexOf(div))).flatMap(identitesDuBloc);
  const numero = div.dataset.numero || "";
  return identitesDuBloc(div).map((p) => ({ ...p, numero,
    rang: avant.filter((a) => a.salarie === p.salarie && a.debut === p.debut).length }));
}

/** La ligne d'`etat-par-engagement` d'un engagement de l'écran : par son numéro s'il est
 *  posé, sinon par son rang — jamais une ligne qui porte un autre numéro que
 *  le sien (la règle de `valider-engagement`). */
function etatDEngagement(p) {
  if (!ETATS_ENGAGEMENTS) return undefined;
  if (p.numero) {
    const n = Object.values(ETATS_ENGAGEMENTS).find((l) => l.salarie === p.salarie
      && l.debut === p.debut && l.numero === p.numero);
    if (n) return n;
  }
  const l = ETATS_ENGAGEMENTS[cleEngagement(p.salarie, p.debut, p.rang)];
  if (l && p.numero && l.numero && l.numero !== p.numero) return undefined;
  return l;
}

/** QUEL contrat `valider-engagement` vise : son numéro s'il en a un, et son rang. */
const contratDe = (l) => ({ salarie: l.salarie, debut: l.debut,
                            numero: l.numero || "", rang: l.rang || 0 });

/* LES TROIS CARRÉS DE LA LIGNE VALIDÉE : DPAE, Contrat, Documents. Le moteur
   rend chacun (`etat-par-engagement`, `carres`) : son nom, sa couleur, ce qui
   reste en un mot, sa phrase et le geste qu'ouvre un clic. La page les
   dessine ; un carré qui a un geste est un bouton. */

/** Le bandeau d'une ligne : un carré par acte. */
function bandeauEtats(ligne) {
  return `<div class="etats-actes">` + (ligne.carres || []).map((c, i) => {
    const clic = !!c.action;
    const classe = c.etat === "en_attente" ? "en-attente" : (c.couleur || "gris");
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
    r = await api("pieces-parties", { salarie: ligne.salarie, debut: ligne.debut });
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

/** Peint les états sur les blocs. Rappelée après chaque frappe : changer le
 *  salarié ou les jours change l'engagement que le bloc désigne. */
function peindreEtats() {
  let refaire = false;
  document.querySelectorAll(".engagement").forEach((div) => {
    const cible = div.querySelector(".etats");
    if (!cible) return;
    const ps = engagementsSaisisDuBloc(div);
    const lignes = ETATS_ENGAGEMENTS
      ? ps.map(etatDEngagement).filter(Boolean)
      : [];
    avisDpaeDeplacee(div, ps, lignes.length === 1 ? lignes[0] : null);
    const connue = ps.length > 0 && lignes.length === ps.length;
    div.classList.toggle("enregistree", connue);
    if (!ETATS_ENGAGEMENTS) { cible.innerHTML = ""; return; }
    if (!connue) {
      fermerEdition(div, false);
      if (!salariesDuBloc(div).length) {
        // Sans salarié, ce bloc n'est pas un engagement : il n'est même pas
        // enregistré. Le dire, plutôt que de laisser un bloc muet qu'on
        // ne peut ni valider ni faire partir.
        cible.innerHTML = `<p class="etats-absent">Choisir qui est engagé.</p>
           <div class="validation"><span class="boutons">${BOUTON_RETIRER}</span></div>`;
        brancherRetirer(div);
        return;
      }
      // Le cas courant et normal : un engagement qu'on vient d'ajouter ou de
      // changer. Pas de pastille grise — une pastille, même grise, se lit
      // comme un état connu. Le message ne dit plus « Enregistrer la saisie
      // d'abord » : le bouton n'existe plus, l'enregistrement se fait tout
      // seul une seconde après la frappe, et il n'y a donc rien à faire —
      // seulement à attendre que l'écran se rattrape.
      // Plus de message ici : « Pas encore dans la saisie
      // enregistrée… » s'affichait en orange sur chaque carte à l'ouverture,
      // le temps d'un aller-retour, et se lisait comme un souci.
      cible.innerHTML = `<div class="validation"><span class="boutons">${BOUTON_RETIRER}</span></div>`;
      brancherRetirer(div);
      return;
    }
    if (lignes.length === 1) {
      const ligne = lignes[0];
      // Un contrat sur plusieurs mois : son chevron est au bord de la case.
      div._ligne = ligne;
      const dp = ligne.etats.dpae || {};
      if (dp.fait && !dp.refusee) {
        div._dpaeFaite = { salarie: ligne.salarie, debut: ligne.debut,
                           nom: ligne.nom, quand: dp.fait };
      }
      cible.innerHTML = `<span class="etats-qui">${echapper(ligne.nom)}
          <span class="discret">— ${echapper(pourQuoi(ligne))}</span></span>`
        + bandeauEtats(ligne) + bandeauValidation(ligne);
      brancherValidation(div, ligne);
      brancherRecu(div);
      fermerEdition(div, !!ligne.valide);
      return;
    }
    // PLUSIEURS ENGAGEMENTS DANS UN BLOC : pas encore validées, sinon le bloc
    // aurait été démultiplié. Si l'une l'a été ailleurs (un autre onglet),
    // on démultiplie maintenant et on repeint.
    if (lignes.some((l) => l.valide)) { dedoubler(div); refaire = true; return; }
    cible.innerHTML = lignes.map((l) =>
      `<span class="etats-qui plusieurs">${echapper(l.nom)}
         <span class="discret">— ${echapper(pourQuoi(l))}</span></span>` + bandeauEtats(l))
      .join("")
      + `<div class="validation">
           <span class="discret">${lignes.length} salariés dans cet engagement</span>
           <span class="boutons">
             ${BOUTON_RETIRER}
             <button class="secondaire v-valider-tout">Valider pour les ${lignes.length}</button>
           </span>
         </div>`;
    cible.querySelector(".v-valider-tout").onclick = (ev) => {
      ev.preventDefault();
      validerPlusieurs(div, lignes);
    };
    brancherRetirer(div);
    brancherRecu(div);
    fermerEdition(div, false);
  });
  if (refaire) return peindreEtats();
}

/** Valide chaque engagement du bloc, PUIS le démultiplie : « il faut le bouton
 *  valider, et une fois validé ça se démultiplie ».
 *  Chaque bloc né ici se repeint validé, fermé, avec son propre
 *  « Modifier… ». Si une validation échoue, on s'arrête là : ce qui est
 *  validé l'est, le reste se voit au repeint. */
async function validerPlusieurs(div, lignes) {
  const mois = document.getElementById("mois").value;
  let faites = 0;
  try {
    for (const l of lignes) {
      await api("valider-engagement", { mois, ...contratDe(l), valide: true });
      faites += 1;
    }
    dire(`${faites} engagements validés.`);
  } catch (e) {
    dire(e.message + (faites ? ` (${faites} validé(s) avant)` : ""),
         "erreur");
  }
  if (faites) dedoubler(div);
  await dessinerEtatsDesEngagements(mois);
}

/** Lit `etat-par-engagement` et repeint. */
async function dessinerEtatsDesEngagements(mois) {
  ETATS_ENGAGEMENTS = null;
  if (!mois) { peindreEtats(); return; }
  try {
    const r = await api("etat-par-engagement", { mois });
    ETATS_ENGAGEMENTS = {};
    r.engagements.forEach((l) => { ETATS_ENGAGEMENTS[cleEngagement(l.salarie, l.debut, l.rang)] = l; });
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
   « On valide, c'est vert, normalement on n'y revient pas, édition bloquée
   temporairement avec un bouton pour modifier avec avertissement » —
   11 et 12/09/2026.

   Trois choses, et la troisième est celle qui compte.

   1. VERT, et sur le bloc entier. Une pastille de plus se serait perdue au
      milieu des quatre autres ; c'est le bloc qui change d'état, pas un
      détail dedans.
   2. ÉDITION FERMÉE. Les champs passent en lecture seule pour de vrai
      (`disabled`), pas seulement grisés : un engagement validé qu'on peut encore
      modifier par distraction n'est pas validé.
   3. L'AVERTISSEMENT DIT CE QUE L'ÉDITEUR A DIT. Pas « êtes-vous sûr ? », qui
      ne renseigne personne — mais la conséquence exacte qu'il a lui-même
      nommée : modifier un engagement validé peut obliger à refaire la DPAE. Et
      quand la DPAE est DÉJÀ partie pour cet engagement, l'écran le sait
      (`etat-par-engagement`) et le dit avec sa date. C'est la différence entre un
      avertissement qu'on lit et un avertissement qu'on clique. */

/** Ferme ou rouvre les champs d'un bloc. Le bouton de validation, lui, reste
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
    if (c.classList.contains("chevron-pluri")) return;  // le chevron du contrat sur plusieurs mois (#127)
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

/** Le bandeau de validation d'un bloc, sous les quatre pastilles. */
function bandeauValidation(ligne) {
  if (ligne.valide) {
    return `<div class="validation faite">
      <span><b>Engagement validé</b> le ${echapper(quand(ligne.valide))}</span>
      <span class="boutons">
        ${BOUTON_RETIRER}
        <button class="modifier v-basculer" data-valide="0">${CRAYON}Modifier l'engagement…</button>
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
    if (b.dataset.valide === "1") return basculerValidation(ligne, true);
    boiteModifier(ligne);
  };
  brancherRetirer(div);
}

/** Le bouton « Retirer cet engagement » d'un bloc, quel que soit son état. Le moteur
 *  décide (`retirer-engagement`) : rien de parti, l'engagement s'efface ; un
 *  envoi parti, il refuse (`EngagementParti`), la page montre sa phrase et
 *  propose de rendre l'engagement non actif. Un bloc pas encore enregistré
 *  n'a rien dans le moteur : il s'efface de l'écran. */
function brancherRetirer(div) {
  const b = div.querySelector(".retirer-engagement");
  if (!b) return;
  b.onclick = async (ev) => {
    ev.preventDefault();
    const mois = document.getElementById("mois").value;
    const lignes = engagementsSaisisDuBloc(div).map(etatDEngagement).filter(Boolean);
    if (!lignes.length) { div.remove(); rafraichirApercu(); dire("Engagement retiré."); return; }
    try {
      for (const l of lignes) {
        try {
          await api("retirer-engagement", { mois, ...contratDe(l) });
        } catch (e) {
          if (e.detail?.type !== "EngagementParti") throw e;
          if (!await boiteNonActif(e.message)) return;
          const r = await api("retirer-engagement", { mois, ...contratDe(l), non_actif: true });
          if (r.a_faire?.length) dire(`${l.nom} : non actif. À faire : ${r.a_faire.join(", ")}.`);
        }
      }
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
 *  carte qui se lit, sans « Modifier… », et ce que leur désactivation rend à faire. */
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

/** « Valider l'engagement » ou « Modifier… ». Si des documents d'un de ses mois sont partis, le moteur pose sa
 *  question (`DocumentsPartis`) : la page la montre telle quelle, et ne renvoie qu'avec la réponse. */
async function validerEngagement(mois, ligne, valide) {
  const charge = { mois, ...contratDe(ligne), valide };
  try { return await api("valider-engagement", charge); } catch (e) {
    if (e.detail?.type !== "DocumentsPartis") throw e;
    const oui = await new Promise((repondre) => {
      ouvrirBoite(`<p>${echapper(e.message)}</p>
        <div class="actions">
          <button class="secondaire" id="dp-non">Annuler</button>
          <button class="primaire" id="dp-oui">Modifier l'engagement</button>
        </div>`, () => repondre(false));
      document.getElementById("dp-non").onclick = () => fermerBoite();
      document.getElementById("dp-oui").onclick = () => { repondre(true); fermerBoite(); };
    });
    if (!oui) return null;
    return api("valider-engagement", { ...charge, confirme: true });
  }
}

async function basculerValidation(ligne, valide) {
  const mois = document.getElementById("mois").value;
  try {
    if (await validerEngagement(mois, ligne, valide) === null) return;
    dire(valide ? "Engagement validé." : "Engagement ouvert à la modification : le revalider ensuite.");
    await dessinerEtatsDesEngagements(mois);
  } catch (e) { dire(e.message, "erreur"); }
}

/* LES ENGAGEMENTS VALIDÉS ENSEMBLE SE ROUVRENT ENSEMBLE. « Valider
 * ces 2 engagements » les valide d'un geste, au même instant, puis l'écran les
 * sépare en un bloc par salarié. Les rouvrir demandait autant de boîtes que
 * de salariés, pour le même spectacle. Sœurs : même spectacle, mêmes bornes,
 * validés à la même seconde. Un même salarié n'est qu'une fois dans un
 * bloc : son jumeau (un autre contrat du même jour) n'est jamais une
 * sœur, même validé à la même seconde. */
function engagementsSoeurs(ligne) {
  const toutes = Object.values(ETATS_ENGAGEMENTS || {});
  const soeurs = toutes.filter((l) => l.valide && l.valide === ligne.valide
    && l.spectacle === ligne.spectacle && l.debut === ligne.debut && l.fin === ligne.fin
    && (l.salarie !== ligne.salarie || l.rang === ligne.rang));
  return soeurs.length ? soeurs : [ligne];
}

async function modifierEngagements(lignes) {
  const mois = document.getElementById("mois").value;
  try {
    for (const l of lignes) {
      if (await validerEngagement(mois, l, false) === null) return;
    }
    dire(lignes.length > 1 ? `Ces ${lignes.length} engagements sont ouverts à la modification.`
                           : "Engagement ouvert à la modification : le revalider ensuite.");
  } catch (e) { dire(e.message, "erreur"); }
  await dessinerEtatsDesEngagements(mois);
}

/* MODIFIER L'ENGAGEMENT : un engagement dont aucun contrat n'est parti se rouvre d'un clic ; un contrat parti se dit
 * d'abord, parce que changer ce qu'il porte fait un avenant à faire signer. La DPAE se dit quand on change le premier
 * jour ou le salarié (`avisDpaeDeplacee`). */
function boiteModifier(ligne) {
  const lignes = engagementsSoeurs(ligne);
  const partis = lignes.filter((l) => l.etats && l.etats.signature
    && (l.etats.signature.fait || l.etats.signature.recu));
  if (!partis.length) return modifierEngagements(lignes);
  const noms = lignes.map((l) => l.nom).join(", ");
  const etat = (l) => {
    const s = l.etats.signature;
    return s.recu ? signeLe(s.recu, s.recu_par) : "envoyé le " + quand(s.fait);
  };
  ouvrirBoite(`
    <h3>Modifier ${lignes.length > 1 ? `ces ${lignes.length} engagements` : "cet engagement"}</h3>
    <p class="discret">${echapper(noms)} — ${echapper(ligne.spectacle)},
      du ${echapper(jjmmaaaa(ligne.debut))} au ${echapper(jjmmaaaa(ligne.fin))}
      · validée${lignes.length > 1 ? "s" : ""} le ${echapper(quand(ligne.valide))}</p>
    <div class="encart attention">
      <b>${partis.length > 1 ? "Les contrats sont déjà partis" : "Le contrat est déjà parti"}</b>
      ${partis.map((l) => `${lignes.length > 1 ? echapper(l.nom) + " : c" : "C"}ontrat ${echapper(etat(l))}.`).join(" ")}
      Il porte les dates, les heures, les cachets et la rémunération : si
      l'un d'eux change, ce contrat n'est plus le bon : son carré passe à
      l'orange, et un avenant est à faire signer au salarié.
    </div>
    <p class="note">Modifier n'annule rien et ne défait rien : ça remet les
      champs en écriture. On revalide ensuite.</p>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="v-rouvrir">Modifier ${lignes.length > 1 ? "les engagements…" : "l'engagement…"}</button>
    </div>`);
  document.getElementById("v-rouvrir").onclick = async () => {
    fermerBoite();
    await modifierEngagements(lignes);
  };
}

/* LA DPAE NE SE DIT QUE SUR CE QU'ELLE DÉCLARE. L'utilisateur :
 * l'avertissement doit correspondre aux vraies attentes de la DPAE, rien
 * d'inventé. Ce qu'elle déclare, c'est R1221-1 du code du travail : le
 * salarié, la date et l'heure d'embauche, la nature et la durée du contrat
 * (sa date de fin, pour un CDD) — ni la rémunération, ni les heures, ni les
 * cachets. L'encart ne paraît donc que si l'une de ces données s'écarte de
 * la DPAE déposée : premier jour ou salarié (le bloc retient l'embauche
 * déclarée la dernière fois qu'il l'a lue), ou fin du contrat
 * (`fin_declaree`, lue au journal du dépôt). Et il dit ce que dit l'Urssaf,
 * page « DPAE » d'urssaf.fr, mise à jour le 15/07/2026 : les informations
 * relatives au contrat de travail se modifient dans l'espace en ligne
 * (Compte › Embauche › Modifier une DPAE) ; pour toute autre modification,
 * une nouvelle DPAE, ou les services de l'Urssaf (3957). Rien de plus : ni
 * délai, ni annulation, qu'aucune source officielle lue ne pose. */
const URSSAF_DPAE = "https://www.urssaf.fr/accueil/employeur/embaucher-gerer-salaries/embaucher/declaration-prealable-embauche.html";

function avisDpaeDeplacee(div, ps, ligne) {
  const cible = div.querySelector(".etats");
  let avis = div.querySelector(".avis-dpae");
  const d = div._dpaeFaite;
  const ecarts = [];
  if (d && !ps.some((p) => p.salarie === d.salarie && p.debut === d.debut)) {
    ecarts.push(ps.some((p) => p.salarie === d.salarie)
      ? `premier jour : la DPAE déclare le ${jjmmaaaa(d.debut)}`
      : `salarié : la DPAE déclare ${d.nom}`);
  }
  const dp = ligne && ligne.etats && ligne.etats.dpae;
  if (dp && dp.fait && !dp.refusee && dp.fin_declaree && dp.fin_declaree !== ligne.fin) {
    ecarts.push(`fin du contrat : la DPAE déclare le ${jjmmaaaa(dp.fin_declaree)},
      l'engagement dit le ${jjmmaaaa(ligne.fin)}`);
  }
  if (!ecarts.length) { if (avis) avis.remove(); return; }
  if (!avis) {
    avis = document.createElement("div");
    avis.className = "encart attention avis-dpae";
    cible.parentNode.insertBefore(avis, cible);
  }
  const quandDpae = d ? d.quand : dp.fait;
  avis.innerHTML = `<b>L'engagement ne correspond plus à la DPAE déposée le ${echapper(quand(quandDpae))}</b>
    <ul>${ecarts.map((x) => `<li>${echapper(x)}</li>`).join("")}</ul>
    Selon l'Urssaf, les informations relatives au contrat de travail se
    modifient dans son espace en ligne (Compte › Embauche › Modifier une
    DPAE) ; pour toute autre modification, on dépose une nouvelle DPAE ou on
    contacte l'Urssaf (3957).
    <a href="${URSSAF_DPAE}" target="_blank" rel="noopener">La page de l'Urssaf</a>`;
}

/* PLUS DE CASE DE SÉLECTION NI DE BARRE D'ACTIONS GROUPÉES :
 * « on pourrait enlever la petite coche de sélection, tout se fait
 * directement quand on clique sur le bouton déposer DPAE ou demander les
 * signatures ». Les deux boîtes de la colonne de droite listent chaque
 * contrat du mois avec sa case, cochée d'avance sur ce qui reste à faire :
 * c'est là que l'on choisit, et rien ne part sans y être passé. */

/* LA BOÎTE DPAE, comme celle des signatures (l'éditeur, 15/09/2026 : « qu'il
 * y ait la liste déroulante et que soient déjà sélectionnés les contrats qui
 * n'ont pas de DPAE »). Chaque contrat du mois enregistré, une case ; cochée
 * d'avance si aucune DPAE n'est partie, ou si elle a été refusée. Un contrat
 * en cours depuis un mois précédent n'a pas de DPAE ce mois-ci : il se voit,
 * grisé, avec le mois où elle a été faite.
 *
 * DEUX TEMPS, ET C'EST VOULU : on ÉCRIT le fichier des embauches cochées,
 * puis on ouvre le récapitulatif de dépôt habituel. Écrire n'envoie rien et
 * ne consomme aucun compteur ; le dépôt reste le même geste, avec la même
 * relecture ligne à ligne et le même mot de passe. Le fichier prend un nom
 * horodaté, à côté du fichier complet du mois, jamais par-dessus. */
async function boiteDpae(mois) {
  if (!mois) { dire("Choisir un mois."); return; }
  // LA SAISIE À L'ÉCRAN D'ABORD. Cette boîte lit la
  // saisie ENREGISTRÉE : un engagement tapé il y a moins d'une seconde, ou qu'un
  // spectacle manquant empêche d'enregistrer, n'y figurerait pas -- et sa
  // DPAE ne partirait pas, sans que rien ne le dise.
  let pasEnregistre = "";
  const ecran = document.getElementById("mois");
  if (ecran && ecran.value === mois && typeof enregistrerSaisie === "function") {
    clearTimeout(MINUTEUR_SAISIE);
    await enregistrerSaisie(true);
    if (JSON.stringify(saisieCourante()) !== DERNIERE_ECRITE) {
      pasEnregistre = document.getElementById("note-saisie").textContent
        || "La saisie à l'écran n'est pas enregistrée.";
    }
  }
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
    ${pasEnregistre ? `<div class="encart attention"><b>Une partie de l'écran
      n'est pas enregistrée</b> ${echapper(pasEnregistre)} Les engagements qui ne
      figurent pas ci-dessous n'auront pas leur DPAE : compléter la saisie,
      puis rouvrir cette fenêtre.</div>` : ""}
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
    try {
      const w = await api("dpae", {
        mois, engagements: choisies.map((l) => ({ salarie: l.salarie, debut: l.debut })),
      });
      fermerBoite();
      // Le dossier porte le fichier à déposer et son récapitulatif ; c'est le
      // premier qui se dépose.
      const f = (w.fichiers || []).find(
        (x) => /\.txt$/i.test(x.nom) && !/recapitulatif/i.test(x.nom));
      dire("Fichier écrit : " + (f ? f.nom : w.dossier));
      await rafraichirActions();
      if (f) boiteDepot("dpae", f.chemin, { testerDabord: true });
    } catch (e) {
      rendre();
      dire(e.message, "erreur");
    }
  };
}

