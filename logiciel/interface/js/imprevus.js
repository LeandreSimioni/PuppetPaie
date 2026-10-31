/* Les imprévus : un arrêt de travail, une absence, ce qu'ils font naître, et la fin d'un contrat qui se signale. */
"use strict";

/* ------------------------------------------------ LES IMPRÉVUS
   Léandre : « simple pour les gens qui ont besoin de simple, compliqué pour
   les gens qui ont besoin de compliqué ». Quelqu'un qui fait des paies
   apprend qu'un salarié est malade : il va sur la ligne de ce salarié, pas
   dans une section du mois qui dirait « Arrêts de travail » tous les mois
   pour rien. Les imprévus sont rares, et rien ici ne doit laisser croire
   qu'il y en a tout le temps.

   LA FORME, EN TROIS ENDROITS, ET PAS UN DE PLUS :

   - LA PORTE est sur la carte de l'engagement : « Un imprévu… » (et « … »
     sur un engagement validé). Elle propose un arrêt de travail, une absence
     (congé parental, sans solde, sabbatique… : une suspension du contrat,
     bloc 65), et, quand la carte sait déjà dire une fin, la rupture avant
     terme — qui n'est pas une boîte de plus : elle ouvre le « Contrat » de
     la carte, où la fin se saisit depuis toujours. Le salarié et le contrat
     sont ceux de la carte : on ne les redemande pas. Sous la carte, une ligne
     par imprévu qui la touche ce mois-ci, avec ses gestes (la reprise, le
     maintien du mois, retirer) et les jours payés pendant l'imprévu, à
     retirer un par un.
   - LA COLONNE DU MOIS ne garde que CE QUI EST DÛ : une ligne par
     obligation à faire ou en retard, sa date butoir, son état, et le bouton
     qui fait l'acte. Vide quand il n'y a rien, titre compris. La phrase et
     les textes d'une obligation ne se lisent qu'à la demande (« pourquoi »).
   - LE DOSSIER DU SALARIÉ (Configuration › Salariés) garde l'histoire : ses
     imprévus passés et en cours, ce qui en est parti, et la même porte.

   LE VOCABULAIRE, tenu partout : un IMPRÉVU (le mot de la porte), un ARRÊT DE
   TRAVAIL, une ABSENCE (ce que la DSN appelle suspension), la REPRISE, le
   MAINTIEN de salaire, CE QUI EST DÛ. Les congés payés d'un permanent ne
   sont pas des imprévus : ils restent dans la saisie du mois.

   RIEN NE SE DÉCIDE ICI (règle 2) : le moteur dit ce que chaque fait ouvre
   (`imprevus` : `maintien_ouvert`, `jours_deja_indemnises_attendus`,
   `cachets`, `obligations` ; `arret-obligations` pour un arrêt pas encore
   écrit), et la page ne montre un champ que parce qu'il le dit. */

let IMPREVUS = null;        // la dernière lecture `imprevus` du mois affiché
let FINS = null;            // la dernière lecture `fins-de-contrat` du mois affiché
let NOMENCLATURES_IMPREVUS = null;   // motifs d'arrêt, de reprise, d'absence : ceux de la dernière lecture

/** Le nom d'un salarié, tel que la saisie le montre ; sa clé à défaut. */
function nomDuSalarie(cle) {
  // La saisie écrit parfois le nom tel qu'on l'a tapé (« Audinot ») là où la fiche a sa clé (« AUDINOT ») : même personne.
  const s = ((ETAT && ETAT.salaries) || []).find((x) => String(x.cle).toUpperCase() === String(cle || "").toUpperCase());
  return s ? s.libelle : String(cle || "");
}

/** « attestation de salaire (DSIJ) » : la première lettre seulement, les sigles restent. */
function minuscule(t) {
  t = String(t || "");
  return t.charAt(0).toLowerCase() + t.slice(1);
}

/** « au plus tard le 16/10/2026 à 10:00 » : la date butoir que le moteur a posée. */
function butoirEnClair(o) {
  if (!o || !o.butoir) return "";
  const h = o.butoir_heure && o.butoir_heure !== "24:00" ? ` à ${o.butoir_heure}` : "";
  return `au plus tard le ${jjmmaaaa(o.butoir)}${h}`;
}

/** Ce qu'un imprévu est, en une ligne : « Arrêt de travail (maladie) » ou le motif d'une absence. */
function quoiDeLImprevu(i) {
  return i.genre === "arret" ? `Arrêt de travail (${i.motif_libelle})` : i.motif_libelle;
}

/** Sa période : du premier au dernier jour d'absence. Un arrêt sans reprise dit sa fin prescrite. */
function periodeDeLImprevu(i) {
  if (i.genre === "arret" && !i.reprise) {
    return `depuis le ${jjmmaaaa(i.premier_jour)}, prescrit jusqu'au ${jjmmaaaa(i.dernier_jour)}`;
  }
  return `du ${jjmmaaaa(i.premier_jour)} au ${jjmmaaaa(i.dernier_jour)}`;
}

/* ------------------------------------------------------ lire, et peindre */

/** Les imprévus et les fins du mois, lus ensemble : la colonne et les cartes en dépendent. */
async function dessinerImprevus(mois) {
  if (!mois) return;
  let imprevus, fins;
  try { imprevus = await api("imprevus", { mois }); }
  catch (e) { imprevus = null; }
  try { fins = await api("fins-de-contrat", { mois }); }
  catch (e) { fins = null; }
  if (MOIS_AFFICHE && MOIS_AFFICHE !== mois) return;      // on a changé de mois entre-temps
  IMPREVUS = imprevus;
  FINS = fins;
  if (imprevus) NOMENCLATURES_IMPREVUS = imprevus;
  peindreDus(mois);
  peindreImprevusDesCartes();
}

/** Après une écriture : relire, et refaire l'aperçu — un imprévu change la paie (la ligne retirée, le maintien, les avis). */
async function rafraichirImprevus() {
  await dessinerImprevus(MOIS_AFFICHE);
  if (MOIS_AFFICHE) calculerApercu();
}

/** Les motifs (arrêt, reprise, absence) : ceux de la dernière lecture, sinon une lecture. */
async function nomenclaturesImprevus() {
  if (NOMENCLATURES_IMPREVUS) return NOMENCLATURES_IMPREVUS;
  NOMENCLATURES_IMPREVUS = await api("imprevus", MOIS_AFFICHE ? { mois: MOIS_AFFICHE } : {});
  return NOMENCLATURES_IMPREVUS;
}

/* ------------------------------------------- la colonne : ce qui est dû
   Une ligne par obligation due. Un arrêt en fait naître une ou plusieurs
   (le signalement, la déclaration d'accident, le signalement de reprise) ;
   une fin de contrat qui se signale, une. Ce qui est fait, ou pas encore dû,
   ne paraît pas : un bloc qui dirait « rien à faire » tous les mois finirait
   par ne plus se lire. */
function peindreDus(mois) {
  const cible = document.getElementById("dus");
  if (!cible) return;
  const rangs = [];
  ((IMPREVUS && IMPREVUS.imprevus) || []).filter((i) => i.genre === "arret").forEach((a) => {
    (a.obligations || []).filter((o) => o.du).forEach((o) => rangs.push({ a, o }));
  });
  ((FINS && FINS.fins) || []).forEach((f) => {
    const o = f.obligation;
    if (f.du) rangs.push({ f, o });
  });
  if (!rangs.length) { cible.innerHTML = ""; return; }
  // Le plus pressé d'abord (`priorite`, que le moteur donne), puis par date butoir.
  const rang = (x) => String((x.o && x.o.priorite) ?? 1) + ((x.o && x.o.butoir) || "9");
  rangs.sort((x, y) => rang(x).localeCompare(rang(y)));
  cible.innerHTML = `<h2>Ce qui est dû</h2>
    ${rangs.map((x, k) => (x.a ? rangObligation(x.a, x.o, k) : rangFin(x.f, k))).join("")}`;
  rangs.forEach((x, k) => {
    const r = cible.querySelector(`[data-du="${k}"]`);
    if (x.a) brancherObligation(r, x.a, x.o);
    else brancherFin(r, mois, x.f);
  });
}

/** L'état d'une obligation : la pastille et son mot. */
function pastilleDu(o) {
  return pastilleDeLEtat(o && o.pastille);
}

/** Le « pourquoi » d'une obligation, replié : la phrase du moteur, le délai et son point de départ, les textes. */
function pourquoiDu(o) {
  if (!o) return "";
  const depart = o.depart || {};
  return `<details class="pourquoi"><summary>pourquoi</summary>
      <p>${echapper(o.phrase || "")}</p>
      ${o.delai ? `<p>${echapper(o.delai.charAt(0).toUpperCase() + o.delai.slice(1))}${depart.quoi
        ? ` — à compter de ${echapper(depart.quoi)}${depart.jour ? `, le ${echapper(jjmmaaaa(depart.jour))}` : ""}` : ""}.</p>` : ""}
      ${depart.presume ? `<p class="discret">Point de départ présumé : ${echapper(depart.presume)}</p>` : ""}
      ${o.destinataire ? `<p class="discret">${echapper(o.destinataire)}</p>` : ""}
      ${(o.textes || []).length ? `<ul class="discret">${o.textes.map((t) =>
        `<li>${echapper(t.reference)}${t.dit ? ` — ${echapper(t.dit)}` : ""}</li>`).join("")}</ul>` : ""}
    </details>`;
}

/* LES GESTES D'UNE OBLIGATION se lisent sur ses champs, pas sur son nom :
   une obligation qui se fait HORS DSN porte la liste des endroits où la
   faire (`ou`, une liste) et se dit faite par la route que voici ; un
   signalement DSN (`ou` en une phrase) s'écrit par le logiciel. Une
   obligation de plus dans `obligations-base.json` s'affiche donc sans rien
   toucher ici ; seule la route qui la dit faite s'ajoute à cette table. */
const ROUTES_OBLIGATION_FAITE = {
  declaration_accident: "accident-declare",
  // Le deuil d'un enfant (motif 19) : pas de signalement, l'attestation de salaire hors DSN.
  attestation_salaire: "attestation-declare",
};
// Le rectificatif d'un arrêt (une subrogation prolongée après le signalement, fiche 1983) est un signalement d'arrêt :
// la même boîte, qui demande l'identifiant du signalement parti puisqu'il y en a déjà un.
const SIGNALEMENTS_DSN = { signalement_arret: "04", signalement_arret_rectificatif: "04", signalement_reprise: "05" };
const CONNAISSANCE = { signalement_arret: "arret", declaration_accident: "accident" };

function rangObligation(a, o, k) {
  const gestes = [];
  if (SIGNALEMENTS_DSN[o.cle]) {
    gestes.push(`<button class="secondaire petit" data-geste="signaler">Signaler…</button>`);
  }
  if (Array.isArray(o.ou) && ROUTES_OBLIGATION_FAITE[o.cle]) {
    gestes.push(`<button class="secondaire petit" data-geste="faite">Faite le…</button>`);
  }
  // Le point de départ ne se dit que là où le délai court de la connaissance
  // (`arret-connaissance`) : l'accident, et le signalement d'un arrêt sans
  // subrogation — subrogé, il court du mois de l'arrêt.
  if (CONNAISSANCE[o.cle] && o.depart && o.subrogation !== true) {
    gestes.push(`<button class="lien" data-geste="connu">Connu le…</button>`);
  }
  return rangDu(k, pastilleDu(o), o.libelle, nomDuSalarie(a.salarie), butoirEnClair(o), gestes.join(""), "", pourquoiDu(o));
}

/** Une ligne de ce qui est dû : la pastille, ce qu'il faut faire et pour qui, la date butoir et les gestes ; dessous,
 *  ce qui manque, et le « pourquoi » replié. */
function rangDu(k, pastille, quoi, qui, butoir, gestes, manque, pourquoi) {
  return `<div class="du" data-du="${k}">
      ${pastille}
      <div class="du-corps">
        <div class="du-quoi"><b>${echapper(quoi)}</b> <span class="discret">${echapper(qui)}</span></div>
        <div class="du-bas"><span class="du-butoir">${echapper(butoir)}</span><span class="du-gestes">${gestes}</span></div>
        ${manque}${pourquoi}
      </div>
    </div>`;
}

function brancherObligation(r, a, o) {
  const b = (g) => r.querySelector(`[data-geste="${g}"]`);
  if (b("signaler")) b("signaler").onclick = () => boiteSignalerArret(a, SIGNALEMENTS_DSN[o.cle]);
  if (b("faite")) b("faite").onclick = () => boiteObligationFaite(a, o);
  if (b("connu")) b("connu").onclick = () => boiteConnaissance(a, CONNAISSANCE[o.cle], o);
}

/* LA FIN D'UN CONTRAT QUI SE SIGNALE (nature 07, 19/09/2026), sous la même
   forme. Un CDI ou un CDD ordinaire qui se termine fait un signalement à
   part ; un CDD d'usage à son terme (031) n'en fait pas — sa fin part dans la
   mensuelle ; rompu avant (036, 037…), il se signale (fiche 2062). C'est le
   moteur qui tranche (`Nature::signalement_fctu`, route `fins-de-contrat`).
   Ce qui manque au signalement se tape dans la carte, sous « Contrat » :
   proposer un bouton qui refuse serait pire que ne rien proposer. Le
   moteur dit chaque manque par le nom de sa case et ce qu'on y met
   (`a_saisir`) ; il se lit sans rien déplier, et « La dire… » mène à la
   case. Une fin d'un mois passé dont le signalement est en retard paraît
   aussi (`mois`) : une échéance manquée ne se tait pas. */
function rangFin(f, k) {
  const manque = f.a_saisir.length;
  const ailleurs = f.mois && f.mois !== MOIS_AFFICHE ? ` (${moisEnClair(f.mois)})` : "";
  return rangDu(k, pastilleDeLEtat(f.pastille), (f.obligation && f.obligation.libelle) || "Signalement de fin de contrat",
    `${nomDuSalarie(f.salarie)}, ${f.nature}, fin le ${jjmmaaaa(f.fin)}${ailleurs}`, butoirEnClair(f.obligation),
    `<button class="secondaire petit" data-geste="signaler"${manque ? " disabled" : ""}>Signaler…</button>
     <button class="lien" data-geste="papiers">Les papiers de fin…</button>`,
    manque ? `<div class="du-manque"><p>À dire dans sa carte, sous « Contrat », avant de signaler :</p>
      ${f.a_saisir.map((m, i) => `<p><b>${echapper(m.case)}</b> : ${echapper(m.dire)}
        <button class="lien" data-case="${i}">La dire…</button></p>`).join("")}</div>` : "",
    pourquoiDu(f.obligation));
}

function brancherFin(r, mois, f) {
  r.querySelector('[data-geste="signaler"]').onclick = () => boiteSignalerFin(f.mois || mois, f);
  r.querySelector('[data-geste="papiers"]').onclick = () => boitePapiersDeFin(f.mois || mois, f.salarie);
  r.querySelectorAll("[data-case]").forEach((b) => {
    b.onclick = () => allerALaCase(f, f.a_saisir[Number(b.dataset.case)].champ);
  });
}

/* LA CASE D'UN MANQUE, dans la carte de l'engagement : le mois de la fin,
   la carte du salarié, rouverte à la modification si elle est validée (le
   chemin de toujours, « Modifier l'engagement… »), le bloc « Contrat »
   déplié, la case sous le curseur. */
const CASES_DE_FIN = {
  motif_rupture: ".e-motif-rupture",
  notification_rupture: ".e-notification",
  preavis: ".e-preavis-type",
  dernier_jour_paye: ".e-dernier-paye",
};

/** Attend que `test()` rende quelque chose (au plus `ms`) ; rend ce qu'il rend, ou null. */
async function attendre(test, ms) {
  for (let t = 0; t < ms; t += 200) {
    const v = test();
    if (v) return v;
    await new Promise((r) => setTimeout(r, 200));
  }
  return null;
}

async function allerALaCase(f, champ) {
  if (f.mois && f.mois !== MOIS_AFFICHE) allerAuMois(f.mois);
  const carte = () => {
    if (f.mois && (MOIS_AFFICHE !== f.mois || document.getElementById("engagements").classList.contains("en-lecture"))) return null;
    const siennes = [...document.querySelectorAll("#engagements .engagement")]
      .filter((d) => salarieDuBloc(d).toUpperCase() === String(f.salarie).toUpperCase());
    return siennes.find((d) => carteDe(d).numero === f.numero || (f.debut && carteDe(d).debut === f.debut)) || siennes[0] || null;
  };
  let div = await attendre(carte, 15000);
  if (!div) { dire(`La carte de ${nomDuSalarie(f.salarie)} ne se trouve pas dans ${moisEnClair(f.mois || MOIS_AFFICHE)}.`, "erreur"); return; }
  if (div.classList.contains("validee") && div._ligne) ouvrirEnModification(div);
  else ouvrirCarte(div);
  const options = div.querySelector(".e-options");
  if (options) options.hidden = false;
  const contrat = div.querySelector(".e-contrat");
  if (contrat) contrat.open = true;
  const cible = (CASES_DE_FIN[champ] && div.querySelector(CASES_DE_FIN[champ])) || div.querySelector(".e-contrat");
  if (!cible) return;
  cible.scrollIntoView({ block: "center" });
  cible.focus();
}

/* --------------------------------------------------------- LA PORTE
   Sur la carte d'un engagement, quand elle porte UN salarié (plusieurs
   cochés, c'est une saisie en cours : on ne sait pas encore de qui on
   parle). « Un imprévu… » au pied d'une carte ouverte, « … » sur un engagement
   validé. */
function majPorteImprevu(div) {
  const b = div.querySelector(".e-imprevu-btn");
  if (b) b.hidden = !salarieDuBloc(div);
}

/** Ce que la carte sait de son contrat : le salarié, le numéro s'il est figé, le début. */
function carteDe(div) {
  const salarie = salarieDuBloc(div);
  return {
    div, salarie,
    numero: div.dataset.numeroDe === salarie ? div.dataset.numero || "" : "",
    debut: debutDuBloc(div),
  };
}

/** La rupture se propose quand le moteur dit qu'elle a un sens pour cet engagement (`contrat.rupture` de l'aperçu, posé
 *  sur la carte) ; elle se saisit dans une carte ouverte. */
function ruptureOuverte(div) {
  if (carteFermee(div) || div.classList.contains("validee")) return null;    // une carte fermée, un engagement validé
  return div._rupture ? div._rupture.porte : null;
}

let MENU_IMPREVU = null;

function fermerMenuImprevu() {
  if (MENU_IMPREVU) { MENU_IMPREVU.remove(); MENU_IMPREVU = null; }
}

/** Le petit menu de la porte, ancré sous le bouton. `ou` : la carte, ou `{salarie}` depuis le dossier du salarié. */
function menuImprevu(ancre, ou) {
  fermerMenuImprevu();
  const rupture = ou.div ? ruptureOuverte(ou.div) : null;
  const menu = document.createElement("div");
  menu.className = "menu-imprevu";
  menu.setAttribute("role", "menu");
  menu.innerHTML = `
    <button type="button" data-choix="arret">Un arrêt de travail
      <span class="discret">maladie, accident, maternité…</span></button>
    <button type="button" data-choix="absence">Une absence
      <span class="discret">congé parental, sans solde, sabbatique…</span></button>
    ${rupture ? `<button type="button" data-choix="rupture">${echapper(rupture)}
      <span class="discret">se saisit sous « Contrat »</span></button>` : ""}`;
  document.body.appendChild(menu);
  const r = ancre.getBoundingClientRect();
  menu.style.top = `${window.scrollY + r.bottom + 4}px`;
  menu.style.left = `${Math.max(8, window.scrollX + r.right - menu.offsetWidth)}px`;
  MENU_IMPREVU = menu;
  menu.querySelector('[data-choix="arret"]').onclick = () => { fermerMenuImprevu(); boiteArret(ou); };
  menu.querySelector('[data-choix="absence"]').onclick = () => { fermerMenuImprevu(); boiteAbsence(ou); };
  const r2 = menu.querySelector('[data-choix="rupture"]');
  if (r2) r2.onclick = () => { fermerMenuImprevu(); ouvrirRupture(ou.div); };
  setTimeout(() => {
    const dehors = (ev) => {
      if (MENU_IMPREVU && !MENU_IMPREVU.contains(ev.target)) {
        fermerMenuImprevu();
        document.removeEventListener("click", dehors);
      }
    };
    document.addEventListener("click", dehors);
  }, 0);
  menu.onkeydown = (ev) => { if (ev.key === "Escape") fermerMenuImprevu(); };
  menu.querySelector("button").focus();
}

/** La rupture : la fin se saisit dans la carte, sous « Contrat », comme toujours. On y va. */
function ouvrirRupture(div) {
  div.querySelector(".e-options").hidden = false;
  const contrat = div.querySelector(".e-contrat");
  contrat.open = true;
  const fin = div.querySelector(".e-fin");
  const cible = fin.value || (div._rupture && div._rupture.fin) ? div.querySelector(".e-motif-rupture") : fin;
  cible.scrollIntoView({ block: "center" });
  cible.focus();
}

/* -------------------------------------------- sous la carte : ses imprévus
   Une ligne par imprévu qui touche CE contrat ce mois-ci : ce que c'est, sa
   période, ses gestes. Et les jours que la saisie paie pendant l'imprévu —
   le moteur les nomme (`cachets`), la page propose de les retirer un par
   un, et c'est la ligne de paie de la carte qui change, comme si on avait
   décoché le jour au calendrier. Rien ne se retire seul. */
function peindreImprevusDesCartes() {
  const cartes = [...document.querySelectorAll("#engagements .engagement")];
  const parCarte = new Map(cartes.map((d) => [d, []]));
  ((IMPREVUS && IMPREVUS.imprevus) || []).forEach((i) => {
    const siennes = cartes.filter((d) => salarieDuBloc(d).toUpperCase() === String(i.salarie).toUpperCase());
    const c = siennes.find((d) => {
      const x = carteDe(d);
      return (i.contrat && x.numero === i.contrat) || (i.contrat_debut && x.debut === i.contrat_debut);
    }) || siennes[0];
    if (c) parCarte.get(c).push(i);
  });
  cartes.forEach((d) => peindreImprevusDeLaCarte(d, parCarte.get(d)));
}

function peindreImprevusDeLaCarte(div, liste) {
  const cible = div.querySelector(".e-imprevus");
  if (!cible) return;
  if (!liste.length) { cible.innerHTML = ""; return; }
  const validee = div.classList.contains("validee");
  cible.innerHTML = liste.map((i, k) => {
    const jours = (i.cachets || []).filter((c) => c.mois === MOIS_AFFICHE);
    return `<div class="imprevu" data-imprevu="${k}">
        <span class="imprevu-quoi"><b>${echapper(quoiDeLImprevu(i))}</b>
          <span class="discret">${echapper(periodeDeLImprevu(i))}</span></span>
        <span class="imprevu-gestes">${gestesImprevu(i, MOIS_AFFICHE)}</span>
        ${jours.length ? `<div class="imprevu-jours">Payé pendant ${i.genre === "arret" ? "l'arrêt" : "l'absence"} :
          ${jours.map((c) => c.dates.map((iso) => `${echapper(c.libelle)} le ${echapper(jjmmaaaa(iso))}
            ${validee ? "" : `<button class="lien" data-retirer="${echapper(iso)}" data-rubrique="${echapper(c.rubrique)}">retirer ce jour</button>`}`).join(" · ")).join(" · ")}
          ${validee ? `<span class="discret">— l'engagement est validé : cliquer sur sa carte pour l'ouvrir, retirer ces jours, puis « Modifier l'engagement ».</span>` : ""}
        </div>` : ""}
      </div>`;
  }).join("");
  liste.forEach((i, k) => {
    const r = cible.querySelector(`[data-imprevu="${k}"]`);
    brancherGestesImprevu(r, i, MOIS_AFFICHE, rafraichirImprevus);
    r.querySelectorAll("[data-retirer]").forEach((b) => {
      b.onclick = () => {
        if (retirerJour(div, b.dataset.rubrique, b.dataset.retirer)) {
          dire(`Le ${jjmmaaaa(b.dataset.retirer)} est retiré de la ligne de paie. La saisie s'enregistre.`);
        } else {
          dire(`Le ${jjmmaaaa(b.dataset.retirer)} n'est sur aucune ligne ouverte de cette carte : le décocher au calendrier.`, "erreur");
        }
      };
    });
  });
}

/** Décoche `iso` sur la ligne de paie de `rubrique` qui le porte, dans la carte : la ligne recompte ses cachets et la
 *  saisie s'enregistre, comme au calendrier. Rend false si aucune ligne ouverte ne le porte. */
function retirerJour(div, rubrique, iso) {
  if (iso.slice(0, 7) !== MOIS_AFFICHE) return false;
  const j = parseInt(iso.slice(8, 10), 10);
  const ligne = [...div.querySelectorAll(".rubrique")].find((r) =>
    r.querySelector(".r-code").value === rubrique && joursDe(r.querySelector(".r-jours")).includes(j)
    && !r.querySelector(".r-jours").disabled);
  if (!ligne) return false;
  basculerJour(ligne, j);
  return true;
}

/** Les gestes d'un imprévu, les mêmes sur la carte et dans le dossier du salarié. `mois` : celui du maintien proposé. */
function gestesImprevu(i, mois) {
  const g = [];
  if (i.genre === "arret") {
    // Le moteur dit ce que l'arrêt ouvre : en cours, il se prolonge ou se reprend plus tôt ; une reprise saisie se
    // corrige tant que son signalement n'est pas parti. Une reprise à la date prévue ne se saisit pas : la DSN la porte.
    if (i.peut_prolonger) g.push(`<button class="lien" data-geste="prolonger">Prolonger l'arrêt…</button>`);
    if (i.peut_reprendre_plus_tot) g.push(`<button class="lien" data-geste="reprise">Le salarié a repris plus tôt…</button>`);
    if (i.reprise_corrigeable) g.push(`<button class="lien" data-geste="reprise">Reprise du ${echapper(jjmmaaaa(i.reprise))} : corriger…</button>`);
    if (i.maintien_ouvert && mois && (i.mois_du_maintien || []).includes(mois)) {
      const m = (i.maintien || {})[mois];
      g.push(`<button class="lien" data-geste="maintien">Maintien ${echapper(deMois(mois))}${
        m != null ? ` : ${echapper(euros(m))}` : "…"}</button>`);
    }
    if (!i.signale && !(i.reporte_par || []).length) g.push(`<button class="lien" data-geste="retirer">Retirer</button>`);
  } else {
    g.push(`<button class="lien" data-geste="fin">Changer la fin…</button>`);
    if (!(i.reporte_par || []).length) g.push(`<button class="lien" data-geste="retirer">Retirer</button>`);
  }
  return g.join("");
}

function brancherGestesImprevu(r, i, mois, apres) {
  const b = (g) => r.querySelector(`.imprevu-gestes [data-geste="${g}"]`);
  if (b("reprise")) b("reprise").onclick = () => boiteReprise(i, apres);
  if (b("prolonger")) b("prolonger").onclick = () => boiteProlonger(i, apres);
  if (b("maintien")) b("maintien").onclick = () => boiteMaintien(i, mois, apres);
  if (b("fin")) b("fin").onclick = () => boiteFinAbsence(i, apres);
  if (b("retirer")) b("retirer").onclick = async () => {
    try {
      if (i.genre === "arret") await api("arret-oublier", { salarie: i.salarie, dernier_jour: i.dernier_jour_travaille });
      else await api("suspension-oublier", { salarie: i.salarie, debut: i.debut });
      dire(`${quoiDeLImprevu(i)} retiré : aucune déclaration ne l'avait porté.`);
      await apres();
    } catch (e) { dire(e.message, "erreur"); }
  };
}

/* ------------------------------------------------ le contrat d'un imprévu
   Le moteur dit les contrats du salarié qui couvrent le jour
   (`arret-contrats`). Depuis une carte, c'est le sien, et on ne demande
   rien ; depuis le dossier du salarié, on ne demande que s'il y en a
   plusieurs. Sans contrat ce jour-là, il n'y a rien à suspendre, et la
   page dit ce que le moteur a répondu. */
async function contratsDuJour(ou, jour, cible) {
  cible.innerHTML = "";
  if (!ou.salarie || !jour) return null;
  let contrats;
  try { contrats = (await api("arret-contrats", { salarie: ou.salarie, jour })).contrats || []; }
  catch (e) { cible.innerHTML = `<span class="erreur">${echapper(e.message)}</span>`; return null; }
  if (!contrats.length) {
    cible.innerHTML = `<span class="erreur">Aucun contrat de ${echapper(nomDuSalarie(ou.salarie))} ne couvre
      le ${echapper(jjmmaaaa(jour))} : il n'y a rien à suspendre.</span>`;
    return null;
  }
  const carte = ou.carte;
  const sien = carte && contrats.find((c) => (carte.numero && c.numero === carte.numero) || c.debut === carte.debut);
  if (contrats.length === 1) return contrats[0];
  if (sien && sien.numero) return sien;
  cible.innerHTML = `Plusieurs contrats couvrent ce jour-là — lequel ?
    <select id="im-contrat">${contrats.map((c) => `<option value="${echapper(c.numero)}"${c === sien ? " selected" : ""}>${
      echapper(c.numero || "sans numéro")}, ${echapper(periodeEnClair(c.debut, c.fin))}</option>`).join("")}</select>`;
  return { choisir: true, contrats };
}

/** Le numéro à envoyer : seulement quand le moteur en a rendu plusieurs. */
function contratAEnvoyer(trouve) {
  if (!trouve || !trouve.choisir) return {};
  const s = document.getElementById("im-contrat");
  return s ? { contrat: s.value } : {};
}

/** D'où vient la boîte : la carte (`{div}`) ou le dossier (`{salarie}`). */
async function provenance(ou) {
  if (ou.div) {
    // Ce qui est tapé dans la carte part d'abord : les contrats se lisent sur la saisie enregistrée.
    clearTimeout(MINUTEUR_SAISIE);
    await enregistrerSaisie(true);
    const carte = carteDe(ou.div);
    return { salarie: carte.salarie, carte, apres: ou.apres || rafraichirImprevus };
  }
  return { salarie: ou.salarie, carte: null, apres: ou.apres || rafraichirImprevus };
}

/* -------------------------------------------------- un arrêt de travail
   Toujours : le dernier jour travaillé, la fin prescrite, le motif, la
   subrogation. Le reste n'apparaît que si un fait l'appelle :
     - la date de l'accident, si le motif en porte une (04, 05, 06 : le
       moteur le dit, `date_accident` du motif) ;
     - le jour où la compagnie l'a su, si le moteur fait naître la
       déclaration d'accident (`arret-obligations`) : c'est le point de départ
       des quarante-huit heures ;
     - les dates de la subrogation, si elle est dite « oui » ;
     - une note, si on la demande.
   Après l'enregistrement, ce que l'arrêt écrit appelle encore (`etapeSuite`). */
async function boiteArret(ou) {
  const p = await provenance(ou);
  let n;
  try { n = await nomenclaturesImprevus(); } catch (e) { dire(e.message, "erreur"); return; }
  const motifs = n.motifs_arret || [];
  const sub = n.subrogation || [];
  ouvrirBoite(`
    <h3>Un arrêt de travail — ${echapper(nomDuSalarie(p.salarie))}</h3>
    <div class="champs-deux">
      <label class="champ-boite">Dernier jour travaillé
        <input type="date" id="ar-dernier"></label>
      <label class="champ-boite">Arrêt prescrit jusqu'au
        <input type="date" id="ar-fin"></label>
    </div>
    <div id="ar-contrats" class="note"></div>
    <label class="champ-boite">Motif
      <select id="ar-motif"><option value="">— à choisir —</option>
        ${motifs.map((m) => `<option value="${echapper(m.code)}"${m.date_accident ? ' data-accident="1"' : ""}>${
          echapper(m.libelle.charAt(0).toUpperCase() + m.libelle.slice(1))}</option>`).join("")}
      </select></label>
    <div id="ar-bloc-accident" hidden>
      <label class="champ-boite">Date de l'accident, ou de la première constatation
        <input type="date" id="ar-accident"></label>
      <div class="champs-deux" id="ar-bloc-connu" hidden>
        <label class="champ-boite">La compagnie l'a su le
          <input type="date" id="ar-connu"></label>
        <label class="champ-boite">à <span class="discret">— si on le sait</span>
          <input type="time" id="ar-connu-heure"></label>
      </div>
    </div>
    <div class="champ-boite">La compagnie perçoit-elle les indemnités journalières à la place du salarié (subrogation) ?
      <div class="oui-non">${sub.map((s) => `<label class="coche"><input type="radio" name="ar-sub" value="${
        echapper(s.code)}"> ${echapper(s.libelle)}</label>`).join("")}</div></div>
    <div class="champs-deux" id="ar-bloc-subrogation" hidden>
      <label class="champ-boite">Subrogation du <input type="date" id="ar-sub-debut"></label>
      <label class="champ-boite">au <input type="date" id="ar-sub-fin"></label>
    </div>
    <button type="button" class="lien" id="ar-note-btn">+ une note</button>
    <label class="champ-boite" id="ar-note-l" hidden>Note <span class="discret">— pour vous, ne part nulle part</span>
      <input type="text" id="ar-note"></label>
    <p class="note" id="ar-ensuite"></p>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="ar-ok">Enregistrer</button>
    </div>`);

  const val = (id) => document.getElementById(id).value;
  const motif = () => (motifs.find((m) => m.code === val("ar-motif")) || null);
  const subrogation = () => (document.querySelector('input[name="ar-sub"]:checked') || {}).value || "";
  let avecDeclaration = false;
  // CE QUE L'ARRÊT FERA NAÎTRE, demandé au moteur (`arret-obligations`, rien n'est écrit) : il décide si la connaissance
  // de l'accident se demande, et la page le dit en une ligne, sans la loi.
  const relireEnsuite = async () => {
    const ligne = document.getElementById("ar-ensuite");
    if (!val("ar-motif") || !val("ar-dernier")) { ligne.textContent = ""; avecDeclaration = false; majFaits(); return; }
    let r;
    try {
      r = await api("arret-obligations", {
        motif: val("ar-motif"), dernier_jour: val("ar-dernier"), fin: val("ar-fin"), subrogation: subrogation(),
        date_accident: val("ar-accident"), connaissance_accident: val("ar-connu"), connaissance_heure: val("ar-connu-heure"),
      });
    } catch (e) { ligne.textContent = ""; return; }
    const os = r.obligations || [];
    avecDeclaration = os.some((o) => o.cle === "declaration_accident");
    majFaits();
    ligne.innerHTML = os.length ? `Il faudra ensuite : ${os.map((o) => `${echapper(minuscule(o.libelle))}${
      o.etat === "a_la_charge_du_salarie" ? " (par le salarié)" : o.butoir ? ` (${echapper(butoirEnClair(o))})` : ""}`).join(" ; ")}.` : "";
  };
  const majFaits = () => {
    const m = motif();
    document.getElementById("ar-bloc-accident").hidden = !(m && m.date_accident);
    document.getElementById("ar-bloc-connu").hidden = !avecDeclaration;
    document.getElementById("ar-bloc-subrogation").hidden = subrogation() !== "01";
  };
  let trouve = null;
  const relireContrats = async () => {
    trouve = await contratsDuJour(p, val("ar-dernier"), document.getElementById("ar-contrats"));
  };
  document.getElementById("ar-motif").onchange = () => { majFaits(); relireEnsuite(); };
  document.querySelectorAll('input[name="ar-sub"]').forEach((x) => { x.onchange = () => { majFaits(); relireEnsuite(); }; });
  document.getElementById("ar-dernier").addEventListener("change", () => { relireContrats(); relireEnsuite(); });
  for (const id of ["ar-fin", "ar-accident", "ar-connu", "ar-connu-heure"]) {
    document.getElementById(id).addEventListener("change", relireEnsuite);
  }
  document.getElementById("ar-note-btn").onclick = () => {
    document.getElementById("ar-note-l").hidden = false;
    document.getElementById("ar-note-btn").hidden = true;
    document.getElementById("ar-note").focus();
  };
  majFaits();

  document.getElementById("ar-ok").onclick = async () => {
    // Ce qui part : les faits toujours dus, et ceux que les faits ont ouverts — rien d'autre.
    const m = motif();
    const charge = {
      salarie: p.salarie, dernier_jour: val("ar-dernier"), fin: val("ar-fin"),
      motif: val("ar-motif"), subrogation: subrogation(), ...contratAEnvoyer(trouve),
    };
    if (m && m.date_accident) charge.date_accident = val("ar-accident");
    if (charge.subrogation === "01") {
      charge.subrogation_debut = val("ar-sub-debut");
      charge.subrogation_fin = val("ar-sub-fin");
    }
    if (val("ar-note").trim()) charge.note = val("ar-note").trim();
    let r;
    try { r = await api("arret-enregistrer", charge); }
    catch (e) { dire(e.message, "erreur"); return; }
    // Le jour où la compagnie a su l'accident : le point de départ des 48 heures.
    if (avecDeclaration && val("ar-connu")) {
      try {
        await api("arret-connaissance", { salarie: p.salarie, dernier_jour: charge.dernier_jour,
          de: "accident", jour: val("ar-connu"), heure: val("ar-connu-heure") });
      } catch (e) { dire("Arrêt enregistré, mais pas le jour où l'accident a été su : " + e.message, "erreur"); }
    }
    await etapeSuite(r.arret, p, "Arrêt enregistré.");
  };
}

/* ------------------------------------------------ une absence (bloc 65)
   Ses deux dates et son motif — la liste que le moteur déclare
   (`motifs_suspension` : les congés payés n'y sont pas). Le contrat est
   celui de la carte. */
async function boiteAbsence(ou) {
  const p = await provenance(ou);
  let n;
  try { n = await nomenclaturesImprevus(); } catch (e) { dire(e.message, "erreur"); return; }
  const motifs = n.motifs_suspension || [];
  ouvrirBoite(`
    <h3>Une absence — ${echapper(nomDuSalarie(p.salarie))}</h3>
    <div class="champs-deux">
      <label class="champ-boite">Du <input type="date" id="ab-debut"></label>
      <label class="champ-boite">Au <input type="date" id="ab-fin"></label>
    </div>
    <div id="ab-contrats" class="note"></div>
    <label class="champ-boite">Motif
      <select id="ab-motif"><option value="">— à choisir —</option>
        ${motifs.map((m) => `<option value="${echapper(m.code)}">${echapper(m.libelle)}${
          m.remunere ? " (payé)" : ""}</option>`).join("")}
      </select></label>
    <button type="button" class="lien" id="ab-note-btn">+ une note</button>
    <label class="champ-boite" id="ab-note-l" hidden>Note <span class="discret">— pour vous, ne part nulle part</span>
      <input type="text" id="ab-note"></label>
    <p class="note">Saisie une fois : chaque mois qu'elle traverse la déclare et, si elle n'est pas payée, en retire le salaire.</p>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="ab-ok">Enregistrer</button>
    </div>`);
  const val = (id) => document.getElementById(id).value;
  let trouve = null;
  document.getElementById("ab-debut").addEventListener("change", async () => {
    trouve = await contratsDuJour(p, val("ab-debut"), document.getElementById("ab-contrats"));
  });
  document.getElementById("ab-note-btn").onclick = () => {
    document.getElementById("ab-note-l").hidden = false;
    document.getElementById("ab-note-btn").hidden = true;
    document.getElementById("ab-note").focus();
  };
  document.getElementById("ab-ok").onclick = async () => {
    const charge = { salarie: p.salarie, debut: val("ab-debut"), fin: val("ab-fin"), motif: val("ab-motif"),
                     ...contratAEnvoyer(trouve) };
    if (val("ab-note").trim()) charge.note = val("ab-note").trim();
    let r;
    try { r = await api("suspension-enregistrer", charge); }
    catch (e) { dire(e.message, "erreur"); return; }
    await etapeSuite(r.suspension, p, "Absence enregistrée. Chaque mois qu'elle traverse la déclarera.");
  };
}

/* ------------------------------------- ce que l'imprévu écrit appelle encore
   Le moteur le dit sur l'imprévu qu'il vient d'écrire : les jours déjà
   indemnisés (`jours_deja_indemnises_attendus` : un permanent arrêté pour
   maladie ou accident, D1226-4), le maintien du mois (`maintien_ouvert` :
   un contrat payé au mois), et les jours que la saisie paie pendant
   l'imprévu (`cachets`). Rien de tout ça : la boîte se ferme. Sinon, une
   seconde page, où rien n'est obligatoire — le maintien ne se connaît
   souvent qu'à la paie, et il se reprend sous la carte. */
async function etapeSuite(i, p, phrase) {
  const maintien = i.maintien_ouvert && MOIS_AFFICHE && (i.mois_du_maintien || []).includes(MOIS_AFFICHE);
  const jours = (i.cachets || []);
  if (!i.jours_deja_indemnises_attendus && !maintien && !jours.length) {
    fermerBoite();
    dire(phrase);
    await p.apres();
    return;
  }
  const div = p.carte ? p.carte.div : null;
  const ici = (c) => div && c.mois === MOIS_AFFICHE && !div.classList.contains("validee");
  ouvrirBoite(`
    <h3>${echapper(phrase)} Ce qu'il appelle encore</h3>
    ${i.jours_deja_indemnises_attendus ? `<label class="champ-boite">Jours d'arrêt déjà indemnisés dans les douze mois
      <span class="discret">— ils réduisent la durée du maintien</span>
      <input type="number" step="1" min="0" id="su-deja" value="${echapper(i.jours_deja_indemnises ?? "")}"></label>` : ""}
    ${maintien ? `<label class="champ-boite">Maintien de salaire ${echapper(deMois(MOIS_AFFICHE))}
      <span class="discret">— le complément versé après les indemnités journalières ; à dire quand on le connaît</span>
      <input type="number" step="0.01" min="0" id="su-maintien"></label>` : ""}
    ${jours.length ? `<div class="champ-boite">Des jours sont payés pendant ${i.genre === "arret" ? "l'arrêt" : "l'absence"}.
      Cocher ceux à retirer de l'engagement :
      <div class="liste-jours">${jours.map((c, k) => c.dates.map((iso) => ici(c)
        ? `<label class="coche"><input type="checkbox" data-jour="${k}" value="${echapper(iso)}">
             ${echapper(c.libelle)} le ${echapper(jjmmaaaa(iso))}</label>`
        : `<div class="discret">${echapper(c.libelle)} le ${echapper(jjmmaaaa(iso))} — à retirer dans la saisie ${
             echapper(deMois(c.mois))}${c.mois === MOIS_AFFICHE ? " (l'engagement est validé : le modifier d'abord)" : ""}</div>`).join("")).join("")}
      </div></div>` : ""}
    <div class="actions">
      <button class="secondaire" id="su-plus-tard">Plus tard</button>
      <button class="primaire" id="su-ok">Enregistrer</button>
    </div>`);
  const finir = async () => { fermerBoite(); await p.apres(); };
  document.getElementById("su-plus-tard").onclick = finir;
  document.getElementById("su-ok").onclick = async () => {
    const deja = document.getElementById("su-deja");
    const montant = document.getElementById("su-maintien");
    const charge = { salarie: i.salarie, dernier_jour: i.dernier_jour_travaille };
    if (deja && deja.value !== "") charge.jours_deja_indemnises = deja.value;
    if (montant && montant.value !== "") { charge.mois = MOIS_AFFICHE; charge.montant = montant.value; }
    if (Object.keys(charge).length > 2) {
      try { await api("arret-maintien", charge); }
      catch (e) { dire(e.message, "erreur"); return; }
    }
    let retires = 0;
    document.querySelectorAll("#boite [data-jour]:checked").forEach((x) => {
      const c = jours[+x.dataset.jour];
      if (retirerJour(div, c.rubrique, x.value)) retires++;
    });
    if (retires) dire(`${retires} jour${retires > 1 ? "s" : ""} retiré${retires > 1 ? "s" : ""} de l'engagement. La saisie s'enregistre.`);
    await finir();
  };
}

/* ------------------------------------------------ reprendre, maintenir, finir */

/** La reprise plus tôt que prévu. Elle ne se signale que si elle est ANTICIPÉE : le moteur fait alors naître le
 *  signalement de reprise, qui paraît dans ce qui est dû. Une reprise déjà saisie se corrige ici ; vide, elle s'efface. */
async function boiteReprise(a, apres) {
  let n;
  try { n = await nomenclaturesImprevus(); } catch (e) { dire(e.message, "erreur"); return; }
  const motifs = n.motifs_reprise || [];
  ouvrirBoite(`
    <h3>${a.reprise ? "La reprise" : "Le salarié a repris plus tôt"} — ${echapper(nomDuSalarie(a.salarie))}</h3>
    <p class="discret">${echapper(quoiDeLImprevu(a))}, ${echapper(periodeDeLImprevu(a))}.</p>
    <label class="champ-boite">Premier jour retravaillé
      <input type="date" id="re-date" value="${echapper(a.reprise || "")}"></label>
    <label class="champ-boite">Comment
      <select id="re-motif">${motifs.map((m) => `<option value="${echapper(m.code)}"${
        m.code === a.reprise_motif ? " selected" : ""}>${echapper(m.libelle)}</option>`).join("")}</select></label>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="re-ok">Enregistrer</button>
    </div>`);
  document.getElementById("re-ok").onclick = async () => {
    try {
      await api("arret-reprise", { salarie: a.salarie, dernier_jour: a.dernier_jour_travaille,
        reprise: document.getElementById("re-date").value, motif_reprise: document.getElementById("re-motif").value });
    } catch (e) { dire(e.message, "erreur"); return; }
    fermerBoite();
    dire("Reprise notée.");
    await apres();
  };
}

/** L'avis de prolongation : la nouvelle fin prescrite, et rien d'autre. Le moteur met l'arrêt à jour et dit ce qui en
 *  découle ; s'il dit que la subrogation s'arrête désormais avant l'arrêt, sa fin se demande ensuite — et seulement
 *  alors (`subrogation_a_revoir`). */
function boiteProlonger(a, apres) {
  ouvrirBoite(`
    <h3>Prolonger l'arrêt — ${echapper(nomDuSalarie(a.salarie))}</h3>
    <p class="discret">${echapper(quoiDeLImprevu(a))}, ${echapper(periodeDeLImprevu(a))}.</p>
    <label class="champ-boite">Prolongé jusqu'au (avis de prolongation)
      <input type="date" id="pr-fin" value=""></label>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="pr-ok">Prolonger</button>
    </div>`);
  document.getElementById("pr-ok").onclick = async () => {
    let r;
    try {
      r = await api("arret-prolonger", { salarie: a.salarie, dernier_jour: a.dernier_jour_travaille,
        fin: document.getElementById("pr-fin").value });
    } catch (e) { dire(e.message, "erreur"); return; }
    dire((r.avertissements || []).join("  ·  ") || "Arrêt prolongé.");
    if (r.subrogation_a_revoir) { boiteSubrogationProlongee(r.arret, r.phrase, apres); return; }
    fermerBoite();
    await apres();
  };
}

/** La fin de la subrogation d'un arrêt prolongé, quand le moteur la demande : il ne connaît pas le maximum que la
 *  convention permet. La garder ne change rien. */
function boiteSubrogationProlongee(a, phrase, apres) {
  ouvrirBoite(`
    <h3>La subrogation — ${echapper(nomDuSalarie(a.salarie))}</h3>
    <p>${echapper(phrase || "")}</p>
    <label class="champ-boite">Subrogation jusqu'au
      <input type="date" id="ps-fin" value="${echapper(a.subrogation_fin || "")}"></label>
    <div class="actions">
      <button class="secondaire" id="ps-garder">Garder le ${echapper(jjmmaaaa(a.subrogation_fin))}</button>
      <button class="primaire" id="ps-ok">Enregistrer</button>
    </div>`);
  document.getElementById("ps-garder").onclick = async () => { fermerBoite(); await apres(); };
  document.getElementById("ps-ok").onclick = async () => {
    let r;
    try {
      r = await api("arret-prolonger", { salarie: a.salarie, dernier_jour: a.dernier_jour_travaille,
        subrogation_fin: document.getElementById("ps-fin").value });
    } catch (e) { dire(e.message, "erreur"); return; }
    fermerBoite();
    dire((r.avertissements || []).join("  ·  ") || "Subrogation notée.");
    await apres();
  };
}

/** Le maintien de salaire d'un mois que l'arrêt traverse, et — seulement si le moteur les attend — les jours déjà
 *  indemnisés (D1226-4). */
function boiteMaintien(a, mois, apres) {
  ouvrirBoite(`
    <h3>Le maintien de salaire ${echapper(deMois(mois))} — ${echapper(nomDuSalarie(a.salarie))}</h3>
    <p class="note">Le complément versé au titre de l'arrêt, une fois déduites les indemnités journalières et la
      prévoyance : il ne se devine pas. Vide, il n'y en a pas.</p>
    <label class="champ-boite">Montant
      <input type="number" step="0.01" min="0" id="ma-montant" value="${echapper((a.maintien || {})[mois] ?? "")}"></label>
    ${a.jours_deja_indemnises_attendus ? `<label class="champ-boite">Jours d'arrêt déjà indemnisés dans les douze mois
      <input type="number" step="1" min="0" id="ma-deja" value="${echapper(a.jours_deja_indemnises ?? "")}"></label>` : ""}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="ma-ok">Enregistrer</button>
    </div>`);
  document.getElementById("ma-ok").onclick = async () => {
    const deja = document.getElementById("ma-deja");
    try {
      await api("arret-maintien", { salarie: a.salarie, dernier_jour: a.dernier_jour_travaille, mois,
        montant: document.getElementById("ma-montant").value, ...(deja ? { jours_deja_indemnises: deja.value } : {}) });
    } catch (e) { dire(e.message, "erreur"); return; }
    fermerBoite();
    dire("Maintien noté : la paie du mois le portera.");
    await apres();
  };
}

function boiteFinAbsence(s, apres) {
  ouvrirBoite(`
    <h3>${echapper(s.motif_libelle)} — ${echapper(nomDuSalarie(s.salarie))}</h3>
    <p class="discret">Depuis le ${echapper(jjmmaaaa(s.debut))}.</p>
    <label class="champ-boite">Jusqu'au <input type="date" id="af-fin" value="${echapper(s.fin || "")}"></label>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="af-ok">Enregistrer</button>
    </div>`);
  document.getElementById("af-ok").onclick = async () => {
    try { await api("suspension-modifier", { salarie: s.salarie, debut: s.debut, fin: document.getElementById("af-fin").value }); }
    catch (e) { dire(e.message, "erreur"); return; }
    fermerBoite();
    dire("Fin de l'absence changée.");
    await apres();
  };
}

/* ------------------------------------------------------ les actes dus */

/** Écrire le signalement (04 : l'arrêt ; 05 : la reprise anticipée), puis le déposer. Deux gestes et deux boîtes :
 *  écrire un fichier n'engage rien, le déposer engage tout. */
function boiteSignalerArret(a, nature) {
  const deja = (a.signalements || []).filter((s) => s.nature === nature).length;
  const reprise = nature === "05";
  ouvrirBoite(`
    <h3>Signaler ${reprise ? "la reprise" : "l'arrêt"} — ${echapper(nomDuSalarie(a.salarie))}</h3>
    <p class="discret">${echapper(quoiDeLImprevu(a))}, ${echapper(periodeDeLImprevu(a))}${
      reprise ? `, reprise le ${echapper(jjmmaaaa(a.reprise))}` : ""}.</p>
    <p class="note">Le logiciel écrit le fichier du signalement ; le dépôt, sur net-entreprises, est l'étape suivante.
      La DSN du mois le répétera : l'une ne remplace pas l'autre.</p>
    ${deja ? `<div class="encart attention"><b>Déjà signalé ${deja} fois</b>
      Un nouveau signalement ne remplace rien : pour corriger ce qui est parti, un annule-et-remplace vise
      l'identifiant rendu par le point de dépôt (sur le certificat de conformité).
      <label class="champ-boite">Identifiant de la déclaration remplacée
        <input type="text" id="si-remplace" placeholder="vide pour un signalement normal"></label></div>` : ""}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="si-ok">Écrire le signalement</button>
    </div>`);
  document.getElementById("si-ok").onclick = async () => {
    const remplace = document.getElementById("si-remplace");
    // Un clic de plus, pendant l'écriture, ne fait pas un second signalement.
    const rendre = occupe(document.getElementById("si-ok"), "Écriture du signalement…");
    let r;
    try {
      r = await api("arret-signaler", { salarie: a.salarie, dernier_jour: a.dernier_jour_travaille, nature,
                                        remplace: remplace ? remplace.value.trim() : "" });
    } catch (e) { rendre(); dire(e.message, "erreur"); return; }
    fermerBoite();
    // Les avis du signalement suivent le message, sur la même ligne.
    dire(["Signalement écrit : " + r.nom + ". Rien n'est parti."].concat(r.avertissements || []).join("  ·  "));
    await rafraichirImprevus();
    boiteDepot("dsn", r.fichier);
  };
}

/** Le jour (et l'heure) où la compagnie a su l'arrêt ou l'accident : le point de départ du délai. Sans lui, le moteur
 *  présume — le plus tôt possible —, et le dit sous « pourquoi ». */
function boiteConnaissance(a, de, o) {
  const depart = (o && o.depart) || {};
  ouvrirBoite(`
    <h3>Quand la compagnie a-t-elle su ${de === "accident" ? "l'accident" : "l'arrêt"} ?</h3>
    <p class="discret">${echapper(nomDuSalarie(a.salarie))} — ${echapper(quoiDeLImprevu(a))}.</p>
    ${depart.presume ? `<p class="note">Sans réponse, le délai court du ${echapper(jjmmaaaa(depart.jour))}, le plus tôt possible.</p>` : ""}
    <div class="champs-deux">
      <label class="champ-boite">Le <input type="date" id="co-jour" value="${echapper(depart.saisi ? depart.jour : "")}"></label>
      <label class="champ-boite">à <span class="discret">— si on le sait</span>
        <input type="time" id="co-heure" value="${echapper(depart.saisi && depart.heure ? depart.heure : "")}"></label>
    </div>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="co-ok">Enregistrer</button>
    </div>`);
  document.getElementById("co-ok").onclick = async () => {
    try {
      await api("arret-connaissance", { salarie: a.salarie, dernier_jour: a.dernier_jour_travaille, de,
        jour: document.getElementById("co-jour").value, heure: document.getElementById("co-heure").value });
    } catch (e) { dire(e.message, "erreur"); return; }
    fermerBoite();
    await rafraichirImprevus();
  };
}

/** Une obligation qui se fait HORS DSN (la déclaration d'accident du travail, par exemple) : le logiciel ne la fait
 *  pas. Il dit où la faire, ce qu'il faut avoir sous la main, et retient qu'elle est faite quand on le lui dit — le
 *  jour, où, la référence —, par la route de `ROUTES_OBLIGATION_FAITE`. Tout vient des champs de l'obligation. */
function boiteObligationFaite(a, o) {
  const fait = o.fait || {};
  const route = ROUTES_OBLIGATION_FAITE[o.cle];
  ouvrirBoite(`
    <h3>${echapper(o.libelle)} — ${echapper(nomDuSalarie(a.salarie))}</h3>
    <p class="discret">${echapper(butoirEnClair(o).replace(/^./, (c) => c.toUpperCase()))}${
      o.destinataire ? ` — ${echapper(o.destinataire.replace(/\.$/, ""))}` : ""}. Elle ne part pas avec la DSN.</p>
    ${(o.ou || []).map((x) => `<p class="note">${echapper(x.libelle)}</p>`).join("")}
    ${(o.a_avoir || []).length ? `<details class="pourquoi"><summary>ce qu'il faut avoir sous la main</summary>
      <ul class="note">${o.a_avoir.map((x) => `<li>${echapper(x)}</li>`).join("")}</ul></details>` : ""}
    <div class="champs-deux">
      <label class="champ-boite">Faite le <input type="date" id="da-le" value="${echapper(fait.le || "")}"></label>
      <label class="champ-boite">Où
        <select id="da-depot">${(o.ou || []).map((x) => `<option value="${echapper(x.code)}"${
          fait.depot === x.code ? " selected" : ""}>${echapper(x.court || x.code)}</option>`).join("")}</select></label>
    </div>
    <label class="champ-boite">Référence <span class="discret">— l'accusé de réception, le numéro du recommandé, s'il y en a un</span>
      <input type="text" id="da-reference" value="${echapper(fait.reference || "")}"></label>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="da-ok">Enregistrer</button>
    </div>`);
  document.getElementById("da-ok").onclick = async () => {
    try {
      await api(route, { salarie: a.salarie, dernier_jour: a.dernier_jour_travaille,
        le: document.getElementById("da-le").value, depot: document.getElementById("da-depot").value,
        reference: document.getElementById("da-reference").value.trim() });
    } catch (e) { dire(e.message, "erreur"); return; }
    fermerBoite();
    dire(`${o.libelle} : noté.`);
    await rafraichirImprevus();
  };
}

/** Écrire le signalement de fin de contrat, puis le déposer. */
function boiteSignalerFin(mois, f) {
  ouvrirBoite(`
    <h3>Signaler la fin du contrat — ${echapper(nomDuSalarie(f.salarie))}</h3>
    <p class="discret">${echapper(f.nature)} — contrat ${echapper(f.numero || "?")}, fin le ${echapper(jjmmaaaa(f.fin))}.</p>
    <p class="note">Il va à France Travail, qui en tire l'attestation employeur : c'est elle qui ouvre les droits du
      salarié. La DSN du mois ne le remplace pas.</p>
    <details class="pourquoi"><summary>remplacer un signalement déjà parti</summary>
      <label class="champ-boite">Identifiant de la déclaration remplacée
        <span class="discret">— JJMMAAAA suivi du rang de l'envoi, lu sur son certificat de conformité</span>
        <input type="text" id="fi-remplace" placeholder="vide pour un signalement normal"></label>
    </details>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="fi-ok">Écrire le signalement</button>
    </div>`);
  document.getElementById("fi-ok").onclick = async () => {
    const rendre = occupe(document.getElementById("fi-ok"), "Écriture du signalement…");
    let r;
    try {
      r = await api("fin-contrat-signaler", { mois, salarie: f.salarie,
                                              remplace: document.getElementById("fi-remplace").value.trim() });
    } catch (e) { rendre(); dire(e.message, "erreur"); return; }
    fermerBoite();
    dire(["Signalement écrit : " + r.nom + " (préavis : " + r.preavis + "). Rien n'est parti."]
      .concat(r.avertissements || []).join("  ·  "));
    await rafraichirImprevus();
    boiteDepot("dsn", r.fichier);
  };
}

/** Les deux papiers dus au salarié qui part. Ils ne partent nulle part : ils se REMETTENT, et le reçu se signe à la main. */
async function boitePapiersDeFin(mois, salarie) {
  let r;
  try { r = await api("fin-contrat-documents", { mois, salarie }); }
  catch (e) { dire(e.message, "erreur"); return; }
  ouvrirBoite(`
    <h3>Le certificat de travail et le reçu pour solde de tout compte</h3>
    <p class="discret">Écrits pour ${echapper(nomDuSalarie(salarie))} : ${r.noms.map((x) => echapper(x)).join(", ")}.</p>
    ${(r.avertissements || []).map((m) => `<div class="encart attention">${echapper(m)}</div>`).join("")}
    <div class="actions">
      <button class="secondaire" id="fp-dossier">Ouvrir le dossier</button>
      <button class="primaire" onclick="fermerBoite()">Fermer</button>
    </div>`);
  document.getElementById("fp-dossier").onclick = async () => {
    try {
      const d = await api("ouvrir-dossier", { mois });
      if (d && d.phrase) dire(d.phrase);
    } catch (e) { dire(e.message, "erreur"); }
  };
}

/* ------------------------------------------ LE DOSSIER DU SALARIÉ
   Dans sa fiche (Configuration › Salariés) : ses imprévus, passés et en
   cours, ce qui en est parti (les signalements, les DSN qui les ont
   portés, la déclaration d'accident dite faite), ce qui est encore dû, et
   la même porte. Rien quand il n'y en a jamais eu, sauf la porte. */
async function dessinerImprevusDuSalarie(cle) {
  const cible = document.getElementById("fiche-imprevus");
  if (!cible || !cle) return;
  let r;
  try { r = await api("imprevus", { salarie: cle }); }
  catch (e) { cible.innerHTML = ""; return; }
  NOMENCLATURES_IMPREVUS = NOMENCLATURES_IMPREVUS || r;
  if (CLE_ACTIVE !== cle) return;      // une autre fiche s'est ouverte entre-temps
  const liste = (r.imprevus || []).slice().sort((x, y) => y.premier_jour.localeCompare(x.premier_jour));
  const apres = async () => { await dessinerImprevusDuSalarie(cle); if (MOIS_AFFICHE) await dessinerImprevus(MOIS_AFFICHE); };
  cible.innerHTML = `
    <div class="fiche-imprevus-tete">
      ${liste.length ? `<h3>Ses imprévus</h3>` : ""}
      <button type="button" class="lien" id="fiche-imprevu-porte">Un imprévu…</button>
    </div>
    ${liste.map((i, k) => `<div class="imprevu" data-imprevu="${k}">
        <span class="imprevu-quoi"><b>${echapper(quoiDeLImprevu(i))}</b>
          <span class="discret">${echapper(periodeDeLImprevu(i))}${i.contrat ? ` · contrat ${echapper(i.contrat)}` : ""}</span></span>
        <span class="imprevu-gestes">${gestesImprevu(i, null)}</span>
        <div class="imprevu-parti">${ceQuiEnEstParti(i)}</div>
      </div>`).join("")}`;
  liste.forEach((i, k) => brancherGestesImprevu(cible.querySelector(`[data-imprevu="${k}"]`), i, null, apres));
  const porte = document.getElementById("fiche-imprevu-porte");
  porte.onclick = (ev) => { ev.stopPropagation(); menuImprevu(porte, { salarie: cle, apres }); };
}

/** Ce qui est parti d'un imprévu, et ce qui en reste dû : une ligne courte. */
function ceQuiEnEstParti(i) {
  const morceaux = [];
  (i.prolongations || []).filter((p) => p.fin !== p.fin_avant).forEach((p) => morceaux.push(`prolongé jusqu'au ${
    echapper(jjmmaaaa(p.fin))} (avis saisi le ${echapper(jjmmaaaa(p.le))})`));
  (i.signalements || []).forEach((s) => morceaux.push(`${s.nature === "05" ? "reprise signalée" : s.type === "03" ? "signalement rectifié" : "signalé"} le ${
    echapper(quand(s.quand || ""))}`));
  if ((i.reporte_par || []).length) morceaux.push(`déclaré par la DSN ${i.reporte_par.map((m) => echapper(deMois(m))).join(", ")}`);
  (i.obligations || []).forEach((o) => {
    if (o.fait && o.fait.le && Array.isArray(o.ou)) morceaux.push(`${echapper(minuscule(o.libelle))} : faite le ${echapper(jjmmaaaa(o.fait.le))}`);
    else if (o.du) morceaux.push(`${pastilleDu(o)} ${echapper(minuscule(o.libelle))}, ${echapper(butoirEnClair(o))}`);
  });
  return morceaux.length ? morceaux.join(" · ") : `<span class="discret">rien n'est encore parti</span>`;
}
