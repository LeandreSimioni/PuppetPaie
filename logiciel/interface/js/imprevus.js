/* Les imprévus : un arrêt de travail, une absence, ce qu'ils font naître, et la fin d'un contrat qui se signale. */
"use strict";

/* ------------------------------------------------ LES IMPRÉVUS
   Simple pour qui a besoin de simple, compliqué pour qui a besoin de compliqué.
   Quelqu'un qui fait des paies
   apprend qu'un salarié est malade : il va sur la ligne de ce salarié, pas
   dans une section du mois qui dirait « Arrêts de travail » tous les mois
   pour rien. Les imprévus sont rares, et rien ici ne doit laisser croire
   qu'il y en a tout le temps.

   LA FORME, EN TROIS ENDROITS, ET PAS UN DE PLUS :

   - LA PORTE est sur la carte de l'engagement : « Imprévu… » (et « … »
     sur un engagement validé). Elle propose un arrêt de travail, une absence
     (congé parental, sans solde, sabbatique… : une suspension du contrat,
     bloc 65), et, quand la carte sait déjà dire une fin, la rupture avant
     terme — qui n'est pas une boîte de plus : elle ouvre le « Contrat » de
     la carte, où la fin se saisit depuis toujours. Le salarié et le contrat
     sont ceux de la carte : on ne les redemande pas. Sous la carte, une ligne
     par imprévu qui la touche ce mois-ci, avec ses gestes (la reprise, les
     indemnités journalières, retirer) et les jours payés pendant l'imprévu, à
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
   (`imprevus` : `ij_attendues`, `jours_deja_indemnises_attendus`,
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
      ${o.delai_dit ? `<p>${echapper(o.delai_dit)}</p>` : ""}
      ${depart.presume ? `<p class="discret"><b>Point de départ présumé</b> ${echapper(depart.presume)}</p>` : ""}
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

function rangObligation(a, o, k) {
  const gestes = [];
  if (SIGNALEMENTS_DSN[o.cle]) {
    gestes.push(`<button class="secondaire petit" data-geste="signaler">Signaler…</button>`);
  }
  if (Array.isArray(o.ou) && ROUTES_OBLIGATION_FAITE[o.cle]) {
    gestes.push(`<button class="secondaire petit" data-geste="faite">Faite le</button>`);
  }
  // Le jour où la compagnie l'a su se dit là où le délai en court : le moteur le dit (`connaissance`).
  if (o.connaissance) {
    gestes.push(`<button class="lien" data-geste="connu">Connu le</button>`);
  }
  return rangDu(k, pastilleDu(o), o.libelle, nomDuSalarie(a.salarie), (o.butoir_dit || ""), gestes.join(""), "", pourquoiDu(o));
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
  if (b("connu")) b("connu").onclick = () => boiteConnaissance(a, o.connaissance.de, o);
}

/* LA FIN D'UN CONTRAT QUI SE SIGNALE (nature 07), sous la même
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
  return rangDu(k, pastilleDeLEtat(f.pastille), (f.obligation && f.obligation.libelle) || "Fin de contrat",
    f.qui_dit, ((f.obligation || {}).butoir_dit || ""),
    `<button class="secondaire petit" data-geste="signaler"${manque ? " disabled" : ""}>Signaler…</button>
     <button class="lien" data-geste="papiers">Papiers de fin</button>`,
    manque ? `<div class="du-manque"><p>${aideDe("a_dire_avant_signaler")}</p>
      ${f.a_saisir.map((m, i) => `<p><b>${echapper(m.case)}</b> : ${echapper(m.dire)}
        <button class="lien" data-case="${i}">La dire</button></p>`).join("")}</div>` : "",
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
  if (!div) return;
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
   parle). « Imprévu… » au pied d'une carte ouverte, « … » sur un engagement
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
      <span class="discret">maladie, accident, maternité</span></button>
    <button type="button" data-choix="absence">Une absence
      <span class="discret">congé parental, sans solde</span></button>
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
        <span class="imprevu-quoi"><b>${echapper(i.quoi)}</b>
          <span class="discret">${echapper(i.periode)}</span></span>
        <span class="imprevu-gestes">${gestesImprevu(i, MOIS_AFFICHE)}</span>
        ${jours.length ? `<div class="imprevu-jours">${echapper(i.paye_pendant)}
          ${jours.map((c) => (c.jours || []).map((j) => `${echapper(j.phrase)}
            ${validee ? "" : `<button class="lien" data-retirer="${echapper(j.iso)}" data-rubrique="${echapper(c.rubrique)}">retirer ce jour</button>`}`).join(" · ")).join(" · ")}
          ${validee ? `<span class="discret">— ${aideDe("imprevu_valide")}</span>` : ""}
        </div>` : ""}
      </div>`;
  }).join("");
  liste.forEach((i, k) => {
    const r = cible.querySelector(`[data-imprevu="${k}"]`);
    brancherGestesImprevu(r, i, MOIS_AFFICHE, rafraichirImprevus);
    r.querySelectorAll("[data-retirer]").forEach((b) => {
      b.onclick = async () => {
        if (!retirerJour(div, b.dataset.rubrique, b.dataset.retirer)) {
          dire((ETAT.aides || {}).jour_hors_ligne, "erreur");
        } else {
          await avantDeQuitter();
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
    if (i.peut_prolonger) g.push(`<button class="lien" data-geste="prolonger">Prolonger l'arrêt</button>`);
    if (i.peut_reprendre_plus_tot) g.push(`<button class="lien" data-geste="reprise">Reprise plus tôt</button>`);
    if (i.reprise_corrigeable) g.push(`<button class="lien" data-geste="reprise">Corriger la reprise</button>`);
    if (i.ij_attendues) {
      g.push(`<button class="lien" data-geste="indemnites">Indemnités journalières${i.ij_brute != null ? "" : "…"}</button>`);
    }
    if (!i.signale && !(i.reporte_par || []).length) g.push(`<button class="lien" data-geste="retirer">Retirer</button>`);
  } else {
    g.push(`<button class="lien" data-geste="fin">Changer la fin</button>`);
    if (!(i.reporte_par || []).length) g.push(`<button class="lien" data-geste="retirer">Retirer</button>`);
  }
  return g.join("");
}

function brancherGestesImprevu(r, i, mois, apres) {
  const b = (g) => r.querySelector(`.imprevu-gestes [data-geste="${g}"]`);
  if (b("reprise")) b("reprise").onclick = () => boiteReprise(i, apres);
  if (b("prolonger")) b("prolonger").onclick = () => boiteProlonger(i, apres);
  if (b("indemnites")) b("indemnites").onclick = () => boiteIndemnites(i, apres);
  if (b("fin")) b("fin").onclick = () => boiteFinAbsence(i, apres);
  if (b("retirer")) b("retirer").onclick = async () => {
    try {
      const r = i.genre === "arret"
        ? await api("arret-oublier", { salarie: i.salarie, dernier_jour: i.dernier_jour_travaille })
        : await api("suspension-oublier", { salarie: i.salarie, debut: i.debut });
      dire(r.phrase);
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
  let r;
  try { r = await api("arret-contrats", { salarie: ou.salarie, jour }); }
  catch (e) { cible.innerHTML = `<span class="erreur">${echapper(e.message)}</span>`; return null; }
  const contrats = r.contrats || [];
  if (!contrats.length) {
    cible.innerHTML = `<span class="erreur">${echapper(r.phrase)}</span>`;
    return null;
  }
  const carte = ou.carte;
  const sien = carte && contrats.find((c) => (carte.numero && c.numero === carte.numero) || c.debut === carte.debut);
  if (contrats.length === 1) return contrats[0];
  if (sien && sien.numero) return sien;
  cible.innerHTML = `Quel contrat
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
   Toujours : le dernier jour travaillé, la fin prescrite, le motif (la
   subrogation se tire du droit, dans le moteur). Le reste n'apparaît que si un fait l'appelle :
     - la date de l'accident, si le motif en porte une (04, 05, 06 : le
       moteur le dit, `date_accident` du motif) ;
     - le jour où la compagnie l'a su, si le moteur fait naître la
       déclaration d'accident (`arret-obligations`) : c'est le point de départ
       des quarante-huit heures ;
     - une note, si on la demande.
   Après l'enregistrement, ce que l'arrêt écrit appelle encore (`etapeSuite`). */
async function boiteArret(ou) {
  const p = await provenance(ou);
  let n;
  try { n = await nomenclaturesImprevus(); } catch (e) { dire(e.message, "erreur"); return; }
  const motifs = n.motifs_arret || [];
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
      <label class="champ-boite" title="${aideDe("date_accident")}">Date de l'accident
        <input type="date" id="ar-accident"></label>
      <div class="champs-deux" id="ar-bloc-connu" hidden>
        <label class="champ-boite">Connu le
          <input type="date" id="ar-connu"></label>
        <label class="champ-boite">à <span class="discret">— si on le sait</span>
          <input type="time" id="ar-connu-heure"></label>
      </div>
    </div>
    <button type="button" class="lien" id="ar-note-btn">+ une note</button>
    <label class="champ-boite" id="ar-note-l" hidden>Note <span class="discret">— ${aideDe("note_imprevu")}</span>
      <input type="text" id="ar-note"></label>
    <p class="note" id="ar-ensuite"></p>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="ar-ok">Enregistrer</button>
    </div>`);

  const val = (id) => document.getElementById(id).value;
  const motif = () => (motifs.find((m) => m.code === val("ar-motif")) || null);
  let avecDeclaration = false;
  // CE QUE L'ARRÊT FERA NAÎTRE, demandé au moteur (`arret-obligations`, rien n'est écrit) : il décide si la connaissance
  // de l'accident se demande, et la page le dit en une ligne, sans la loi.
  const relireEnsuite = async () => {
    const ligne = document.getElementById("ar-ensuite");
    if (!val("ar-motif") || !val("ar-dernier")) { ligne.textContent = ""; avecDeclaration = false; majFaits(); return; }
    let r;
    try {
      r = await api("arret-obligations", {
        motif: val("ar-motif"), dernier_jour: val("ar-dernier"), fin: val("ar-fin"), salarie: p.salarie, ...contratAEnvoyer(trouve),
        date_accident: val("ar-accident"), connaissance_accident: val("ar-connu"), connaissance_heure: val("ar-connu-heure"),
      });
    } catch (e) { ligne.textContent = ""; return; }
    const os = r.obligations || [];
    avecDeclaration = os.some((o) => o.cle === "declaration_accident");
    majFaits();
    ligne.textContent = r.ensuite || "";
  };
  const majFaits = () => {
    const m = motif();
    document.getElementById("ar-bloc-accident").hidden = !(m && m.date_accident);
    document.getElementById("ar-bloc-connu").hidden = !avecDeclaration;
  };
  let trouve = null;
  const relireContrats = async () => {
    trouve = await contratsDuJour(p, val("ar-dernier"), document.getElementById("ar-contrats"));
  };
  document.getElementById("ar-motif").onchange = () => { majFaits(); relireEnsuite(); };
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
      motif: val("ar-motif"), ...contratAEnvoyer(trouve),
    };
    if (m && m.date_accident) charge.date_accident = val("ar-accident");
    if (val("ar-note").trim()) charge.note = val("ar-note").trim();
    if (avecDeclaration && val("ar-connu")) charge.connu = { jour: val("ar-connu"), heure: val("ar-connu-heure") };
    let r;
    try { r = await api("arret-enregistrer", charge); }
    catch (e) { dire(e.message, "erreur"); return; }
    await etapeSuite(r.arret, p, r.phrase);
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
        ${motifs.map((m) => `<option value="${echapper(m.code)}">${echapper(m.libelle_dit || m.libelle)}</option>`).join("")}
      </select></label>
    <button type="button" class="lien" id="ab-note-btn">+ une note</button>
    <label class="champ-boite" id="ab-note-l" hidden>Note <span class="discret">— ${aideDe("note_imprevu")}</span>
      <input type="text" id="ab-note"></label>
    <p class="note">${aideDe("absence")}</p>
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
    await etapeSuite(r.suspension, p, r.phrase);
  };
}

/* ------------------------------------- ce que l'imprévu écrit appelle encore
   Le moteur le dit sur l'imprévu qu'il vient d'écrire : ses indemnités
   journalières (`ij_attendues` : l'arrêt ouvre le maintien de salaire, par
   la loi ou la convention ; `ij_majoree_attendue` : un accident du travail
   au-delà du 28e jour), les jours déjà indemnisés
   (`jours_deja_indemnises_attendus`), et les jours que la saisie paie
   pendant l'imprévu (`cachets`). Rien de tout ça : la boîte se ferme.
   Sinon, une seconde page, où rien n'est obligatoire — les indemnités se
   connaissent au relevé de la caisse, et se reprennent sous la carte. */

/** Les cases des indemnités journalières d'un arrêt, et ses jours déjà indemnisés : seulement ce que le moteur attend. */
function champsIndemnites(a, x) {
  const montant = (id, libelle, v) => `<label class="champ-boite">${libelle}
      <input type="number" step="0.01" min="0" id="${x}-${id}" value="${echapper(v ?? "")}"></label>`;
  return `${a.ij_attendues ? `<p class="note">${aideDe("indemnites_journalieres")}</p>
    <div class="champs-deux">${montant("ijb", "Indemnité brute par jour", a.ij_brute)}${montant("ijn", "Indemnité nette par jour", a.ij_nette)}</div>` : ""}
    ${a.ij_majoree_attendue ? `<div class="champs-deux">${montant("ijbm", "Brute majorée par jour", a.ij_brute_majoree)}${
      montant("ijnm", "Nette majorée par jour", a.ij_nette_majoree)}</div>` : ""}
    ${a.jours_deja_indemnises_attendus ? `<label class="champ-boite" title="${aideDe("jours_deja_indemnises")}">Jours déjà indemnisés
      <input type="number" step="1" min="0" id="${x}-deja" value="${echapper(a.jours_deja_indemnises ?? "")}"></label>` : ""}`;
}

/** Ce que les cases de `champsIndemnites` disent, pour `arret-indemnites` ; `null` si rien n'est saisi. */
function chargeIndemnites(a, x) {
  const charge = { salarie: a.salarie, dernier_jour: a.dernier_jour_travaille };
  const lire = (id, cle) => { const c = document.getElementById(`${x}-${id}`); if (c && c.value !== "") charge[cle] = c.value; };
  lire("ijb", "ij_brute"); lire("ijn", "ij_nette"); lire("ijbm", "ij_brute_majoree"); lire("ijnm", "ij_nette_majoree");
  lire("deja", "jours_deja_indemnises");
  return Object.keys(charge).length > 2 ? charge : null;
}

async function etapeSuite(i, p, phrase) {
  const jours = (i.cachets || []);
  if (!i.ij_attendues && !i.jours_deja_indemnises_attendus && !jours.length) {
    fermerBoite();
    dire(phrase);
    await p.apres();
    return;
  }
  const div = p.carte ? p.carte.div : null;
  const ici = (c) => div && c.mois === MOIS_AFFICHE && !div.classList.contains("validee");
  ouvrirBoite(`
    <p>${echapper(phrase)}</p>
    <h3>Ce qu'il appelle encore</h3>
    ${champsIndemnites(i, "su")}
    ${jours.length ? `<div class="champ-boite">${aideDe("jours_payes")}
      <div class="liste-jours">${jours.map((c, k) => (c.jours || []).map((j) => ici(c)
        ? `<label class="coche"><input type="checkbox" data-jour="${k}" value="${echapper(j.iso)}">
             ${echapper(j.phrase)}</label>`
        : `<div class="discret">${echapper(j.phrase_du_mois)}</div>`).join("")).join("")}
      </div>
      ${jours.some((c) => !ici(c) && c.mois === MOIS_AFFICHE) ? `<p class="discret">${aideDe("imprevu_valide")}</p>` : ""}</div>` : ""}
    <div class="actions">
      <button class="secondaire" id="su-plus-tard">Plus tard</button>
      <button class="primaire" id="su-ok">Enregistrer</button>
    </div>`);
  const finir = async () => { fermerBoite(); await p.apres(); };
  document.getElementById("su-plus-tard").onclick = finir;
  document.getElementById("su-ok").onclick = async () => {
    const charge = chargeIndemnites(i, "su");
    if (charge) {
      try { await api("arret-indemnites", charge); }
      catch (e) { dire(e.message, "erreur"); return; }
    }
    let retires = 0;
    document.querySelectorAll("#boite [data-jour]:checked").forEach((x) => {
      const c = jours[+x.dataset.jour];
      if (retirerJour(div, c.rubrique, x.value)) retires++;
    });
    if (retires) await avantDeQuitter();
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
    <h3>${echapper(a.reprise_titre)} — ${echapper(nomDuSalarie(a.salarie))}</h3>
    <p class="discret">${echapper(a.quoi)}, ${echapper(a.periode)}</p>
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
    let r;
    try {
      r = await api("arret-reprise", { salarie: a.salarie, dernier_jour: a.dernier_jour_travaille,
        reprise: document.getElementById("re-date").value, motif_reprise: document.getElementById("re-motif").value });
    } catch (e) { dire(e.message, "erreur"); return; }
    fermerBoite();
    dire(r.phrase);
    await apres();
  };
}

/** L'avis de prolongation : la nouvelle fin prescrite, et rien d'autre. Le moteur met l'arrêt à jour et dit ce qui en
 *  découle. */
function boiteProlonger(a, apres) {
  ouvrirBoite(`
    <h3>Prolonger l'arrêt — ${echapper(nomDuSalarie(a.salarie))}</h3>
    <p class="discret">${echapper(a.quoi)}, ${echapper(a.periode)}</p>
    <label class="champ-boite" title="${aideDe("prolongation")}">Prolongé jusqu'au
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
    dire(r.phrase);
    fermerBoite();
    await apres();
  };
}


/** Les indemnités journalières d'un arrêt, par jour, brutes et nettes, telles que le relevé de la caisse les écrit ; et,
 *  seulement si le moteur les attend, celles d'un accident du travail au-delà du 28e jour et les jours déjà indemnisés. */
function boiteIndemnites(a, apres) {
  ouvrirBoite(`
    <h3>Indemnités journalières — ${echapper(nomDuSalarie(a.salarie))}</h3>
    <p class="discret">${echapper(a.quoi)}, ${echapper(a.periode)}</p>
    ${champsIndemnites(a, "ma")}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="ma-ok">Enregistrer</button>
    </div>`);
  document.getElementById("ma-ok").onclick = async () => {
    const charge = chargeIndemnites(a, "ma");
    if (!charge) { fermerBoite(); return; }
    let r;
    try { r = await api("arret-indemnites", charge); }
    catch (e) { dire(e.message, "erreur"); return; }
    fermerBoite();
    dire(r.phrase);
    await apres();
  };
}

function boiteFinAbsence(s, apres) {
  ouvrirBoite(`
    <h3>${echapper(s.motif_libelle)} — ${echapper(nomDuSalarie(s.salarie))}</h3>
    <p class="discret">${echapper(s.depuis_dit || "")}</p>
    <label class="champ-boite">Jusqu'au <input type="date" id="af-fin" value="${echapper(s.fin || "")}"></label>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="af-ok">Enregistrer</button>
    </div>`);
  document.getElementById("af-ok").onclick = async () => {
    let r;
    try { r = await api("suspension-modifier", { salarie: s.salarie, debut: s.debut, fin: document.getElementById("af-fin").value }); }
    catch (e) { dire(e.message, "erreur"); return; }
    fermerBoite();
    dire(r.phrase);
    await apres();
  };
}

/* ------------------------------------------------------ les actes dus */

/** Écrire le signalement (04 : l'arrêt ; 05 : la reprise anticipée), puis le déposer. Deux gestes et deux boîtes :
 *  écrire un fichier n'engage rien, le déposer engage tout. */
function boiteSignalerArret(a, nature) {
  const deja = (a.deja_signale || {})[nature];
  const reprise = nature === "05";
  ouvrirBoite(`
    <h3>${echapper((a.signaler_titre || {})[nature])} — ${echapper(nomDuSalarie(a.salarie))}</h3>
    <p class="discret">${echapper(a.quoi)}, ${echapper(a.periode)}${
      reprise && a.reprise_dit ? `, ${echapper(a.reprise_dit)}` : ""}</p>
    <p class="note">${aideDe("signalement_dsn")}</p>
    ${deja ? `<div class="encart attention"><b>${echapper(deja)}</b>
      ${aideDe("deja_signale")}
      <label class="champ-boite" title="${aideDe("identifiant_remplace")}">Déclaration remplacée
        <input type="text" id="si-remplace"></label></div>` : ""}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="si-ok">Écrire le signalement</button>
    </div>`);
  document.getElementById("si-ok").onclick = async () => {
    const remplace = document.getElementById("si-remplace");
    // Un clic de plus, pendant l'écriture, ne fait pas un second signalement.
    const rendre = occupe(document.getElementById("si-ok"), "Écriture…");
    let r;
    try {
      r = await api("arret-signaler", { salarie: a.salarie, dernier_jour: a.dernier_jour_travaille, nature,
                                        remplace: remplace ? remplace.value.trim() : "" });
    } catch (e) { rendre(); dire(e.message, "erreur"); return; }
    fermerBoite();
    // Les avis du signalement suivent le message, sur la même ligne.
    dire(r.phrase);
    await rafraichirImprevus();
    boiteDepot({ quoi: "dsn", fichier: r.fichier });
  };
}

/** Le jour (et l'heure) où la compagnie a su l'arrêt ou l'accident : le point de départ du délai. Sans lui, le moteur
 *  présume — le plus tôt possible —, et le dit sous « pourquoi ». */
function boiteConnaissance(a, de, o) {
  const depart = (o && o.depart) || {};
  ouvrirBoite(`
    <h3>${echapper(o.connaissance.titre)}</h3>
    <p class="discret">${echapper(nomDuSalarie(a.salarie))} — ${echapper(a.quoi)}</p>
    ${depart.presume ? `<p class="note"><b>Point de départ présumé</b> ${echapper(depart.presume)}</p>` : ""}
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
    <p class="discret capitale">${echapper(o.butoir_dit || "")}${
      o.destinataire ? ` — ${echapper(o.destinataire)}` : ""}</p>
    <p class="note">${aideDe("hors_dsn")}</p>
    ${(o.ou || []).map((x) => `<p class="note">${echapper(x.libelle)}</p>`).join("")}
    ${(o.a_avoir || []).length ? `<details class="pourquoi"><summary>pièces à avoir</summary>
      <ul class="note">${o.a_avoir.map((x) => `<li>${echapper(x)}</li>`).join("")}</ul></details>` : ""}
    <div class="champs-deux">
      <label class="champ-boite">Faite le <input type="date" id="da-le" value="${echapper(fait.le || "")}"></label>
      <label class="champ-boite">Où
        <select id="da-depot">${(o.ou || []).map((x) => `<option value="${echapper(x.code)}"${
          fait.depot === x.code ? " selected" : ""}>${echapper(x.court || x.code)}</option>`).join("")}</select></label>
    </div>
    <label class="champ-boite" title="${aideDe("reference_faite")}">Référence
      <input type="text" id="da-reference" value="${echapper(fait.reference || "")}"></label>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="da-ok">Enregistrer</button>
    </div>`);
  document.getElementById("da-ok").onclick = async () => {
    let r;
    try {
      r = await api(route, { salarie: a.salarie, dernier_jour: a.dernier_jour_travaille,
        le: document.getElementById("da-le").value, depot: document.getElementById("da-depot").value,
        reference: document.getElementById("da-reference").value.trim() });
    } catch (e) { dire(e.message, "erreur"); return; }
    fermerBoite();
    dire(r.phrase);
    await rafraichirImprevus();
  };
}

/** Écrire le signalement de fin de contrat, puis le déposer. */
function boiteSignalerFin(mois, f) {
  ouvrirBoite(`
    <h3>Signaler la fin — ${echapper(nomDuSalarie(f.salarie))}</h3>
    <p class="discret">${echapper(f.nature)} — ${echapper(f.contrat_dit)}</p>
    <p class="note">${aideDe("signalement_fin")}</p>
    <details class="pourquoi"><summary>Annule et remplace</summary>
      <label class="champ-boite" title="${aideDe("identifiant_remplace")}">Déclaration remplacée

        <input type="text" id="fi-remplace"></label>
    </details>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="fi-ok">Écrire le signalement</button>
    </div>`);
  document.getElementById("fi-ok").onclick = async () => {
    const rendre = occupe(document.getElementById("fi-ok"), "Écriture…");
    let r;
    try {
      r = await api("fin-contrat-signaler", { mois, salarie: f.salarie,
                                              remplace: document.getElementById("fi-remplace").value.trim() });
    } catch (e) { rendre(); dire(e.message, "erreur"); return; }
    fermerBoite();
    dire(r.phrase);
    await rafraichirImprevus();
    boiteDepot({ quoi: "dsn", fichier: r.fichier });
  };
}

/** Les deux papiers dus au salarié qui part. Ils ne partent nulle part : ils se REMETTENT, et le reçu se signe à la main. */
async function boitePapiersDeFin(mois, salarie) {
  let r;
  try { r = await api("fin-contrat-documents", { mois, salarie }); }
  catch (e) { dire(e.message, "erreur"); return; }
  ouvrirBoite(`
    <h3>Les papiers de fin</h3>
    <p class="discret">${echapper(r.phrase)}</p>
    ${(r.avertissements || []).map((m) => `<div class="encart attention">${echapper(m)}</div>`).join("")}
    <div class="actions">
      <button class="secondaire" id="fp-dossier">Ouvrir le dossier</button>
      <button class="primaire" onclick="fermerBoite()">Fermer</button>
    </div>`);
  document.getElementById("fp-dossier").onclick = () => ouvrirEtDire(r.dossier);
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
      <button type="button" class="lien" id="fiche-imprevu-porte">Imprévu…</button>
    </div>
    ${liste.map((i, k) => `<div class="imprevu" data-imprevu="${k}">
        <span class="imprevu-quoi"><b>${echapper(i.quoi)}</b>
          <span class="discret">${echapper(i.periode)}${i.contrat_dit ? ` · ${echapper(i.contrat_dit)}` : ""}</span></span>
        <span class="imprevu-gestes">${gestesImprevu(i, null)}</span>
        <div class="imprevu-parti">${ceQuiEnEstParti(i)}</div>
      </div>`).join("")}`;
  liste.forEach((i, k) => brancherGestesImprevu(cible.querySelector(`[data-imprevu="${k}"]`), i, null, apres));
  const porte = document.getElementById("fiche-imprevu-porte");
  porte.onclick = (ev) => { ev.stopPropagation(); menuImprevu(porte, { salarie: cle, apres }); };
}

/** Ce qui est parti d'un imprévu, et ce qui en reste dû : les morceaux que le moteur dit (`parti`), sur une ligne. */
function ceQuiEnEstParti(i) {
  return (i.parti || []).map((p) => p.discret
    ? `<span class="discret">${echapper(p.phrase)}</span>`
    : `${p.pastille ? `${pastilleDeLEtat(p.pastille)} ` : ""}${echapper(p.phrase)}`).join(" · ");
}
