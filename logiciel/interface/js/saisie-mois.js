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

/** Les engagements qu'un bloc écrit : un par salarié coché. */
function engagementsDuBloc(div, mois) {
  if (carteFermee(div)) return engagementsLus(div).map((e) => JSON.parse(JSON.stringify(e)));
  // L'ENGAGEMENT N'ÉCRIT PLUS DE `dates` : chacune de ses lignes porte les
  // siennes, avec son lieu, et le moteur en fait la réunion
  // (`operations.dates_engagement`).
  //
  // UN ENGAGEMENT PAR SALARIÉ COCHÉ. Le bloc en porte plusieurs à l'écran
  // pour aller vite ; le fichier, lui, garde le format du moteur — un
  // salarié, un engagement. Chacun reçoit sa copie de ce que le formulaire
  // dit : un bloc ne regroupe que des engagements identiques (`regrouper`).
  const spectacle = div.querySelector(".e-spectacle").value;
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
  const dateModification = div.querySelector(".e-date-modification").value;
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
    if (nat.mensuelle) {
      // LES FAITS, PAS LES LIGNES : `lignes_du_mois` en fait des lignes de
      // bulletin — le prorata d'un mois incomplet, l'indemnité de congés et
      // la retenue qui la couvre, les imprévus, le solde de tout compte.
      // Écrire `lignes` ici couperait tout ce calcul.
      const recours = div.querySelector(".e-recours").value;
      if (recours) e.motif_recours = recours;
      if (salaire !== "") e.salaire_mensuel = +salaire;
      if (quotite !== "") e.quotite_contrat = +quotite;
      if (dateModification) e.date_modification = dateModification;
      const avant = avantDuBloc(div);
      if (avant) e.avant = avant;
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

function saisieCourante() {
  const mois = document.getElementById("mois").value;
  const engagements = [...document.querySelectorAll(".engagement")]
    .flatMap((div) => engagementsDuBloc(div, mois));
  // `taux_cachet` et `date_paiement` ne se saisissent plus mais restent
  // ÉCRITS : le moteur s'en sert — l'un comme défaut d'un engagement qui ne
  // dit pas son taux, l'autre comme date de paiement du mois (rappelée sous
  // le mois). Le spectacle et le lieu du mois, le moteur les pose sur chaque
  // engagement à la lecture (route `saisie`) : rien d'autre ne s'écrit ici.
  return { mois, taux_cachet: DERNIER_TAUX, date_paiement: SAISIE_DATE_PAIEMENT, engagements };
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
  const saisie = saisieCourante();
  const cible = document.getElementById("apercu");
  document.getElementById("apercu-sous-titre").textContent = IMPORTE ? "tenu avant PuppetPaie" : "";
  if (IMPORTE) {
    // Un mois tenu avant PuppetPaie ne se calcule pas ici : ses paies sont celles de l'ancien logiciel.
    const n = IMPORTE.paies.length;
    cible.innerHTML = `<p class="discret">Mois tenu avant PuppetPaie : ${n} paie${n > 1 ? "s" : ""}
      lue${n > 1 ? "s" : ""} dans l'historique importé, rien ne se calcule ici.${IMPORTE.incompletes
      ? ` ${IMPORTE.incompletes} carte${IMPORTE.incompletes > 1 ? "s disent" : " dit"} ce que l'import n'a pas pu lire.` : ""}</p>`;
    return;
  }
  // Dès qu'un engagement nomme son salarié, le moteur dit ce que le mois en compte et ce qui lui manque (`apercu`).
  if (!saisie.engagements.length) {
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
      if (deLu && (deLu.paies || []).length !== saisie.engagements.length) deLu = null;
    }
    const a = deLu ? structuredClone(deLu) : await api("apercu", { saisie, blocs: tailles });
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
        div._rupture = null;
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
      majRupture(div);
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

/* LES ENGAGEMENTS D'UN APERÇU, RENDUES À LEUR BLOC. Un bloc qui porte trois
 * salariés cochés a produit TROIS engagements — `saisieCourante` les
 * démultiplie, parce que le moteur ne connaît qu'un salarié par engagement.
 * Compter un par bloc décalait donc tout ce qui suit dès le deuxième, et
 * l'écran posait la ligne « bulletin … » du voisin sous le mauvais nom. */
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
 * « Modifier… » rouvre la saisie entière (et « Retirer » avec elle).
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
  const net = div._total ? euros(div._total.net_a_payer) : "";
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
  // VOIR SANS MODIFIER (l'utilisateur, 29/09/2026) : relire les heures d'un
  // service de répétition ne doit pas demander de rouvrir l'engagement. LA CARTE
  // S'OUVRE D'UN CLIC SUR ELLE, carrés compris : un clic sur la carte ne fait partir aucun envoi.
  // « Modifier » est dans la boîte du détail.
  rendreOuvrable(cible, () => boiteDetail(div._ligne, div._paies || []), "Ouvrir le détail de la paie");
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

/** Un jour d'une ligne, nu ou avec son propre lieu (`{"jour", "lieu"}`). */
const jourEnClair = (x) => jjmmaaaa(jourDe(x)) + (x && typeof x === "object" && x.lieu ? ` (${x.lieu})` : "");

/** Le détail d'un engagement validé, en lecture : ce qui est payé ligne par
 *  ligne, les jours, les montants. « Modifier… » dedans. */
function boiteDetail(ligne, ps) {
  const nombre = (x) => (x === null || x === undefined || x === "") ? ""
    : String(x).replace(".", ",");
  const bloc = (p) => {
    const lignes = (p.remuneration || []).map((r) => `<tr>
        <td>${echapper(r.designation)}</td>
        <td class="nombre">${echapper(nombre(r.quantite))}</td>
        <td class="nombre">${r.taux ? echapper(euros(r.taux)) : ""}</td>
        <td class="nombre">${echapper(euros(r.montant))}</td></tr>`).join("");
    const dates = (p.dates_travail || []).map(jjmmaaaa).join(", ");
    // CE QUI A ÉTÉ SAISI, ligne par ligne (30/09/2026 : « je vois 2 services
    // mais pas combien d'heures chaque service ; je veux tout voir sans avoir
    // à modifier »). Le moteur rend les heures par unité ET leur total.
    const h = (x) => nombre(x) + " h";
    const saisies = (p.lignes_saisies || []).map((l) => `
      <div class="detail-saisie">
        <b>${echapper(nombre(l.quantite))} × ${echapper(l.libelle || l.rubrique)}</b>
        à ${echapper(euros(l.taux))}
        ${l.heures_par_unite ? ` — <b>${echapper(h(l.heures_par_unite))}</b>
          ${l.unite === "heure" ? "" : `par ${echapper(l.unite || "unité")}`}` : ""}
        ${l.heures ? `, ${echapper(h(l.heures))} en tout` : ""}
        ${l.dates.length ? `<br><span class="discret">Jours :</span> ${echapper(l.dates.map(jourEnClair).join(", "))}` : ""}
        ${l.planning.length ? `<br><span class="discret">Prévu ensuite :</span> ${echapper(l.planning.map(jourEnClair).join(", "))}` : ""}
        ${l.lieu ? `<br><span class="discret">Lieu :</span> ${echapper(l.lieu)}` : ""}
      </div>`).join("");
    const refs = [p.numero_contrat ? `contrat n° ${p.numero_contrat}` : "",
                  p.numero_bulletin ? `bulletin n° ${p.numero_bulletin}` : "",
                  p.nature && p.nature.libelle ? p.nature.libelle : ""].filter(Boolean).join(" · ");
    return `
      ${refs ? `<p class="discret">${echapper(refs)}</p>` : ""}
      ${saisies}
      <p>${dates ? `<b>Jours travaillés :</b> ${echapper(dates)}<br>` : ""}
        ${p.heures ? `<b>Heures déclarées :</b> ${echapper(nombre(p.heures))}<br>` : ""}
        ${p.cachets ? `<b>Cachets :</b> ${echapper(nombre(p.cachets))}<br>` : ""}
        ${p.lieu && !saisies ? `<b>Lieu :</b> ${echapper(p.lieu)}<br>` : ""}
        ${p.emploi && p.emploi.libelle ? `<b>Emploi :</b> ${echapper(p.emploi.libelle)}` : ""}
        ${p.emploi && p.emploi.cadre ? " (cadre)" : ""}</p>
      <table class="tableau-detail">
        <thead><tr><th>Ce qui est payé</th><th class="nombre">Nombre</th>
          <th class="nombre">Taux</th><th class="nombre">Montant</th></tr></thead>
        <tbody>${lignes}</tbody>
        <tfoot>
          <tr><td colspan="3">Brut</td><td class="nombre">${echapper(euros(p.brut))}</td></tr>
          <tr><td colspan="3"><b>Net à payer</b></td><td class="nombre"><b>${echapper(euros(p.net_a_payer))}</b></td></tr>
          <tr><td colspan="3" class="discret">Coût employeur</td><td class="nombre discret">${echapper(euros(p.cout_employeur))}</td></tr>
        </tfoot>
      </table>`;
  };
  ouvrirBoite(`
    <h3>${echapper(ligne.nom)}</h3>
    <p class="discret">${echapper(pourQuoi(ligne))}, ${echapper(periodeEnClair(ligne.debut, ligne.fin))}
      · validée le ${echapper(quand(ligne.valide))}</p>
    ${recalculDe(ligne)}
    ${ps.length ? ps.map(bloc).join("") : `<p class="discret">Le détail n'est pas encore calculé.</p>`}
    <div class="actions">
      ${(ligne.carres || []).filter((c) => c.lien).map((c) =>
        `<button class="lien d-pieces" data-acte="${echapper(c.cle)}">${echapper(c.lien)}</button>`).join("")}
      <button class="secondaire" onclick="fermerBoite()">Fermer</button>
      <button class="modifier" id="d-modifier">${CRAYON}Modifier l'engagement…</button>
    </div>`);
  const m = document.getElementById("d-modifier");
  if (m) m.onclick = () => { fermerBoite(); boiteModifier(ligne); };
  document.querySelectorAll(".d-pieces").forEach((b) => {
    b.onclick = () => { fermerBoite(); boitePieces(ligne, b.dataset.acte); };
  });
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
      ? `Net <b>${echapper(euros(total.net_a_payer))}</b>
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
 * — le passage du brut au coût n'est pas un pourcentage — : elle envoie la
 * saisie telle quelle et le moteur rejoue la paie jusqu'à tomber juste.
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
  POUR_VISER = true;
  let saisie;
  try { saisie = saisieCourante(); } finally { POUR_VISER = false; }
  try {
    const r = await api("ajuster", {
      saisie, engagements,
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
  // Les mêmes lignes que `lignesDuBloc` a envoyées pour viser, dans le même
  // ordre : une ligne sans quantité n'est pas partie, donc elle ne compte
  // pas dans les rangs ; une ligne sans taux, si.
  POUR_VISER = true;
  let rangs;
  try { rangs = [...div.querySelectorAll(".rubrique")].filter(ligneEnvoyee); }
  finally { POUR_VISER = false; }
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
        <b>${euros(g.net_a_payer)}</b>
      </li>`).join("")}
    </ul>${phrase}`;
}

/* ------------------------------------------------------- charger un mois */
let MOIS_AFFICHE = null;        // le mois REELLEMENT charge dans l'ecran
let AVANT_EMBAUCHE = null;      // la premiere embauche, quand ce mois passe la precede (rien n'y est dû), ou null
/* UN MOIS DONT LA DSN NE SE REMPLACE PLUS : ses engagements se modifient, mais la première modification
 * demande confirmation, une fois par séance d'édition du mois. Le moteur refuse l'enregistrement sans elle
 * (`ModificationApresEcheance`) et dit ce qu'il faut savoir (`dsn_deposee` de la route `saisie`). */
let DSN_DEPOSEE = null;         // ce que le moteur dit du mois déclaré (`dsn_deposee`), ou null
let MOIS_CONFIRME = null;       // le mois dont on a confirmé la modification, pour cette séance d'édition
let CONFIRMATION_OUVERTE = null; // le mois dont la boîte de confirmation est ouverte

/** Le fichier en blocs : ce qui a été saisi d'un coup pour plusieurs
 *  salariés (saisieCourante) revient d'un coup — même spectacle, mêmes
 *  lignes, qui se suivent, et AUCUN validé. Un engagement validé, lui, est
 *  déjà démultiplié : il a son bloc, fermé, et n'absorbe rien. */
function regrouper(engagements) {
  const groupes = [];
  // UN BLOC NE REGROUPE QUE DES ENGAGEMENTS IDENTIQUES, au salarié près : le
  // formulaire montre un engagement et l'écrit pour chaque salarié coché.
  // Deux engagements qui diffèrent en quoi que ce soit — le motif de
  // recours de deux CDD aux mêmes bornes, un salaire, un numéro, un lieu
  // propre à un jour — ont chacun leur bloc. Ni un engagement validé. Ce qui
  // manque (`a_completer`) nomme son salarié : il ne sépare pas le bloc.
  const empreinteDe = (e) => {
    const { salarie, a_completer, ...reste } = e;
    return JSON.stringify(Object.keys(reste).sort().map((k) => [k, reste[k]]));
  };
  engagements.forEach((e, rang) => {
    const empreinte = empreinteDe(e);
    const dernier = groupes[groupes.length - 1];
    if (dernier && !e.valide && !dernier.valide && dernier.empreinte === empreinte
        && !dernier.salaries.includes(e.salarie)) {
      dernier.salaries.push(e.salarie);
      dernier.rangs.push(rang);
    } else {
      groupes.push({ ...e, salaries: [e.salarie], rangs: [rang], empreinte });
    }
  });
  return groupes;
}

function afficherRappelsMois() {
  document.getElementById("rappels-mois").innerHTML =
    `${DERNIER_TAUX == null ? "aucun cachet par défaut"
      : `cachet à <b>${echapper(euros(DERNIER_TAUX))}</b> par défaut`} ·
     paiement le ${echapper(jjmmaaaa(SAISIE_DATE_PAIEMENT))} ·
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
  IMPORTE = r.importe || null;
  IMPORTES = r.importes || [];
  MOIS_AFFICHE = mois;
  document.getElementById("mois").value = mois;
  afficherMoisChoisi();
  // LE CACHET PAR DÉFAUT, LE MOTEUR LE DIT : le dernier
  // cachet du mois chargé, sinon son `taux_cachet`, sinon le dernier saisi du
  // dossier (`defauts`). Faute de réponse, on garde celui de l'écran.
  const defauts = r.defauts || {};
  if (defauts.taux_cachet != null) DERNIER_TAUX = +defauts.taux_cachet;
  // Chaque autre rubrique garde le dernier taux que le mois lui donne : ce
  // n'est qu'une commodité de saisie, que le moteur n'utilise pas.
  (SAISIE.engagements || []).forEach((e) => (e.lignes || []).forEach((l) => {
    if (!(+l.taux > 0) || !l.rubrique || l.rubrique === "cachet") return;
    TAUX_PAR_RUBRIQUE[l.rubrique] = +l.taux;
  }));
  // La date de paiement d'une saisie existante fait foi -- on ne réécrit pas
  // le 30 sur un mois payé le 15. Sinon, celle que le moteur propose (la fin
  // du mois).
  SAISIE_DATE_PAIEMENT = SAISIE.date_paiement || defauts.date_paiement || "";
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
  const groupes = regrouper(SAISIE.engagements || []);
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
  DERNIERE_ECRITE = JSON.stringify(saisieCourante());
  // Un repeint montre ses cartes tout de suite : les états qu'on a déjà de
  // ce mois s'y posent, une coque ne reste pas vide le temps de les relire.
  if (repeint && ETATS_ENGAGEMENTS) peindreEtats();
    AVANT_EMBAUCHE = r.avant_premiere_embauche || null;
  DSN_DEPOSEE = r.dsn_deposee || null;
  // Une confirmation ne vaut que pour LE mois confirmé : on la redonne en revenant.
  if (MOIS_CONFIRME !== mois) MOIS_CONFIRME = null;
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
  majFermeture(mois, r.existe);
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
  const enCours = JSON.stringify(saisieCourante()) !== DERNIERE_ECRITE;
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
    if (defauts.taux_cachet != null) DERNIER_TAUX = +defauts.taux_cachet;
    if (!SAISIE_DATE_PAIEMENT) SAISIE_DATE_PAIEMENT = defauts.date_paiement || "";
    afficherRappelsMois();
    AVANT_EMBAUCHE = r.avant_premiere_embauche || null;
    DSN_DEPOSEE = r.dsn_deposee || null;
    IMPORTE = r.importe || null;
    IMPORTES = r.importes || [];
    majFermeture(mois, r.existe);
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

/** Ce que l'écran dit d'un mois : un mois tenu avant PuppetPaie ou d'avant la
 *  première embauche se lit ; tout autre mois se saisit. */
function majFermeture(mois, existe) {
  majAvisDsn(mois);
  majAvisDemo(mois);
  // UN MOIS TENU AVANT PUPPETPAIE : il se lit, rien ne s'y ajoute et rien ne le rouvre — sa DSN est
  // faite ailleurs, ou le moteur dit qu'il n'en a pas trace ; ce n'est pas « en retard » (règle 4 : rien ne la refait).
  if (IMPORTE) {
    document.getElementById("ajouter-engagement").hidden = true;
    document.getElementById("engagements").classList.add("lecture-seule");
    noteSaisie("");
    return;
  }
  // UN MOIS D'AVANT LA PREMIÈRE EMBAUCHE : le moteur dit qu'aucune DSN n'y est due ; rien ne s'y ajoute et rien
  // ne le rouvre « en retard ».
  if (AVANT_EMBAUCHE) {
    document.getElementById("ajouter-engagement").hidden = true;
    document.getElementById("engagements").classList.add("lecture-seule");
    noteSaisie(`Ce mois précède la première embauche de la compagnie (${jjmmaaaa(AVANT_EMBAUCHE)}) : aucune DSN n'y est due. `
      + "Si la compagnie employait déjà, la date de première embauche se corrige dans Configuration › Entreprise.");
    return;
  }
  document.getElementById("ajouter-engagement").hidden = false;
  document.getElementById("engagements").classList.remove("lecture-seule");
  noteSaisie(existe
    ? "Saisie chargée. L'enregistrement est automatique."
    : "Nouveau mois — l'enregistrement est automatique dès la première saisie.");
}

/** Ce que le moteur dit d'un mois dont la DSN est déposée, au-dessus des engagements : l'annule-et-remplace que la
 *  modification fera partir, ou que la modification se confirme et se corrige dans la DSN suivante. */
function majAvisDsn(mois) {
  const avis = document.getElementById("avis-dsn");
  const d = !IMPORTE && MOIS_AFFICHE === mois ? DSN_DEPOSEE : null;
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

/** LA BOÎTE QUI DEMANDE CONFIRMATION : le texte est celui du refus du moteur. « Modifier quand même »
 *  renvoie la même saisie avec la confirmation (`renvoyer`), et la séance d'édition du mois n'en redemande plus ;
 *  « Annuler » remet la saisie telle qu'elle est enregistrée. */
function confirmerModification(mois, texte, renvoyer) {
  if (CONFIRMATION_OUVERTE === mois) return;
  CONFIRMATION_OUVERTE = mois;
  clearTimeout(MINUTEUR_SAISIE);
  // Une boîte déjà ouverte (l'accueil du jour…) se met de côté, et revient après la réponse.
  const remettre = mettreDeCote();
  let confirme = false;
  const annuler = async () => {
    CONFIRMATION_OUVERTE = null;
    if (remettre) remettre();
    if (confirme) return;
    noteSaisie("Modification annulée : la saisie reste telle qu'elle est enregistrée.");
    if (MOIS_AFFICHE === mois) await chargerMois(mois, true);
  };
  ouvrirBoite(`
    <h3>Modifier les engagements ${echapper(deMois(mois))} ?</h3>
    <div class="encart grave"><b>Ce mois est déjà déclaré</b>${echapper(texte)}</div>
    <div class="actions">
      <button class="secondaire" id="mc-annuler">Annuler</button>
      <button class="primaire" id="mc-modifier">Modifier quand même</button>
    </div>`, annuler);
  document.getElementById("mc-annuler").onclick = () => fermerBoite();
  document.getElementById("mc-modifier").onclick = async () => {
    confirme = true;
    MOIS_CONFIRME = mois;
    fermerBoite();
    await renvoyer();
  };
}

/* UN MOIS IMPORTÉ EST UN MOIS COMME LES AUTRES : ses paies ont leurs cases.
 * Un mois tenu avant PuppetPaie n'a pas de fichier de paie : le
 * moteur le rend depuis l'historique importé (`importe` de la route `saisie`,
 * `import::mois`) — une carte par paie, comme celle d'un engagement validé :
 * quand, qui et pour quoi, ses états, son net ; un clic sur la carte montre
 * chaque case remplie de ce que l'import a lu, avec d'où elle vient. Ce que
 * l'import n'a pas pu lire se dit sur la carte, case par case, et pourquoi —
 * jamais un blanc muet. La page n'en décide rien : elle affiche. */
let IMPORTE = null;   // le mois tenu avant PuppetPaie, tel que le moteur le rend, ou null
let IMPORTES = [];    // les cartes des engagements importés d'un mois qui a aussi une saisie

const CASES_IMPORTEES = [
  ["brut", "Brut"], ["net_imposable", "Net imposable"], ["net_a_payer", "Net à payer"],
  ["pas", "Prélèvement à la source"], ["net_verse", "Net versé (DSN)"],
  ["cotisations_salariales", "Cotisations salariales"], ["cotisations_patronales", "Cotisations patronales"],
  ["cout_employeur", "Coût employeur"],
];
const SOURCES_IMPORTEES = { bulletin: "bulletin", DSN: "DSN", AEM: "AEM", historique: "historique" };

function dessinerMoisImporte() {
  const cible = document.getElementById("mois-importe");
  // Le cachet par défaut et la date de paiement servent à saisir : un mois tenu avant n'en a pas l'usage.
  document.getElementById("rappels-mois").hidden = !!IMPORTE;
  if (!IMPORTE && IMPORTES.length) {
    // UN MOIS À LA FOIS IMPORTÉ ET SAISI : les engagements importés, en cartes fermées, à côté de ceux de la saisie.
    const mois = MOIS_AFFICHE;
    const moisCourt = MOIS_COURTS[parseInt(mois.slice(5, 7), 10) - 1] || "";
    cible.innerHTML = `
      <p class="mi-tete"><b>Importé${IMPORTES.length > 1 ? "s" : ""}</b> <span class="discret">— ${IMPORTES.length}
        engagement${IMPORTES.length > 1 ? "s" : ""} de l'ancien logiciel · lecture seule</span></p>
      ${IMPORTES.map((p, i) => carteImportee(p, i, moisCourt)).join("")}`;
    ouvrirLesCartesImportees(cible);
    cible.hidden = false;
    return;
  }
  if (!IMPORTE) { cible.hidden = true; cible.innerHTML = ""; return; }
  const mois = IMPORTE.mois;
  const moisCourt = MOIS_COURTS[parseInt(mois.slice(5, 7), 10) - 1] || "";
  const t = IMPORTE.totaux || {};
  const n = IMPORTE.paies.length;
  const total = (k, mot) => t[k] && t[k].montant != null
    ? `${mot} ${euros(t[k].montant)}${t[k].connus < n ? ` (${t[k].connus} engagement${t[k].connus > 1 ? "s" : ""} sur ${n})` : ""}` : "";
  cible.innerHTML = `
    <p class="mi-tete"><b>Tenu avant PuppetPaie</b> <span class="discret">— d'après l'historique importé ·
      ${n} paie${n > 1 ? "s" : ""} ${[total("brut", "· brut"), total("net_a_payer", "· net")].filter(Boolean).join(" ")}
      · lecture seule</span></p>
    ${IMPORTE.paies.map((p, i) => carteImportee(p, i, moisCourt)).join("")}`;
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

/* Changer de mois enregistre d'abord CE QUI EST À L'ÉCRAN, et sous son vrai
 * mois. La case a déjà changé quand on arrive ici : on y remet l'ancien mois
 * le temps de l'écriture, sans quoi les engagements d'août partiraient dans le
 * fichier de septembre — la case du mois est ce qui date les jours joués. */
document.getElementById("mois").onchange = (e) => {
  fermerCalendriers();
  const neuf = e.target.value;
  let ecrite = null;
  if (MOIS_AFFICHE && MOIS_AFFICHE !== neuf) {
    clearTimeout(MINUTEUR_SAISIE);
    e.target.value = MOIS_AFFICHE;
    ecrite = saisieAEcrire();
    e.target.value = neuf;
    // Partie : un autre clic avant que le nouveau mois soit peint ne la renvoie pas.
    if (ecrite) DERNIERE_ECRITE = JSON.stringify(ecrite.s);
  }
  // LE NOUVEAU MOIS N'ATTEND PAS L'ÉCRITURE DE L'ANCIEN : la saisie quittée est prise telle qu'elle est
  // (`saisieAEcrire`, sous son vrai mois), part au moteur, et le nouveau mois s'affiche en même temps. Une fois
  // l'écriture faite, le mois affiché se vérifie : elle a pu le changer (un contrat en cours, une numérotation).
  const chargement = chargerMois(neuf);
  if (ecrite) enregistrerEnPartant(ecrite, chargement);
};

/** Ce que l'écran porte et qui n'est pas encore écrit (le mois de la case), ou `null`. */
function saisieAEcrire() {
  const s = saisieCourante();
  if (!s.mois) return null;
  const empreinte = JSON.stringify(s);
  if (empreinte === DERNIERE_ECRITE) return null;
  return { s, confirme: MOIS_CONFIRME === s.mois };
}

/** La saisie d'un mois qu'on vient de quitter : écrite sans repeindre (l'écran montre déjà un autre mois), puis le mois
 *  affiché vérifié. Un échec se dit : c'est du travail qu'on croit posé. */
async function enregistrerEnPartant({ s, confirme }, chargement) {
  const ecrire = (oui) => appelerMoteur("enregistrer-saisie",
    { mois: s.mois, saisie: s, modifier_apres_echeance: oui });
  try {
    await ecrire(confirme);
    oublierCouts();
    appelerMoteur("etat").then((e) => { ETAT.historique = e.historique; }, () => {});
  } catch (e) {
    // Le mois quitté était déclaré et sa DSN ne se remplace plus : on le demande ici aussi ; « Annuler » le laisse
    // tel qu'il est enregistré.
    if (e.detail?.type === "ModificationApresEcheance") {
      confirmerModification(s.mois, e.message, async () => {
        try { await ecrire(true); dire(`${moisEnClair(s.mois)} : modification enregistrée.`); }
        catch (e2) { dire(`${moisEnClair(s.mois)} : non enregistré — ${e2.message}`, "erreur"); }
      });
      return;
    }
    if (e.detail?.type !== "SaisieIncomplete") dire(`${moisEnClair(s.mois)} : non enregistré — ${e.message}`, "erreur");
    return;
  }
  // Le mois affiché se vérifie quand il est peint (la vérification part tout de suite : le moteur refait l'état).
  const mois = DEMANDE_MOIS;
  if (mois) verifierLeMois(mois, chargement.catch(() => {}));
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
let DERNIERE_ECRITE = null;      // le JSON tel qu'il a été enregistré

function noteSaisie(texte) {
  document.getElementById("note-saisie").textContent = texte;
}

/** Enregistre si quelque chose a changé. Rend true si le fichier a été écrit.
 *
 *  UN ENREGISTREMENT À LA FOIS. L'enregistrement part une
 *  seconde après la dernière frappe ; un mois long met plus d'une seconde à se
 *  calculer, et le suivant partait pendant que le précédent tournait encore.
 *  Ils se suivent : chacun attend la fin du précédent, puis compare ce qui est
 *  à l'écran à ce qui vient d'être écrit — rien ne repart pour rien. */
let ENREGISTREMENT = Promise.resolve(false);
function enregistrerSaisie(silencieux) {
  const tour = ENREGISTREMENT.then(() => enregistrerSaisieSeule(silencieux));
  ENREGISTREMENT = tour.catch(() => false);
  return tour;
}

async function enregistrerSaisieSeule(silencieux) {
  const s = saisieCourante();
  if (!s.mois) return false;
  // La boîte de confirmation est ouverte : on attend sa réponse, on ne la redemande pas à chaque frappe.
  if (CONFIRMATION_OUVERTE === s.mois) return false;
  const empreinte = JSON.stringify(s);
  if (empreinte === DERNIERE_ECRITE) return false;
  try {
    const r = await api("enregistrer-saisie", { mois: s.mois, saisie: s,
                                      modifier_apres_echeance: MOIS_CONFIRME === s.mois });
    DERNIERE_ECRITE = empreinte;
    if (r && "dsn_deposee" in r && MOIS_AFFICHE === s.mois) { DSN_DEPOSEE = r.dsn_deposee; majAvisDsn(s.mois); }
    oublierCouts();
    // L'heure du moteur (`enregistre_a`), pas celle du navigateur : au banc, le temps est simulé.
    // Un engagement auquel il manque un fait s'enregistre aussi : sa carte dit ce qui manque.
    const aCompleter = (r.a_completer || []).filter(Boolean).length;
    noteSaisie("Enregistré à " + r.enregistre_a.slice(11, 16) + (aCompleter
      ? ` — ${aCompleter > 1 ? `${aCompleter} engagements à compléter avant de les valider` : "un engagement à compléter avant de le valider"}.`
      : "."));
    ETAT.historique = await api("etat").then((e) => e.historique);
    // Ce qui vient d'etre enregistre devient declarable : les blocs qui
    // disaient << pas dans la saisie enregistree >> doivent cesser de le dire,
    // et la colonne << ce qui part >> compte cet engagement (elle
    // dirait << aucun engagement ce mois-ci >> jusqu'au rechargement).
    await rafraichirActions();
    // La fin d'un contrat se lit sur la saisie ENREGISTRÉE : tant qu'elle
    // n'était pas relue ici, on tapait le motif de rupture et le bloc
    // continuait d'annoncer qu'il manquait. Les jours payés pendant un
    // imprévu aussi : un jour retiré de la ligne doit cesser d'être nommé.
    await dessinerImprevus(s.mois);
    return true;
  } catch (e) {
    // Un engagement dont on n'a pas encore choisi les jours n'est pas une faute :
    // c'est la saisie en cours. On le dit en une ligne, sans la règle du
    // fichier (« lignes: [] ») ni d'alerte.
    // Ce que la norme ne permet pas de déclarer après la limite (un contrat déclaré retiré, des dates qui le sortent
    // d'un autre mois où il est saisi) : le
    // moteur dit pourquoi et quoi faire, et la saisie revient telle qu'elle est enregistrée.
    if (e.detail?.type === "ModificationNonDeclarable") {
      dire(e.message, "erreur");
      if (MOIS_AFFICHE === s.mois) await chargerMois(s.mois, true);
      noteSaisie("Non enregistré — " + e.message);
      return false;
    }
    if (e.detail?.type === "ModificationApresEcheance") {
      noteSaisie("Pas encore enregistré : la modification est à confirmer.");
      confirmerModification(s.mois, e.message, () => enregistrerSaisie(silencieux));
      return false;
    }
    if (e.detail?.type === "SaisieIncomplete") {
      noteSaisie(/spectacle/.test(e.message)
        ? "Pas encore enregistré : choisir le spectacle."
        : "Pas encore enregistré : choisir les jours travaillés.");
      return false;
    }
    // Un échec d'enregistrement ne peut pas rester discret : c'est du travail
    // qu'on croit posé et qui ne l'est pas.
    noteSaisie("Non enregistré — " + e.message);
    if (!silencieux) dire(e.message, "erreur");
    return false;
  }
}

/** Appelée à chaque frappe. Enregistre une seconde après la dernière. */
function enregistrerPlusTard() {
  clearTimeout(MINUTEUR_SAISIE);
  noteSaisie("Modifié…");
  MINUTEUR_SAISIE = setTimeout(() => enregistrerSaisie(true), 1000);
}

