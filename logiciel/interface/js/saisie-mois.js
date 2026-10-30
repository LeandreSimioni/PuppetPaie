/* Le mois : la saisie courante, l'apercu, charger un mois, la grille des mois, enregistrer. */
"use strict";

/* CE QU'ON VOIT EST CE QUI EST ENREGISTRÉ : on clique sur Valider, ça
 * enregistre le formulaire, et on n'affiche que ce qui est affiché. Une carte ouverte écrit son engagement
 * depuis son formulaire, et depuis lui seul : toute donnée qu'un engagement
 * peut porter y a son champ quand elle a une valeur (`CHAMPS_RARES`, le lieu
 * propre à un jour, le libellé d'une prime…). Une carte fermée n'a pas de
 * formulaire et ne se modifie pas : elle écrit les engagements du fichier
 * qu'elle montre, tels quels (`engagementsLus`). Ce que le moteur tient seul —
 * la validation et sa carte figée, la réouverture, `_origine` — il le reprend
 * du fichier (`moteur/src/routes/mois.rs`), quoi que la page envoie. */

/** Les engagements de la saisie lue qu'une carte montre (`data-rangs`). */
function engagementsLus(div) {
  const tous = (SAISIE && SAISIE.engagements) || [];
  return (div.dataset.rangs || "").split(",").filter(Boolean).map((i) => tous[+i]).filter(Boolean);
}

/** Une carte fermée : son formulaire n'est pas construit. */
function carteFermee(div) {
  return !div.querySelector(".ligne1");
}

/** La fin de ce contrat se signale-t-elle (nature 07), pour ce motif ? Le
 *  moteur le dit (`Nature::signalement_fctu`, par `natures`) ; la page ne
 *  fait que le lire. Sans motif choisi, c'est le motif par défaut. */
function finSeSignale(nat, motif) {
  if (motif && nat.signalement_par_motif && motif in nat.signalement_par_motif)
    return !!nat.signalement_par_motif[motif];
  return !!nat.signalement_fctu;
}

/** Ce que la fin du contrat déclarera : le motif, pour toute nature qui en a
 *  (un CDDU rompu avant son terme le dit aussi), et, quand cette fin se
 *  signale, les faits que le moteur ne devine pas, plus le dernier jour payé
 *  qui ne s'écrit que s'il s'écarte de la fin du contrat. */
function ruptureDuBloc(div, nat, fin, e) {
  // Un contrat aux cachets n'écrit pas sa fin : le moteur la donne (`contrat.rupture`, son dernier jour travaillé).
  if (!(fin || (div._rupture && div._rupture.fin))) return;
  if (!(nat.motifs_rupture || []).length && !nat.signalement_fctu) return;
  const dans = (s) => div.querySelector(s).value;
  if (dans(".e-motif-rupture")) e.motif_rupture = dans(".e-motif-rupture");
  // Le reste ne part que si cette fin SE SIGNALE : le moteur le dit, motif
  // par motif (`signalement_par_motif`). Un CDDU à son terme (031) part
  // dans la DSN du mois ; rompu avant, il se signale (fiche 2062).
  if (!finSeSignale(nat, e.motif_rupture)) return;
  // Les faits du motif, seulement ceux que le moteur a demandés.
  const faits = (nat.faits_de_rupture || {})[e.motif_rupture] || [];
  if (faits.includes("notification_rupture") && dans(".e-notification"))
    e.notification_rupture = dans(".e-notification");
  if (faits.includes("engagement_licenciement") && dans(".e-engagement-licenciement"))
    e.engagement_licenciement = dans(".e-engagement-licenciement");
  if (faits.includes("signature_convention") && dans(".e-signature-convention"))
    e.signature_convention = dans(".e-signature-convention");
  if (dans(".e-preavis-type")) {
    e.preavis = { type: dans(".e-preavis-type") };
    if (dans(".e-preavis-debut")) e.preavis.debut = dans(".e-preavis-debut");
    if (dans(".e-preavis-fin")) e.preavis.fin = dans(".e-preavis-fin");
  }
  if (faits.includes("transaction") && dans(".e-transaction")) e.transaction = dans(".e-transaction");
  if (dans(".e-dernier-paye")) e.dernier_jour_paye = dans(".e-dernier-paye");
  if (faits.includes("en_droit_a_la_retraite") && dans(".e-en-droit"))
    e.en_droit_a_la_retraite = dans(".e-en-droit") === "oui";
  if (faits.includes("preavis_non_effectue")
      && div.querySelector(".e-preavis-non-effectue").checked) {
    e.preavis_non_effectue = true;
    if (dans(".e-preavis-mois") !== "") e.preavis_mois = +dans(".e-preavis-mois");
  }
  if (faits.includes("precarite_due")
      && div.querySelector(".e-precarite-ecartee").checked) {
    e.precarite_due = false;
    if (dans(".e-precarite-pourquoi")) e.precarite_pourquoi = dans(".e-precarite-pourquoi");
  }
}

/** L'engagement qu'une carte écrit (aucun tant qu'elle n'a pas de salarié). */
function engagementsDuBloc(div, mois) {
  // Une carte fermée, ou ouverte pour être modifiée : ce qu'on y tape ne s'écrit qu'à « Modifier l'engagement » ; seuls
  // l'aperçu et le montant visé la lisent (`LIRE_LES_MODIFICATIONS`).
  if (carteFermee(div) || (div.classList.contains("en-modification") && !LIRE_LES_MODIFICATIONS))
    return engagementsLus(div).map((e) => JSON.parse(JSON.stringify(e)));
  // L'ENGAGEMENT N'ÉCRIT PLUS DE `dates` : chacune de ses lignes porte les
  // siennes, avec son lieu, et le moteur en fait la réunion
  // (`operations.dates_engagement`). Une carte porte un salarié : elle écrit son engagement.
  const spectacle = div.querySelector(".e-spectacle").value;
  const etablissement = div.querySelector(".e-etablissement").value;
  const lignes = lignesDuBloc(div, mois);
  const metier = div.querySelector(".e-metier").value;
  const cadre = div.querySelector(".e-cadre").checked;
  const debut = div.querySelector(".e-debut").value;
  const fin = div.querySelector(".e-fin").value;
  const numeroDuBloc = div.dataset.numero || "";
  const nat = natureDe(div.querySelector(".e-nature").value);
  const faits = faitsDuBloc(div);
  const salaire = div.querySelector(".e-salaire").value;
  const quotite = div.querySelector(".e-quotite").value;
  const avenant = div._avenant || {};
  return salariesDuBloc(div).map((salarie) => {
    // LE NUMÉRO EST CELUI D'UN CONTRAT, DONC D'UN SALARIÉ : celui du
    // bloc s'il est le sien. Un salarié coché en plus n'hérite pas du
    // contrat d'un autre — il recevra le sien à la production (le moteur
    // refuse un numéro sur deux salariés).
    const numero = div.dataset.numeroDe === salarie ? numeroDuBloc : "";
    // UN PERMANENT N'A PAS DE SPECTACLE, et ce n'est pas un détail de mise
    // en page : le numéro d'objet lui est interdit (CCH-11).
    const e = nat.mensuelle ? { salarie } : { salarie, spectacle };
    // La nature ne s'écrit que si elle s'écarte du défaut : un engagement
    // muet est un CDDU du spectacle.
    if (nat.cle && nat.cle !== ETAT.nature_defaut) e.nature = nat.cle;
    if (debut) e.debut = debut;
    if (fin) e.fin = fin;
    if (numero) e.numero = numero;
    // L'établissement ne s'écrit que s'il n'est pas le siège.
    if (etablissement) e.etablissement = etablissement;
    if (nat.mensuelle) {
      // LES FAITS, PAS LES LIGNES : `lignes_du_mois` en fait des lignes de
      // bulletin — le prorata d'un mois incomplet, l'indemnité de congés et
      // la retenue qui la couvre, les imprévus, le solde de tout compte.
      // Écrire `lignes` ici couperait tout ce calcul.
      const recours = div.querySelector(".e-recours").value;
      if (recours) e.motif_recours = recours;
      if (salaire !== "") e.salaire_mensuel = +salaire;
      if (quotite !== "") e.quotite_contrat = +quotite;
      // L'avenant du mois, tel que le moteur l'a posé (« À partir de ») : la carte ne le saisit pas.
      if (avenant.date_modification) e.date_modification = avenant.date_modification;
      if (avenant.avant) e.avant = avenant.avant;
      if (faits.conges.length) e.conges_pris = faits.conges;
      if (faits.primes.length) e.primes = faits.primes;
    } else {
      // UN MOIS DU CONTRAT SANS CACHET : un contrat aux bornes
      // dites, et aucune ligne remplie, s'écrit `lignes: []` — la DSN porte
      // le contrat avec un versement à zéro, sans bulletin.
      if (lignes.length) e.lignes = JSON.parse(JSON.stringify(lignes));
      else if (debut && fin) e.lignes = [];
    }
    ruptureDuBloc(div, nat, fin, e);
    // Le métier ne s'écrit que s'il est choisi : un engagement sans `metier`
    // prend celui de la fiche. `cadre` voyage avec lui, seulement quand il
    // s'écarte du défaut du catalogue.
    if (metier) {
      e.metier = metier;
      const m = metierDe(metier);
      if (!m || cadre !== !!m.cadre_par_defaut) e.cadre = cadre;
    }
    // Ce que le formulaire montre parce que l'engagement le porte : la date
    // de paiement propre au contrat, son lieu, l'emploi et les statuts
    // déclarés… (`CHAMPS_RARES`).
    raresDuBloc(div, e);
    ecritDuBloc(div, e);
    return e;
  });
}

/* UNE CARTE OUVERTE POUR ÊTRE MODIFIÉE (`ouvrirEnModification`) ne s'écrit pas : la saisie du mois garde son
 * engagement tel qu'il est enregistré. L'aperçu et le montant visé, qui n'écrivent rien, la lisent telle qu'elle est. */
let LIRE_LES_MODIFICATIONS = false;

/** Les engagements validés ouverts pour être modifiés : chacun désigné, avec ce que sa carte montre (`mois_designe`). */
function modifieesDeLaPage(mois) {
  LIRE_LES_MODIFICATIONS = true;
  try {
    return [...document.querySelectorAll(".engagement.en-modification")].filter((c) => c._ligne)
      .map((c) => ({ ...contratDe(c._ligne), saisies: engagementsDuBloc(c, mois) }));
  } finally { LIRE_LES_MODIFICATIONS = false; }
}

/* L'APERÇU ET LE MONTANT VISÉ CALCULENT LE MOIS ENREGISTRÉ, et les cartes ouvertes pour être modifiées : ce qui est
 * tapé s'enregistre d'abord, sans attendre la seconde de l'enregistrement automatique. */
async function enregistrerAvantDeCalculer() {
  if (MINUTEUR_SAISIE) { clearTimeout(MINUTEUR_SAISIE); MINUTEUR_SAISIE = null; }
  await enregistrerSaisie(true);
}

async function apercuDuMois(mois, blocs) {
  await enregistrerAvantDeCalculer();
  return api("apercu", { mois, modifiees: modifieesDeLaPage(mois), blocs });
}
/** Les engagements que les cartes du mois affiché montrent, celles ouvertes pour être modifiées telles qu'on les a
 *  tapées : ce que l'aperçu compte. Rien ne s'écrit d'elles (chaque carte écrit son engagement). */
function engagementsALEcran() {
  const mois = document.getElementById("mois").value;
  LIRE_LES_MODIFICATIONS = true;
  try {
    return [...document.querySelectorAll(".engagement")].flatMap((div) => engagementsDuBloc(div, mois));
  } finally { LIRE_LES_MODIFICATIONS = false; }
}

function rafraichirApercu() {
  clearTimeout(MINUTEUR);
  MINUTEUR = setTimeout(calculerApercu, 350);   // on ne calcule pas a chaque frappe
  // Et on enregistre. C'est le seul point de passage de TOUTES les
  // modifications de la saisie -- y accrocher l'enregistrement, c'est
  // s'assurer qu'aucune frappe ne reste dans la page seulement.
  enregistrerPlusTard();
  // Les etats, eux, ne coutent rien : ils sont deja lus. On les repeint tout
  // de suite, parce que changer le salarie ou les jours change l'engagement que ce
  // bloc DESIGNE -- et laisser 350 ms l'ancien etat sous le nouveau nom, c'est
  // exactement le mensonge qu'on veut eviter.
  peindreEtats();
}

/** `lu` : l'aperçu tel que la lecture du mois le porte (la saisie enregistrée) ; sans lui, on le demande au moteur. */
async function calculerApercu(lu) {
  const engagements = engagementsALEcran();
  const cible = document.getElementById("apercu");
  // Les engagements importés ne se calculent pas ici : ce sont ceux de l'ancien logiciel.
  if (!engagements.length && IMPORTES.length) {
    cible.innerHTML = `<p class="discret">Engagements de l'ancien logiciel : rien ne se calcule ici.</p>`;
    return;
  }
  // Dès qu'un engagement nomme son salarié, le moteur dit ce que le mois en compte et ce qui lui manque (`apercu`).
  if (!engagements.length) {
    // Le permanent ne se nomme qu'à une compagnie qui en emploie.
    const perm = !!(ETAT.configuration || {}).permanents;
    cible.innerHTML = `<p class="discret">Ajouter un engagement, une ligne de
      paie et ses jours joués${perm ? " — ou, pour un permanent, son salaire mensuel" : ""}.</p>`;
    return;
  }
  cible.classList.add("attente");
  try {
    // Les trois nombres de chaque carte, sommés par le moteur (`blocs` : combien d'engagements chaque carte porte) ;
    // la lecture du mois les rend pour une carte par engagement.
    const blocs = [...document.querySelectorAll(".engagement")];
    const tailles = blocs.map((div) => salariesDuBloc(div).length);
    // La lecture du mois ne calcule que les engagements qui se calculent : elle ne sert que si elle les a tous.
    let deLu = null;
    if (lu && tailles.every((n) => n === 1)) {
      try { deLu = lireReponse(lu); } catch (e) { deLu = null; }
      if (deLu && (deLu.paies || []).length !== engagements.length) deLu = null;
    }
    const a = deLu ? structuredClone(deLu) : await apercuDuMois(document.getElementById("mois").value, tailles);
    cible.innerHTML = vueApercu(a);
    // Les dates resolues, sous chaque engagement : ce que le moteur a compris.
    const parBloc = engagementsParBloc(a, blocs);
    blocs.forEach((div, i) => {
      const part = parBloc[i];
      const p = part[0];
      // Un engagement auquel il manque un fait est enregistré tel quel : la carte dit ce qui manque, sans montant.
      const manques = part.filter((x) => x && x.a_completer);
      if (manques.length) {
        div._total = null;
        peindreChiffres(div, null);
        div._paies = [];
        // La rupture que le dernier aperçu calculé a dite reste : un fait refusé (une date de notification hors du
        // contrat) ne fait pas disparaître de la saisie le motif et les dates de la fin.
        peindreCarte(div);
        div.querySelector(".resolu").innerHTML = "";
        // Les cases du contrat écrit se disent avant que le reste soit complet (la période d'essai d'un CDD).
        div._ecrits = p && p.contrat && p.contrat.ecrits;
        montrerEcrit(div, div._ecrits);
        div.querySelector(".avis").innerHTML = `<div class="encart attention avis-engagement"><b>À compléter avant de valider</b>${
          manques.map((x) => `<div>${erreurEnClair(x.a_completer)}</div>`).join("")}</div>`;
        return;
      }
      div._total = part.length ? (a.blocs || [])[i] : null;
      peindreChiffres(div, div._total);
      div._paies = part;
      // La rupture que le moteur propose pour cet engagement (`contrat.rupture`) : la porte « Un imprévu… » et la fin.
      div._rupture = (p && p.contrat && p.contrat.rupture) || null;
      if (div.querySelector(".e-nature") && div.querySelector(".e-rupture")) majRupture(div);
      peindreCarte(div);
      // Ce qui a été numéroté : un contrat en cours garde le numéro de son
      // premier mois, et c'est ici qu'il revient à l'écran quand la saisie
      // ne le portait pas encore.
      if (p && p.numero_contrat && p.contrat && !p.contrat.premier_mois
          && !div.dataset.numero) {
        div.dataset.numero = p.numero_contrat;
        div.dataset.numeroDe = salariesDuBloc(div)[0] || "";
        majContrat(div);
      }
      // Un mois du contrat sans cachet n'a pas de bulletin : on le dit à la
      // place du numéro, et le brut à zéro suit.
      // UN CONTRAT SUR PLUSIEURS MOIS : le chevron au bord de
      // la case, d'après ce que le moteur rend (`poserChevrons`).
      if (p) poserChevrons(div, p.contrat, p.debut, p.fin);
      // Les cases du contrat écrit que le moteur demande pour cet engagement.
      montrerEcrit(div, p && p.contrat && p.contrat.ecrits);
      div.querySelector(".resolu").innerHTML = p
        ? `${p.contrat && p.contrat.sans_paie
             ? `<b>aucun cachet ce mois-ci</b> — pas de bulletin, la DSN porte le contrat`
             : `bulletin <b>${echapper(p.numero_bulletin)}</b>`} · contrat ${echapper(p.numero_contrat)}
           · ${echapper(jjmmaaaa(p.debut))} → ${echapper(jjmmaaaa(p.fin))}${
             p.contrat && p.contrat.a_cheval
               ? ` <span class="discret">(paie du ${echapper(jjmmaaaa(p.contrat.periode.debut))} au ${echapper(jjmmaaaa(p.contrat.periode.fin))})</span>`
               : ""}
           · ${echapper(designation(p))}${p.emploi && p.emploi.libelle
             ? ` · ${echapper(p.emploi.libelle.toLowerCase())}${p.emploi.cadre ? " (cadre)" : ""}` : ""}
           · brut <b>${echapper(euros(p.brut))}</b>`
        : "";
      // Ce que le moteur a à dire de CET engagement, en deux tons (24/09/2026,
      // engagement.rs, Avis) : en orange ce qui demande quelque chose -- un
      // taux sous le SMIC --, en gris ce qui renseigne seulement -- un
      // contrat qui se poursuit le mois suivant, ou le minimum que notre
      // lecture de la convention donne, avec son lien. Rien
      // n'est bloquant.
      div.querySelector(".avis").innerHTML =
        avisDuBloc(part).map(
          (a) => `<div class="encart attention avis-engagement">${echapper(a)}</div>`).join("")
        + avisDuBloc(part, "informations").map(
          (a) => `<div class="encart info avis-engagement">${echapperLiens(a)}</div>`).join("")
        // Le conseil que la saisie appelle, au plus un par carte, quand la compagnie a coché l'option.
        + conseilDuBloc(part);
    });
  } catch (e) {
    cible.innerHTML = `<div class="encart erreur">${erreurEnClair(e)}</div>`;
  } finally {
    cible.classList.remove("attente");
  }
}

/* UNE ERREUR DU MOTEUR, LISIBLE. Le nom de l'exception
 * (« FileNotFoundError », « ErreurOperation ») s'affichait en gras, en tête :
 * il ne dit rien à qui fait la paie. La première ligne du message dit quoi
 * faire ; le reste — la règle, la commande — se déplie si on le veut. */
function erreurEnClair(e) {
  const lignes = String(e.message || e).split("\n").map((l) => l.trim()).filter(Boolean);
  const tete = lignes.shift() || "Erreur";
  return `${echapper(tete)}${lignes.length ? `<details class="discret"><summary>en savoir plus</summary>
    ${lignes.map((l) => `<div>${echapper(l)}</div>`).join("")}</details>` : ""}`;
}

/* UN BLOC PARLE D'UNE SEULE VOIX. Deux salariés payés ensemble
 * recevaient chacun le même avis — « FAVART Célimène : contrat… » puis
 * « MARAIS Marin : contrat… », quatre encarts pour deux choses à dire.
 * Le moteur ouvre ses avis par le nom (`qui` : « NOM Prénom » suivi de
 * « : » ou de « , ») ; ce qui suit est comparé, et le même avis ne
 * s'affiche qu'une fois, pour tous ceux qu'il concerne. Un avis qui ne
 * commence pas par un nom du bloc reste tel quel. */
function avisDuBloc(part, liste = "avertissements") {
  const groupes = new Map();
  part.forEach((x) => {
    const s = x.salarie || {};
    const qui = `${s.nom || ""} ${s.prenom || ""}`.trim();
    (x[liste] || []).forEach((a) => {
      const sep = qui && [" : ", ", "].find((c) => a.startsWith(qui + c));
      const cle = sep ? sep + a.slice(qui.length + sep.length) : "\u0000" + a;
      if (!groupes.has(cle)) groupes.set(cle, { texte: a, noms: [] });
      if (sep && !groupes.get(cle).noms.includes(qui)) groupes.get(cle).noms.push(qui);
    });
  });
  return [...groupes.entries()].map(([cle, g]) => {
    if (g.noms.length < 2) return g.texte;
    const noms = g.noms.slice(0, -1).join(", ") + " et " + g.noms[g.noms.length - 1];
    return noms + cle;
  });
}

/* LES ENGAGEMENTS D'UN APERÇU, RENDUS À LEUR CARTE : une carte sans salarié n'en a pas, les autres un chacune. */
function engagementsParBloc(a, blocs) {
  let rang = 0;
  return blocs.map((div) => {
    const combien = salariesDuBloc(div).length;
    const part = (a.paies || []).slice(rang, rang + combien);
    rang += combien;
    return part;
  });
}

/* L'ENGAGEMENT VALIDÉ, EN UNE LIGNE D'AGENDA.
 *
 * Validé, un engagement ne se remplit plus : il se PARCOURT. On y cherche,
 * dans cet ordre, quand (les jours travaillés, le repère d'un engagement), qui
 * et pour quoi, ce qui reste à faire (DPAE, contrat signé, documents), et
 * combien — le net seulement, ce qu'on vire ; brut et coût sont dans
 * l'aperçu à droite de l'écran, les répéter ici serait du bruit. Numéros
 * de bulletin et de contrat, date de validation : en infobulle.
 * Un clic sur elle l'ouvre, toutes ses cases modifiables (`ouvrirEnModification`).
 *
 * Deux sources, qui n'arrivent pas au même moment : l'aperçu du moteur
 * (`div._paies` : jours, ce qui est payé, net) et l'état de l'engagement
 * (`div._ligne` : actes, validation). La ligne se repeint à l'arrivée de
 * chacune ; ce qui manque encore se tait. */
function peindreCarte(div) {
  const cible = div.querySelector(".e-resume");
  if (!cible) return;
  const ligne = div._ligne, ps = div._paies || [];
  if (!div.classList.contains("validee") || !ligne) {
    cible.innerHTML = "";
    cible.classList.remove("ouvrable");
    cible.removeAttribute("tabindex");
    cible.removeAttribute("role");
    return;
  }
  const p = ps[0];
  const mois = document.getElementById("mois").value;
  // QUAND. Les jours travaillés du mois ; un permanent n'en a pas : son mois.
  const dates = p ? (p.dates_travail || []).filter((d) => d.startsWith(mois)) : [];
  const jours = dates.length ? joursEnClair(dates)
    : (p && p.nature && p.nature.mensuelle ? "mois" : "");
  const moisCourt = MOIS_COURTS[parseInt(mois.slice(5, 7), 10) - 1] || "";
  // POUR QUOI. Le spectacle, puis ce qui est payé en mots.
  const paye = p ? designation(p).toLowerCase() : "";
  // Un contrat sur plusieurs mois porte sa pastille à côté du nom.
  poserChevrons(div, ligne.contrat, ligne.debut, ligne.fin);
  const net = div._total ? euros(div._total.net_paye) : "";
  const detail = [p && p.numero_bulletin ? `bulletin ${p.numero_bulletin}` : "",
                  p && p.numero_contrat ? `contrat ${p.numero_contrat}` : "",
                  `validée le ${quand(ligne.valide)}`].filter(Boolean).join(" · ");
  cible.innerHTML = `
    <div class="carte-quand" title="${echapper(dates.map(jjmmaaaa).join(", "))}">
      <b class="${jours.includes(" · ") ? "epars" : ""}">${echapper(jours)}</b><span>${echapper(moisCourt)}</span>
    </div>
    <div class="carte-qui" title="${echapper(detail)}">
      <span class="carte-nom"><b>${echapper(ligne.nom)}</b></span>
      <span class="discret">${echapper(pourQuoi(ligne))}${paye ? " · " + echapper(paye) : ""}${
        ligne.contrat && ligne.contrat.repris ? " · contrat repris de votre ancien logiciel" : ""}</span>
    </div>
    <div class="carte-actes">${bandeauEtats(ligne, false)}</div>
    <div class="carte-net" title="net à payer, avant impôt">${echapper(net)}</div>
    ${recalculDe(ligne)}${versionsDe(ligne)}
    <span class="carte-modifier">
      <button class="lien carte-imprevu" title="Un imprévu : un arrêt de travail, une absence…"
        aria-label="Un imprévu">&hellip;</button></span>`;
  // LA CARTE S'OUVRE D'UN CLIC SUR ELLE, carrés compris, toutes ses cases modifiables : un clic n'écrit rien et ne
  // fait partir aucun envoi. Rien ne s'écrit avant « Modifier l'engagement » (`ouvrirEnModification`).
  rendreOuvrable(cible, () => ouvrirEnModification(div), "Ouvrir l'engagement pour le modifier");
  // LA PORTE DES IMPRÉVUS sur un engagement validé : un arrêt ou
  // une absence arrive souvent après la paie. js/imprevus.js.
  const porte = cible.querySelector(".carte-imprevu");
  porte.onclick = (ev) => { ev.preventDefault(); ev.stopPropagation(); menuImprevu(porte, { div }); };
  const frise = cible.querySelector(".carte-versions");
  if (frise) frise.onclick = (ev) => { ev.preventDefault(); ev.stopPropagation(); boiteFrise(ligne); };
}

/* UN ENGAGEMENT VALIDÉ DONT LA PAIE S'EST RECALCULÉE D'ELLE-MÊME : le moteur dit quand et pourquoi (`recalcul` de
 * `etat-par-engagement`) ; la carte le montre tel quel. */
function recalculDe(ligne) {
  return ligne && ligne.recalcul ? `<div class="encart carte-recalcul">${echapper(ligne.recalcul)}</div>` : "";
}

/* UN ENGAGEMENT MODIFIÉ APRÈS UN ENVOI : la ligne que le moteur écrit (« Modifié le 12/11, après l'envoi du contrat »,
 * `versions.phrase` d'`etat-par-engagement`) ouvre la frise de ses versions. Jamais modifié après un envoi : rien. */
function versionsDe(ligne) {
  return ligne && ligne.versions
    ? `<button class="lien carte-versions">${echapper(ligne.versions.phrase)}</button>` : "";
}

/** La frise : par version, sa date, ce qui a changé depuis la précédente, et ce qui est parti sur elle. */
function boiteFrise(ligne) {
  const vs = ligne.versions.versions || [];
  ouvrirBoite(`
    <h3>${echapper(ligne.nom)}</h3>
    <p class="discret">${echapper(pourQuoi(ligne))}, ${echapper(periodeEnClair(ligne.debut, ligne.fin))}</p>
    <ol class="frise">${vs.map((v) => `
      <li>
        <b>Version ${echapper(v.version)}</b> <span class="discret">— le ${echapper(v.jour)}</span>
        ${v.changements.length ? `<ul class="frise-changements">${v.changements.map((c) =>
          `<li>${echapper(c)}</li>`).join("")}</ul>` : ""}
        ${v.partis.length ? `<p class="frise-partis">Parti : ${v.partis.map(echapper).join(" ; ")}</p>` : ""}
      </li>`).join("")}
    </ol>
    <div class="actions"><button class="secondaire" onclick="fermerBoite()">Fermer</button></div>`);
}

/** Les trois nombres du bloc. Une case en cours de frappe ne se repeint pas :
 *  l'aperçu revient 350 ms après la dernière touche, et il écraserait le
 *  montant qu'on est justement en train de viser. */
function peindreChiffres(div, total) {
  const champs = { net: "net_a_payer", brut: "brut", cout: "cout_employeur" };
  const net = div.querySelector(".e-net");
  if (net) {
    // Le net (ce qu'on vire) et le coût employeur (ce que la compagnie sort), tels que le moteur les somme pour la
    // carte (`blocs` de l'aperçu). Le brut reste dans l'aperçu.
    net.innerHTML = total
      ? `Net <b>${echapper(euros(total.net_paye))}</b>
         <span class="e-cout">Coût employeur <b>${echapper(euros(total.cout_employeur))}</b></span>${total.engagements > 1
          ? ` <span class="discret">pour ${total.engagements} salariés</span>` : ""}`
      : `<span class="discret">Le net s'affiche dès que l'engagement est complet.</span>`;
  }
  div.querySelectorAll(".e-vise").forEach((c) => {
    if (c === document.activeElement) return;
    c.value = total ? total[champs[c.dataset.cible]] : "";
  });
}

/* VISER UN TOTAL, ET EN DÉDUIRE LE TAUX.
 *
 * L'enveloppe vient avant la paie : on sait qu'on a 500 € pour ce plateau,
 * et on cherche ce que ça fait de cachet. La page ne calcule rien elle-même
 * — le passage du brut au coût n'est pas un pourcentage — : elle désigne les engagements du bloc, et le moteur
 * rejoue leur paie, telle qu'elle est enregistrée ou ouverte pour être modifiée, jusqu'à tomber juste.
 *
 * Le bloc entier est visé, tous ses salariés cochés compris : « 500 € pour
 * payer les gens », c'est 500 € en tout, répartis au prorata de ce qui est
 * déjà saisi. */
async function ajusterBloc(div, champ) {
  const note = div.querySelector(".e-chiffres-note");
  const montant = (champ.value || "").trim();
  // Vidée : on ne vise plus rien, et l'aperçu remettra le nombre calculé.
  if (montant === "") { rafraichirApercu(); return; }
  const blocs = [...document.querySelectorAll(".engagement")];
  let debut = 0;
  for (const b of blocs) {
    if (b === div) break;
    debut += salariesDuBloc(b).length;
  }
  const combien = salariesDuBloc(div).length;
  if (!combien) {
    note.innerHTML = `<span class="rouge">Cocher d'abord qui est payé.</span>`;
    return;
  }
  const engagements = [];
  for (let k = 0; k < combien; k++) engagements.push(debut + k);
  note.textContent = "calcul…";
  const mois = document.getElementById("mois").value;
  try {
    await enregistrerAvantDeCalculer();
    const r = await api("ajuster", {
      mois, modifiees: modifieesDeLaPage(mois), engagements,
      cible: champ.dataset.cible, montant: Number(montant),
    });
    appliquerAjustement(div, r);
    // LA CASE MONTRE CE QU'ON ATTEINT, PAS CE QU'ON A TAPÉ. Un montant visé
    // qu'on laisse à l'écran, c'est un chiffre faux de deux centimes qui
    // reste sous les yeux — et l'aperçu ne repeint pas une case où le curseur
    // est encore (on a tapé Entrée, rien n'a perdu le focus).
    champ.value = r.atteint;
    note.innerHTML = phraseAjustement(r);
  } catch (e) {
    note.innerHTML = `<span class="rouge">${echapper(e.message)}</span>`;
  }
}

/** Les taux que le moteur a trouvés, réécrits dans les cases. Les
 *  engagements d'un même bloc portent les mêmes lignes : le premier suffit. */
function appliquerAjustement(div, r) {
  const e = (r.engagements || [])[0];
  if (!e) return;
  // Les lignes que la carte a enregistrées (`lignesDuBloc`), dans le même ordre : une ligne vide n'est pas partie,
  // donc elle ne compte pas dans les rangs.
  const rangs = [...div.querySelectorAll(".rubrique")].filter(ligneEnvoyee);
  (e.lignes || []).forEach((l) => {
    const cible = rangs[l.rang];
    if (cible) cible.querySelector(".r-taux").value = l.taux;
  });
  if (e.salaire_mensuel != null) div.querySelector(".e-salaire").value = e.salaire_mensuel;
  rafraichirApercu();
}

/** Ce qui est ATTEINT, jamais ce qui a été demandé. Un taux se paie au
 *  centime : deux cachets ne tombent pas toujours sur 500,00 € pile, et la
 *  page qui afficherait le montant visé mentirait de deux centimes. */
function phraseAjustement(r) {
  const noms = { net: "Net salarié", brut: "Brut", cout: "Coût entreprise" };
  const ecart = Number(r.ecart || 0);
  return `${echapper(noms[r.cible] || r.cible)} visé ${echapper(euros(r.vise))}
    → <b>${echapper(euros(r.atteint))}</b> `
    + (ecart
        ? `<span class="discret">(${ecart > 0 ? "+" : ""}${echapper(euros(r.ecart))} —
           un taux se paie au centime)</span>`
        : `<span class="discret">— au centime près</span>`);
}

/* Ce que porte le contrat, en toutes lettres : « 2 Cachets », « 1 Journée de
 * travail, 6 h de Masterclass ». La phrase vient du MOTEUR (`remuneration`) ;
 * la page ne la fabrique pas depuis un nombre de cachets, sans quoi un contrat
 * sans cachet s'afficherait « 0 cachet(s) ». */
function designation(p) {
  const r = p.remuneration || [];
  return r.length ? r.map((l) => l.designation).join(", ") : "—";
}

/* LE MOIS, EN DEUX NOMBRES : ce qu'on vire (le net) et ce que le mois coûte à la compagnie, puis une ligne par salarié,
 * son net, le coût et le brut au survol. Le moteur les somme (`total_net`, `total_cout`, `par_salarie` de l'aperçu). */
function vueApercu(a) {
  const groupes = a.par_salarie || [];
  // Ce que les chiffres ne comptent pas (un engagement à compléter, ou aucun), le moteur le dit (`phrase_du_mois`).
  const phrase = a.phrase_du_mois ? `<p class="discret">${echapper(a.phrase_du_mois)}</p>` : "";
  if (!groupes.length) return phrase;
  return `
    <div class="mois-chiffres">
      <div><span>Net à virer</span><b>${euros(a.total_net)}</b></div>
      <div><span>Coût employeur</span><b>${euros(a.total_cout)}</b></div>
    </div>
    <ul class="mois-salaries">${groupes.map((g) => `
      <li title="coût employeur ${echapper(euros(g.cout_employeur))} · brut ${echapper(euros(g.brut))}">
        <span>${echapper(g.nom)}
          <span class="discret">· ${g.engagements} engagement${g.engagements > 1 ? "s" : ""}</span></span>
        <b>${euros(g.net_paye)}</b>
      </li>`).join("")}
    </ul>${phrase}`;
}

/* ------------------------------------------------------- charger un mois */
let MOIS_AFFICHE = null;        // le mois REELLEMENT charge dans l'ecran
let DSN_DEPOSEE = null;         // ce que le moteur dit du mois déclaré (`dsn_deposee`), ou null


function afficherRappelsMois() {
  const t = TAUX_PAR_RUBRIQUE.cachet;
  document.getElementById("rappels-mois").innerHTML =
    `${t == null ? "aucun cachet par défaut" : `cachet à <b>${echapper(euros(t))}</b> par défaut`} ·
     le lieu et les jours se posent sur chaque ligne de paie`;
}

let DEMANDE_MOIS = null;
/** `repeint` : l'état frais d'un mois déjà affiché (`repeindreLeMois`) — les blocs se refont sans se cacher, et ce qui
 *  était déplié le reste. */
async function chargerMois(mois, repeint = false) {
  DEMANDE_MOIS = mois;
  // TOUT LE MOIS D'UN COUP, lu (pas calculé) : les lectures ci-dessous le prennent dans `LU`. Le dernier état connu,
  // même périmé : il se vérifie derrière (`verifierLeMois`, plus bas).
  let lu;
  try { lu = await lireLeMois(mois, true); } catch (e) { if (DEMANDE_MOIS === mois) document.getElementById("engagements").classList.remove("en-lecture"); throw e; }
  if (DEMANDE_MOIS !== mois) return;
  if (!lu.perime) moisConfirme(mois);
  const r = await api("saisie", { mois });
  // Un mois plus récent a été demandé pendant que celui-ci arrivait : on le
  // laisse tomber, sinon il repeindrait l'écran par-dessus le bon.
  if (DEMANDE_MOIS !== mois) return;
  SAISIE = r.saisie;
  IMPORTES = r.importes || [];
  MOIS_AFFICHE = mois;
  document.getElementById("mois").value = mois;
  afficherMoisChoisi();
  // LE TAUX DU CACHET D'UNE LIGNE NEUVE, LE MOTEUR LE DIT (`defauts.taux_cachet`) ; chaque autre rubrique part du
  // dernier taux que le mois lui donne. Ce n'est qu'une commodité de saisie, à l'écran.
  const defauts = r.defauts || {};
  if (defauts.taux_cachet != null) TAUX_PAR_RUBRIQUE.cachet = +defauts.taux_cachet;
  (SAISIE.engagements || []).forEach((e) => (e.lignes || []).forEach((l) => {
    if (!(+l.taux > 0) || !l.rubrique || l.rubrique === "cachet") return;
    TAUX_PAR_RUBRIQUE[l.rubrique] = +l.taux;
  }));
  afficherRappelsMois();
  const cible = document.getElementById("engagements");
  // ON PRÉCHARGE, PUIS ON AFFICHE : « quand on
  // change de mois on arrive sur cette version toute ouverte alors que les
  // paies sont fermées en vert ». Un bloc naît ouvert ; il ne se replie
  // qu'à l'arrivée de l'état de son engagement (`etat-par-engagement`). Le démarrage
  // l'attendait déjà sous son voile ; un changement de mois ne
  // l'attendait pas. Les blocs restent donc cachés jusque-là, et l'on
  // arrive directement sur les cartes. La page ne devine pas elle-même ce
  // qui est validé : un engagement se lit validé aussi quand la DSN réelle du
  // mois est partie, et c'est le moteur qui le sait.
  //
  // LES CARTES NAISSENT FERMÉES : une coque, que l'état de son
  // engagement ouvre s'il n'est pas validé (`fermerEdition`, `ouvrirCarte`).
  // Un repeint garde ouvertes celles qui l'étaient, et leurs dépliants.
  const avant = repeint ? [...cible.querySelectorAll(".engagement")] : [];
  const deplies = repeint ? avant.map((div) =>
    [...div.querySelectorAll("details")].map((d) => d.open)) : null;
  const defilement = window.scrollY;
  if (!repeint) {
    cible.classList.add("en-lecture");
    // Les états du mois quitté ne se posent pas sur les cartes de celui-ci.
    ETATS_ENGAGEMENTS = null;
  }
  cible.innerHTML = "";
  // Une carte par engagement : une carte porte un salarié.
  const groupes = (SAISIE.engagements || []).map((e, rang) => ({ ...e, salaries: [e.salarie], rangs: [rang] }));
  const ouvertes = avant.length === groupes.length ? avant.map((div) => !carteFermee(div)) : [];
  groupes.forEach((g, i) => cible.appendChild(ligneEngagement(g, { fermee: !ouvertes[i] })));
  if (deplies) {
    const blocs = [...cible.querySelectorAll(".engagement")];
    if (blocs.length === deplies.length) {
      blocs.forEach((div, i) => div.querySelectorAll("details").forEach((d, k) => {
        if (deplies[i][k] !== undefined) d.open = deplies[i][k];
      }));
    }
    window.scrollTo(0, defilement);
  }
  // Ce qui vient d'etre charge est, par definition, ce qui est sur le disque :
  // on le note, pour que l'enregistrement automatique ne reecrive pas un
  // fichier identique a chaque fois qu'on effleure un champ.
  clearTimeout(MINUTEUR_SAISIE);
  cible.querySelectorAll(".engagement").forEach(marquerEcrite);
  // Un repeint montre ses cartes tout de suite : les états qu'on a déjà de
  // ce mois s'y posent, une coque ne reste pas vide le temps de les relire.
  if (repeint && ETATS_ENGAGEMENTS) peindreEtats();
  DSN_DEPOSEE = r.dsn_deposee || null;
  // Une confirmation ne vaut que pour LE mois confirmé : on la redonne en revenant.
  // Les blocs se montrent quand leurs états sont posés -- ou, quoi qu'il
  // arrive, au bout de quelques secondes : une lecture qui traîne ne doit
  // pas cacher la saisie.
  const montrer = () => {
    if (DEMANDE_MOIS !== mois) return;
    // Sans état lu, une carte n'a rien qui la dise validée : elle s'ouvre.
    cible.querySelectorAll(".engagement:not(.validee)").forEach(ouvrirCarte);
    cible.classList.remove("en-lecture");
  };
  setTimeout(montrer, 5000);
  // Les contrats qui courent encore arrivent AVANT les etats des engagements :
  // ceux-ci se posent sur les blocs, et un bloc absent n'aurait rien ou se
  // poser.
  try {
    await poserContratsEnCours(mois);
  } catch (e) {
    montrer();
    throw e;
  }
  if (DEMANDE_MOIS !== mois) return;
  majAvisDuMois(mois, r.existe);
  dessinerMoisImporte();
  document.getElementById("resultat").innerHTML = "";
  // L'aperçu de la saisie enregistrée est dans la lecture : il ne se recalcule que quand on modifie la saisie.
  calculerApercu(lu.lectures.apercu);
  // Ce qui est parti de ce mois-la se lit au meme moment que ce qu'il
  // contient : un ecran qui montrerait la saisie sans l'etat du journal
  // laisserait rejouer un depot deja fait.
  // Les lectures partent ensemble et on RENVOIE leur fin : le demarrage
  // l'attend pour ne montrer la page qu'une fois peinte (« je vois les paies
  // ouvertes qui se ferment », 22/09/2026). Les autres appelants l'ignorent.
  const lectures = [
    dessinerActions(mois),
    dessinerEmbauches(mois),
    // Les imprévus et les fins de contrat : ce qui est dû, et la ligne de
    // chaque imprévu sous sa carte (js/imprevus.js).
    dessinerImprevus(mois),
    // Ou en est CHAQUE engagement. Se lit apres le chargement des blocs : les etats
    // se posent sur eux, et un bloc absent n'aurait rien ou se poser.
    dessinerEtatsDesEngagements(mois).finally(montrer),
  ];
  const peint = Promise.allSettled(lectures);
  // UN ÉTAT PÉRIMÉ SE VÉRIFIE DERRIÈRE : le moteur le refait déjà ; on attend l'état frais, et l'écran se
  // repeint de lui-même s'il a changé.
  if (lu.perime && !repeint) verifierLeMois(mois, peint);
  return peint;
}

/** L'ÉTAT FRAIS D'UN MOIS DÉJÀ AFFICHÉ. Rien n'a changé : rien ne bouge. Sinon on repeint ce qui vient
 *  du moteur — les états des engagements, les actes, les embauches, les imprévus, l'aperçu — sans refaire les
 *  blocs ; ceux-ci ne se refont que si la saisie lue elle-même a changé (un fichier modifié hors de cette page), et
 *  jamais par-dessus une saisie en cours, qui reste ce qu'elle est et s'enregistre comme d'habitude. */
async function repeindreLeMois(mois, ancien, frais) {
  const memes = (k) => JSON.stringify(ancien.lectures[k]) === JSON.stringify(frais.lectures[k]);
  const cles = new Set([...Object.keys(ancien.lectures), ...Object.keys(frais.lectures)]);
  if ([...cles].every(memes)) return;
  const enCours = !toutEstEcrit();
  const saisieDe = (lu) => JSON.stringify(((lu.lectures.saisie || {}).resultat || {}).saisie);
  if (!enCours && (saisieDe(ancien) !== saisieDe(frais) || !memes("contrats-en-cours"))) {
    await chargerMois(mois, true);
    return;
  }
  // La saisie lue n'a pas changé (ou l'on est en train d'en taper une) : ce que le moteur en dit seulement.
  try {
    const r = await api("saisie", { mois });
    if (MOIS_AFFICHE !== mois) return;
    const defauts = r.defauts || {};
    if (defauts.taux_cachet != null) TAUX_PAR_RUBRIQUE.cachet = +defauts.taux_cachet;
    afficherRappelsMois();
    DSN_DEPOSEE = r.dsn_deposee || null;
    IMPORTES = r.importes || [];
    majAvisDuMois(mois, r.existe);
    dessinerMoisImporte();
  } catch (e) { /* la saisie se lit telle quelle : rien à changer */ }
  if (!enCours) calculerApercu(frais.lectures.apercu);
  await Promise.allSettled([
    dessinerActions(mois),
    dessinerEmbauches(mois),
    dessinerImprevus(mois),
    dessinerEtatsDesEngagements(mois),
  ]);
}

/** Ce que l'écran dit d'un mois, au-dessus de ses engagements. Ce que le moteur refuse d'y écrire (un mois qui
 *  porte des engagements importés, un mois d'avant la première embauche), il le dit à l'écriture. */
function majAvisDuMois(mois, existe) {
  majAvisDsn(mois);
  majAvisDemo(mois);
  noteSaisie(existe
    ? "Saisie chargée. L'enregistrement est automatique."
    : "Nouveau mois — l'enregistrement est automatique dès la première saisie.");
}

/** Ce que le moteur dit d'un mois dont la DSN est déposée, au-dessus des engagements : l'annule-et-remplace que la
 *  modification fera partir, ou que la modification se confirme et se corrige dans la DSN suivante. */
function majAvisDsn(mois) {
  const avis = document.getElementById("avis-dsn");
  const d = MOIS_AFFICHE === mois ? DSN_DEPOSEE : null;
  avis.hidden = !d;
  if (!d) return;
  avis.className = "encart " + (ENCART_DE_COULEUR[d.couleur] ?? "");
  avis.textContent = d.phrase;
}

/** UN MOIS SAISI EN DÉMO, dans une compagnie gardée au déblocage : la mention du moteur (route `etat`,
 *  `demo`) — ses engagements n'ont été ni déclarées ni envoyées. Les pastilles disent ce qui reste à faire. */
function majAvisDemo(mois) {
  const avis = document.getElementById("avis-demo");
  const d = ETAT && ETAT.demo;
  avis.hidden = !(d && (d.mois || []).includes(mois));
  if (!avis.hidden) avis.textContent = d.phrase;
}

/* LES ENGAGEMENTS IMPORTÉS QUI TOUCHENT LE MOIS (`importes` de la route `saisie`, `import::mois::cartes`) : une carte
 * fermée par engagement, comme celle d'un engagement validé : quand, qui et pour quoi, ses états, son net ; un clic sur
 * la carte montre chaque case remplie de ce que l'import a lu, avec d'où elle vient. Ce que l'import n'a pas pu lire
 * se dit sur la carte, case par case, et pourquoi. La page n'en décide rien : elle affiche. */
let IMPORTES = [];

const CASES_IMPORTEES = [
  ["brut", "Brut"], ["net_imposable", "Net imposable"], ["net_a_payer", "Net à payer"],
  ["pas", "Prélèvement à la source"], ["net_verse", "Net versé (DSN)"],
  ["cotisations_salariales", "Cotisations salariales"], ["cotisations_patronales", "Cotisations patronales"],
  ["cout_employeur", "Coût employeur"],
];
const SOURCES_IMPORTEES = { bulletin: "bulletin", DSN: "DSN", AEM: "AEM", historique: "historique" };

function dessinerMoisImporte() {
  const cible = document.getElementById("mois-importe");
  if (!IMPORTES.length) { cible.hidden = true; cible.innerHTML = ""; return; }
  const moisCourt = MOIS_COURTS[parseInt(MOIS_AFFICHE.slice(5, 7), 10) - 1] || "";
  cible.innerHTML = `
    <p class="mi-tete"><b>Importé${IMPORTES.length > 1 ? "s" : ""}</b> <span class="discret">— ${IMPORTES.length}
      engagement${IMPORTES.length > 1 ? "s" : ""} de l'ancien logiciel</span></p>
    ${IMPORTES.map((p, i) => carteImportee(p, i, moisCourt)).join("")}`;
  ouvrirLesCartesImportees(cible);
  cible.hidden = false;
}

/** La carte s'ouvre d'un clic sur elle (`rendreOuvrable`) : ses cases se déplient, un second clic les replie. */
function ouvrirLesCartesImportees(cible) {
  cible.querySelectorAll(".carte-importee").forEach((div) => {
    const cases = div.querySelector(".ci-cases");
    rendreOuvrable(div.querySelector(".e-resume"), () => {
      cases.hidden = !cases.hidden;
      div.classList.toggle("ouverte", !cases.hidden);
    }, "Voir ou replier les cases de l'engagement");
  });
}

function carteImportee(p, i, moisCourt) {
  // QUAND : les jours travaillés que le bulletin imprime ; sans eux, la période du contrat, telle que le moteur la dit.
  const jours = p.dates.length ? joursEnClair(p.dates)
    : (p.periode || "");
  const quoi = [p.spectacle.titre || p.spectacle.numero_objet,
    p.cachets ? `${p.cachets} cachet${p.cachets > 1 ? "s" : ""}` : "",
    p.heures ? `${p.heures} h` : ""].filter(Boolean).join(" · ");
  const pastille = (cle, titre) => {
    const e = p.etats[cle] || {};
    return `<span class="etat-acte ${e.couleur || "gris"}" title="${echapper(e.phrase || "")}">
      <i class="carre"></i>${echapper(titre)}</span>`;
  };
  // LE MONTANT : le net à payer ; sans lui (une DSN seule ne le déclare pas), le brut, et la carte le dit.
  const net = p.montants.net_a_payer ? euros(p.montants.net_a_payer)
    : p.montants.brut ? `<span class="discret">brut</span> ${euros(p.montants.brut)}` : "";
  // CE QUI MANQUE SE DIT SUR LA CARTE, en une ligne (quels champs) ; le pourquoi est sous chaque case vide.
  const manque = p.manque.length ? `
    <div class="ci-manque"><b>Non lu par l'import :</b> ${p.manque.map((m) => echapper(m.libelle)).join(" · ")}
      <span class="discret">— un clic sur la carte montre ses cases, et sous chacune pourquoi</span></div>` : "";
  const dits = new Set();
  const casePlate = (libelle, valeur, source, champ) => {
    const absent = valeur ? null : p.manque.find((m) => m.champ === champ);
    const pourquoi = absent && !dits.has(champ) ? (dits.add(champ), `<span class="ci-pourquoi">${echapper(absent.phrase)}</span>`) : "";
    return `<div class="ci-case${valeur ? "" : " vide"}${absent ? " non-lu" : ""}"><span class="champ">${echapper(libelle)}</span>
      <span class="ci-valeur">${valeur ? echapper(valeur) : `<i>${absent ? "non lu" : "—"}</i>`}</span>${
      valeur && source ? `<span class="ci-source">${echapper(source)}</span>` : ""}${pourquoi}</div>`;
  };
  const lignes = (p.lignes || []).length ? `
    <table class="ci-lignes"><tr><th>Ligne du bulletin</th><th class="nombre">Quantité</th><th class="nombre">Taux</th><th class="nombre">Montant</th></tr>
      ${p.lignes.map((l) => `<tr><td>${echapper(l.libelle || "")}</td><td class="nombre">${echapper(l.quantite || "")}</td>
        <td class="nombre">${l.taux ? echapper(euros(l.taux)) : ""}</td><td class="nombre">${l.montant ? echapper(euros(l.montant)) : ""}</td></tr>`).join("")}
    </table>` : "";
  const origine = [p.declare_par === "DSN" ? "DSN" : "", p.bulletin ? `bulletin${p.bulletin.numero ? " " + p.bulletin.numero : ""}` : "",
    p.atteste_par === "AEM" ? "AEM" : ""].filter(Boolean).join(" + ");
  return `
    <div class="carte-importee${p.manque.length ? " incomplete" : ""}" data-rang="${i}">
      <div class="e-resume">
        <div class="carte-quand" title="${echapper(p.dates.length ? p.dates.map(jjmmaaaa).join(", ") : "début et fin du contrat : les jours exacts ne sont pas lus")}">
          <b class="${jours.includes(" · ") || jours.includes(" – ") ? "epars" : ""}">${echapper(jours)}</b><span>${echapper(moisCourt)}</span></div>
        <div class="carte-qui" title="${echapper(origine ? "lu dans : " + origine : "")}">
          <span class="carte-nom"><b>${echapper(p.nom)}</b> <span class="mot-etat">importé</span></span>
          <span class="discret">${echapper(quoi || "spectacle non lu")}</span></div>
        <div class="carte-actes"><div class="etats-actes">${pastille("dpae", "DPAE")}${pastille("signature", "Contrat")}${pastille("documents", "Documents")}</div></div>
        <div class="carte-net" title="${p.montants.net_a_payer ? "net à payer" : "brut : le net à payer n'est pas lu"}">${net}</div>
      </div>
      ${manque}
      <div class="ci-cases" hidden>
        <div class="ci-grille">
          ${casePlate("Salarié", p.nom, "fiche", "salarie")}
          ${casePlate("Spectacle", [p.spectacle.titre, p.spectacle.numero_objet && p.spectacle.numero_objet !== p.spectacle.titre ? `n° ${p.spectacle.numero_objet}` : ""].filter(Boolean).join(" — "), "", "spectacle")}
          ${casePlate("Contrat", p.contrat, "", "contrat")}
          ${casePlate("Début", p.debut ? jjmmaaaa(p.debut) : "", "", "debut")}
          ${casePlate("Fin", p.fin ? jjmmaaaa(p.fin) : "", "", "fin")}
          ${casePlate("Jours travaillés", p.dates.map((d) => d.slice(8, 10)).join(" · "), "bulletin", "dates")}
          ${casePlate("Cachets", p.cachets != null ? String(p.cachets) : "", p.atteste_par || "", "cachets")}
          ${casePlate("Heures", p.heures != null ? String(p.heures) : "", p.atteste_par || "", "cachets")}
          ${CASES_IMPORTEES.map(([k, mot]) => casePlate(mot, p.montants[k] ? euros(p.montants[k]) : "",
              SOURCES_IMPORTEES[p.sources[k]] || "", k)).join("")}
        </div>
        ${lignes}
      </div>
    </div>`;
}

/* Changer de mois écrit d'abord LES CARTES CHANGÉES, sous leur vrai mois. La case a déjà changé quand on arrive ici :
 * on y remet l'ancien mois le temps de les lire, sans quoi les engagements d'août partiraient dans le fichier de
 * septembre — la case du mois est ce qui date les jours joués. */
document.getElementById("mois").onchange = (e) => {
  fermerCalendriers();
  const neuf = e.target.value;
  let attente = [];
  if (MOIS_AFFICHE && MOIS_AFFICHE !== neuf) {
    clearTimeout(MINUTEUR_SAISIE);
    e.target.value = MOIS_AFFICHE;
    attente = ecrituresEnAttente();
    e.target.value = neuf;
  }
  // LE NOUVEAU MOIS N'ATTEND PAS L'ÉCRITURE DE L'ANCIEN : les cartes quittées partent au moteur, et le nouveau mois
  // s'affiche en même temps. Une fois écrites, le mois affiché se vérifie : elles ont pu le changer (un contrat en cours).
  const chargement = chargerMois(neuf);
  if (attente.length) enregistrerEnPartant(attente, chargement);
};

/** Les cartes d'un mois qu'on vient de quitter : écrites sans repeindre (l'écran montre déjà un autre mois), puis le
 *  mois affiché vérifié. Un échec se dit : c'est du travail qu'on croit posé. */
async function enregistrerEnPartant(attente, chargement) {
  const mois = attente[0][1].mois;
  try {
    for (const [div, charge] of attente) await ecrireLaCarte(div, charge, appelerMoteur);
    oublierCouts();
    appelerMoteur("etat").then((e) => { ETAT.historique = e.historique; }, () => {});
  } catch (e) {
    dire(`${moisEnClair(mois)} : non enregistré — ${e.message}`, "erreur");
    return;
  }
  // Le mois affiché se vérifie quand il est peint (la vérification part tout de suite : le moteur refait l'état).
  if (DEMANDE_MOIS) verifierLeMois(DEMANDE_MOIS, chargement.catch(() => {}));
}

/* LE CHOIX DU MOIS. « Un truc simple, qu'on puisse cliquer sur
 * toute la case pour arriver aux choix, et que le défilement soit plus
 * rapide. » Une case : ‹ Octobre 2026 ›. Un clic sur une flèche charge le mois
 * TOUT DE SUITE (16/10/2026 : plus d'attente pour voir si un autre clic
 * suit) ; un clic qui arrive pendant le chargement le fait abandonner
 * (`DEMANDE_MOIS`, chargerMois), et c'est le dernier mois demandé qui se
 * peint. Un clic sur le nom ouvre l'année : douze mois, un clic pour y
 * aller. Un point marque les mois qui ont une saisie.
 *
 * La valeur reste dans le champ caché #mois et passe par son événement
 * « change » : la saisie à l'écran s'enregistre d'abord sous son vrai mois. */
let ANNEE_GRILLE = null;

/** Le nom dans la case — appelé partout où la valeur change. */
function afficherMoisChoisi() {
  const x = moisLu(document.getElementById("mois").value);
  const nom = document.getElementById("mois-nom");
  nom.textContent = x ? `${MOIS_FR[x[1] - 1].replace(/^./, (c) => c.toUpperCase())} ${x[0]}` : "Choisir un mois";
}

/** Aller à un mois : le nom et le chargement, tout de suite. */
function allerAuMois(v) {
  const c = document.getElementById("mois");
  c.value = v;
  afficherMoisChoisi();
  // LE NOM ET LES CARTES DISENT LE MÊME MOIS (09/10/2026 : « quand je change de
  // mois on reste sur septembre ») : dès que le nom change, les cartes de
  // l'ancien mois s'effacent ; elles reviennent si l'on revient dessus.
  document.getElementById("engagements").classList.toggle("en-lecture", v !== MOIS_AFFICHE);
  // Revenir au mois affiché pendant qu'un autre se charge, c'est aussi le
  // demander : sans quoi l'autre finirait de se peindre sous ce nom-ci.
  if (v !== MOIS_AFFICHE || DEMANDE_MOIS !== MOIS_AFFICHE) c.dispatchEvent(new Event("change"));
}

function moisVoisin(pas) {
  const x = moisLu(document.getElementById("mois").value);
  if (!x) return;
  const d = new Date(x[0], x[1] - 1 + pas, 1);
  allerAuMois(moisEnTexte(d.getFullYear(), d.getMonth() + 1));
}

function dessinerGrilleMois() {
  const grille = document.getElementById("mois-grille");
  const choisi = document.getElementById("mois").value;
  const ceMois = aujourdhuiIso().slice(0, 7);   // le mois du moteur
  const saisis = new Set((ETAT.historique || []).map((h) => h.mois));
  grille.innerHTML = `
    <div class="choix-mois-annee">
      <button type="button" class="choix-mois-pas" data-annee="-1" aria-label="Année précédente">&lsaquo;</button>
      <b>${ANNEE_GRILLE}</b>
      <button type="button" class="choix-mois-pas" data-annee="1" aria-label="Année suivante">&rsaquo;</button>
    </div>
    <div class="choix-mois-douze">${MOIS_ABR.map((n, i) => {
      const v = moisEnTexte(ANNEE_GRILLE, i + 1);
      const classes = [v === choisi ? "choisi" : "", v === ceMois ? "maintenant" : "",
                       saisis.has(v) ? "saisi" : ""].join(" ");
      return `<button type="button" class="${classes}" data-mois="${v}"
                title="${MOIS_FR[i]} ${ANNEE_GRILLE}${saisis.has(v) ? " — saisi" : ""}">${n}</button>`;
    }).join("")}</div>`;
  grille.querySelectorAll("[data-annee]").forEach((b) => {
    b.onclick = (e) => { e.stopPropagation(); ANNEE_GRILLE += +b.dataset.annee; dessinerGrilleMois(); };
  });
  grille.querySelectorAll("[data-mois]").forEach((b) => {
    b.onclick = () => { fermerGrilleMois(); allerAuMois(b.dataset.mois); };
  });
}

function ouvrirGrilleMois() {
  const x = moisLu(document.getElementById("mois").value);
  ANNEE_GRILLE = x ? x[0] : anneeDuJour();
  dessinerGrilleMois();
  document.getElementById("mois-grille").hidden = false;
  document.getElementById("mois-nom").setAttribute("aria-expanded", "true");
}
function fermerGrilleMois() {
  document.getElementById("mois-grille").hidden = true;
  document.getElementById("mois-nom").setAttribute("aria-expanded", "false");
}

document.getElementById("mois-avant").onclick = () => moisVoisin(-1);
document.getElementById("mois-apres").onclick = () => moisVoisin(1);
document.getElementById("mois-nom").onclick = () =>
  (document.getElementById("mois-grille").hidden ? ouvrirGrilleMois() : fermerGrilleMois());
// Les flèches du clavier sur la case font défiler, comme les boutons.
document.getElementById("mois-nom").onkeydown = (e) => {
  if (e.key === "ArrowLeft") { e.preventDefault(); moisVoisin(-1); }
  if (e.key === "ArrowRight") { e.preventDefault(); moisVoisin(1); }
};
document.addEventListener("click", (e) => {
  if (!e.target.closest("#choix-mois")) fermerGrilleMois();
});
document.addEventListener("keydown", (e) => { if (e.key === "Escape") fermerGrilleMois(); });

/* UN CLIC SUR UNE DATE OUVRE LE CALENDRIER, PARTOUT. Sans cela il
 * fallait viser la petite icône au bout de la case. */
document.addEventListener("click", (e) => {
  const c = e.target;
  if (!(c instanceof HTMLInputElement) || !/^(date|month|datetime-local)$/.test(c.type)) return;
  if (c.disabled || c.readOnly || typeof c.showPicker !== "function") return;
  try { c.showPicker(); } catch (err) { /* déjà ouvert, ou refusé : l'icône reste */ }
});

/* Quitter l'onglet ou fermer la fenêtre : on écrit ce qui attend encore. Une
 * seconde de battement suffit à perdre une frappe si on ferme juste après. */
document.addEventListener("visibilitychange", () => {
  if (document.visibilityState === "hidden") {
    clearTimeout(MINUTEUR_SAISIE);
    enregistrerSaisie(true);
  }
});



/* ------------------------------------------- l'enregistrement automatique
   « Je comprends pas les boutons en bas produire le mois et enregistrer la
   saisie, tout est déjà dans les cases ».

   Il a raison, et le bouton est parti. Ce qui est tapé dans une case EST la
   saisie ; demander un clic pour le confirmer, c'était demander de tenir en
   tête un état — « est-ce que j'ai enregistré ? » — que la machine connaît
   mieux. Trois choses pour que ça reste honnête :

   1. UNE SECONDE APRÈS LA DERNIÈRE FRAPPE, pas à chaque touche. Écrire le
      fichier vingt fois pendant qu'on tape un nom ne sert personne.
   2. LA LIGNE D'ÉTAT DIT L'HEURE. « Enregistré à 14:32 » — sans quoi
      l'enregistrement automatique est un enregistrement dont on ne sait pas
      s'il a eu lieu, c'est-à-dire pire que le bouton.
   3. RIEN NE PART POUR AUTANT. Enregistrer, c'est écrire les engagements (`engagements/`)
      et rien d'autre : ce qui part se fabrique à son départ. */
let MINUTEUR_SAISIE = null;

function noteSaisie(texte) {
  document.getElementById("note-saisie").textContent = texte;
}

/* UNE CARTE ÉCRIT SON ENGAGEMENT, ET LUI SEUL (`enregistrer-saisie`). Elle est désignée par sa place dans l'écran du
 * mois (`data-place`), que le moteur a rendue : la lecture du mois pour une carte lue, la première écriture pour une
 * carte neuve. Ce qu'elle a écrit en dernier (`_ecrite`) dit si elle a changé depuis. */

/** Une carte en saisie : son formulaire est construit, et son engagement n'est pas validé (un engagement validé se
 *  modifie d'un geste, « Modifier l'engagement »). */
function carteEnSaisie(div) {
  return !carteFermee(div) && !div.classList.contains("en-modification") && !(div._ligne && div._ligne.valide);
}

/** Ce que la carte écrit (`{mois, saisie, place}`), ou `null` : une carte qui n'est pas en saisie, ou sans salarié. */
function chargeDeLaCarte(div, mois) {
  if (!mois || !carteEnSaisie(div)) return null;
  const saisie = engagementsDuBloc(div, mois)[0];
  if (!saisie) return null;
  const charge = { mois, saisie };
  if (div.dataset.place) charge.place = +div.dataset.place;
  return charge;
}

/** La carte telle qu'elle est est celle qui est écrite (une carte qu'on vient d'ouvrir sur ce qui est enregistré). */
function marquerEcrite(div) {
  const c = chargeDeLaCarte(div, document.getElementById("mois").value);
  div._ecrite = c ? JSON.stringify(c) : undefined;
}

/** Les cartes du mois affiché qui ont changé depuis leur dernière écriture, chacune avec ce qu'elle écrit. */
function ecrituresEnAttente() {
  const mois = document.getElementById("mois").value;
  return [...document.querySelectorAll("#engagements .engagement")]
    .map((div) => [div, chargeDeLaCarte(div, mois)])
    .filter(([div, c]) => c && JSON.stringify(c) !== div._ecrite);
}

/** Tout ce qui est à l'écran est-il écrit ? */
function toutEstEcrit() {
  return !ecrituresEnAttente().length;
}

/** Écrit une carte. Rend la réponse du moteur ; la carte garde sa place et ce qu'elle a écrit. */
async function ecrireLaCarte(div, charge, appel = api) {
  const r = await appel("enregistrer-saisie", charge);
  div.dataset.place = String(r.place);
  div._ecrite = JSON.stringify({ ...charge, place: r.place });
  return r;
}

/** Écrit les cartes qui ont changé. Rend true si une carte a été écrite.
 *
 *  UN ENREGISTREMENT À LA FOIS. L'enregistrement part une seconde après la dernière frappe ; les écritures se suivent :
 *  chacune attend la fin de la précédente, puis compare ce qui est à l'écran à ce qui vient d'être écrit — rien ne
 *  repart pour rien. */
let ENREGISTREMENT = Promise.resolve(false);
function enregistrerSaisie(silencieux) {
  const tour = ENREGISTREMENT.then(() => enregistrerLesCartes(silencieux));
  ENREGISTREMENT = tour.catch(() => false);
  return tour;
}

async function enregistrerLesCartes(silencieux) {
  const attente = ecrituresEnAttente();
  if (!attente.length) return false;
  const mois = attente[0][1].mois;
  let aCompleter = 0;
  let heure = "";
  for (const [div, charge] of attente) {
    try {
      const r = await ecrireLaCarte(div, charge);
      if (r.a_completer) aCompleter += 1;
      heure = r.enregistre_a.slice(11, 16);
    } catch (e) {
      // Un échec d'enregistrement ne peut pas rester discret : c'est du travail qu'on croit posé et qui ne l'est pas.
      noteSaisie("Non enregistré — " + e.message);
      if (!silencieux) dire(e.message, "erreur");
      return false;
    }
  }
  oublierCouts();
  // L'heure du moteur (`enregistre_a`), pas celle du navigateur : au banc, le temps est simulé. Un engagement auquel il
  // manque un fait s'enregistre aussi : sa carte dit ce qui manque.
  noteSaisie("Enregistré à " + heure + (aCompleter
    ? ` — ${aCompleter > 1 ? `${aCompleter} engagements à compléter avant de les valider` : "un engagement à compléter avant de le valider"}.`
    : "."));
  ETAT.historique = await api("etat").then((e) => e.historique);
  // Ce qui vient d'être enregistré compte dans la colonne « ce qui part », et la fin d'un contrat comme les jours payés
  // pendant un imprévu se lisent sur ce qui est enregistré.
  await rafraichirActions();
  await dessinerImprevus(mois);
  return true;
}

/** Appelée à chaque frappe. Enregistre une seconde après la dernière. */
function enregistrerPlusTard() {
  clearTimeout(MINUTEUR_SAISIE);
  noteSaisie("Modifié…");
  MINUTEUR_SAISIE = setTimeout(() => { MINUTEUR_SAISIE = null; enregistrerSaisie(true); }, 1000);
}

/* LA PAGE QU'ON QUITTE (recharger, fermer) dans la seconde qui suit une frappe : chaque carte changée part au moteur tout
 * de suite, et la requête survit à la page (`keepalive`). */
window.addEventListener("pagehide", () => {
  if (!MINUTEUR_SAISIE) return;
  clearTimeout(MINUTEUR_SAISIE);
  MINUTEUR_SAISIE = null;
  ecrituresEnAttente().forEach(([, charge]) => {
    fetch("/api/enregistrer-saisie?jeton=" + encodeURIComponent(JETON), {
      method: "POST", keepalive: true,
      headers: { "Content-Type": "application/json", "X-Jeton": JETON },
      body: JSON.stringify(charge),
    }).catch(() => {});
  });
});

