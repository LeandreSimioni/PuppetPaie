/* Ce qui part : actions, DSN, DPAE, depot, repetition, envoi, signature, avenant. */
"use strict";

/* =======================================================================
   LE MOIS : CE QUI PART D'ICI
   =======================================================================
   Quatre actes, un seul écran pour les quatre : déposer la DSN, déposer la
   DPAE, envoyer les documents aux salariés, demander la signature des
   contrats. Le moteur sait les faire depuis le 11/09/2026 ; ils n'avaient
   pas d'écran, et un bouton qui n'existe pas est une fonction qui n'existe
   pas.

   TROIS RÈGLES, ET AUCUNE N'EST TENUE ICI — c'est tout le sujet :

   1. La phrase d'état vient du JOURNAL, par le moteur (`journal.resume`).
      Elle distingue trois choses qu'un écran confondrait vite : jamais
      parti, parti sans réponse, parti et répondu. « En attente » affiché
      comme « conforme », c'est une déclaration qu'on croit faite.
   2. Rien ne part d'un clic. Chaque bouton ouvre un RÉCAPITULATIF, et le
      récapitulatif demande le mot de passe DU LOGICIEL — pas celui de
      l'organisme, que cette page ne voit jamais, ne demande jamais et ne
      reçoit jamais. Le moteur va le chercher lui-même au coffre.
   3. Le mois déclaré y est écrit EN GROS. Ce n'est pas de la décoration :
      recopier le mois était le seul geste qu'on ne peut pas faire sans
      regarder l'écran, et il a été retiré du parcours. Un mot de passe se
      tape de mémoire, les yeux ailleurs — donc c'est ce bloc-là qui porte
      désormais toute la lecture (docstring d'`operations.Preuve`).

   La séance se voit AVANT le clic, et pourtant les boutons restent actifs.
   Un garde-fou posé dans un front est un garde-fou qu'on contourne par
   l'autre face : c'est `operations` qui refuse, et lui seul. La page
   prévient, parce que découvrir un refus après avoir tapé son mot de passe
   est une mauvaise façon d'apprendre qu'il fallait ouvrir la séance.
*/
let MOIS_ACTES = null;          // le dernier /api/etat-du-mois lu

/* LE FIL DES ENVOIS N'EST PLUS ICI (20/10/2026, tickets #219, #220) : tous les envois du mois et tous leurs retours, lus,
 * sont dans l'Historique (js/historique.js, route `historique-mois`). L'étape garde son état et son bouton ; un lien
 * mène au mois dans l'Historique. */

/** La phrase d'un coup d'œil, dans la tête de la boîte : courte quand c'est
 *  fini, la phrase du bilan sinon (elle dit ce qui reste). */
function phraseCourte(cle, couleur, phrase, courte) {
  // Le bilan peut dire lui-même sa phrase courte : « rien ce mois-ci » pour
  // un contrat en cours, dont la DPAE et la signature relèvent d'un autre
  // mois — « toutes envoyées » serait une promesse que ce mois ne tient pas.
  if (courte) return courte;
  if (couleur === "bien") return {
    "dpae": "toutes envoyées", "contrat-signature": "envoyés et signés",
    "documents": "envoyés à tous", "dsn": "déposée et passée",
  }[cle] || phrase;
  if (couleur === "refus") return "REFUSÉ — à revoir";
  if (couleur === "alerte") return {
    "dpae": "en attente / à faire", "contrat-signature": "en attente / à faire",
    "documents": "en partie", "dsn": "retour attendu",
  }[cle] || phrase;
  return {
    "dpae": "à déposer", "contrat-signature": "à envoyer",
    "documents": "à envoyer", "dsn": "à déposer",
  }[cle] || phrase;
}

function couleurActe(ligne) {
  if (!ligne) return "";
  const retours = ligne.retours || [];
  // Un bilan KO sans certificat : c'est un refus, et ça se voit en rouge.
  if (retours.some((r) => r.verdict === "KO") && !retours.some((r) => String(r.nature) === "41")) return "refus";
  return retours.length ? "bien" : "alerte";
}

/* IL N'Y A PLUS DE SÉANCE (16/09/2026). Il y avait ici un encart « Séance
 * fermée — un dépôt sera refusé » : la séance s'ouvrait avec la fenêtre et
 * valait tant qu'elle vivait, et le dépôt l'exigeait EN PLUS du mot de passe
 * retapé. Deux preuves pour un geste, dont une invisible. Depuis la fenêtre
 * unique, la seule preuve est le mot de passe du logiciel demandé dans
 * popup de l'acte (`demanderMotDePasse`) ; rien à annoncer entre les actes. */

/** La date limite de la DSN d'un mois, telle que l'échéancier la calcule
 *  (le jour se déduit de l'effectif) : « 15/10/2026 ». Lue une fois par
 *  séance ; sans elle, l'étape se tait sur sa date. */
let ECHEANCIER = null;
async function limiteDsn(mois) {
  try {
    if (!ECHEANCIER) ECHEANCIER = await api("echeancier");
  } catch (e) { return ""; }
  const x = [...(ECHEANCIER.ce_mois || []), ...(ECHEANCIER.a_venir || [])]
    .find((y) => y.cle === "dsn:" + mois);
  return x && x.date ? jjmmaaaa(x.date) : "";
}

/** LE CHEMIN D'UN DÉPÔT, DANS L'ORDRE OÙ IL SE FAIT (26/09/2026 : « pour
 *  la DPAE il faut cliquer sur fabriquer, ensuite la mettre en test, attendre
 *  les résultats du test, et envoyer »). Le moteur dit où en est le fichier
 *  (`bilan.etape`) ; le bouton de l'étape dit le geste suivant, et ouvre
 *  directement la boîte de dépôt de CE fichier quand il est déjà fabriqué. */
function cheminDuDepot(a, bilan) {
  if (a.cle !== "dpae" && a.cle !== "dsn") return;
  const nom = a.cle === "dsn" ? "la DSN" : "la DPAE";
  if (bilan && bilan.attente) {
    a.bouton = a.cle === "dsn" ? "Voir la DSN déposée…" : "Voir les DPAE…";
    return;
  }
  const etape = bilan && bilan.etape;
  if (!etape || !bilan.fichier) return;
  a.bouton = {
    ecrite: `Tester ${nom}…`,
    en_test: "Voir le test en cours…",
    testee: `Envoyer ${nom} testée…`,
    test_refuse: "Voir le refus du test…",
    paie_changee: `Réécrire ${nom} et la tester…`,
  }[etape] || a.bouton;
  const fichier = bilan.fichier;
  // UNE PAIE MODIFIÉE REPASSE PAR LE TEST (30/09/2026, l'éditeur : « si on
  // modifie une paie on doit passer en mode test à nouveau, simple et
  // efficace »). Le fichier testé ne déclare plus la paie : la DSN se refait
  // depuis les engagements validés, et la boîte de dépôt s'ouvre sur le test.
  if (etape === "paie_changee") {
    a.ouvrir = () => deposerLaDsn(document.getElementById("mois").value);
    return;
  }
  a.ouvrir = () => boiteDepot(a.cle, fichier);
}

/* -------------------------------------------- la colonne des quatre actes */
async function dessinerActions(mois) {
  const cible = document.getElementById("actions-mois");
  if (!mois) { cible.innerHTML = ""; return; }
  cible.innerHTML = `<p class="discret">Lecture du journal…</p>`;
  let e;
  try {
    e = await api("etat-du-mois", { mois });
  } catch (err) {
    cible.innerHTML = `<p class="discret">${echapper(err.message)}</p>`;
    return;
  }
  MOIS_ACTES = e;

  // Quel fichier se dépose est tranché par le moteur (`fichiers_deposables`) :
  // un mois porte souvent deux DSN, et la bonne est la dernière écrite.
  const dsn = (e.fichiers.dsn || [])[0];
  const dpae = (e.fichiers.dpae || [])[0];

  /* DANS L'ORDRE OÙ ÇA SE FAIT, et non dans l'ordre où c'est important.
     « L'ordre à droite est absurde : c'est d'abord DPAE, puis contrat, puis
     DSN, puis documents ». Il décrit le calendrier réel
     d'un mois, et c'est le seul ordre qui vaille pour une colonne qu'on
     descend :

       la DPAE part AVANT le premier jour travaillé (c'est la loi) ;
       le contrat se signe autour de l'embauche ;
       la DSN part le 5 ou le 15 du mois suivant ;
       les documents partent une fois la paie faite.

     La DSN était en tête parce qu'elle est l'acte le plus lourd. Une colonne
     n'est pas un classement par gravité : c'est une liste qu'on descend. */
  const actes = [
    { cle: "dpae", titre: "DPAE", court: "DPAE", detail: "les déclarations préalables à l'embauche",
      bouton: "Envoyer les DPAE…", fichier: dpae, toujours: true,
      // La boîte des embauches, comme celle des signatures : chaque contrat
      // du mois, une case, cochée d'avance s'il reste à déclarer
      // (15/09/2026). Le fichier complet du mois ne se redépose jamais tel
      // quel — il redéclarerait ce qui est parti.
      ouvrir: () => boiteDpae(mois) },
    { cle: "contrat-signature", titre: "Contrats à signer", court: "CONTRATS",
      detail: "envoyés avec le bouton « Je signe » — la réponse est relevée toute seule",
      bouton: "Demander la signature…", toujours: true,
      ouvrir: () => boiteSignature(mois) },
    // « La DSN ne doit pas être liée à la sélection : elle fait
    // obligatoirement tout le mois, pas comme les DPAE » (12/09/2026). Elle
    // n'a jamais figuré dans la barre des actions groupées, et elle a quitté
    // les pastilles des blocs de paie le même jour. Le détail le dit ici, à
    // l'endroit où on clique.
    { cle: "dsn", titre: "DSN", court: "DSN",
      detail: "tout le mois d'un coup — une DSN ne se découpe pas",
      bouton: "Déposer la DSN…",
      fichier: dsn, toujours: true,
      // La DSN se fabrique à son dépôt, depuis les engagements validés du mois,
      // puis part en test d'abord. Un seul geste.
      ouvrir: () => deposerLaDsn(mois) },
    { cle: "documents", titre: "Documents aux salariés", court: "DOCUMENTS",
      detail: "",
      bouton: "Envoyer les documents…", toujours: true,
      ouvrir: () => boiteEnvoi(mois) },
  ];

  // UNE LIGNE PAR ACTE, ET LA COULEUR EN UN COUP D'OEIL. La demande, le
  // 13/09/2026 : « un truc court que je vois en un coup d'œil, et si je
  // clique j'ai le reste avec les détails et le bouton » — puis, devant le
  // résultat : « c'est ça ». Quand l'acte est fini (vert), la boîte est
  // repliée sur sa phrase courte ; sinon elle est ouverte, avec le fil des
  // envois et le bouton. Le chevron rouvre toujours.
  /* LES ÉTAPES DU MOIS (23/09/2026). Un parcours, pas quatre boîtes : DPAE,
     contrats, DSN, documents, numérotées et reliées. Une étape faite tient en
     une ligne cochée ; seule L'ÉTAPE SUIVANTE — la première qui n'est pas
     faite — est dépliée, avec son bouton en principal. Les autres se
     déplient d'un clic. Le fil des envois et leurs retours se lisent dans
     l'Historique : c'est un historique, pas un état. */
  const limite = await limiteDsn(mois);
  const lignes = actes.map((a) => {
    const bilan = (e.bilans && e.bilans[a.cle]) || null;
    const couleur = bilan ? bilan.couleur : couleurActe(e.actes[a.cle]);
    const phrase = bilan ? bilan.phrase : e.resumes[a.cle];
    cheminDuDepot(a, bilan);
    // DES DOCUMENTS À RENVOYER (ticket #240) : la paie de certains salariés a
    // changé depuis l'envoi. Le moteur le compte et nomme le geste.
    if (a.cle === "documents" && bilan && bilan.bouton) a.bouton = bilan.bouton;
    // CE QUI PART CE MOIS-CI : le moteur le nomme (`pieces` des paies), la page le dit.
    if (a.cle === "documents") a.detail = (bilan && bilan.detail) || "";
    // L'ANNULE-ET-REMPLACE (30/09/2026). Une DSN réelle est partie : tant que
    // la norme l'admet (veille de l'échéance, minuit), la refaire reste à
    // portée de main — et c'est LE geste quand la paie a changé depuis. Après,
    // la correction part d'elle-même dans la DSN d'un mois suivant.
    const r = a.cle === "dsn" && bilan && bilan.remplacement;
    if (r && r.deposee) {
      const refaire = () => deposerLaDsn(mois);
      const libelle = `Refaire la DSN (annule-et-remplace, ordre ${r.prochain_ordre})…`;
      if (bilan.etape) {
        // Un annule-et-remplace est déjà écrit : son chemin passe devant.
      } else if (r.a_envoyer) {
        a.bouton = libelle; a.ouvrir = refaire;
      } else if (r.admis) {
        a.second = { bouton: libelle, ouvrir: refaire };
      }
    }
    return { a, couleur, phrase, fini: couleur === "bien", vide: !!(bilan && bilan.vide),
             // EN ATTENTE : il reste un retour ou une signature, rien que
             // nous puissions faire — l'étape ne passe pas devant la suivante.
             attente: !!(bilan && bilan.attente),
             courte: phraseCourte(a.cle, couleur, phrase, bilan && bilan.courte) };
  });
  const suivante = lignes.findIndex((l) => !l.fini && !l.attente);
  const NOMS = { "dpae": "DPAE", "contrat-signature": "Contrats",
                 "dsn": "DSN", "documents": "Documents" };
  cible.innerHTML = `<ol class="etapes">` + lignes.map((l, i) => {
    const a = l.a, dispo = a.toujours || !!a.fichier;
    const nEnvois = (e.lignes || []).filter((x) => x.quoi === a.cle).length;
    const echeance = a.cle === "dsn" && limite && !l.fini
      ? ` <span class="etape-limite">avant le ${echapper(limite)}</span>` : "";
    return `
      <li class="etape ${echapper(l.couleur)} ${l.fini ? "faite" : ""} ${i === suivante ? "suivante" : ""}">
        <details ${i === suivante ? "open" : ""}>
          <summary>
            <span class="etape-rond">${l.fini ? "✓" : i + 1}</span>
            <span class="etape-nom">${echapper(NOMS[a.cle] || a.titre)}</span>
            <span class="etape-court">${echapper(l.courte)}</span>${echeance}
          </summary>
          <div class="etape-corps">
            <div class="etape-phrase">${echapper(l.phrase)}</div>
            ${a.detail ? `<div class="discret etape-detail">${echapper(a.detail)}</div>` : ""}
            ${a.fichier ? `<div class="acte-fichier">${echapper(a.fichier.nom)}
               ${a.fichier.ecrit ? `<span class="discret">écrit le ${echapper(quand(a.fichier.ecrit))}</span>` : ""}</div>` : ""}
            ${l.vide ? "" : `<button class="${i === suivante ? "primaire" : "secondaire"}" data-acte="${echapper(a.cle)}"
                    ${dispo ? "" : "disabled"}>${echapper(a.bouton)}</button>`}
            ${a.second ? `<button class="secondaire" data-second="${echapper(a.cle)}">${echapper(a.second.bouton)}</button>` : ""}
            ${dispo ? "" : `<p class="note">${echapper(a.absent)}</p>`}
            ${nEnvois ? `<button class="lien etape-envois" data-historique="${echapper(mois)}">voir ${nEnvois > 1
              ? `les ${nEnvois} envois` : "l'envoi"} et leurs retours dans l'Historique</button>` : ""}
          </div>
        </details>
      </li>`;
  }).join("") + `</ol>`;

  cible.querySelectorAll("[data-acte]").forEach((b) => {
    b.onclick = () => actes.find((a) => a.cle === b.dataset.acte).ouvrir();
  });
  cible.querySelectorAll("[data-second]").forEach((b) => {
    b.onclick = () => actes.find((a) => a.cle === b.dataset.second).second.ouvrir();
  });
  cible.querySelectorAll("[data-historique]").forEach((b) => {
    b.onclick = () => ouvrirMoisHistorique(b.dataset.historique);
  });
}

/* Ce qu'il faut relire après avoir déposé ou envoyé : l'état du mois affiché,
 * et la liste des embauches, qu'un dépôt de DPAE vient de raccourcir. */
async function rafraichirActions() {
  const mois = document.getElementById("mois").value;
  await dessinerActions(mois);
  await dessinerEmbauches(mois);
  await dessinerEtatsDesPaies(mois);
}

/* ---------------------------------------- le récapitulatif d'un dépôt */

/** CE QUE LA DSN DÉCLARE, contrat par contrat, LU DANS LE FICHIER
 *  (30/09/2026, l'éditeur : « ce que l'utilisateur veut voir et vérifier,
 *  c'est que tous les contrats sont dans la DSN »). Comparé aux contrats
 *  saisis du mois : un contrat absent du fichier, ou que la paie dit
 *  autrement, se dit en tête. */
function recapitulatifDsn(r) {
  if (!r) return "";
  if (r.erreur) return `<div class="encart attention"><b>Récapitulatif illisible</b>
    ${echapper(r.erreur)}</div>`;
  const ecarts = r.salaries.reduce((n, s) => n + s.contrats.filter((c) => c.ecarts.length).length, 0);
  // Un nombre du fichier, à la française ; une correction le signe d'un vrai
  // moins (« −184 h » : ce qui s'ajoute à ce qui a déjà été déclaré).
  const signe = (x) => String(x).replace(/^-/, "−");
  const nombre = (x) => signe(Number(x).toLocaleString("fr-FR", { maximumFractionDigits: 2 }));
  const vide = (x) => !x || Number(x) === 0;
  // L'activité en une colonne : « 4 cachets · 4 j », « 151,67 h ».
  const activite = (c) => [
    vide(c.cachets) ? "" : `${nombre(c.cachets)} cachet${Math.abs(Number(c.cachets)) > 1 ? "s" : ""}`,
    vide(c.heures) ? "" : `${nombre(c.heures)} h`,
    vide(c.jours) ? "" : `${nombre(c.jours)} j`,
  ].filter(Boolean).join(" · ") || "—";
  // Les dates sur une ligne : « 06/10 → 09/10/2026 » dans une même année.
  const dates = (c) => {
    if (!c.fin || c.fin === c.debut) return jjmmaaaa(c.debut);
    const d = jjmmaaaa(c.debut), f = jjmmaaaa(c.fin);
    return `${c.debut.slice(0, 4) === c.fin.slice(0, 4) ? d.slice(0, 5) : d} → ${f}`;
  };
  const lignes = r.salaries.map((s) => s.contrats.map((c, i) => `
    <tr class="${c.ecarts.length ? "ecart" : ""}${c.correction ? " correction" : ""}">
      <td>${i === 0 ? `<b>${echapper(s.nom)}</b> ${echapper(s.prenoms)}` : ""}</td>
      <td>${echapper(c.numero)}</td>
      <td class="dates">${echapper(dates(c))}</td>
      <td>${echapper(c.emploi)}</td>
      <td class="num">${c.correction ? "correction : " : ""}${echapper(activite(c))}</td>
      <td class="num">${c.remunere ? echapper(signe(euros(c.brut))) : "—"}</td>
      <td class="fin">${c.correction ? "mois déjà déclaré"
        : c.fin_declaree ? "fin le " + echapper(jjmmaaaa(c.fin_declaree)) : "en cours"}</td>
    </tr>
    ${c.ecarts.length ? `<tr class="ecart"><td></td><td colspan="6">
      <span class="pastille alerte">${echapper(c.ecarts.join(" ; "))}</span></td></tr>` : ""}`).join("")).join("");
  return `
    <div class="encart ${r.conforme ? "bien" : r.compare ? "attention" : ""}">
      <b>${r.contrats} contrat${r.contrats > 1 ? "s" : ""} dans ce fichier${r.corrections
        ? ` (et ${r.corrections} correction${r.corrections > 1 ? "s" : ""} d'un mois déjà déclaré)` : ""}${r.compare
        ? `, ${r.contrats_saisis} saisi${r.contrats_saisis > 1 ? "s" : ""} dans la paie du mois` : ""}</b>
      ${r.conforme ? "Tous les contrats de la paie sont dans la DSN, tels que la paie les dit."
        : r.compare ? `${r.absents.length ? r.absents.length + " contrat(s) de la paie manquent au fichier. " : ""}${ecarts
          ? ecarts + " contrat(s) du fichier ne disent pas ce que dit la paie : réécrire le mois." : ""}`
        : "Pas de paie saisie à comparer : le fichier se lit seul."}
    </div>
    ${r.absents.length ? `<ul class="absents">${r.absents.map((x) =>
      `<li><span class="pastille alerte">absent du fichier</span> ${echapper(x.nom)} — contrat
       ${echapper(x.numero)}, du ${echapper(jjmmaaaa(x.debut))}</li>`).join("")}</ul>` : ""}
    <table class="recap-dsn">
      <tr><th>Salarié</th><th>Contrat</th><th>Dates</th><th>Emploi</th>
        <th class="num">Activité</th><th class="num">Brut</th><th>Fin</th></tr>
      ${lignes}
    </table>`;
}

function corpsDsn(a) {
  const v = a.ordres || [];
  return `
    <p class="discret">${echapper(a.nom)} — ${echapper(a.libelle_type)}
      (type ${echapper(a.type_declaration)}), ordre ${echapper(a.ordre)},
      fraction ${echapper(a.fraction)}, constituée le ${echapper(a.constitution)}.
      ${a.salaries} salarié(s), ${a.lignes} lignes, SIREN ${echapper(a.siren)}.</p>
    ${a.remplace ? `<div class="encart"><b>Annule et remplace</b>
      Ce fichier remplace la déclaration ${echapper(a.remplace)}.</div>` : ""}
    ${a.remplacement?.fichier_remplace ? `<div class="encart attention">
      <b>Annule-et-remplace intégral — ordre ${echapper(a.ordre)}</b>
      Il remplace la DSN réelle déjà partie pour ce mois (ordre
      ${echapper(a.remplacement.dernier_ordre)}), ordres de télépaiement
      compris : c'est lui qui vaudra. Admis jusqu'au
      ${echapper(jjmmaaaa(a.remplacement.limite))} à minuit.</div>` : ""}
    ${a.pas_bareme ? `<div class="encart attention">
      <b>Prélèvement à la source : ${a.pas_bareme} versement(s) sur
      ${a.pas_versements} partent sur la grille par défaut</b>
      Faute de taux transmis par la DGFiP, c'est la grille de l'article 204 H
      du CGI qui s'applique — c'est ce que la loi prévoit, et le dépôt n'est
      pas bloqué. Mais elle prélève le plus souvent <b>plus</b> que le taux
      réel du salarié, qui ne récupère la différence qu'à sa déclaration de
      revenus l'année suivante. Relever les retours avant de déposer peut
      encore changer cela.</div>` : ""}
    ${recapitulatifDsn(a.recapitulatif)}
    <details class="replie">
      <summary>Ordres de télépaiement (${v.length}) — la conséquence de ce que
        la DSN déclare ; un dépôt réel les déclenche tous</summary>
      <div class="encart ${enFictive() ? "info" : "attention"}">
      ${v.length
        ? `<table class="ordres">
             <tr><th>À qui</th><th>Pour quoi</th><th class="nombre">Montant</th><th>Prélevé</th></tr>
             ${v.map((x) => `<tr>
               <td><b>${echapper(x.organisme)}</b></td>
               <td>${echapper(x.objet)}</td>
               <td class="nombre"><b>${echapper(euros(x.montant))}</b></td>
               <td title="${echapper(x.source)}">${echapper(x.quand)}</td></tr>
             ${x.credit ? `<tr class="credit"><td></td><td colspan="3">
               <div class="encart" title="${echapper(x.credit.source)}">
               <b>${echapper(x.credit.titre)}</b>
               ${echapper(x.credit.que_faire)}</div></td></tr>` : ""}`).join("")}
           </table>
           Chaque organisme prélèvera ce montant sur le compte de la compagnie. Ce ne sont pas des indications.
           <span class="discret">La date se lit dans la DSN ou dans le texte qui la fixe (au survol).</span>`
        : "Aucun ordre de télépaiement dans ce fichier."}
      </div>
    </details>`;
}

/** Avant ce jour, la DPAE est plus tôt que ne le dit le texte (R1221-4 : au
 *  plus tôt huit jours avant l'embauche). L'Urssaf l'enregistre quand même
 *  (test du 02/10/2026) : on prévient, on ne bloque pas. */
function dpaeTropTot(a) {
  // Le jour du moteur (#236), pas l'horloge du navigateur.
  return !!(a && a.pas_avant && aujourdhuiIso() && a.pas_avant > aujourdhuiIso());
}

function corpsDpae(a) {
  const deja = (a.embauches || []).filter((x) => x.deja_declaree).length;
  return `
    ${dpaeTropTot(a) ? `<div class="encart attention"><b>Plus de huit jours avant
      l'embauche</b> Le code du travail dit « au plus tôt dans les huit jours
      précédant l'embauche » (art. R1221-4) : à la lettre, à partir du
      ${echapper(jjmmaaaa(a.pas_avant))}. L'Urssaf l'enregistre quand même —
      elle ne contrôle pas ce délai.</div>` : ""}
    <p class="discret">${echapper(a.nom)} — ${a.enregistrements}
      enregistrement(s), ${a.octets} octets, indicateur
      ${echapper(a.indicateur)}.</p>
    <table>
      <tr><th>Salarié</th><th>Contrat</th><th>Du</th><th>Au</th>
        <th>Trace d'envoi</th></tr>
      ${(a.embauches || []).map((x) => `
        <tr>
          <td>${echapper(x.nom)}</td>
          <td>${echapper(x.type_contrat || "")}</td>
          <td>${echapper(jjmmaaaa(x.debut))}</td>
          <td>${x.fin ? echapper(jjmmaaaa(x.fin))
                      : `<span class="discret">sans terme</span>`}</td>
          <td>${x.deja_declaree
                ? `<span class="pastille alerte">déjà déclarée</span>`
                : `<span class="pastille bien">à déclarer</span>`}</td>
        </tr>`).join("")}
    </table>
    ${deja ? `<div class="encart attention">
      <b>${deja} embauche(s) de ce fichier ont déjà une trace d'envoi</b>
      Le fichier porte TOUT le mois : le déposer les redéclare. Déclarer deux
      fois la même embauche n'est pas anodin — l'Urssaf reçoit deux
      déclarations pour un seul contrat.</div>` : ""}`;
}

/** LA DSN DU MOIS SE FABRIQUE À SON DÉPÔT : le moteur l'écrit depuis les
 *  engagements validés (`dsn-a-deposer`), puis la boîte de dépôt s'ouvre sur
 *  elle, en test d'abord. Ce que le moteur en dit se lit sous la barre. */
async function deposerLaDsn(mois) {
  let r;
  try {
    r = await api("dsn-a-deposer", { mois });
  } catch (e) { dire(e.message, "erreur"); return; }
  const dit = (r.avertissements || []).concat(r.informations || []);
  if (dit.length) dire(dit.join("  ·  "));
  await rafraichirActions();
  if (r.chemin) await boiteDepot("dsn", r.chemin, { testerDabord: true });
}

async function boiteDepot(quoi, fichier, { testerDabord = false } = {}) {
  let a;
  try {
    a = await api("depot-apercu", { quoi, fichier });
  } catch (e) { dire(e.message, "erreur"); return; }

  ouvrirBoite(`
    <h3>Déposer ${quoi === "dsn" ? "la DSN" : "la DPAE"}</h3>
    ${moisEnGros(a.mois)}
    ${quoi === "dsn" ? corpsDsn(a) : corpsDpae(a)}
    ${a.deja_depose ? `<div class="encart attention">
      <b>Ce fichier a déjà été déposé</b>
      Le ${echapper(quand(a.deja_depose.quand))}, en mode
      ${echapper(a.deja_depose.mode)}. Redéposer le même fichier ne remplace
      rien : un annule-et-remplace se produit, il ne se redépose pas.</div>` : ""}
    <div id="d-bilan">${encartRepetition(a.repetition, quoi, a)}</div>
    ${enFictive() ? noteFictive(quoi === "dsn" ? "net-entreprises" : "l'Urssaf")
      : `<p class="note">Un dépôt réel demande le mot de passe <b>du logiciel</b>,
      qui ne sert qu'à prouver que quelqu'un est là. Celui de l'organisme ne
      passe pas par cette page : le moteur va le chercher au coffre.</p>`}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="d-test">Envoyer le test</button>
      <button class="secondaire" id="d-reel" disabled>
        ${enFictive() ? `Envoyer — simulé (${echapper(a.mois)})` : `Envoyer le réel — ${echapper(a.mois)}`}</button>
    </div>
    ${a.reel ? `<p class="discret"><button type="button" class="lien" id="d-sans-test">envoyer
      ${enFictive() ? "sans attendre le test — simulé…" : "en réel sans attendre le test…"}</button></p>` : ""}
    ${a.reel ? "" : `<p class="note">Le fichier est codé <b>envoi de test</b> :
      déposé « pour de vrai », il ne déclarerait rien et rien ne serait
      conservé.</p>`}`);

  // Le mot de passe tapé pour le dépôt sert au renvoi (boiteRenvoiDsn) :
  // on ne le redemande pas. Une erreur l'oublie, un refus aussi.
  let mdpConnu = null;
  const lancer = async (reel, forcer = false, contournement = "") => {
    let mdp = "";
    if (reel) {
      mdp = mdpConnu || await demanderMotDePasse("le dépôt réel de " + (quoi === "dsn" ? "la DSN" : "la DPAE"), { envoi: true });
      if (!mdp) return;
      mdpConnu = mdp;
    }
    document.getElementById("d-test").disabled = true;
    document.getElementById("d-reel").disabled = true;
    const rendre = occupe(document.getElementById(reel ? "d-reel" : "d-test"),
                          reel ? "Dépôt réel en cours…" : "Envoi du test en cours…");
    try {
      const r = await api("deposer", { quoi, fichier, reel, motdepasse: mdp,
                                       forcer, contournement });
      rendre();
      if (!reel) {
        // LE TEST NE FERME PLUS LA BOÎTE (25/09/2026) : « ça a quitté la
        // fenêtre au lieu de rester et d'attendre les retours pour proposer
        // d'envoyer direct ». On reste, on attend le compte rendu, et le
        // bouton du réel s'allume ici même.
        dire("Dépôt de test envoyé — on attend son compte rendu.", "envoye");
        attendreBilan();
        rafraichirActions();
        return;
      }
      fermerBoite();
      dire("Dépôt réel effectué — on relève ses retours.", "envoye");
      if (quoi === "dsn") avisDocumentsApresDsn(r.mois);
      await rafraichirActions();
      suivreDepot(quoi, r.flux, r.mois);
    } catch (e) {
      rendre();
      const test = document.getElementById("d-test"), reelB = document.getElementById("d-reel");
      if (test) test.disabled = false;
      if (reelB) reelB.disabled = !a.reel;
      if (e.detail?.type === "RepetitionManquante") {
        // « Aucun acte irréversible sans une répétition réussie du même
        // contenu. » On ne renvoie pas l'utilisateur chercher une commande :
        // le dépôt de test est un bouton, ici, tout de suite.
        return boiteRepetition(e.detail.etat, quoi,
                               () => lancer(false),
                               (motif) => lancer(true, false, motif));
      }
      if (e.detail?.type === "DsnDejaDeposee") {
        // « On attend le retour, et si vraiment on y tient, après un
        // avertissement, on peut renvoyer ». Le mot de
        // passe déjà tapé sert au renvoi : on ne le redemande pas.
        return boiteRenvoiDsn(e.detail.etat, () => lancer(true, true));
      }
      mdpConnu = null;
      dire(e.message, "erreur");
    }
  };
  // LE COMPTE RENDU DU TEST, ATTENDU DANS LA BOÎTE. Toutes les dix secondes,
  // cinq minutes au plus : il arrive le plus souvent dans la minute. Le réel
  // reste éteint tant qu'on attend ; conforme, il devient LE bouton ; refusé,
  // on dit pourquoi et il reste éteint. Passé cinq minutes, on cesse de
  // demander (chaque tour relève les retours chez l'organisme : rien ne doit
  // tourner sans fin) et le réel reste éteint ; le relevé ordinaire rangera le
  // compte rendu, et l'étape du mois ouvrira alors le réel.
  const zone = () => document.getElementById("d-bilan");
  const boutonReel = (principal) => {
    const b = document.getElementById("d-reel"), t = document.getElementById("d-test");
    if (!b) return;
    b.disabled = !a.reel || !principal;
    b.className = principal ? "primaire" : "secondaire";
    if (t) t.className = principal ? "secondaire" : "primaire";
    if (principal && a.reel) b.focus();
    const sansTest = document.getElementById("d-sans-test");
    if (sansTest && principal) sansTest.hidden = true;
  };
  // La boîte peut se fermer pendant l'attente : l'écoute continue, et le dit
  // en bas de page quand le compte rendu arrive (« après un test aussi »).
  // `el` est CETTE boîte : une autre ouverte entre-temps n'est pas touchée.
  const attendreBilan = async () => {
    const el = zone();
    const ici = () => el && el.isConnected;
    const ecrire = (html) => { if (ici()) el.innerHTML = html; };
    const test = document.getElementById("d-test");
    boutonReel(false);
    if (test) test.disabled = true;
    const libre = () => { if (ici() && test) test.disabled = false; };
    const nom = quoi === "dsn" ? "La DSN" : "La DPAE";
    // Seul le bilan du test (le certificat, ou le bilan d'anomalies) ouvre le
    // réel ou le ferme ; l'accusé d'enregistrement dit « envoi reçu », pas
    // « conforme » (tickets #221, #222 ; le moteur refuse le réel sans lui, DSN
    // comme DPAE).
    for (let tour = 0; tour < 30; tour++) {
      let r = null;
      try { r = await api("bilan-repetition", { quoi, fichier }); } catch (e) { /* on réessaie */ }
      if (r && r.conforme && r.compte_rendu === "ok") {
        ecrire(encartRepetition(r, quoi, a));
        libre();
        if (ici()) boutonReel(true);
        else dire(`${nom} est testée — conforme. Le dépôt réel est prêt.`);
        rafraichirActions();
        return;
      }
      if (r && !r.conforme && (r.ratees || []).length) {
        const l = r.ratees[r.ratees.length - 1];
        const anos = (l.retours || []).flatMap((x) => x.anomalies || []);
        const motif = anos.join(" ; ") || l.reponse || "motif non lu";
        ecrire(`<div class="encart attention"><b>Le test est refusé</b>
          ${echapper(motif)}
          Le même refus attendrait le dépôt réel : c'est cela qu'il faut corriger.</div>`);
        libre();
        if (!ici()) dire(`Le test de ${nom.toLowerCase()} est refusé : ${motif}`, "erreur");
        rafraichirActions();
        return;
      }
      ecrire((r && r.conforme ? encartRepetition(r, quoi, a)
        : `<div class="encart"><b>Test envoyé — son compte rendu est attendu</b> Le dépôt réel se fait
            ici, une fois le test revenu conforme.</div>`)
        + (r && r.incident ? `<p class="discret">${echapper(r.incident)}</p>` : ""));
      await new Promise((ok) => setTimeout(ok, enFictive() ? 3000 : 10000));
    }
    ecrire(`<div class="encart attention"><b>Le compte rendu n'est
      pas arrivé en cinq minutes</b> L'envoi est reçu, mais rien ne dit encore
      qu'il est conforme : le dépôt réel attend ce compte rendu. Il se rangera
      au prochain relevé, et l'étape du mois ouvrira alors le dépôt réel.</div>`);
    libre();
  };

  document.getElementById("d-test").onclick = () => lancer(false);
  document.getElementById("d-reel").onclick = () => lancer(true);
  // LE TEST D'ABORD, TOUJOURS (ticket #103, 02/10/2026 : « on propose d'abord
  // un test, on regarde si tout est ok et après on propose le réel »). Le
  // réel reste éteint tant que le test n'est pas revenu conforme. Passer
  // outre reste possible, mais c'est un petit lien, et le moteur demande
  // alors un motif écrit (RepetitionManquante → boiteRepetition).
  const sansTest = document.getElementById("d-sans-test");
  if (sansTest) sansTest.onclick = () => lancer(true);
  // Un test déjà fait pour ce contenu : s'il attend encore son compte rendu,
  // on l'attend ici ; s'il est arrivé, le réel est le bouton principal.
  const rep = a.repetition;
  if (rep && rep.conforme) {
    if (rep.compte_rendu === "ok") boutonReel(true);
    else attendreBilan();
  } else {
    // Pas de test pour ce contenu : c'est lui qui vient d'abord.
    document.getElementById("d-test").className = "primaire";
    document.getElementById("d-reel").className = "secondaire";
    // LE MOIS QU'ON VIENT DE FERMER PART EN TEST TOUT SEUL (29/09/2026) :
    // « on produit la DSN de test complète et on attend le retour du test,
    // ensuite on propose d'envoyer la DSN réelle ». Un test n'engage rien.
    if (testerDabord) lancer(false);
  }
}

/** APRÈS LA DSN RÉELLE, LES DOCUMENTS DES SALARIÉS. Rien ne s'annonce ici
 *  (« on ne promet rien ») : ce qui manque encore aux documents -- l'AER de
 *  France Travail, quand elle est due -- c'est le moteur qui le dit, dans
 *  l'étape 4 (`documents_prets`). */
function avisDocumentsApresDsn(mois) {
  ouvrirBoite(`
    <h3>DSN envoyée</h3>
    ${moisEnGros(mois)}
    <div class="encart info"><b>Les documents des salariés</b> Ils partent de
      l'étape 4, « Envoyer les documents… », qui dit ce qu'il leur manque
      encore.</div>
    <div class="actions">
      <button class="primaire" onclick="fermerBoite()">Compris</button>
    </div>`);
}

/** L'ÉCOUTE DES RETOURS, JUSTE APRÈS UN DÉPÔT RÉEL (25/09/2026 : « pour que
 *  ça devienne vert le plus vite possible »). Toutes les quinze secondes,
 *  dix minutes au plus, sans attendre la veille de vingt minutes ; l'écran
 *  se repeint dès que le dépôt est tranché. */
async function suivreDepot(quoi, flux, mois) {
  if (!flux) return;
  const nom = quoi === "dsn" ? "La DSN" : "La DPAE";
  for (let tour = 0; tour < 40; tour++) {
    await new Promise((ok) => setTimeout(ok, enFictive() ? 4000 : 15000));
    let r = null;
    try { r = await api("suivre-depot", { quoi, flux, mois }); } catch (e) { continue; }
    if (r && r.fini) {
      dire(r.passe ? `${nom} est passée — retour de l'organisme reçu.`
                   : `${nom} est refusée — voir le motif dans l'étape.`, r.passe ? "" : "erreur");
      await rafraichirActions();
      return;
    }
  }
}

/** Où en est la répétition de CE contenu — avant de proposer le dépôt réel.
 *  Un bouton qui refusera au clic est un bouton qui ment. */
/** Les embauches que porte une DPAE, nommées (ticket #105, 02/10/2026 :
 *  deux salariés partis dans un seul dépôt, et l'écran disait « Cette DPAE »
 *  au singulier, sans dire pour qui -- on a cru qu'une seule était partie). */
function embauchesDe(a) {
  const l = (a && a.embauches) || [];
  if (!l.length) return "";
  return ` (${l.length} embauche${l.length > 1 ? "s" : ""} : ${l.map((x) => x.nom).join(", ")})`;
}

/** Ce que le certificat de conformité (nature 41) dit de chaque personne. */
function certificatParPersonne(ligne) {
  const dpae = ((ligne && ligne.retours) || []).flatMap((x) => x.dpae || []);
  if (!dpae.length) return "";
  return `<ul class="liste-certificat">${dpae.map((x) => `
    <li>${echapper([x.nom, x.prenom].filter(Boolean).join(" "))} :
      ${x.enregistree ? `<span class="pastille bien">enregistrée${x.dossier
          ? ` — dossier ${echapper(x.dossier)}` : ""}</span>`
        : x.deja_existante ? `<span class="pastille">déjà existante</span>`
        : `<span class="pastille erreur">${echapper(x.libelle || x.code || "refusée")}</span>`}</li>`).join("")}</ul>`;
}

function encartRepetition(r, quoi, a) {
  if (!r) return "";
  const cette = (quoi === "dsn" ? "Cette DSN" : "Cette DPAE")
    + (quoi === "dsn" ? "" : echapper(embauchesDe(a)));
  // Les deux phrases de l'éditeur (25/09/2026) : « cette DPAE est déjà
  // testée, voulez-vous l'envoyer » — ou « elle est déjà envoyée en test,
  // on attend les retours, qui arrivent bientôt ».
  if (r.conforme && r.compte_rendu === "ok") {
    return `<div class="encart bien">
      <b>${cette} est testée — conforme. Voulez-vous l'envoyer ?</b>
      Testée le ${echapper(quand(r.conforme.quand))} : le même contenu est
      passé au bac à sable sans anomalie. ${enFictive() ? "L'envoi, simulé, est le bouton « Envoyer — simulé »." : "Le dépôt réel est le bouton « Envoyer le réel »."}
      ${certificatParPersonne(r.conforme)}</div>`;
  }
  if (r.conforme) {
    return `<div class="encart">
      <b>${cette} est en test — son compte rendu est attendu</b>
      Envoyée en test le ${echapper(quand(r.conforme.quand))}. ${enFictive() ? "L'envoi simulé se" : "Le dépôt réel se"}
      fait ici, une fois ce compte rendu revenu conforme.</div>`;
  }
  // Jamais essayé : ce n'est pas une mise en garde, c'est l'étape suivante
  // (26/09/2026 : l'orange se garde pour ce qui demande de corriger).
  const rate = (r.ratees || []).length || (r.perimees || []).length;
  return `<div class="encart ${rate ? "attention" : ""}">
    <b>${rate ? "Pas de test valable pour ce contenu" : "D'abord, le dépôt de test"}</b>
    ${echapper(situationRepetition(r))} ${enFictive() ? "L'envoi simulé se fait une fois le" : "Le dépôt réel se fait une fois le"}
    test revenu conforme.</div>`;
}

function situationRepetition(etat) {
  const ratees = etat.ratees || [], perimees = etat.perimees || [];
  if (ratees.length) {
    const l = ratees[ratees.length - 1];
    const anos = (l.retours || []).filter((x) => x.verdict === "KO").flatMap((x) => x.anomalies || []);
    return `Ce contenu a été essayé en test le ${quand(l.quand)}`
      + ` et l'essai a échoué${anos.length ? ` : ${anos.join(" ; ").replace(/\.$/, "")}` : ""}.`
      + ` C'est cela qu'il faut corriger, le même refus attend en réel.`;
  }
  if (perimees.length) {
    return `Un fichier du même nom a bien été essayé le ${quand(perimees[perimees.length - 1].quand)},`
      + ` mais son contenu a changé depuis : un octet qui bouge rend le test caduc.`;
  }
  return "Ce contenu n'a jamais été déposé en test.";
}

/** Le verrou test → réel. Deux issues, et la première est la bonne : faire
 *  la répétition. La seconde existe parce que le service de test sera
 *  indisponible un 5 du mois à 23 h — et elle demande un motif écrit, qui
 *  part au journal des dépôts. */
function boiteRepetition(etat, quoi, repeter, quandMeme) {
  ouvrirBoite(`
    <h3>Ce contenu n'a pas été répété</h3>
    <div class="encart attention">
      <b>Aucun dépôt de test ne répond de ce fichier</b>
      ${echapper(situationRepetition(etat))}
    </div>
    <p class="note">Un dépôt de test exécute les mêmes contrôles et rend le
      même bilan, sans rien conserver et sans rien déclarer. C'est la seule
      façon de savoir qu'un dépôt réel passera — et une déclaration ne se
      rattrape pas, elle s'annule-et-remplace.</p>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="r-tester">Faire le dépôt de test</button>
    </div>
    <p class="note" style="margin-top:18px"><a href="#" id="r-outre">Déposer
      sans répétition</a> — si le service de test est indisponible et que
      l'échéance ne peut pas attendre.</p>
    <div id="r-motif" class="cachee">
      <p class="note">Dire ce qui empêchait le test, en une phrase : c'est ce
        qu'on relira le jour où ce dépôt posera question. Le motif est inscrit
        au journal des dépôts.</p>
      <textarea id="r-raison" rows="2" placeholder="le service de test de net-entreprises répond 503 depuis ce matin"></textarea>
      <div class="actions">
        <button class="secondaire" id="r-quand-meme">${enFictive() ? "Déposer sans répétition — simulé" : "Déposer sans répétition"}</button>
      </div>
    </div>`);
  document.getElementById("r-tester").onclick = () => { fermerBoite(); repeter(); };
  document.getElementById("r-outre").onclick = (e) => {
    e.preventDefault();
    document.getElementById("r-motif").classList.remove("cachee");
    document.getElementById("r-raison").focus();
  };
  document.getElementById("r-quand-meme").onclick = () => {
    const motif = document.getElementById("r-raison").value.trim();
    if (motif.length < 10) {
      dire("Le motif se lit en une phrase : dire ce qui empêchait le test.");
      return;
    }
    fermerBoite();
    quandMeme(motif);
  };
}

/** Une DSN réelle est déjà partie pour ce mois. Dire où elle en est, et ne
 *  renvoyer que si on le veut vraiment. */
function boiteRenvoiDsn(etat, renvoyer) {
  const quand = etat && etat.quand ? quand_(etat.quand) : "?";
  const situation = etat.passee
    ? `Le bilan de conformité est arrivé : <b>elle est passée</b>, il n'y a
       rien à faire de notre côté.`
    : `<b>Son retour n'est pas encore arrivé</b> : on attend, on ne renvoie pas.`;
  ouvrirBoite(`
    <h3>Une DSN est déjà partie pour ce mois</h3>
    <div class="encart attention">
      <b>Déposée le ${echapper(quand)}${etat.flux ? ` — flux ${echapper(etat.flux)}` : ""}</b>
      ${situation}
    </div>
    <p class="note">Renvoyer quand même fait un <b>annule-et-remplace</b> à
      net-entreprises : la nouvelle DSN remplace la précédente, avec ses ordres
      de télépaiement. À ne faire que si c'est voulu — une paie corrigée, par
      exemple — et jamais parce qu'on n'a pas encore vu le retour.</p>
    <div class="actions">
      <button class="primaire" onclick="fermerBoite()">Ne pas renvoyer</button>
      <button class="secondaire" id="d-renvoyer">${enFictive() ? "Renvoyer quand même — simulé" : "Renvoyer quand même"}</button>
    </div>`);
  document.getElementById("d-renvoyer").onclick = () => { fermerBoite(); renvoyer(); };
}
const quand_ = (iso) => quand(iso);

/* ------------------------------------------ les documents aux salariés */
async function boiteEnvoi(mois) {
  let a;
  try {
    a = await api("envoi-apercu", { mois });
  } catch (e) { dire(e.message, "erreur"); return; }

  // LE MOTEUR DIT S'IL PEUT (documents_prets) : DSN déposée et passée, AER
  // revenue, documents écrits. Et une boîte courriel pour ENVOYER (le mot de
  // passe d'application dans Configuration > Mots de passe) — sans elle,
  // les documents se donnent à la main depuis le dossier du mois. Le compte
  // Google n'y est pour rien : il ne porte que le Drive (18/09/2026).
  const peut = a.pret && a.boite_liee;
  ouvrirBoite(`
    <h3>Envoyer les documents</h3>
    ${moisEnGros(mois)}
    ${a.boite_liee ? `<p class="discret">Depuis ${echapper(a.expediteur)}.</p>`
      : `<div class="encart"><b>Aucune boîte courriel</b>
         Posez le mot de passe d'application de la boîte de la compagnie
         (Configuration › Mots de passe) pour envoyer par courriel. En
         attendant, les documents s'ouvrent depuis le dossier du mois et se
         remettent à la main.</div>`}
    ${(a.empechements || []).map((x) =>
      `<div class="encart attention">${echapper(x)}</div>`).join("")}
    ${(a.avertissements || []).concat(a.informations || []).map((x) =>
      `<div class="encart info">${echapper(x)}</div>`).join("")}
    ${!a.messages.length ? "" : `<table class="table-envoi">
      <colgroup><col class="c-salarie"><col class="c-adresse"><col class="c-pieces"><col class="c-taille"></colgroup>
      <tr><th>Salarié</th><th>Adresse</th><th>Pièces</th><th>Taille</th></tr>
      ${a.messages.map((m) => `
        <tr>
          <td>${echapper(m.nom)}${m.renvoi ? `<br><span class="pastille alerte">documents corrigés —
            remplacent ceux du ${echapper(m.envoye_le)}</span>` : ""}</td>
          <td>${echapper(m.destinataire)}</td>
          <td class="pieces">${m.pieces.map((x) => `<div>${echapper(x)}</div>`).join("")}</td>
          <td class="num">${Math.round(m.octets / 1024)} ko</td>
        </tr>
        ${m.renvoi ? `<tr><td colspan="4" class="discret">Le courriel dit ce qui a changé :
          <ul>${m.changements.map((c) => `<li>${echapper(c)}</li>`).join("")}</ul></td></tr>` : ""}`).join("")}
    </table>`}
    ${a.renvois ? `<div class="encart info"><b>La paie a changé depuis l'envoi</b>
      Seuls les salariés dont la paie a changé reçoivent à nouveau leurs
      documents, refaits, avec un courriel qui dit ce qui a changé.</div>` : ""}
    ${a.deja ? `<div class="encart attention"><b>Déjà envoyé</b>
      Le ${echapper(quand(a.deja.quand))} — ${echapper(a.deja.reponse || "")}.
      Renvoyer enverra une seconde fois.</div>` : ""}
    ${!peut ? "" : enFictive() ? noteFictive("les salariés", false)
      : `<p class="note">Un courriel n'engage pas l'employeur comme une
      déclaration — personne ne prélève rien — mais il part chez quelqu'un et
      ne revient pas. Le mot de passe du logiciel est demandé à l'envoi.</p>`}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Fermer</button>
      ${a.messages.length ? `<button class="secondaire" id="e-dossier">Ouvrir le dossier des documents</button>` : ""}
      <button class="primaire" id="e-envoyer" ${peut ? "" : "disabled"}
              title="${peut ? "" : echapper((a.empechements || []).concat(
                a.boite_liee ? [] : ["Aucune boîte courriel."]).join(" "))}">${echapper(a.bouton)}${enFictive() ? " — simulé" : ""}</button>
    </div>`);

  const dossier = document.getElementById("e-dossier");
  if (dossier) dossier.onclick = () => ouvrirDossier(dossier, mois, "documents");
  document.getElementById("e-envoyer").onclick = async () => {
    const mdp = await demanderMotDePasse("envoyer les documents à " + a.messages.length + " salarié(s)", { envoi: true });
    if (!mdp) return;
    const rendre = occupe(document.getElementById("e-envoyer"),
                          "Envoi des documents en cours…");
    const repere = await repereCourrielsFictifs();
    try {
      const r = await api("envoyer-documents", { mois, motdepasse: mdp });
      fermerBoite();
      dire(r.partis.length + " message(s) envoyé(s).", "envoye");
      afficherEnvoi("Documents aux salariés", mois, r.partis, r.avertissements);
      fenetreCourrielsFictifs("documents", repere);
      await rafraichirActions();
    } catch (e) {
      rendre();
      dire(e.message, "erreur");
    }
  };
}

/* --------------------------------------------- les contrats à faire signer */

/** « signé par courriel le … » ou « signé, reçu le … » : le journal sait
 *  d'où vient la mention (contrats_recus_comment), l'écran ne fait que
 *  l'écrire. */
function signeLe(quandRecu, par) {
  return (par === "courriel" ? "signé d'un clic le " : "signé, reçu le ") + quand(quandRecu);
}

async function boiteSignature(mois, seulement) {
  let a;
  try {
    a = await api("signature-apercu", { mois });
  } catch (e) { dire(e.message, "erreur"); return; }

  // Cochés d'avance : ceux dont aucune demande n'est partie. Redemander une
  // signature déjà demandée se fait — on relance quelqu'un — mais ça se
  // décide, ça ne se coche pas tout seul.
  //
  // `seulement` : la sélection venue des cases de la colonne de gauche. Elle
  // REMPLACE la règle du dessus — quelqu'un qui a coché trois paies a déjà
  // choisi, et lui recocher les autres serait défaire son geste. La liste
  // reste entière : on voit ce qu'on n'a pas pris, et on peut le reprendre.
  // Coché d'office : ce qui n'est ni demandé ni revenu signé. Un contrat
  // reçu ne repart pas, même dans une sélection.
  // Un contrat caduc (la paie a changé depuis son envoi) se recoche d'office :
  // celui qui est parti n'est plus le bon.
  // Un contrat SIGNÉ qui a changé ne repart pas en entier : il part en
  // AVENANT (30/09/2026, « sinon il peut réclamer tous les contrats »),
  // coché d'avance comme le reste de ce qui est à faire.
  const avenantAFaire = (c) => c.caduc && c.recu_le;
  const choisi = (c) => (seulement ? seulement.includes(c.salarie)
                                   : c.caduc || (!c.recu_le && !c.demande_le))
                        && (c.caduc || !c.recu_le);
  a.contrats.sort((x, y) => (y.caduc && y.recu_le ? 1 : 0) - (x.caduc && x.recu_le ? 1 : 0));
  ouvrirBoite(`
    <h3>Demander la signature des contrats</h3>
    ${moisEnGros(mois)}
    <div class="encart">
      <b>Signature d'un clic</b>
      Le contrat part par courriel avec un bouton « Je signe ». Le salarié
      appuie dessus, sa messagerie s'ouvre sur une réponse toute écrite avec
      un code unique, il l'envoie. Rien à faire ensuite : la réponse est
      relevée toute seule, à l'ouverture et avec la sauvegarde régulière —
      la signature passe au journal, le message est gardé comme preuve, et
      le salarié reçoit son contrat contresigné.${a.attendues ? ` <b>${a.attendues}
      réponse${a.attendues > 1 ? "s" : ""} attendue${a.attendues > 1 ? "s" : ""}.</b>` : ""}
      Un contrat signé à la main se note toujours d'un clic sur sa pastille.
    </div>
    ${enFictive() ? noteFictive("les salariés").replace("sa réponse arrive comme la vraie",
      "la signature revient comme la vraie") : ""}
    ${noteDpaeAvance()}
    <div class="liste-choix">
      ${a.contrats.map((c) => `
        <label class="choix">
          <input type="checkbox" class="s-qui" value="${echapper(c.salarie)}"
                 data-debut="${echapper(c.debut)}" data-numero="${echapper(c.numero || "")}" ${avenantAFaire(c) ? 'data-avenant="1"' : ""}
                 ${choisi(c) ? "checked" : ""}>
          <span><b>${echapper(c.nom)}</b> — ${echapper(c.spectacle)},
            ${echapper(periodeEnClair(c.debut, c.fin))}
            ${c.numero ? `<span class="discret">(contrat n° ${echapper(c.numero)})</span>` : ""}
            <br><span class="discret">${echapper(c.destinataire)
              || "aucun courriel dans la fiche — l'envoi sera refusé"}</span>
            ${c.caduc && c.recu_le ? `<br><span class="pastille alerte">${echapper(c.pourquoi
              || "la paie a changé depuis la signature")} — signé : partira en avenant</span>
              <button class="lien s-avenant" data-salarie="${echapper(c.salarie)}"
                data-debut="${echapper(c.debut)}" data-numero="${echapper(c.numero || "")}">voir l'avenant</button>`
              : c.avenant_attendu ? `<br><span class="pastille alerte">avenant n° ${echapper(String(c.avenant.numero))}
              envoyé le ${echapper(quand(c.avenant.quand))} — pas encore revenu signé</span>`
              : c.caduc ? `<br><span class="pastille alerte">${echapper(c.pourquoi
              || "la paie a changé depuis l'envoi")}${c.demande_le ? ` (envoyé le ${echapper(quand(c.demande_le))})` : ""} —
              ce contrat est à renvoyer</span>`
              : c.recu_le ? `<br><span class="pastille bien">${echapper(signeLe(c.recu_le, c.recu_par))}</span>`
              : c.demande_le && c.main_propre ? `<br><span class="pastille alerte">imprimé le
              ${echapper(quand(c.demande_le))} pour une remise en main propre — pas encore revenu signé</span>`
              : c.demande_le ? `<br><span class="pastille alerte">demandé le
              ${echapper(quand(c.demande_le))} — pas encore revenu</span>
              ${a.boite_liee ? `<button class="secondaire petit s-relancer"
                data-salarie="${echapper(c.salarie)}" data-debut="${echapper(c.debut)}"
                data-numero="${echapper(c.numero || "")}"
                title="Renvoie le même contrat, avec un mot de rappel">Relancer</button>` : ""}` : ""}</span>
        </label>`).join("")}
    </div>
    ${a.boite_liee ? "" : `<div class="encart"><b>Aucune boîte courriel</b>
      Posez le mot de passe d'application de la boîte de la compagnie
      (Configuration › Mots de passe) pour envoyer les contrats par courriel.
      En attendant, ils s'impriment pour être signés à la main.</div>`}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Fermer</button>
      ${a.produit ? `<button class="secondaire" id="s-dossier">Ouvrir le dossier des contrats</button>` : ""}
      <button class="secondaire" id="s-imprimer" title="Les contrats cochés, en PDF, à imprimer et à faire signer à la main : ils partent sous leur numéro">Imprimer pour une remise en main propre</button>
      <button class="primaire" id="s-envoyer" ${a.boite_liee ? "" : "disabled"}
              title="${a.boite_liee ? "" : "Aucune boîte courriel."}">${enFictive() ? "Envoyer à la signature — simulé" : "Envoyer à la signature"}</button>
    </div>`);

  const dossierS = document.getElementById("s-dossier");
  if (dossierS) dossierS.onclick = () => ouvrirDossier(dossierS, mois, "contrats");
  // IMPRIMER : le contrat part à la main. C'est un départ, il prend son
  // numéro ; le salarié le signe, et sa pastille se coche à son retour.
  document.getElementById("s-imprimer").onclick = async () => {
    const coches = [...document.querySelectorAll(".s-qui:checked")].filter((c) => !c.dataset.avenant);
    if (!coches.length) { dire("Aucun contrat coché."); return; }
    const contrats = coches.map((c) => ({ salarie: c.value, debut: c.dataset.debut, numero: c.dataset.numero }));
    const bouton = document.getElementById("s-imprimer");
    const rendre = occupe(bouton, "Impression…");
    try {
      const r = await api("contrats-imprimer", { mois, contrats });
      rendre();
      dire(r.contrats.length + " contrat(s) à imprimer : " + r.contrats.map((c) => "n° " + c.numero).join(", ") + ".", "envoye");
      await ouvrirDossier(bouton, mois, "contrats");
      await rafraichirActions();
    } catch (e) {
      rendre();
      dire(e.message, "erreur");
    }
  };
  // RELANCER (30/09/2026) : le MÊME contrat, redemandé parce que la
  // signature n'est pas revenue — distinct de « Envoyer », qui sert à un
  // contrat jamais parti ou changé depuis. Le journal le dit, et la
  // signature déjà donnée n'est jamais défaite par une relance.
  document.querySelectorAll(".s-avenant").forEach((b) => {
    b.onclick = (ev) => {
      ev.preventDefault();                 // le bouton est dans un <label>
      fermerBoite();
      boiteAvenant(mois, b.dataset.salarie, b.dataset.debut, b.dataset.numero);
    };
  });
  document.querySelectorAll(".s-relancer").forEach((b) => {
    b.onclick = async (ev) => {
      ev.preventDefault();                 // le bouton est dans un <label>
      const contrats = [{ salarie: b.dataset.salarie, debut: b.dataset.debut, numero: b.dataset.numero }];
      const mdp = await demanderMotDePasse("relancer la signature de ce contrat", { envoi: true });
      if (!mdp) return;
      const rendre = occupe(b, "Relance…");
      try {
        const r = await api("demander-signature",
                            { mois, contrats, motdepasse: mdp, relance: true });
        fermerBoite();
        dire("Relance partie : " + r.partis.join(", ") + ".", "envoye");
        await rafraichirActions();
      } catch (e) {
        rendre();
        dire(e.message, "erreur");
      }
    };
  });
  document.getElementById("s-envoyer").onclick = async () => {
    // Par CONTRAT : un salarié peut avoir un contrat coché et un autre non.
    const coches = [...document.querySelectorAll(".s-qui:checked")];
    // Les contrats signés qui ont changé partent en AVENANT, les autres en
    // contrat : un seul bouton, un seul mot de passe (30/09/2026).
    const avenants = coches.filter((c) => c.dataset.avenant)
      .map((c) => ({ salarie: c.value, debut: c.dataset.debut, numero: c.dataset.numero, mois }));
    // Le numéro distingue deux contrats du même salarié qui commencent le même jour (retours Windows, B).
    const contrats = coches.filter((c) => !c.dataset.avenant)
      .map((c) => ({ salarie: c.value, debut: c.dataset.debut, numero: c.dataset.numero }));
    if (!coches.length) { dire("Aucun contrat coché."); return; }
    const quoi = [contrats.length ? contrats.length + " contrat(s)" : "",
                  avenants.length ? avenants.length + " avenant(s)" : ""].filter(Boolean).join(" et ");
    const mdp = await demanderMotDePasse("envoyer " + quoi + " à la signature", { envoi: true });
    if (!mdp) return;
    const rendre = occupe(document.getElementById("s-envoyer"), "Envoi de " + quoi + " en cours…");
    const repere = await repereCourrielsFictifs();
    try {
      const r = contrats.length
        ? await api("demander-signature", { mois, contrats, motdepasse: mdp })
        : { partis: [], contrats: [] };
      // Les avenants d'un même salarié partent ensemble, dans un seul
      // courriel, comme ses contrats (30/09/2026).
      if (avenants.length) {
        const av = await api("avenants-envoyer", { avenants, motdepasse: mdp });
        r.partis.push(...av.partis);
        r.contrats.push(...av.avenants.map((a) => ({ fichier: a.fichier })));
      }
      fermerBoite();
      dire(quoi + " envoyé(s) à la signature.", "envoye");
      afficherEnvoi("Contrats à signer", mois, [...new Set(r.partis)],
                    r.contrats.map((c) => c.fichier));
      if (enFictive()) {
        attendreSignaturesFictives();
        fenetreCourrielsFictifs("signature", repere);
      }
      await rafraichirActions();
    } catch (e) {
      rendre();
      dire(e.message, "erreur");
    }
  };
}

/** L'AVENANT (30/09/2026). Le contrat parti se lit (son PDF), la paie
 *  d'aujourd'hui s'y compare, article par article : ce qui a changé se
 *  montre, l'ancien texte à côté du nouveau. L'avenant part à la signature
 *  par le même circuit « Je signe » que le contrat. Un contrat qui n'a pas
 *  commencé peut aussi être refait en entier (« annule et remplace »). */
async function boiteAvenant(mois, salarie, debut, numero) {
  let a;
  try {
    // Le numéro et le mois disent QUEL contrat, si le salarié en a deux qui commencent le même jour (retours Windows, B).
    a = await api("avenant-apercu", { salarie, debut, numero: numero || "", mois });
  } catch (e) { dire(e.message, "erreur"); return; }
  const mods = a.modifications || [];
  const cite = (textes) => textes.map((x) => `<p class="citation">« ${echapper(x)} »</p>`).join("");
  ouvrirBoite(`
    <h3>Avenant au contrat n° ${echapper(a.contrat || "")}</h3>
    <p class="discret">${echapper(a.nom)} — ${echapper(a.spectacle)}, du
      ${echapper(jjmmaaaa(a.debut))} au ${echapper(jjmmaaaa(a.fin))} ;
      ${a.signe ? "signé le " + echapper(quand(a.signe))
                : "envoyé le " + echapper(jjmmaaaa(a.date_contrat)) + ", pas encore signé"}</p>
    ${(a.avenants || []).map((x) => `<p class="discret">Avenant n° ${echapper(String(x.numero))}
      envoyé le ${echapper(quand(x.quand))} — ${x.signe ? "signé le " + echapper(quand(x.signe))
      : "pas encore revenu signé"}</p>`).join("")}
    ${mods.length ? `
      <div class="encart">Le contrat parti se compare à la paie d'aujourd'hui :
        ${mods.length > 1 ? mods.length + " articles ont changé" : "un article a changé"}.
        L'avenant n° ${a.numero} cite l'ancien texte et le nouveau ; le reste du
        contrat ne change pas.</div>
      ${mods.map((m) => `<div class="modif-avenant">
        <b>Article ${m.numero} — ${echapper(m.article)}</b>
        <div class="discret">Avant</div>${cite(m.avant)}
        <div class="discret">Désormais</div>${cite(m.apres)}
      </div>`).join("")}
      <label>Date d'effet de l'avenant
        <input type="date" id="av-effet" value="${echapper(a.effet)}"></label>`
    : `<div class="encart">Le contrat parti dit déjà tout ce que dit la paie :
        il n'y a rien à modifier.</div>`}
    ${mods.length && a.remplacable ? `<p class="discret">Le contrat n'a pas commencé : il peut aussi être
      refait en entier. Le nouveau porte en tête « annule et remplace le contrat
      n° ${echapper(a.contrat || "")} du ${echapper(jjmmaaaa(a.date_contrat))} ».</p>` : ""}
    <p class="discret"><a href="#" id="av-pieces">Voir ce qui est parti (contrat envoyé, signé, preuve)</a></p>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Fermer</button>
      ${mods.length && a.remplacable ? `<button class="secondaire" id="av-remplace">${enFictive() ? "Contrat qui annule et remplace — simulé…" : "Contrat qui annule et remplace…"}</button>` : ""}
      ${mods.length ? `<button class="primaire" id="av-envoyer">${enFictive() ? "Envoyer l'avenant à la signature — simulé…" : "Envoyer l'avenant à la signature…"}</button>` : ""}
    </div>`);
  document.getElementById("av-pieces").onclick = (ev) => {
    ev.preventDefault();
    fermerBoite();
    boitePieces({ salarie, debut, numero, nom: a.nom, fin: a.fin, spectacle: a.spectacle,
                  etats: { signature: { fait: a.date_contrat, recu: a.signe } } }, "signature");
  };
  const envoyer = document.getElementById("av-envoyer");
  if (envoyer) envoyer.onclick = async () => {
    const effet = document.getElementById("av-effet").value || a.effet;
    const mdp = await demanderMotDePasse("envoyer l'avenant n° " + a.numero + " à " + a.nom, { envoi: true });
    if (!mdp) return;
    const rendre = occupe(envoyer, "Envoi de l'avenant…");
    try {
      const r = await api("avenant-envoyer", { salarie, debut, numero: numero || "", mois, effet, motdepasse: mdp });
      fermerBoite();
      dire("Avenant n° " + r.numero + " envoyé à la signature : " + r.partis.join(", ") + ".", "envoye");
      afficherEnvoi("Avenant à signer", a.mois, r.partis, [r.fichier]);
      await rafraichirActions();
    } catch (e) {
      rendre();
      dire(e.message, "erreur");
    }
  };
  const remplace = document.getElementById("av-remplace");
  if (remplace) remplace.onclick = async () => {
    const mdp = await demanderMotDePasse("envoyer le contrat qui annule et remplace le n° " + (a.contrat || ""), { envoi: true });
    if (!mdp) return;
    const rendre = occupe(remplace, "Envoi du contrat…");
    try {
      const r = await api("demander-signature", { mois: a.mois, contrats: [{ salarie, debut, numero: numero || a.contrat || "" }],
                                                  motdepasse: mdp, remplace: true });
      fermerBoite();
      dire("Contrat qui annule et remplace le n° " + (a.contrat || "") + " envoyé : "
           + r.partis.join(", ") + ".", "envoye");
      afficherEnvoi("Contrat à signer", a.mois, r.partis, r.contrats.map((c) => c.fichier));
      await rafraichirActions();
    } catch (e) {
      rendre();
      dire(e.message, "erreur");
    }
  };
}

/** Ouvre le dossier du mois dans l'explorateur, et le DIT dans la boîte :
 *  le bandeau du haut est sous la boîte, on ne le voit pas d'ici. Une
 *  fenêtre de l'explorateur peut s'ouvrir derrière PuppetPaie — le chemin
 *  écrit ici permet de la retrouver. */
async function ouvrirDossier(bouton, mois, quoi) {
  let note = bouton.parentNode.querySelector(".note-dossier");
  if (!note) {
    note = document.createElement("p");
    note.className = "note note-dossier";
    bouton.parentNode.appendChild(note);
  }
  try {
    const r = await api("ouvrir-dossier", { mois, quoi });
    // L'explorateur passe devant ; le chemin reste dit ici (#141), s'il
    // s'ouvrait derrière la fenêtre.
    note.classList.remove("erreur");
    note.textContent = r && r.dossier ? `Dossier ouvert : ${r.dossier}` : "";
  } catch (e) {
    note.textContent = e.message;
    note.classList.add("erreur");
  }
}

function afficherEnvoi(titre, mois, partis, details) {
  document.getElementById("resultat").innerHTML = `
    <div class="carte" style="margin-top:20px">
      <h2>${echapper(titre)} — ${echapper(moisEnClair(mois))}</h2>
      <div class="encart bien"><b>${partis.length} message(s) parti(s)</b>
        ${echapper(partis.join(", "))}</div>
      ${(details || []).length
        ? `<p class="note">${echapper(details.join(" · "))}</p>` : ""}
    </div>`;
}

