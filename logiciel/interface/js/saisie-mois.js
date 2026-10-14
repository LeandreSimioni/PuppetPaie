/* Le mois : la saisie courante, l'apercu, charger un mois, la grille des mois, enregistrer, produire. */
"use strict";

/* CE QUE LE BLOC MONTRE EN UNE SEULE VALEUR, ET QUE `engagementsDuBloc`
 * ÉCRIT POUR CHAQUE SALARIÉ COCHÉ (09/10/2026). Une seule liste, deux usages :
 *   - l'écriture : ces champs-là, c'est l'écran qui les dit, ou les retire
 *     quand il les laisse vides ; tout AUTRE champ de l'engagement chargé est
 *     porté tel quel (`engagementEcrit`) ;
 *   - le regroupement : deux engagements qui diffèrent sur l'un d'eux ne
 *     tiennent pas dans un même bloc (`regrouper`), sans quoi l'enregistrement
 *     recopierait la valeur du premier sur le second.
 * Un champ que l'écran apprend à écrire s'ajoute ICI : absent de la liste, il
 * serait à la fois repris de l'ancien fichier et écrit par l'écran, et les
 * salariés d'un bloc se le partageraient. */
const CHAMPS_DU_BLOC = [
  "spectacle", "nature", "debut", "fin", "numero", "lignes", "metier", "cadre",
  "motif_recours", "salaire_mensuel", "quotite_contrat", "date_modification",
  "conges_pris", "absences", "primes",
  "motif_rupture", "notification_rupture", "engagement_licenciement",
  "signature_convention", "preavis", "transaction", "dernier_jour_paye",
  "en_droit_a_la_retraite", "preavis_non_effectue", "preavis_mois",
  "precarite_due", "precarite_pourquoi",
];
/* Ce que le moteur tient seul : une paie se valide et se rouvre par sa route,
 * et `fusionner` (routes_ecritures.rs) reprend ces deux champs du fichier.
 * Les porter depuis la lecture revaliderait une paie rouverte entre-temps. */
const CHAMPS_DU_MOTEUR = ["valide", "rouverte"];
/* Ce que `saisieCourante` écrit au niveau du mois. */
const CHAMPS_DU_MOIS = ["mois", "taux_cachet", "date_paiement", "engagements"];

const copieJson = (v) => JSON.parse(JSON.stringify(v));

/** L'engagement qui part au fichier : `ecran` (le salarié et les champs de
 *  CHAMPS_DU_BLOC que l'écran remplit), puis tout ce que `origine` — cet
 *  engagement tel qu'il a été chargé, ou rien pour un neuf — portait d'autre. */
function engagementEcrit(origine, ecran) {
  const e = { ...ecran };
  Object.entries(origine || {}).forEach(([k, v]) => {
    if (k in e || k === "salarie" || CHAMPS_DU_BLOC.includes(k) || CHAMPS_DU_MOTEUR.includes(k)) return;
    e[k] = copieJson(v);
  });
  return e;
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
  if (!fin || (!(nat.motifs_rupture || []).length && !nat.signalement_fctu)) return;
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
  // L'ENGAGEMENT N'ÉCRIT PLUS DE `dates` : chacune de ses lignes porte les
  // siennes, avec son lieu, et le moteur en fait la réunion
  // (`operations.dates_engagement`). Une seule source, donc pas de
  // divergence possible entre « les jours du contrat » et « les jours
  // payés ».
  //
  // UN ENGAGEMENT PAR SALARIÉ COCHÉ. Le bloc en porte plusieurs à l'écran
  // pour aller vite ; le fichier, lui, garde le format du moteur — un
  // salarié, un engagement. Chacun reçoit sa propre copie des lignes.
  // Un bloc sans salarié n'est pas une paie et n'écrit rien.
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
  const salaries = salariesDuBloc(div);
  return salaries.map((salarie) => {
    // LE NUMÉRO EST CELUI D'UN CONTRAT, DONC D'UN SALARIÉ (#139) : celui que
    // son engagement portait, sinon celui du bloc s'il est le sien
    // (`numeroDe`). Un salarié coché en plus n'hérite pas du contrat d'un autre — il
    // recevra le sien à la production (le moteur refuse un numéro sur deux
    // salariés).
    const origine = origineDe(div, salarie);
    const numero = (origine && origine.numero) || (div.dataset.numeroDe === salarie ? numeroDuBloc : "");
    // UN PERMANENT N'A PAS DE SPECTACLE, et ce n'est pas un détail de
    // mise en page : le numéro d'objet lui est interdit (CCH-11). Écrire
    // la case vide, ce serait écrire un rattachement à rien.
    const e = nat.mensuelle ? { salarie } : { salarie, spectacle };
    // La nature ne s'écrit que si elle s'écarte du défaut : un engagement
    // muet est un CDDU du spectacle, et cinq ans de saisies le restent
    // sans qu'on y touche.
    if (nat.cle && nat.cle !== ETAT.nature_defaut) e.nature = nat.cle;
    // Le contrat, quand il est dit : ses bornes (un contrat qui déborde
    // du mois n'existe que par elles) et son numéro, figé à la production.
    if (debut) e.debut = debut;
    if (fin) e.fin = fin;
    if (numero) e.numero = numero;
    // UN MOIS DU CONTRAT SANS CACHET (15/09/2026) : un contrat aux bornes
    // dites, et aucune ligne remplie, s'écrit `lignes: []` — le moteur en
    // fait une DSN qui porte le contrat avec un versement à zéro, sans
    // bulletin. Sans bornes, on n'écrit rien : c'est une saisie incomplète,
    // et le moteur le dit.
    if (nat.mensuelle) {
      // LES FAITS, PAS LES LIGNES. `lignes_du_mois` en fait des lignes de
      // bulletin — le prorata d'un mois incomplet, l'indemnité de congés
      // et la retenue qui la couvre, le bloc 65 d'une suspension, le
      // solde de tout compte du dernier mois. Écrire `lignes` ici
      // couperait tout ce calcul : c'est la porte de sortie du moteur
      // (« une saisie qui écrit ses lignes elle-même les garde »), et
      // elle n'est pas pour cet écran.
      const recours = div.querySelector(".e-recours").value;
      if (recours) e.motif_recours = recours;
      if (salaire !== "") e.salaire_mensuel = +salaire;
      if (quotite !== "") e.quotite_contrat = +quotite;
      if (dateModification) e.date_modification = dateModification;
      if (faits.conges.length) e.conges_pris = faits.conges;
      if (faits.absences.length) e.absences = faits.absences;
      if (faits.primes.length) e.primes = faits.primes;
      ruptureDuBloc(div, nat, fin, e);
    } else {
      if (lignes.length) e.lignes = JSON.parse(JSON.stringify(lignes));
      else if (debut && fin) e.lignes = [];
      ruptureDuBloc(div, nat, fin, e);
    }
    // Le métier ne s'écrit que s'il est choisi : un engagement sans
    // `metier` prend celui de la fiche, et toutes les saisies d'avant
    // restent lisibles telles quelles. `cadre` voyage avec lui, seulement
    // quand il s'écarte du défaut du catalogue.
    if (metier) {
      e.metier = metier;
      const m = metierDe(metier);
      if (!m || cadre !== !!m.cadre_par_defaut) e.cadre = cadre;
    }
    // Ce que l'écran ne montre pas, il le PORTE : la date de paiement
    // propre à ce contrat, son lieu, l'emploi et les statuts déclarés…
    // L'origine est celle de CE salarié dans CE bloc (`origineDe`).
    return engagementEcrit(origineDe(div, salarie), e);
  });
}

function saisieCourante() {
  const mois = document.getElementById("mois").value;
  const engagements = [...document.querySelectorAll(".engagement")]
    .flatMap((div) => engagementsDuBloc(div, mois));
  // `taux_cachet` et `date_paiement` ne s'affichent plus mais restent ECRITS :
  // le moteur s'en sert -- l'un comme defaut d'un engagement qui ne dit pas
  // son taux, l'autre comme date de paiement du contrat. Ce qui a disparu,
  // c'est la saisie, pas la valeur. `lieu` ne se SAISIT plus : il appartient
  // au cachet. Celui qu'un fichier porte au niveau du mois, comme tout champ
  // du mois que l'ecran n'ecrit pas, est repris de la saisie chargee.
  const ecrite = {
    mois,
    taux_cachet: DERNIER_TAUX,
    date_paiement: SAISIE_DATE_PAIEMENT,
    engagements,
  };
  const lue = SAISIE && SAISIE.mois === mois ? SAISIE : {};
  Object.entries(lue).forEach(([k, v]) => {
    if (!CHAMPS_DU_MOIS.includes(k)) ecrite[k] = copieJson(v);
  });
  return ecrite;
}

function rafraichirApercu() {
  clearTimeout(MINUTEUR);
  MINUTEUR = setTimeout(calculerApercu, 350);   // on ne calcule pas a chaque frappe
  // Et on enregistre. C'est le seul point de passage de TOUTES les
  // modifications de la saisie -- y accrocher l'enregistrement, c'est
  // s'assurer qu'aucune frappe ne reste dans la page seulement.
  enregistrerPlusTard();
  // Les etats, eux, ne coutent rien : ils sont deja lus. On les repeint tout
  // de suite, parce que changer le salarie ou les jours change la paie que ce
  // bloc DESIGNE -- et laisser 350 ms l'ancien etat sous le nouveau nom, c'est
  // exactement le mensonge qu'on veut eviter.
  peindreEtats();
}

/** `lu` : l'aperçu tel que la lecture du mois le porte (la saisie enregistrée) ; sans lui, on le demande au moteur. */
async function calculerApercu(lu) {
  const saisie = saisieCourante();
  const cible = document.getElementById("apercu");
  // Les jours sont sur les lignes de paie : un engagement « a des jours » dès
  // qu'une de ses lignes en porte.
  const aDesJours = (e) => (e.lignes || []).some((l) => (l.dates || []).length);
  // UN PERMANENT N'A PAS DE JOURS, et c'est normal : il est payé au mois.
  // La première version de ce garde-fou ne connaissait que les cachets, et
  // un CDI saisi en entier restait devant « ajouter des jours joués »
  // pendant que sa paie, elle, était calculable.
  const aDeQuoiPayer = (e) => aDesJours(e) || e.salaire_mensuel != null;
  if (!saisie.engagements.length || !saisie.engagements.some(aDeQuoiPayer)) {
    // Le permanent ne se nomme qu'à une compagnie qui en emploie.
    const perm = !!(ETAT.configuration || {}).permanents;
    cible.innerHTML = `<p class="discret">Ajouter un engagement, une ligne de
      paie et ses jours joués${perm ? " — ou, pour un permanent, son salaire mensuel" : ""}.</p>`;
    return;
  }
  cible.classList.add("attente");
  try {
    const a = lu ? structuredClone(lireReponse(lu)) : await api("apercu", { saisie });
    cible.innerHTML = vueApercu(a);
    // Les dates resolues, sous chaque engagement : ce que le moteur a compris.
    const blocs = [...document.querySelectorAll(".engagement")];
    const parBloc = paiesParBloc(a, blocs);
    blocs.forEach((div, i) => {
      const part = parBloc[i];
      const p = part[0];
      // Les trois nombres du bloc — sommés quand il porte plusieurs salariés,
      // parce que l'enveloppe qu'on y vise est celle du bloc entier.
      peindreChiffres(div, part);
      div._paies = part;
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
      // Ce que le moteur a à dire de CETTE paie, en deux tons (24/09/2026,
      // engagement.rs, Avis) : en orange ce qui demande quelque chose -- un
      // taux sous le SMIC --, en gris ce qui renseigne seulement -- un
      // contrat qui se poursuit le mois suivant, ou le minimum que notre
      // lecture de la convention donne, avec son lien (journal §161). Rien
      // n'est bloquant.
      div.querySelector(".avis").innerHTML =
        avisDuBloc(part).map(
          (a) => `<div class="encart attention avis-paie">${echapper(a)}</div>`).join("")
        + avisDuBloc(part, "informations").map(
          (a) => `<div class="encart info avis-paie">${echapperLiens(a)}</div>`).join("");
    });
  } catch (e) {
    cible.innerHTML = `<div class="encart erreur">${erreurEnClair(e)}</div>`;
  } finally {
    cible.classList.remove("attente");
  }
}

/* UNE ERREUR DU MOTEUR, LISIBLE (30/09/2026). Le nom de l'exception
 * (« FileNotFoundError », « ErreurOperation ») s'affichait en gras, en tête :
 * il ne dit rien à qui fait la paie. La première ligne du message dit quoi
 * faire ; le reste — la règle, la commande — se déplie si on le veut. */
function erreurEnClair(e) {
  const lignes = String(e.message || e).split("\n").map((l) => l.trim()).filter(Boolean);
  const tete = lignes.shift() || "Erreur";
  return `${echapper(tete)}${lignes.length ? `<details class="discret"><summary>en savoir plus</summary>
    ${lignes.map((l) => `<div>${echapper(l)}</div>`).join("")}</details>` : ""}`;
}

/* UN BLOC PARLE D'UNE SEULE VOIX (24/09/2026). Deux salariés payés ensemble
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

/* LES PAIES D'UN APERÇU, RENDUES À LEUR BLOC. Un bloc qui porte trois
 * salariés cochés a produit TROIS engagements — `saisieCourante` les
 * démultiplie, parce que le moteur ne connaît qu'un salarié par engagement.
 * Compter un par bloc décalait donc tout ce qui suit dès le deuxième, et
 * l'écran posait la ligne « bulletin … » du voisin sous le mauvais nom. */
function paiesParBloc(a, blocs) {
  let rang = 0;
  return blocs.map((div) => {
    const combien = salariesDuBloc(div).length;
    const part = (a.paies || []).slice(rang, rang + combien);
    rang += combien;
    return part;
  });
}

/* LA PAIE VALIDÉE, EN UNE LIGNE D'AGENDA (23/09/2026).
 *
 * Validée, une paie ne se remplit plus : elle se PARCOURT. On y cherche,
 * dans cet ordre, quand (les jours travaillés, le repère d'une paie), qui
 * et pour quoi, ce qui reste à faire (DPAE, contrat signé, documents), et
 * combien — le net seulement, ce qu'on vire ; brut et coût sont dans
 * l'aperçu à droite de l'écran, les répéter ici serait du bruit. Numéros
 * de bulletin et de contrat, date de validation : en infobulle.
 * « Modifier… » rouvre la saisie entière (et « Retirer » avec elle).
 *
 * Deux sources, qui n'arrivent pas au même moment : l'aperçu du moteur
 * (`div._paies` : jours, ce qui est payé, net) et l'état de la paie
 * (`div._ligne` : actes, validation). La ligne se repeint à l'arrivée de
 * chacune ; ce qui manque encore se tait. */
function peindreCarte(div) {
  const cible = div.querySelector(".e-resume");
  if (!cible) return;
  const ligne = div._ligne, ps = div._paies || [];
  if (!div.classList.contains("validee") || !ligne) { cible.innerHTML = ""; return; }
  const p = ps[0];
  const mois = document.getElementById("mois").value;
  // QUAND. Les jours travaillés du mois ; un permanent n'en a pas : son mois.
  const dates = p ? (p.dates_travail || []).filter((d) => d.startsWith(mois)) : [];
  const jours = dates.length ? joursEnClair(dates)
    : (p && p.nature && p.nature.mensuelle ? "mois" : "");
  const moisCourt = MOIS_COURTS[parseInt(mois.slice(5, 7), 10) - 1] || "";
  // POUR QUOI. Le spectacle, puis ce qui est payé en mots.
  const paye = p ? designation(p).toLowerCase() : "";
  const suite = ligne.contrat && !ligne.contrat.dernier_mois
    ? ` · contrat jusqu'en ${moisEnClair(ligne.contrat.jusqu_a)}` : "";
  const net = ps.length
    ? euros(ps.reduce((t, x) => t + Number(x.net_a_payer || 0), 0))
    : "";
  const detail = [p && p.numero_bulletin ? `bulletin ${p.numero_bulletin}` : "",
                  p && p.numero_contrat ? `contrat ${p.numero_contrat}` : "",
                  `validée le ${quand(ligne.valide)}`].filter(Boolean).join(" · ");
  cible.innerHTML = `
    <div class="carte-quand" title="${echapper(dates.map(jjmmaaaa).join(", "))}">
      <b class="${jours.includes(" · ") ? "epars" : ""}">${echapper(jours)}</b><span>${echapper(moisCourt)}</span>
    </div>
    <div class="carte-qui" title="${echapper(detail)}">
      <b>${echapper(ligne.nom)}</b>
      <span class="discret">${echapper(pourQuoi(ligne))}${paye ? " · " + echapper(paye) : ""}${echapper(suite)}</span>
    </div>
    <div class="carte-actes">${bandeauEtats(ligne)}</div>
    <div class="carte-net" title="net à payer, avant impôt">${echapper(net)}</div>
    <button class="secondaire carte-modifier">Voir le détail</button>`;
  // VOIR SANS MODIFIER (l'utilisateur, 29/09/2026) : relire les heures d'un
  // service de répétition ne doit pas demander de rouvrir la paie.
  // « Modifier » est dans la boîte du détail.
  cible.querySelector(".carte-modifier").onclick = (ev) => {
    ev.preventDefault();
    boiteDetail(ligne, ps);
  };
  brancherRecu(cible);
}

/** Le détail d'une paie validée, en lecture : ce qui est payé ligne par
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
        ${l.dates.length ? `<br><span class="discret">Jours :</span> ${echapper(l.dates.map(jjmmaaaa).join(", "))}` : ""}
        ${l.planning.length ? `<br><span class="discret">Prévu ensuite :</span> ${echapper(l.planning.map(jjmmaaaa).join(", "))}` : ""}
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
    ${ps.length ? ps.map(bloc).join("") : `<p class="discret">Le détail n'est pas encore calculé.</p>`}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Fermer</button>
      ${MOIS_FERME && MOIS_FORCE !== document.getElementById("mois").value ? "" : `<button class="modifier" id="d-modifier">${CRAYON}Modifier…</button>`}
    </div>`);
  const m = document.getElementById("d-modifier");
  if (m) m.onclick = () => { fermerBoite(); boiteRouvrir(ligne); };
}

/** Les trois nombres du bloc. Une case en cours de frappe ne se repeint pas :
 *  l'aperçu revient 350 ms après la dernière touche, et il écraserait le
 *  montant qu'on est justement en train de viser. */
function peindreChiffres(div, paies) {
  const champs = { net: "net_a_payer", brut: "brut", cout: "cout_employeur" };
  const net = div.querySelector(".e-net");
  if (net) {
    // Le net (ce qu'on vire) et le coût employeur (ce que la compagnie sort) :
    // les deux nombres qui décident d'une paie. Le brut reste dans l'aperçu.
    const somme = (k) => euros(paies.reduce((t, p) => t + Number(p[k] || 0), 0));
    net.innerHTML = paies.length
      ? `Net <b>${echapper(somme("net_a_payer"))}</b>
         <span class="e-cout">Coût employeur <b>${echapper(somme("cout_employeur"))}</b></span>${paies.length > 1
          ? ` <span class="discret">pour ${paies.length} salariés</span>` : ""}`
      : `<span class="discret">Le net s'affiche dès que la paie est complète.</span>`;
  }
  div.querySelectorAll(".e-vise").forEach((c) => {
    if (c === document.activeElement) return;
    c.value = paies.length
      ? paies.reduce((t, p) => t + Number(p[champs[c.dataset.cible]] || 0), 0).toFixed(2)
      : "";
  });
}

/* VISER UN TOTAL, ET EN DÉDUIRE LE TAUX (22/09/2026).
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
  GARDER_SANS_TAUX = true;
  let saisie;
  try { saisie = saisieCourante(); } finally { GARDER_SANS_TAUX = false; }
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
  GARDER_SANS_TAUX = true;
  let rangs;
  try { rangs = [...div.querySelectorAll(".rubrique")].filter(ligneEnvoyee); }
  finally { GARDER_SANS_TAUX = false; }
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

/* L'APERCU NE PORTE PAS LE MONTANT URSSAF : il se CALCULE avec la DSN, au
 * centime, comme le bordereau (paie/src/bordereau.rs), et la boite
 * << Fermer le mois >> le montre. */
/* PAR SALARIÉ, UNE LIGNE. « C'est plus facile ensuite pour faire les paies :
 * j'ai juste à mettre le montant du net » ; puis, devant
 * le détail paie par paie : « pas de détail dans l'aperçu ! ». Donc une ligne
 * par salarié, sommée, et le net est ce qu'on vire. Les sommes se font ici,
 * sur les chaînes du moteur, à deux décimales — ce n'est qu'un aperçu, le
 * bulletin fait foi. */
function vueApercu(a) {
  /* LE MOIS, EN DEUX NOMBRES (23/09/2026). Ce qu'on tire de l'aperçu, c'est
     ce qu'on vire à chacun et ce que le mois coûte à la compagnie. Le brut
     et le détail des cachets sont dans les paies, les bulletins et les
     exports : les répéter ici faisait un tableau à cinq colonnes qu'on ne
     lisait qu'en partie. Une ligne par salarié, son net ; le coût au survol. */
  const somme = (xs) => xs.reduce((t, x) => t + Number(x || 0), 0);
  const eur = euros;
  const groupes = [];
  a.paies.forEach((p) => {
    const cle = p.salarie.nom + "|" + p.salarie.prenom;
    let g = groupes.find((x) => x.cle === cle);
    if (!g) { g = { cle, salarie: p.salarie, paies: [] }; groupes.push(g); }
    g.paies.push(p);
  });
  if (!groupes.length) return `<p class="discret">Aucune paie ce mois-ci.</p>`;
  const net = somme(a.paies.map((p) => p.net_a_payer));
  const cout = somme(a.paies.map((p) => p.cout_employeur));
  return `
    <div class="mois-chiffres">
      <div><span>Net à virer</span><b>${eur(net)}</b></div>
      <div><span>Coût employeur</span><b>${eur(cout)}</b></div>
    </div>
    <ul class="mois-salaries">${groupes.map((g) => `
      <li title="coût employeur ${echapper(eur(somme(g.paies.map((p) => p.cout_employeur))))} · brut ${echapper(eur(somme(g.paies.map((p) => p.brut))))}">
        <span>${echapper(g.salarie.nom)} ${echapper(g.salarie.prenom)}
          <span class="discret">· ${g.paies.length} paie${g.paies.length > 1 ? "s" : ""}</span></span>
        <b>${eur(somme(g.paies.map((p) => p.net_a_payer)))}</b>
      </li>`).join("")}
    </ul>`;
}

/* ------------------------------------------------------- charger un mois */
let MOIS_AFFICHE = null;        // le mois REELLEMENT charge dans l'ecran
let MOIS_FERME = null;          // la date limite depassee de ce mois, ou null
let MOIS_FORCE = null;          // le mois ferme que l'on a rouvert, en connaissance de cause

/** Le fichier en blocs : ce qui a été saisi d'un coup pour plusieurs
 *  salariés (saisieCourante) revient d'un coup — même spectacle, mêmes
 *  lignes, qui se suivent, et AUCUN validé. Une paie validée, elle, est
 *  déjà démultipliée : elle a son bloc, fermé, et n'absorbe rien. */
function regrouper(engagements) {
  const groupes = [];
  // L'EMPREINTE D'UN BLOC, C'EST TOUT CE QU'IL MONTRE EN UNE SEULE VALEUR
  // (CHAMPS_DU_BLOC, la liste que l'écriture suit aussi). Le bloc affiche les
  // valeurs du premier engagement et l'enregistrement les recopie sur chaque
  // salarié coché : deux engagements qui diffèrent sur UN de ces champs —
  // le motif de recours de deux CDD aux mêmes bornes (09/10/2026), un
  // salaire, un numéro figé, propre à un salarié — ne se regroupent donc pas.
  // Vide, absent ou liste vide se valent : la page n'écrit ni l'un ni l'autre.
  const vide = (v) => v === undefined || v === null || v === ""
    || (Array.isArray(v) && !v.length);
  const empreinteDe = (e) => JSON.stringify(CHAMPS_DU_BLOC.map((k) => (vide(e[k]) ? null : e[k])));
  engagements.forEach((e) => {
    const empreinte = empreinteDe(e);
    const dernier = groupes[groupes.length - 1];
    if (dernier && !e.valide && !dernier.valide && dernier.empreinte === empreinte
        && !dernier.salaries.includes(e.salarie)) {
      dernier.salaries.push(e.salarie);
      dernier.origines[e.salarie] = e;
    } else {
      // `origines` : chaque salarié du bloc garde SON engagement tel qu'il a
      // été lu ; ce qu'il porte sans l'afficher lui revient à l'écriture.
      groupes.push({ ...e, salaries: [e.salarie], origines: { [e.salarie]: e }, empreinte });
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
async function chargerMois(mois) {
  DEMANDE_MOIS = mois;
  // TOUT LE MOIS D'UN COUP, lu (pas calculé) : les lectures ci-dessous le prennent dans `LU`.
  let lu;
  try { lu = await lireLeMois(mois); } catch (e) { if (DEMANDE_MOIS === mois) document.getElementById("engagements").classList.remove("en-lecture"); throw e; }
  if (DEMANDE_MOIS !== mois) return;
  const r = await api("saisie", { mois });
  // Un mois plus récent a été demandé pendant que celui-ci arrivait : on le
  // laisse tomber, sinon il repeindrait l'écran par-dessus le bon.
  if (DEMANDE_MOIS !== mois) return;
  SAISIE = r.saisie;
  MOIS_AFFICHE = mois;
  document.getElementById("mois").value = mois;
  afficherMoisChoisi();
  // LE CACHET PAR DÉFAUT, LE MOTEUR LE DIT (#140, règle 2) : le dernier
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
  // ON PRÉCHARGE, PUIS ON AFFICHE (ticket #113, 02/10/2026) : « quand on
  // change de mois on arrive sur cette version toute ouverte alors que les
  // paies sont fermées en vert ». Un bloc naît ouvert ; il ne se replie
  // qu'à l'arrivée de l'état de sa paie (`etat-par-paie`). Le démarrage
  // l'attendait déjà sous son voile (22/09/2026) ; un changement de mois ne
  // l'attendait pas. Les blocs restent donc cachés jusque-là, et l'on
  // arrive directement sur les cartes. La page ne devine pas elle-même ce
  // qui est validé : une paie se lit validée aussi quand la DSN réelle du
  // mois est partie, et c'est le moteur qui le sait.
  cible.classList.add("en-lecture");
  cible.innerHTML = "";
  regrouper(SAISIE.engagements || []).forEach((g) => cible.appendChild(ligneEngagement(g)));
  // Ce qui vient d'etre charge est, par definition, ce qui est sur le disque :
  // on le note, pour que l'enregistrement automatique ne reecrive pas un
  // fichier identique a chaque fois qu'on effleure un champ.
  clearTimeout(MINUTEUR_SAISIE);
  DERNIERE_ECRITE = JSON.stringify(saisieCourante());
  // Un mois dont la DSN etait due (le 15 du mois suivant) ne se touche
  // plus : lecture seule, pas d'ajout, pas d'enregistrement automatique.
  MOIS_FERME = r.hors_delai || null;
  // Rouvrir ne vaut que pour LE mois rouvert : on change de mois, on
  // retrouve la lecture seule.
  if (MOIS_FORCE !== mois) MOIS_FORCE = null;
  // Les blocs se montrent quand leurs états sont posés -- ou, quoi qu'il
  // arrive, au bout de quelques secondes : une lecture qui traîne ne doit
  // pas cacher la saisie.
  const montrer = () => {
    if (DEMANDE_MOIS === mois) cible.classList.remove("en-lecture");
  };
  setTimeout(montrer, 5000);
  // Les contrats qui courent encore arrivent AVANT les etats des paies :
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
  dessinerDeclareAvant(mois, r.existe);
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
    dessinerReprise(),
    dessinerArrets(mois),
    dessinerFins(mois),
    // Ou en est CHAQUE paie. Se lit apres le chargement des blocs : les etats
    // se posent sur eux, et un bloc absent n'aurait rien ou se poser.
    dessinerEtatsDesPaies(mois).finally(montrer),
  ];
  return Promise.allSettled(lectures);
}

/** Ce que la fermeture d'un mois change à l'écran — et ce que sa réouverture
 *  rend. Un seul endroit : trois booléens éparpillés, c'est un écran qui se
 *  contredit. `MOIS_FERME` est la date limite dépassée ; `MOIS_FORCE` dit
 *  qu'on a répondu à l'avertissement pour ce mois-là. */
function majFermeture(mois, existe) {
  const ferme = !!MOIS_FERME && MOIS_FORCE !== mois;
  const rouvert = !!MOIS_FERME && MOIS_FORCE === mois;
  document.getElementById("ajouter-engagement").hidden = ferme;
  document.getElementById("engagements").classList.toggle("lecture-seule", ferme);
  // UN MOIS FERMÉ, TROIS CAS, UN SEUL BOUTON (30/09/2026). Le bouton disait
  // « le rouvrir (DSN en retard) » à un mois dont la DSN était passée, et
  // rouvrir un mois d'une année close finissait, quatre clics plus loin,
  // sur un refus. Désormais : année close, on le dit et rien d'autre ; DSN
  // passée, on corrige (la correction part dans la DSN suivante) ; sinon,
  // la DSN est en retard et on rouvre.
  const h = (ETAT.historique || []).find((x) => x.mois === mois) || {};
  const b = document.getElementById("rouvrir-mois");
  b.hidden = !ferme || !!h.clos || (["passee", "attente"].includes(h.dsn) && !h.corrigeable);
  if (["passee", "attente"].includes(h.dsn) && h.corrigeable) {
    b.textContent = "Corriger ce mois…";
    b.classList.remove("alerte");
    b.onclick = () => boiteCorriger(mois);
  } else {
    b.textContent = "Rouvrir ce mois (DSN en retard)";
    b.classList.add("alerte");
    b.onclick = () => boiteRouvrirMois(mois, MOIS_FERME);
  }
  noteSaisie(ferme
    ? h.clos
      ? `L'année ${mois.slice(0, 4)} est close : ce mois se lit. Pour le modifier, rouvrir l'année (Historique › L'année).`
      : ["passee", "attente"].includes(h.dsn)
        ? "Sa DSN est partie : ce mois se lit. Une correction part dans la DSN suivante."
        : `Sa DSN était due le ${jjmmaaaa(MOIS_FERME)} : ce mois se lit, et se rouvre pour la déposer en retard.`
    : rouvert
      ? `Mois rouvert — sa DSN était due le ${jjmmaaaa(MOIS_FERME)} : ce qui part d'ici part en retard.`
      : existe
        ? "Saisie chargée. L'enregistrement est automatique."
        : "Nouveau mois — l'enregistrement est automatique dès la première saisie.");
}

/** Un mois PASSÉ sans fichier de paie (avant PuppetPaie) : ce que la
 *  compagnie a déclaré ce mois-là, lu dans l'historique importé (AEM ou
 *  bulletins d'un autre logiciel) — le calendrier du mois et la liste des contrats, en lecture
 *  seule, à la place du vide. « Je voulais que l'historique soit au même
 *  endroit que les paies » (16/09/2026). */
async function dessinerDeclareAvant(mois, existe) {
  const cible = document.getElementById("declare-avant");
  cible.hidden = true; cible.innerHTML = "";
  const courant = new Date().toISOString().slice(0, 7);
  if (existe || mois >= courant) return;
  let contrats = [];
  try { contrats = (await api("contrats-passes", { mois })).contrats || []; } catch (e) { return; }
  if (MOIS_AFFICHE !== mois || !contrats.length) return;
  const plusieurs = new Set(contrats.map((c) => c.salarie)).size > 1;
  const nCachets = contrats.reduce((n, c) => n + c.cachets, 0);
  cible.innerHTML = `
    <div class="declare-avant">
      <div class="da-tete"><b>Déclaré en ${echapper(moisEnClair(mois))}</b>
        <span class="discret">— avant PuppetPaie, d'après l'historique importé ·
        ${contrats.length} contrat${contrats.length > 1 ? "s" : ""}, ${nCachets} cachet${nCachets > 1 ? "s" : ""} · lecture seule</span></div>
      <div class="da-corps">
        <div class="c-grille">${casesMoisPasse(mois, contrats, plusieurs)}</div>
        <div class="c-passes">${contrats.map((c) => ligneContratPasse(c, plusieurs)).join("")}</div>
      </div>
      <p class="note">Début, fin et nombre de cachets : c'est tout ce que l'attestation
        portait — les jours exacts ne sont connus que pour les mois produits ici. Les cachets
        chez d'autres employeurs n'y sont pas.</p>
    </div>`;
  cible.hidden = false;
}

/* Changer de mois enregistre d'abord CE QUI EST À L'ÉCRAN, et sous son vrai
 * mois. La case a déjà changé quand on arrive ici : on y remet l'ancien mois
 * le temps de l'écriture, sans quoi les engagements d'août partiraient dans le
 * fichier de septembre — la case du mois est ce qui date les jours joués. */
document.getElementById("mois").onchange = async (e) => {
  fermerCalendriers();
  const neuf = e.target.value;
  if (MOIS_AFFICHE && MOIS_AFFICHE !== neuf) {
    clearTimeout(MINUTEUR_SAISIE);
    e.target.value = MOIS_AFFICHE;
    await enregistrerSaisie(true);
    e.target.value = neuf;
  }
  chargerMois(neuf);
};

/* LE CHOIX DU MOIS (25/09/2026). « Un truc simple, qu'on puisse cliquer sur
 * toute la case pour arriver aux choix, et que le défilement soit plus
 * rapide. » Une case : ‹ Octobre 2026 ›. Les flèches changent le nom tout de
 * suite et ne chargent le mois qu'une fois qu'on s'arrête de cliquer —
 * trois clics rapides font UN chargement, pas trois. Un clic sur le nom
 * ouvre l'année : douze mois, un clic pour y aller. Un point marque les
 * mois qui ont une saisie.
 *
 * La valeur reste dans le champ caché #mois et passe par son événement
 * « change » : la saisie à l'écran s'enregistre d'abord sous son vrai mois. */
let MINUTEUR_MOIS = null;
let ANNEE_GRILLE = null;

/** Le nom dans la case — appelé partout où la valeur change. */
function afficherMoisChoisi() {
  const x = moisLu(document.getElementById("mois").value);
  const nom = document.getElementById("mois-nom");
  nom.textContent = x ? `${MOIS_FR[x[1] - 1].replace(/^./, (c) => c.toUpperCase())} ${x[0]}` : "Choisir un mois";
}

/** Aller à un mois : le nom tout de suite, le chargement après `attente` ms. */
function allerAuMois(v, attente = 0) {
  const c = document.getElementById("mois");
  c.value = v;
  afficherMoisChoisi();
  // LE NOM ET LES CARTES DISENT LE MÊME MOIS (09/10/2026 : « quand je change de
  // mois on reste sur septembre ») : dès que le nom change, les cartes de
  // l'ancien mois s'effacent ; elles reviennent si l'on revient dessus.
  document.getElementById("engagements").classList.toggle("en-lecture", v !== MOIS_AFFICHE);
  clearTimeout(MINUTEUR_MOIS);
  MINUTEUR_MOIS = setTimeout(() => {
    if (c.value !== MOIS_AFFICHE) c.dispatchEvent(new Event("change"));
  }, attente);
}

function moisVoisin(pas) {
  const x = moisLu(document.getElementById("mois").value);
  if (!x) return;
  const d = new Date(x[0], x[1] - 1 + pas, 1);
  // 30 ms : de quoi faire d'un double clic UN chargement ; un mois se lit, il ne se calcule plus (09/10/2026).
  allerAuMois(moisEnTexte(d.getFullYear(), d.getMonth() + 1), 30);
}

function dessinerGrilleMois() {
  const grille = document.getElementById("mois-grille");
  const choisi = document.getElementById("mois").value;
  const aujourdhui = new Date();
  const ceMois = moisEnTexte(aujourdhui.getFullYear(), aujourdhui.getMonth() + 1);
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
  ANNEE_GRILLE = x ? x[0] : new Date().getFullYear();
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

/* UN CLIC SUR UNE DATE OUVRE LE CALENDRIER, PARTOUT (25/09/2026). Sans cela il
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
   3. RIEN NE PART POUR AUTANT. Enregistrer, c'est écrire paies/AAAA-MM.json
      et rien d'autre. Produire consomme des numéros et écrit des fichiers qui
      partiront : ça reste un bouton, et ça le restera. */
let MINUTEUR_SAISIE = null;
let DERNIERE_ECRITE = null;      // le JSON tel qu'il a été enregistré

function noteSaisie(texte) {
  document.getElementById("note-saisie").textContent = texte;
}

/** Enregistre si quelque chose a changé. Rend true si le fichier a été écrit. */
async function enregistrerSaisie(silencieux) {
  const s = saisieCourante();
  if (!s.mois) return false;
  // Un mois ferme ne s'ecrit plus -- sauf celui qu'on a rouvert en repondant
  // a l'avertissement, et le moteur exige alors qu'on le redise (`forcer`).
  if (MOIS_FERME && MOIS_FORCE !== s.mois) return false;
  const empreinte = JSON.stringify(s);
  if (empreinte === DERNIERE_ECRITE) return false;
  try {
    await api("enregistrer-saisie", { mois: s.mois, saisie: s,
                                      forcer: MOIS_FORCE === s.mois });
    DERNIERE_ECRITE = empreinte;
    oublierCouts();
    const h = new Date().toTimeString().slice(0, 5);
    noteSaisie("Enregistré à " + h + ".");
    ETAT.historique = await api("etat").then((e) => e.historique);
    // Ce qui vient d'etre enregistre devient declarable : les blocs qui
    // disaient << pas dans la saisie enregistree >> doivent cesser de le dire,
    // et la colonne << ce qui part >> compte desormais cette paie (elle
    // disait << aucune paie ce mois-ci >> tant qu'on ne rechargeait pas).
    await rafraichirActions();
    // La fin d'un contrat se lit sur la saisie ENREGISTRÉE : tant qu'elle
    // n'était pas relue ici, on tapait le motif de rupture et le bloc
    // continuait d'annoncer qu'il manquait.
    await dessinerFins(s.mois);
    return true;
  } catch (e) {
    // Une paie dont on n'a pas encore choisi les jours n'est pas une faute :
    // c'est la saisie en cours. On le dit en une ligne, sans la règle du
    // fichier (« lignes: [] ») ni d'alerte (30/09/2026).
    if (e.detail?.type === "SaisieIncomplete") {
      noteSaisie(/spectacle/.test(e.message)
        ? "Pas encore enregistré : choisir le spectacle."
        : "Pas encore enregistré : choisir les jours travaillés.");
      return false;
    }
    // Un échec d'enregistrement ne peut pas rester discret : c'est du travail
    // qu'on croit posé et qui ne l'est pas.
    noteSaisie("PAS enregistré — " + e.message);
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

/* ---------------------------------------------------------- production */
/** FERMER LE MOIS (29/09/2026) — bulletins, certificats, DSN. Plus de
 *  montant URSSAF à recopier : le moteur calcule le bordereau comme la DSN
 *  le porte (paie/src/bordereau.rs). Plus de contrats ni de DPAE :
 *  ils sont partis avant, et le moteur refuse de fermer sinon (MoisPasPret).
 *  `apres` enchaîne sur le dépôt, qui part d'abord en test. */
async function boiteProduire(mois, apres) {
  clearTimeout(MINUTEUR_SAISIE);
  await enregistrerSaisie(true);
  const saisie = saisieCourante();
  if (mois && saisie.mois !== mois) {
    dire("Le mois affiché n'est pas " + mois + "."); return;
  }
  let urssaf = null;
  try {
    urssaf = (await api("apercu", { saisie })).urssaf;
  } catch (e) { dire(e.message, "erreur"); return; }

  // Un mois dont la DSN réelle est partie se refait en ANNULE-ET-REMPLACE :
  // on le dit avant, avec l'ordre et la limite que le moteur a calculés.
  const ar = MOIS_ACTES && MOIS_ACTES.mois === saisie.mois
    && MOIS_ACTES.bilans?.dsn?.remplacement;
  const refaire = !!(ar && ar.deposee);
  ouvrirBoite(`
    <h3>${refaire ? "Refaire la DSN du mois ?" : "Fermer le mois ?"}</h3>
    ${moisEnGros(saisie.mois)}
    ${refaire ? `<div class="encart attention"><b>Annule-et-remplace intégral —
      ordre ${echapper(ar.prochain_ordre)}</b> Une DSN réelle est déjà partie
      pour ce mois. Celle-ci la remplace en entier, ordres de télépaiement
      compris. La norme l'admet jusqu'au ${echapper(jjmmaaaa(ar.limite))} à
      minuit ; au-delà, la correction passe par la DSN du mois suivant.</div>` : ""}
    <p>Le logiciel écrit les bulletins, les certificats Congés Spectacles et
      la DSN du mois. La DSN part aussitôt <b>en test</b> ; si le test passe,
      on vous propose de l'envoyer pour de vrai.</p>
    ${urssaf === null || urssaf === undefined ? "" : `<p class="note">Télépaiement
      URSSAF du mois : <b>${echapper(euros(urssaf))}</b>, calculé comme le
      bordereau de la DSN, code par code.</p>`}
    <div id="b-manques"></div>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="b-lancer">Fermer le mois</button>
    </div>`);

  document.getElementById("b-lancer").onclick = async () => {
    const charge = {
      mois: saisie.mois, saisie,
      // Un mois rouvert : le moteur refuserait sans ce mot-la.
      forcer: MOIS_FORCE === saisie.mois,
    };
    document.getElementById("b-lancer").disabled = true;
    try {
      const r = await api("produire", charge);
      fermerBoite();
      // La production a figé les numéros de contrat dans la saisie : on
      // recharge le mois pour que l'écran les porte (et les renvoie).
      await chargerMois(saisie.mois);
      afficherResultat(r);
      dire(r.fichiers.length + " fichier(s) écrits dans " + r.dossier);
      ETAT = await api("etat");
      await rafraichirActions();      // les fichiers a deposer viennent d'apparaitre
      if (apres) await apres();
    } catch (e) {
      if (e.detail?.type === "MoisPasPret") {
        // Ce qui manque, embauche par embauche : c'est une liste, pas un
        // refus sec. Le bouton reste éteint -- il n'y a rien à retenter ici.
        document.getElementById("b-manques").innerHTML = `
          <div class="encart attention"><b>Pas encore : il reste à envoyer</b>
          <ul>${e.detail.manques.map((m) => `<li>${echapper(m.phrase)}</li>`).join("")}</ul>
          DPAE et contrats partent aux étapes 1 et 2.</div>`;
        return;
      }
      document.getElementById("b-lancer").disabled = false;
      dire(e.message, "erreur");
    }
  };
}

function afficherResultat(r) {
  const fichiers = r.fichiers.map(
    (f) => `<li>${echapper(f.nom)} <span class="discret">(${f.octets} o)</span></li>`).join("");
  const etapes = r.etapes.map((e, i) => `
    <li><b>${echapper(e.titre)}</b> — ${echapper(e.detail)}
      ${e.commande ? `<br><code class="discret">${echapper(e.commande)}</code>` : ""}</li>`).join("");
  const avert = r.avertissements.map(
    (a) => `<div class="encart attention">${echapper(a)}</div>`).join("")
    + (r.informations || []).map(
    (a) => `<div class="encart info">${echapperLiens(a)}</div>`).join("");
  document.getElementById("resultat").innerHTML = `
    <div class="carte" style="margin-top:20px">
      <h2>${echapper(r.libelle)} — produit</h2>
      ${avert}
      <div class="encart bien">
        <b>Numéros consommés</b>
        ${r.numeros.bulletin_premier
          ? `bulletins ${r.numeros.bulletin_premier} à ${r.numeros.bulletin_dernier}`
          : "aucun bulletin (aucun cachet ce mois-ci)"}${
          // Plus d'AEM depuis le 01/06/2026 (France Travail l'edite depuis la
          // DSN) ; et pas de certificat CS un mois ou aucun contrat ne finit.
          r.numeros.cs_dernier >= r.numeros.cs_premier
            ? ` · certificats CS ${r.numeros.cs_premier} à ${r.numeros.cs_dernier}`
            : " · aucun certificat CS ce mois-ci (contrat en cours)"}
      </div>
      <div class="colonnes">
        <div><h2>Fichiers</h2><ul>${fichiers}</ul>
          <p class="note">Dans ${echapper(r.dossier)}</p></div>
        <div><h2>À faire ensuite</h2><ol>${etapes}</ol>
          <p class="note">Les dépôts et les envois restent manuels : une
             déclaration part sous la responsabilité de l'employeur.</p></div>
      </div>
    </div>`;
}

