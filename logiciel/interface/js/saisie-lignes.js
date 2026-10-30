/* Une ligne de paie : rubriques, engagement, nature, faits, le calendrier des jours. */
"use strict";

/* =======================================================================
   PAIE
   ===================================================================== */
/** La rubrique par son code, dans le catalogue rendu par le moteur. */
function rubriqueDe(code) {
  return (ETAT.rubriques || []).find((r) => r.code === code);
}

/** Le déroulant des rubriques, dans l'ordre où le moteur les rend. */
function optionsRubriques(choisie, nat) {
  // CE QUI S'APPLIQUE À LA NATURE DU CONTRAT (30/09/2026, « si compagnie
  // simple, interface simple »). Le moteur dit à quelle forme de paie chaque
  // rubrique s'adresse (`reservee_a`, lu sur les champs que le calcul
  // applique) ; la page compare, elle ne connaît aucune rubrique. Sans
  // nature (`nat` absent) rien ne se filtre, et la rubrique DÉJÀ choisie
  // reste toujours offerte : une saisie ne change pas sous les doigts.
  const forme = nat ? (nat.mensuelle ? "mensuelle" : "activite") : null;
  const propose = (r) => !forme || !r.reservee_a || r.reservee_a === forme || r.code === choisie;
  return (ETAT.rubriques || []).filter(propose).map((r) => `<option value="${echapper(r.code)}" ${r.code === choisie ? "selected" : ""}>
      ${echapper(r.libelle)}</option>`).join("");
}

/* LES LISTES COMMUNES SE FABRIQUENT UNE FOIS. Les rubriques, les
 * métiers (deux cents options), les natures, les spectacles, les salariés à
 * cocher : la même liste dans chaque carte, et chaque carte la refabriquait —
 * échapper, assembler, analyser. Elle se fabrique une fois par source (les
 * tableaux que le moteur a rendus dans ETAT : un ETAT relu en porte d'autres,
 * et la liste se refait), puis chaque élément en reçoit une copie. */
const LISTES = new Map();
function poserListe(cible, cle, sources, fabriquer) {
  let l = LISTES.get(cle);
  if (!l || l.sources.length !== sources.length || l.sources.some((s, i) => s !== sources[i])) {
    const gabarit = document.createElement("template");
    gabarit.innerHTML = fabriquer();
    l = { sources, gabarit };
    LISTES.set(cle, l);
  }
  cible.replaceChildren(l.gabarit.content.cloneNode(true));
}

/** Le déroulant des rubriques d'une ligne, depuis la liste commune de sa forme
 *  de paie. La rubrique choisie que cette forme n'offre pas (une saisie ne
 *  change pas sous les doigts) fait une liste à elle. */
function poserRubriques(select, choisie, nat) {
  const forme = nat ? (nat.mensuelle ? "mensuelle" : "activite") : "";
  const r = rubriqueDe(choisie);
  if (!forme || !r || !r.reservee_a || r.reservee_a === forme) {
    poserListe(select, "rubriques:" + forme, [ETAT.rubriques], () => optionsRubriques(null, nat));
  } else {
    select.innerHTML = optionsRubriques(choisie, nat);
  }
  select.value = choisie;
  // Une rubrique hors catalogue : la première de la liste, comme un déroulant sans choix.
  if (select.value !== choisie) select.selectedIndex = 0;
  select.dataset.forme = forme;
}

/** Une ligne de rémunération : une rubrique, une quantité, un taux.
 *  Le champ « heures » n'apparaît que si la rubrique le demande — une journée
 *  de 5 h, une répétition de 3 h. Un cachet ne le demande pas : ses 12 heures
 *  viennent du règlement, pas de nous. */
function ligneRubrique(l = {}, nat) {
  const div = document.createElement("div");
  div.className = "rubrique";
  // LA RUBRIQUE D'UNE LIGNE NEUVE, le moteur la dit (la première du
  // déroulant serait « Absence » dans une compagnie neuve) : par forme
  // de paie (`etat.rubrique_par_defaut`, `catalogue::par_defaut`).
  const forme = nat && nat.mensuelle ? "mensuelle" : "activite";
  const code = l.rubrique || (ETAT.rubrique_par_defaut || {})[forme] || "";
  // Une ligne neuve part du dernier taux donné, pas d'un champ vide : c'est
  // là que « on met de base le dernier taux » se voit. Le dernier taux DE SA
  // RUBRIQUE : « le cachet c'est le cachet, les heures c'est les heures ».
  // UNE LIGNE EN POURCENTAGE (majoration, précarité — 30/09/2026) : le
  // fichier dit `base` et `taux_majoration` ; les deux mêmes champs les
  // montrent, « 120 € à 25 % ».
  if (l.base !== undefined || l.taux_majoration !== undefined) {
    l = { ...l, quantite: l.base ?? "", taux: l.taux_majoration ?? "" };
  }
  if (l.taux === undefined || l.taux === null || l.taux === "") l = { ...l, taux: tauxParDefaut(code) };
  /* TOUT SUR LA MÊME LIGNE, et elle se lit comme une phrase :

       1 cachet à 130 €   Théâtre de la Ville   le 12
       4 cachets à 130 €  Théâtre 71            le 14 15 17 18

     « Pour moi tout est sur la même ligne : les dates, les lieux et le
     cachet » (12/09/2026). C'est la bonne maille, et elle règle le lieu par
     cachet d'un seul coup : une ligne de paie EST un groupe de cachets, à un
     taux, à un endroit, à des dates. Le lieu n'a plus à être rattaché à une
     date d'un côté et à un engagement de l'autre — il est là, sur la ligne
     qui le porte.

     Ce qui a disparu au passage : le rappel d'unité en fin de ligne
     (« cachet », qui redisait le déroulant) et les « 12 h » du cachet, qui
     sont une constante du règlement. Ce qui reste en mots : « à » et « € »,
     les deux seuls qui disent lequel des nombres est le taux. */
  div.innerHTML = `
    <!-- LES JOURS D'ABORD (23/09/2026) : ce sont eux qui situent l'engagement et
         qui comptent les cachets. -->
    <button type="button" class="r-jours-btn" title="cocher les jours dans le calendrier">—</button>
    <input class="r-quantite" type="number" min="0" step="0.5" placeholder="qté"
           value="${l.quantite ?? ""}">
    <select class="r-code"></select>
    <span class="r-mot discret">à</span>
    <input class="r-taux" type="number" min="0" step="0.01" placeholder="taux"
           value="${l.taux ?? ""}">
    <span class="r-mot discret">€</span>
    <input class="r-lieu" placeholder="lieu — figure au contrat"
           value="${echapper(l.lieu || "")}">
    <!-- LE GENRE d'un avantage en nature (repas, logement, véhicule, NTIC,
         autres : S21.G00.54.001) — seulement sur la rubrique qui le demande,
         et ses valeurs viennent du moteur (« genres »). Vide : celui de la
         rubrique, qui n'est écrit nulle part. -->
    <select class="r-genre cachee" title="genre de l'avantage en nature (bloc 54)"></select>
    <!-- LES JOURS SE COCHENT DANS UN CALENDRIER (15/09/2026 : « plus
         esthétique et clair que de taper 12, 24, 26 »). Le bouton dit les
         jours cochés ; le champ caché garde l'écriture « 14 15 17 18 » que
         tout le reste de la page lit (joursDe). Les jours cochés dans un
         mois SUIVANT sont le planning de la ligne (data-planning). -->
    <input class="r-jours" type="hidden"
           value="${echapper((l.dates || []).map((x) => +jourDe(x).slice(8)).join(" "))}">
    <input class="r-heures" type="number" min="0" step="0.5" placeholder="heures"
           value="${l.heures ?? ""}">
    <button class="supprimer" title="retirer cette ligne">&times;</button>
    <!-- LE LIEU PROPRE À UN JOUR : une date du fichier peut dire le sien,
         {"jour", "lieu"} (le moteur l'accepte, \`jours_des_lignes\`). Il se
         montre ici, jour par jour, seulement quand la ligne en porte un ; vide,
         le jour prend le lieu de la ligne. -->
    <div class="r-lieux-jours">${lieuxDesJours(l)}</div>
    <div class="r-dit discret cachee"></div>`;
  poserRubriques(div.querySelector(".r-code"), code, nat);
  const majAffichage = () => {
    const r = rubriqueDe(div.querySelector(".r-code").value);
    const veutHeures = !!r && r.saisie.includes("heures");
    // `.cachee` et non l'attribut `hidden` : la feuille de style pose
    // `input { display: block }`, qui l'emporterait sur le defaut du
    // navigateur. Le champ resterait visible sans qu'on comprenne pourquoi.
    // Des heures déjà écrites se montrent, quelle que soit la rubrique : ce
    // qui est dans le fichier se voit (le moteur dit si elles y ont leur place).
    const h = div.querySelector(".r-heures");
    h.classList.toggle("cachee", !veutHeures && h.value === "");
    // UNE LIGNE « MONTANT » (prime, acompte, indemnité, défraiement réel —
    // 30/09/2026) : ni quantité, ni jours, ni lieu. On tape la somme ; la
    // quantité vaut 1 et reste écrite, le moteur lit quantité × taux.
    const montant = !!r && r.saisie.includes("montant") && !r.saisie.includes("quantite");
    const q = div.querySelector(".r-quantite");
    if (montant && q.value === "") q.value = 1;
    q.classList.toggle("cachee", montant);
    div.querySelector(".r-jours-btn").classList.toggle("cachee", montant);
    div.querySelector(".r-lieu").classList.toggle("cachee", montant);
    div.querySelector(".r-mot").classList.toggle("cachee", montant);
    div.querySelector(".r-taux").placeholder = montant ? "montant" : "taux";
    // Ce que la ligne fait, quand le moteur le dit (`dit` de la rubrique : l'acompte).
    const dit = div.querySelector(".r-dit");
    dit.textContent = (r && r.dit) || "";
    dit.classList.toggle("cachee", !(r && r.dit));
    // Une ligne en pourcentage : « base € à taux % », sans jours ni lieu.
    const pct = !!r && r.saisie.includes("taux_majoration");
    const mots = div.querySelectorAll(".r-mot");
    mots[0].textContent = pct ? "€ à" : "à";
    mots[1].textContent = pct ? "%" : "€";
    if (pct) {
      q.placeholder = "base €";
      div.querySelector(".r-taux").placeholder = "%";
      div.querySelector(".r-jours-btn").classList.add("cachee");
      div.querySelector(".r-lieu").classList.add("cachee");
    } else {
      q.placeholder = "qté";
    }
    // Le genre, sur la rubrique qui en a un : les valeurs sont celles du
    // moteur. Le choix se garde d'une rubrique à l'autre tant qu'il existe.
    const genres = (r && r.genres) || [];
    const g = div.querySelector(".r-genre");
    g.classList.toggle("cachee", !genres.length);
    const voulu = g.dataset.pose === "1" ? g.value : (l.genre || "");
    poserListe(g, "genres:" + (r ? r.code : ""), [genres], () => `<option value="">— genre par défaut —</option>`
      + genres.map((x) => `<option value="${echapper(x.code)}">${echapper(x.code)} — ${
        echapper(String(x.libelle).replace(/^Avantage en nature : /, ""))}</option>`).join(""));
    g.value = genres.some((x) => x.code === voulu) ? voulu : "";
    g.dataset.pose = "1";
  };
  div.querySelector(".r-genre").onchange = rafraichirApercu;
  // Changer la rubrique d'une ligne dont le taux n'a pas été tapé : il suit
  // la rubrique. Un taux de cachet ne devient pas un taux horaire.
  div.dataset.code = code;
  div.querySelector(".r-code").onchange = () => {
    const neuf = div.querySelector(".r-code").value;
    const t = div.querySelector(".r-taux");
    if (t.value === "" || String(tauxParDefaut(div.dataset.code)) === t.value) {
      t.value = tauxParDefaut(neuf);
    }
    div.dataset.code = neuf;
    majAffichage(); rafraichirApercu();
  };
  div.querySelector(".supprimer").onclick = () => { div.remove(); rafraichirApercu(); };
  div.querySelectorAll("input").forEach((c) => { c.oninput = rafraichirApercu; });
  // Taper un taux ici, c'est décider du prochain DE LA MÊME RUBRIQUE : le
  // cachet par défaut ne bouge que sur une ligne de cachets (25/09/2026 : un
  // taux horaire était devenu « cachet à 200 € par défaut »).
  div.querySelector(".r-taux").oninput = () => {
    const v = div.querySelector(".r-taux").value;
    const r = div.querySelector(".r-code").value;
    // Une carte ouverte pour être modifiée ne s'écrit pas : son taux ne devient pas celui du mois.
    if (v !== "" && +v > 0 && !div.closest(".en-modification")) {
      if (r === "cachet") { DERNIER_TAUX = +v; afficherRappelsMois(); }
      else TAUX_PAR_RUBRIQUE[r] = +v;
    }
    rafraichirApercu();
  };
  /* LES JOURS COMPTENT LES CACHETS. Taper « 14 15 17 18 » sur une ligne de
     cachets, c'est dire quatre cachets — les recompter à la main serait un
     deuxième endroit où se tromper, et l'AEM déclare les deux (nombre de
     cachets ET nombre de jours). On ne le fait QUE pour les rubriques dont
     l'unité est le cachet : six heures de masterclass tiennent en un jour, et
     là la quantité et les dates disent deux choses différentes. */
  div.querySelector(".r-jours").oninput = () => {
    const r = rubriqueDe(div.querySelector(".r-code").value);
    if (r && r.unite === "cachet") {
      const n = joursDe(div.querySelector(".r-jours")).length;
      div.querySelector(".r-quantite").value = n || "";
    }
    rafraichirApercu();
  };
  div.dataset.planning = JSON.stringify((l.planning || []).map(jourDe).filter(Boolean));
  div.querySelector(".r-jours-btn").onclick = (ev) => {
    ev.preventDefault();
    ouvrirCalendrier(div);
  };
  majJoursBouton(div);
  majAffichage();
  return div;
}

/* ------------------------------------------------------------ le calendrier
   « J'aime bien l'idée de pouvoir cocher les dates dans un petit planning,
   plus esthétique et clair que de taper 12, 24, 26. Du coup avec une flèche
   on pourrait aller au mois suivant. Avec un petit avertissement : vous êtes
   en train de créer un contrat sur plusieurs mois. Valider ? » (15/09/2026)

   Un mois à la fois, sous la ligne de paie. Les jours du mois de la saisie
   se cochent comme jours joués (ils comptent les cachets) ; ceux des mois
   suivants sont le PLANNING de la ligne — pas payés ce mois-ci, mais connus :
   le contrat imprimé les liste, la fin du contrat se cale sur le dernier, et
   le mois venu, le contrat repris d'office les retrouve déjà cochés.
   On remonte AUSSI avant le mois de la saisie, mais pour LIRE (16/09/2026,
   « j'aimerais pouvoir accéder à l'historique au moins des contrats dans le
   calendrier ») : les mois passés montrent les contrats déclarés des
   salariés du bloc — en plage grisée quand on ne connaît que début, fin et
   nombre de cachets (l'historique importé des AEM), en jours pleins quand le
   mois a été produit ici. Rien ne s'y coche : un cachet d'octobre se paie
   dans le fichier d'octobre. C'est un plancher, pas une position : les
   autres employeurs du salarié n'y sont pas. */
function planningDe(ligne) {
  try { return JSON.parse(ligne.dataset.planning || "[]"); } catch (e) { return []; }
}

/** Ce que dit le bouton : « le 12, 13 », puis le planning « +1 nov. +3 déc. ». */
function majJoursBouton(ligne) {
  const b = ligne.querySelector(".r-jours-btn");
  if (!b) return;
  const jours = joursDe(ligne.querySelector(".r-jours"));
  const plan = planningDe(ligne);
  const parMois = {};
  plan.forEach((iso) => { const m = iso.slice(0, 7); parMois[m] = (parMois[m] || 0) + 1; });
  let texte = jours.length ? jours.join(", ") : "— jours —";
  const suite = Object.keys(parMois).sort()
    .map((m) => `+${parMois[m]} ${MOIS_ABR[+m.slice(5, 7) - 1]}`).join(" ");
  b.textContent = texte + (suite ? " " + suite : "");
  b.classList.toggle("vide", !jours.length);
  b.classList.toggle("planning", !!suite);
  // Le lieu propre à un jour ne se montre que tant que ce jour est coché.
  const mois = document.getElementById("mois").value;
  const coches = new Set(jours.map((j) => `${mois}-${String(j).padStart(2, "0")}`).concat(plan));
  ligne.querySelectorAll(".r-lieu-jour").forEach((c) => { c.closest("label").hidden = !coches.has(c.dataset.iso); });
}

/** Les cases d'un mois PASSÉ : une plage de contrat en gris, un jour joué
 *  connu en gris plein, le libellé au survol. Partagé entre le calendrier
 *  d'une ligne et la vue du mois. */
function casesMoisPasse(vue, contrats, avecNom) {
  const an = +vue.slice(0, 4), mo = +vue.slice(5, 7);
  const nb = new Date(an, mo, 0).getDate();
  const premier = (new Date(an, mo - 1, 1).getDay() + 6) % 7;
  const plage = new Map(), joue = new Map();
  contrats.forEach((c) => {
    const libelle = (avecNom ? c.nom.split(" ")[0] + " · " : "") + c.titre;
    (c.dates || []).forEach((iso) => joue.set(iso, (joue.get(iso) || []).concat(libelle)));
    for (let j = 1; j <= nb; j++) {
      const iso = `${vue}-${String(j).padStart(2, "0")}`;
      if (iso >= c.debut && iso <= c.fin) plage.set(iso, (plage.get(iso) || []).concat(libelle));
    }
  });
  let cases = "";
  for (let i = 0; i < premier; i++) cases += `<span class="c-vide"></span>`;
  for (let j = 1; j <= nb; j++) {
    const iso = `${vue}-${String(j).padStart(2, "0")}`;
    const cls = joue.has(iso) ? "hist-joue" : (plage.has(iso) ? "hist" : "");
    const titre = (joue.get(iso) || plage.get(iso) || []).join(" / ");
    cases += `<span class="c-jour lu ${cls}" ${titre ? `title="${echapper(titre)}"` : ""}>${j}</span>`;
  }
  return JOURS_SEMAINE.map((x) => `<span class="c-sem">${x}</span>`).join("") + cases;
}

/** Une ligne de la liste des contrats d'un mois passé. */
function ligneContratPasse(c, avecNom) {
  return `<div class="c-passe" title="${echapper(c.numero_objet)}">
      <span>${echapper((avecNom ? c.nom.split(" ")[0] + " · " : "") + c.titre)}</span>
      <span class="discret">${echapper(jjmmaaaa(c.debut).slice(0, 5))}→${echapper(jjmmaaaa(c.fin).slice(0, 5))}
        · ${c.cachets} cachet${c.cachets > 1 ? "s" : ""}${c.heures ? ` + ${c.heures} h` : ""}</span>
    </div>`;
}

function fermerCalendriers() {
  document.querySelectorAll(".calendrier").forEach((c) => c.remove());
}

function ouvrirCalendrier(ligne, moisForce) {
  fermerCalendriers();
  // `moisForce` : la correction d'un mois deja depose se coche dans SON mois,
  // pas dans celui qu'affiche l'ecran de paie.
  const mois = moisForce || document.getElementById("mois").value;
  if (!mois) { dire("Choisir le mois d'abord."); return; }
  const bloc = ligne.closest(".engagement");
  const enCours = bloc && bloc.classList.contains("en-cours");
  const fin = bloc ? bloc.querySelector(".e-fin").value : "";
  const cal = document.createElement("div");
  cal.className = "calendrier";
  ligne.appendChild(cal);
  // Sous le bouton des jours, sans déborder de la ligne.
  const btn = ligne.querySelector(".r-jours-btn");
  cal.style.left = Math.max(0, Math.min(btn.offsetLeft, ligne.clientWidth - 272)) + "px";
  let vue = mois;                       // le mois affiché, AAAA-MM
  let attente = null;                   // un jour de planning qui attend « Valider »
  // Les contrats passés, par mois affiché, chargés à la demande. `null`
  // tant qu'on attend le serveur : la grille se dessine sans, puis se redessine.
  const passes = new Map();
  const salaries = bloc ? salariesDuBloc(bloc) : [];
  const chargerPasses = (m) => {
    if (passes.has(m)) return;
    passes.set(m, null);
    api("contrats-passes", { mois: m, salaries }).then((r) => {
      passes.set(m, r.contrats || []);
      if (vue === m && cal.isConnected) dessiner();
    }).catch(() => passes.set(m, []));
  };

  const dessiner = () => {
    const an = +vue.slice(0, 4), mo = +vue.slice(5, 7);
    const nb = new Date(an, mo, 0).getDate();
    const premier = (new Date(an, mo - 1, 1).getDay() + 6) % 7;   // lundi = 0
    const jours = new Set(joursDe(ligne.querySelector(".r-jours")));
    const plan = new Set(planningDe(ligne));
    const ici = vue === mois;
    const passe = vue < mois;
    if (passe) chargerPasses(vue);
    const contrats = passe ? (passes.get(vue) || []) : [];
    let cases = "";
    for (let i = 0; i < premier && !passe; i++) cases += `<span class="c-vide"></span>`;
    for (let j = 1; j <= nb && !passe; j++) {
      const iso = `${vue}-${String(j).padStart(2, "0")}`;
      const coche = ici ? jours.has(j) : plan.has(iso);
      const horsContrat = enCours && fin && iso > fin;
      cases += `<button type="button" class="c-jour ${coche ? (ici ? "joue" : "prevu") : ""}
        ${horsContrat ? "hors" : ""}" data-iso="${iso}" data-j="${j}"
        ${horsContrat ? "disabled title=\"après la fin du contrat\"" : ""}>${j}</button>`;
    }
    const nJoues = jours.size, nPlan = plan.size;
    const listePasses = !passe ? "" : (passes.get(vue) === null
      ? `<div class="c-passes discret">…</div>`
      : `<div class="c-passes">${contrats.length
          ? contrats.map((c) => ligneContratPasse(c, salaries.length !== 1)).join("")
          : `<span class="discret">${salaries.length ? "aucun contrat déclaré ce mois-là" : "cocher un salarié pour voir ses contrats"}</span>`}
        </div>`);
    cal.innerHTML = `
      <div class="c-tete">
        <button type="button" class="c-prec" title="mois précédent">‹</button>
        <b>${echapper(moisEnClair(vue))}</b>
        <button type="button" class="c-suiv" title="mois suivant">›</button>
      </div>
      <div class="c-grille">
        ${passe ? casesMoisPasse(vue, contrats, salaries.length !== 1)
                : JOURS_SEMAINE.map((x) => `<span class="c-sem">${x}</span>`).join("") + cases}
      </div>
      ${listePasses}
      ${attente ? `
        <div class="c-avert">
          <b>Vous êtes en train de créer un contrat sur plusieurs mois</b>
          — du ${echapper(jjmmaaaa(debutDuBloc(bloc) || mois + "-01"))} au
          ${echapper(jjmmaaaa(attente))}. Les cachets de ${echapper(moisEnClair(attente.slice(0, 7)))}
          se paieront dans le fichier de ce mois-là (« reprendre un contrat en
          cours » les retrouvera cochés) ; ce mois-ci partent le contrat et la
          DPAE pour le contrat entier. Valider ?
          <div class="c-actions">
            <button type="button" class="secondaire c-annuler">Annuler</button>
            <button type="button" class="primaire c-valider">Valider</button>
          </div>
        </div>` : `
        <div class="c-pied">
          <span class="discret">${passe ? "mois passé : ce qui a été déclaré, en lecture seule"
            : `${ici ? "jours joués ce mois-ci" : "planning : payé dans son mois"}
            · ${nJoues} joué${nJoues > 1 ? "s" : ""}${nPlan ? `, ${nPlan} prévu${nPlan > 1 ? "s" : ""}` : ""}`}</span>
          <button type="button" class="secondaire c-ok">OK</button>
        </div>`}`;
    cal.querySelector(".c-prec").onclick = () => { vue = moisPrecedent(vue); attente = null; dessiner(); };
    cal.querySelector(".c-suiv").onclick = () => { vue = moisSuivant(vue); attente = null; dessiner(); };
    const ok = cal.querySelector(".c-ok");
    if (ok) ok.onclick = fermerCalendriers;
    const valider = cal.querySelector(".c-valider");
    if (valider) valider.onclick = () => {
      bloc.dataset.pluriOk = "1";
      const iso = attente; attente = null;
      basculerPlanning(ligne, iso);
      dessiner();
    };
    const annuler = cal.querySelector(".c-annuler");
    if (annuler) annuler.onclick = () => { attente = null; dessiner(); };
    cal.querySelectorAll(".c-jour:not([disabled])").forEach((b) => {
      b.onclick = () => {
        if (ici) {
          basculerJour(ligne, +b.dataset.j);
        } else {
          const iso = b.dataset.iso;
          const dejaPluri = (bloc && bloc.dataset.pluriOk === "1") || enCours
            || planningDuBloc(bloc).length || (fin && fin.slice(0, 7) > mois);
          if (!planningDe(ligne).includes(iso) && !dejaPluri) { attente = iso; dessiner(); return; }
          basculerPlanning(ligne, iso);
        }
        dessiner();
      };
    });
  };
  dessiner();
  // Fermer en cliquant ailleurs, ou à la touche Échap.
  setTimeout(() => {
    const dehors = (ev) => {
      if (!cal.isConnected) { document.removeEventListener("click", dehors); return; }
      // `composedPath` et non `contains` : un clic sur un jour redessine la
      // grille, et le bouton cliqué n'est déjà plus dans la page quand on
      // arrive ici -- `contains` dirait « dehors » et refermerait tout.
      const chemin = ev.composedPath ? ev.composedPath() : [];
      if (!chemin.includes(cal) && ev.target !== ligne.querySelector(".r-jours-btn")) {
        fermerCalendriers(); document.removeEventListener("click", dehors);
      }
    };
    document.addEventListener("click", dehors);
  }, 0);
  cal.onkeydown = (ev) => { if (ev.key === "Escape") fermerCalendriers(); };
}

/** Coche ou décoche un jour du mois de la saisie : la ligne compte ses
 *  cachets, comme quand on tapait les jours. */
function basculerJour(ligne, j) {
  const champ = ligne.querySelector(".r-jours");
  const jours = new Set(joursDe(champ));
  if (jours.has(j)) jours.delete(j); else jours.add(j);
  champ.value = [...jours].sort((a, b) => a - b).join(" ");
  champ.dispatchEvent(new Event("input"));
  majJoursBouton(ligne);
}

/** Coche ou décoche un jour de planning (un mois suivant), et cale la fin
 *  du contrat sur le dernier jour prévu — sauf sur un contrat en cours, dont
 *  la fin a été fixée par son premier mois. */
function basculerPlanning(ligne, iso) {
  const plan = new Set(planningDe(ligne));
  if (plan.has(iso)) plan.delete(iso); else plan.add(iso);
  ligne.dataset.planning = JSON.stringify([...plan].sort());
  majJoursBouton(ligne);
  const bloc = ligne.closest(".engagement");
  if (bloc && !bloc.classList.contains("en-cours")) {
    const tout = planningDuBloc(bloc);
    const finChamp = bloc.querySelector(".e-fin");
    const dernier = tout.length ? tout[tout.length - 1] : "";
    if (dernier && (!finChamp.value || finChamp.value < dernier || finChamp.value === bloc.dataset.finAuto)) {
      finChamp.value = dernier; bloc.dataset.finAuto = dernier;
    } else if (!dernier && bloc.dataset.finAuto && finChamp.value === bloc.dataset.finAuto) {
      finChamp.value = ""; bloc.dataset.finAuto = "";
    }
    majContrat(bloc);
  }
  rafraichirApercu();
}

/** Tout le planning d'un bloc, toutes lignes confondues, trié. */
function planningDuBloc(bloc) {
  const vus = new Set();
  if (bloc) bloc.querySelectorAll(".rubrique").forEach((r) => planningDe(r).forEach((x) => vus.add(x)));
  return [...vus].sort();
}

function debutDuBloc(bloc) {
  if (!bloc) return "";
  const jours = joursDuBloc(bloc);
  const ecrit = carteFermee(bloc) ? (engagementsLus(bloc)[0] || {}).debut : bloc.querySelector(".e-debut").value;
  return ecrit || jours[0] || "";
}

/** Les jours tapés « 14 15 17 18 », en nombres, sans doublon et triés. */
function joursDe(champ) {
  const vus = new Set();
  String(champ.value).split(/[^0-9]+/).filter(Boolean).forEach((x) => vus.add(+x));
  return [...vus].sort((a, b) => a - b);
}

/** Les lignes viennent du moteur, y compris pour une saisie ancienne écrite
 *  en `cachets` : c'est lui qui dit ce qu'il a compris, la page ne redevine
 *  rien. Une saisie neuve n'en a pas encore — on offre alors une ligne vide. */
function lignesDe(e) {
  return (e.lignes && e.lignes.length) ? e.lignes : [{}];
}

/* LE DERNIER TAUX DONNÉ, et c'est tout ce qui reste du « par défaut du mois ».
   « Pas besoin de taux de cachet : on met de base le dernier taux donné.
   Simple et efficace » (12/09/2026). Il se reprend de la saisie chargée, et
   il suit ce qu'on tape : la ligne de paie suivante, et le mois suivant,
   partent de là. Il continue d'être écrit dans le fichier — le moteur s'en
   sert comme défaut d'un engagement qui ne dit pas son taux — mais plus
   personne n'a à le saisir. Sa valeur de départ, c'est le MOTEUR qui la dit
   (route `saisie`, `defauts.taux_cachet`) : la page n'en choisit pas. */
let DERNIER_TAUX = null;
/* Le dernier taux donné pour chaque AUTRE rubrique (heure, service de
 * répétition…). Le cachet garde le sien dans DERNIER_TAUX, qui s'enregistre. */
const TAUX_PAR_RUBRIQUE = {};
function tauxParDefaut(code) {
  return code === "cachet" ? (DERNIER_TAUX ?? "") : (TAUX_PAR_RUBRIQUE[code] ?? "");
}
let SAISIE_DATE_PAIEMENT = "";   // celle du fichier, ou celle que le moteur propose

/* =======================================================================
   LE LIEU ET LES JOURS SONT SUR LA LIGNE DE PAIE
   =======================================================================
   Décidé le 11/09/2026 sur un exemple réel — quatre Zerbinette dans le
   mois, un à Berlin, un à Paris, deux à Avallon — écrit le 12, puis recadré par
   lui le même jour, et c'est le recadrage qui a tout simplifié :

     « Pour moi tout est sur la même ligne : les dates, les lieux et le
       cachet. Du coup on peut faire 1 cachet à 130 € au Théâtre de la Ville
       le 12. Ou 4 cachets à 130 € au Théâtre 71 le 14, 15, 17, 18. »

   La première version posait le lieu sur la DATE, avec un héritage à trois
   étages (la date, l'engagement, le mois) et une zone dépliante sous chaque
   engagement pour les cas où les villes diffèrent. Ça marchait, et c'était
   une mécanique à expliquer. Sa maille à lui n'a pas besoin d'héritage : une
   ligne de paie EST un groupe de cachets, à un taux, à un endroit, à des
   dates. Deux villes, deux lignes. Rien à hériter, rien à déplier, rien à
   trancher entre « ce qui est tapé » et « ce qui est repris ».

   C'est TOUJOURS UNE SEULE PAIE : même spectacle, donc même numéro d'objet,
   donc un contrat, un bulletin, une AEM. Les jours de l'engagement sont la
   réunion des jours de ses lignes — `joursDuBloc` ici, `dates_engagement`
   dans le moteur, et les deux composent la même liste.

   Le lieu ne se déclare NULLE PART : ni DSN, ni AEM, ni DPAE. Il figure au
   contrat, qui groupe les dates par lieu, et nulle part ailleurs. */

/** Le jour d'une date écrite d'une façon ou de l'autre — « 2026-10-02 » ou
 *  {"jour": "2026-10-02", "lieu": "Berlin"}. */
/* CE QU'UN ENGAGEMENT PEUT PORTER ET QUE LA SAISIE COURANTE NE DEMANDE PAS.
 * Le moteur lit ces champs dans la saisie (`engagement::
 * engagements`, `solde.rs`) ; un contrat repris, un import, une saisie à la
 * main les écrivent. « Rien de remplissable s'il n'y a pas à le remplir » :
 * chacun n'apparaît dans « Plus d'options » que s'il a une valeur — et alors
 * il se voit, se modifie, se vide, et s'écrit depuis là, comme le reste. */
const CHAMPS_RARES = [
  { cle: "date_paiement", libelle: "Payé le", type: "date",
    titre: "la date de paiement propre à ce contrat (sinon celle du mois)" },
  { cle: "lieu", libelle: "Lieu du contrat", type: "text",
    titre: "le lieu que le contrat imprime quand une ligne de paie n'en dit pas" },
  { cle: "taux_cachet", libelle: "Cachet du contrat", type: "number",
    titre: "le taux du cachet de ce contrat quand aucune ligne de cachet ne le dit" },
  { cle: "jours", libelle: "Jours travaillés déclarés", type: "number",
    titre: "les jours travaillés que la DSN déclare (sinon les jours cochés)" },
  { cle: "emploi", libelle: "Emploi déclaré", type: "text" },
  { cle: "pcs_ese", libelle: "PCS-ESE", type: "text" },
  { cle: "code_emploi_ft", libelle: "Code emploi France Travail", type: "text" },
  { cle: "statut", libelle: "Statut", type: "text" },
  { cle: "statut_conventionnel", libelle: "Statut conventionnel", type: "text" },
  { cle: "statut_categoriel_rc", libelle: "Statut catégoriel (retraite complémentaire)", type: "text" },
  { cle: "quotite_reference", libelle: "Quotité de référence", type: "number",
    titre: "la durée de référence de l'entreprise pour ce contrat (S21.G00.40.012)" },
  { cle: "modalite_temps_travail", libelle: "Modalité du temps de travail", type: "text" },
  { cle: "indemnite_licenciement", libelle: "Indemnité de licenciement", type: "number",
    titre: "le montant retenu, entre l'indemnité légale et la conventionnelle" },
  { cle: "faute_grave", libelle: "faute grave", type: "checkbox",
    titre: "une fin de période d'essai pour faute grave : pas d'indemnité du délai de prévenance (L1221-25)" },
];

const aUneValeur = (v) => v !== undefined && v !== null && v !== "" && v !== false;

/** Les champs rares que l'engagement porte, posés dans le formulaire. */
function poserRares(div, e) {
  const cible = div.querySelector(".e-rares");
  cible.innerHTML = CHAMPS_RARES.filter((c) => aUneValeur(e[c.cle])).map((c) => c.type === "checkbox"
    ? `<label class="coche" title="${echapper(c.titre || "")}"><input type="checkbox" class="e-rare"
         data-cle="${c.cle}" data-type="checkbox" checked> ${echapper(c.libelle)}</label>`
    : `<label title="${echapper(c.titre || "")}">${echapper(c.libelle)}
         <input class="e-rare" data-cle="${c.cle}" data-type="${c.type}" type="${c.type}"
           ${c.type === "number" ? `step="any"` : ""} value="${echapper(String(e[c.cle]))}"></label>`).join("");
  cible.hidden = !cible.children.length;
  cible.querySelectorAll(".e-rare").forEach((c) => { c.onchange = rafraichirApercu; c.oninput = rafraichirApercu; });
}

/** Ce que les champs rares du formulaire écrivent. */
function raresDuBloc(div, e) {
  div.querySelectorAll(".e-rare").forEach((c) => {
    if (c.dataset.type === "checkbox") { if (c.checked) e[c.dataset.cle] = true; return; }
    const v = c.value.trim();
    if (v !== "") e[c.dataset.cle] = c.dataset.type === "number" ? +v : v;
  });
}

/* LES CASES DU CONTRAT ÉCRIT (`.e-ecrit`) : la valeur que l'engagement porte.
 * Une case se montre quand le moteur la demande pour cet engagement
 * (`montrerEcrit`, après chaque aperçu), ou quand elle a déjà une valeur :
 * rien de saisi ne devient invisible. */
function poserEcrit(div, e) {
  div.querySelectorAll(".e-ecrit-champ").forEach((c) => {
    c.value = e[c.dataset.cle] ?? "";
    c.onchange = rafraichirApercu;
  });
  montrerEcrit(div, []);
}

function montrerEcrit(div, demandes) {
  div.querySelectorAll(".e-ecrit [data-ecrit]").forEach((l) => {
    const c = l.querySelector(".e-ecrit-champ");
    l.hidden = !(demandes || []).includes(l.dataset.ecrit) && c.value.trim() === "";
  });
  nommerLesCases(div);
}

/** La ligne repliée du contrat nomme ce qu'il y a à dire dedans (le motif
 *  d'un CDD, la période d'essai…) : ce qui se remplit se trouve sans
 *  chercher. */
function nommerLesCases(div) {
  const d = div.querySelector(".e-contrat");
  if (!d) return;
  const noms = [...d.querySelectorAll(".e-recours-bloc, .e-ecrit [data-ecrit]")]
    .filter((l) => !l.hidden)
    .map((l) => (l.firstChild.textContent || "").trim().toLowerCase())
    .filter(Boolean);
  if (noms.length) d.dataset.cases = noms.join(", ");
  else delete d.dataset.cases;
}

/** Ce que les cases du contrat écrit écrivent. */
function ecritDuBloc(div, e) {
  div.querySelectorAll(".e-ecrit-champ").forEach((c) => {
    const v = c.value.trim();
    if (v !== "") e[c.dataset.cle] = c.dataset.type === "number" ? +v : v;
  });
}

/* LES TERMES DU CONTRAT AVANT UN AVENANT EN COURS DE MOIS (`avant`,
 * periodes.rs : le salaire mensuel et la quotité d'avant, sur lesquels se paie
 * la période d'avant l'avenant). Ils se montrent quand l'engagement les porte,
 * ou quand l'avenant est daté : c'est ce fait-là qui les appelle. */
function poserAvant(div, e) {
  const avant = e.avant || {};
  div.querySelector(".e-avant-salaire").value = avant.salaire_mensuel ?? "";
  div.querySelector(".e-avant-quotite").value = avant.quotite_contrat ?? "";
  const majAvant = () => {
    const dit = div.querySelector(".e-avant-salaire").value !== "" || div.querySelector(".e-avant-quotite").value !== "";
    div.querySelector(".e-avant").hidden = !avenantSurLaCarte(div) || (!dit && !div.querySelector(".e-date-modification").value);
  };
  div.querySelector(".e-date-modification").addEventListener("change", majAvant);
  div.querySelectorAll(".e-avant input").forEach((c) => { c.oninput = rafraichirApercu; });
  majAvant();
  div.querySelector(".e-debut").addEventListener("change", () => majAvenant(div));
  majAvenant(div);
}

/* L'AVENANT NE SE PROPOSE QU'À UN CONTRAT QUI EXISTE DÉJÀ ET QUI SE SAISIT.
 * Un CDI saisi pour la première fois montrait « Avenant à compter du » : rien
 * à remplir, puisque ce qu'on tape ce mois-ci, ce sont les termes du contrat.
 * La case paraît quand le contrat a commencé avant le mois affiché — c'est ce
 * fait-là qui l'appelle —, ou quand elle porte déjà une date. Un engagement
 * validé, ouvert pour être modifié, ne la montre pas, ni les termes d'avant :
 * la date du changement, le moteur la demande à « Modifier l'engagement »
 * (« À partir de quel mois ? »), quand ce qui est parti l'appelle. */
function avenantSurLaCarte(div) {
  return !(div._ligne && div._ligne.valide);
}

function majAvenant(div) {
  const case_ = div.querySelector(".e-date-modification");
  const debut = div.querySelector(".e-debut").value;
  const mois = document.getElementById("mois").value || "";
  const cachee = !avenantSurLaCarte(div) || (!case_.value && !(debut && mois && debut.slice(0, 7) < mois));
  const l = case_.closest("label");
  l.hidden = cachee;
  l.style.display = cachee ? "none" : "";   // la feuille de style pose `display` sur ces étiquettes
}

function avantDuBloc(div) {
  const s = div.querySelector(".e-avant-salaire").value, q = div.querySelector(".e-avant-quotite").value;
  if (s === "" && q === "") return null;
  const a = {};
  if (s !== "") a.salaire_mensuel = +s;
  if (q !== "") a.quotite_contrat = +q;
  return a;
}

/** Les jours d'une ligne qui disent leur propre lieu, un champ chacun. */
function lieuxDesJours(l) {
  const propres = [...(l.dates || []), ...(l.planning || [])]
    .filter((x) => x && typeof x === "object" && x.lieu);
  if (!propres.length) return "";
  return `<span class="discret">lieu propre à un jour :</span> ` + propres.map((x) =>
    `<label class="r-lieu-jour-l">le ${echapper(jjmmaaaa(x.jour).slice(0, 5))}
       <input class="r-lieu-jour" data-iso="${echapper(x.jour)}" value="${echapper(x.lieu)}"></label>`).join("");
}

/** Le catalogue des métiers, tel que le moteur le rend (route « etat »),
 *  groupé par famille dans SON ordre. La page ne trie pas, elle affiche. */
function metierDe(code) {
  return (ETAT.emplois || []).find((m) => m.code === code) || null;
}

function optionsMetiers(choisi, vide = "— celui de la fiche —") {
  const familles = ETAT.familles_emplois || [];
  const parFamille = {};
  (ETAT.emplois || []).forEach((m) => { (parFamille[m.famille] ||= []).push(m); });
  let html = `<option value="">${echapper(vide)}</option>`;
  familles.forEach((f) => {
    if (!parFamille[f]) return;
    html += `<optgroup label="${echapper(f)}">` + parFamille[f].map((m) =>
      `<option value="${echapper(m.code)}">${echapper(m.libelle)}${m.cadre_par_defaut ? " (cadre)" : ""}</option>`)
      .join("") + `</optgroup>`;
  });
  // Un code hors catalogue mais présent dans le fichier reste proposé tel
  // quel : on ne perd pas un métier parce que l'écran ne le connaît pas.
  // Un métier décoché dans l'onglet Métiers garde son nom là où il est choisi.
  if (choisi && !metierDe(choisi)) {
    const ailleurs = (ETAT.emplois_autres || []).find((m) => m.code === choisi);
    html += `<option value="${echapper(choisi)}">${echapper(ailleurs ? ailleurs.libelle : choisi)}</option>`;
  }
  return html;
}

/** Le déroulant des métiers d'une carte, depuis la liste commune ; un métier
 *  hors catalogue s'y ajoute, comme dans `optionsMetiers`. */
function poserMetiers(select, choisi) {
  poserListe(select, "metiers", [ETAT.emplois, ETAT.familles_emplois], () => optionsMetiers(""));
  if (choisi && !metierDe(choisi)) {
    const ailleurs = (ETAT.emplois_autres || []).find((m) => m.code === choisi);
    const o = document.createElement("option");
    o.value = choisi;
    o.textContent = ailleurs ? ailleurs.libelle : choisi;
    select.appendChild(o);
  }
  select.value = choisi || "";
}

/** Le déroulant des spectacles, depuis la liste commune. */
function poserSpectacles(select) {
  poserListe(select, "spectacles", [ETAT.spectacles], () => `<option value="">— à choisir —</option>`
    + (ETAT.spectacles || []).map(
      (s) => `<option value="${echapper(s.cle)}">${echapper(s.titre)}</option>`).join(""));
}

/** Un engagement : QUI, QUOI, QUAND, COMBIEN.
 *
 *  Le « quoi » était en haut du formulaire, une fois pour le mois. C'est juste
 *  la plupart du temps, et faux exactement quand ça compte : deux spectacles,
 *  deux lieux dans le même mois.
 *
 *  LE SPECTACLE N'EST PAS TRAITÉ COMME LES AUTRES, et c'est délibéré. Le lieu
 *  et le taux se répètent d'un engagement à l'autre : ils ont un défaut de mois, et
 *  l'engagement s'en écarte quand il le faut. Le spectacle, lui, EST la raison
 *  d'avoir plusieurs engagements — un contrat porte un numéro d'objet, donc un
 *  spectacle, et le mois en compte plusieurs. Il est donc une case de cet
 *  engagement, toujours visible, sans défaut et sans héritage. */
/* LA CARTE NAÎT FERMÉE : elle ne s'ouvre que quand on clique dessus.
 * Une carte lue dans le fichier du mois (`chargerMois`) naît COQUE : ce qui se
 * voit d'un engagement validé (`.e-resume`, ses imprévus, ses états) et ce que
 * l'aperçu y peint — la carte figée que le moteur rend pour un engagement validé.
 * Elle sait seulement quels engagements du fichier elle montre (`data-rangs`) :
 * tant que son formulaire n'existe pas, rien ne la modifie, et elle les écrit
 * tels quels (`engagementsDuBloc`). Son formulaire se construit à la première
 * occasion qui l'ouvre — l'état de son engagement dit qu'il n'est pas validé, on
 * on l'ouvre d'un clic pour la modifier, les états tardent (`fermerEdition`,
 * `ouvrirCarte`) — et il reste construit tant que la carte vit ; dès lors
 * c'est lui qui écrit. Naissent ouvertes : une carte neuve (« Ajouter un
 * engagement »), un contrat en cours repris, une carte dédoublée, et celle qui
 * était ouverte avant un repeint. */
function ligneEngagement(e = {}, { fermee = false } = {}) {
  const div = document.createElement("div");
  div.className = "engagement";
  div.innerHTML = `
    <div class="resolu"></div>
    <!-- L'ENGAGEMENT VALIDÉ, EN UNE LIGNE : quand, qui, pour
         quoi, ce qui reste à faire, le net. Voir peindreCarte(). -->
    <div class="e-resume"></div>
    <!-- SES IMPRÉVUS DU MOIS (15/10/2026) : une ligne par arrêt ou absence
         qui touche ce contrat, avec ses gestes et les jours payés pendant
         l'imprévu. Vide le plus souvent. Visible aussi sur un engagement validé. -->
    <div class="e-imprevus"></div>
    <div class="avis"></div>
    <div class="etats"></div>`;
  // Le numéro et ce qu'il faut pour reconnaître l'engagement de la carte : posés
  // dès la coque, l'état de l'engagement s'y retrouve avant toute ouverture.
  div.dataset.numero = e.numero || "";
  // À qui ce numéro appartient : un salarié coché en plus n'en hérite pas.
  div.dataset.numeroDe = e.numero ? (e.salarie || "") : "";
  const mois = document.getElementById("mois").value;
  if (e.debut && mois && e.debut.slice(0, 7) < mois) div.classList.add("en-cours");
  if (e.rangs) div.dataset.rangs = e.rangs.join(",");
  if (fermee && e.rangs) return div;
  construireCarte(div, e);
  return div;
}

/** Ouvre une carte fermée : son formulaire se construit, une fois, sur les
 *  engagements qu'elle montre. Ce qui était à l'écran sans modification le
 *  reste : l'enregistrement automatique ne réécrit pas le fichier pour rien. */
function ouvrirCarte(div) {
  if (!carteFermee(div)) return;
  const lus = engagementsLus(div);
  const intacte = DERNIERE_ECRITE !== null && JSON.stringify(saisieCourante()) === DERNIERE_ECRITE;
  construireCarte(div, { ...(lus[0] || {}), salaries: lus.map((x) => x.salarie) });
  if (div._paies) peindreChiffres(div, div._total);
  // Les cases du contrat écrit que le moteur a demandées pour cet engagement, au dernier aperçu.
  const p = (div._paies || [])[0];
  montrerEcrit(div, p && p.contrat ? p.contrat.ecrits : div._ecrits);
  if (intacte) DERNIERE_ECRITE = JSON.stringify(saisieCourante());
}

/** Les salariés à cocher d'une carte : la liste commune, cochée pour `voulus`,
 *  et un salarié du fichier absent des actifs (fiche mise de côté) garde sa
 *  case, cochée : on ne perd pas un engagement parce qu'on a archivé quelqu'un. */
function poserQui(liste, voulus) {
  const actifs = ETAT.salaries || [];
  const connus = new Set(actifs.map((s) => s.cle));
  poserListe(liste, "salaries", [actifs], () => actifs.map(
    (s) => `<label><input type="checkbox" class="e-qui-case" value="${echapper(s.cle)}"> ${
      echapper(s.libelle)}</label>`).join(""));
  liste.querySelectorAll(".e-qui-case").forEach((c) => { c.checked = c.defaultChecked = voulus.has(c.value); });
  liste.insertAdjacentHTML("beforeend", [...voulus].filter((c) => !connus.has(c)).map(
    (c) => `<label><input type="checkbox" class="e-qui-case" value="${echapper(c)}"
       checked> ${echapper(c)} <span class="discret">(fiche mise de côté)</span></label>`).join(""));
}

/** Le formulaire d'une carte, posé devant sa coque et branché. */
function construireCarte(div, e) {
  // QUI : une liste à COCHER, pas un menu, et UN bloc pour tous les cochés.
  //
  // L'ancienne option « Les deux » qui dédoublait le bloc à la sélection
  // était taillée pour une compagnie à deux têtes, et dédoublait trop tôt.
  // La demande : « il faut pouvoir cocher dedans, x Célimène
  // x Marin, pas d'option les deux d'un coup » ; puis : « le dédoublement
  // doit se faire une fois validé, pas avant — c'est ce qui va me faire
  // aller vite quand j'ai plusieurs employés pour le même spectacle avec la
  // même paye ». Donc : on coche qui on veut, on saisit UNE fois, on valide
  // — et LÀ le bloc se démultiplie, un bloc par salarié (validerPlusieurs).
  //
  // Le moteur, lui, ne connaît toujours qu'un salarié par engagement : c'est
  // saisieCourante qui écrit un engagement par salarié coché, et chargerMois
  // qui regroupe au retour ce qui est identique et pas encore validé.
  const voulus = new Set(e.salaries || (e.salarie ? [e.salarie] : []));
  div.insertAdjacentHTML("afterbegin", `
    <div class="ligne1">
      <!-- Pas de « + » ici : une fiche neuve se crée dans Configuration.
           La saisie d'un engagement doit rester simple et agréable ; on y choisit,
           on n'y administre pas (13/09/2026). -->
      <div class="champ">Salarié
        <details class="e-qui">
          <summary class="vide">— cocher —</summary>
          <div class="e-qui-liste"></div>
        </details>
      </div>
      <!-- LA NATURE DU CONTRAT (19/09/2026). Elle ne se devine pas : un
           engagement muet est un CDDU du spectacle, et c'est ce qu'ont été
           les 146 contrats de l'historique. Ce qui manquait, c'était de
           pouvoir dire AUTRE CHOSE — tant qu'elle n'était pas ici, un
           permanent n'avait aucun endroit où exister dans cet écran, et
           donnees/contrats-base.json le savait tout seul. Elle change la
           forme du bloc : un permanent n'a ni spectacle ni cachets, il a un
           salaire au mois et des faits. -->
      <label class="e-nature-bloc">Nature
        <select class="e-nature"></select></label>
      <label class="e-spectacle-bloc">Spectacle
        <select class="e-spectacle"></select></label>
    </div>
    <!-- Les jours et le lieu sont sur chaque ligne de
         paie, qui porte la phrase entière — « 4 cachets à 130 € au
         Théâtre 71 le 14 15 17 18 ». Voir ligneRubrique(). -->
    <div class="rubriques"></div>
    <button class="ajouter-rubrique discret">+ une ligne de paie</button>
    <!-- UN PERMANENT NE SE SAISIT PAS EN LIGNES DE PAIE, ET C'EST TOUT
         L'INTÉRÊT. Ce qu'on tape, ce sont des FAITS — un salaire au mois,
         des congés pris, une prime — et c'est
         le moteur (engagement::conges::lignes_du_mois) qui en fait des lignes : le
         prorata d'un mois incomplet, les deux calculs de l'indemnité de
         congés, la retenue et l'indemnité qui la couvre. Un arrêt ou une
         suspension n'est pas un fait du mois : c'est un IMPRÉVU, saisi une
         fois depuis la carte (« Un imprévu… », js/imprevus.js), dont le moteur tire la ligne de
         chaque mois qu'il traverse. Taper les lignes à la main, ce serait refaire à la main
         ce que la loi impose de calculer. -->
    <div class="e-permanent" hidden>
      <div class="e-mois-paye">
        <label>Salaire mensuel brut
          <input type="number" step="0.01" min="0" class="e-salaire"
                 placeholder="celui du contrat signé"></label>
        <label title="la durée du travail du contrat, en heures par mois (S21.G00.40.013)">Heures par mois
          <input type="number" step="0.01" min="0" class="e-quotite">
          <button type="button" class="lien e-temps-plein" hidden></button></label>
        <!-- UN AVENANT SE DATE. Ce que cette saisie change au
             contrat (quotité, nature, motif, date de début) se déclare avec
             l'ancienne valeur dans le bloc Changements contrat de la DSN, daté
             du jour où il prend effet (S21.G00.41.001) : le moteur le lit ici,
             il ne le devine pas. Il ne se recopie pas au mois suivant. -->
        <label title="le jour où prend effet ce que cette saisie change au contrat — la date que la DSN déclare (S21.G00.41.001)">Avenant à compter du
          <input type="date" class="e-date-modification"></label>
        <span class="e-avant" hidden><span class="discret">avant l'avenant :</span>
          <label>salaire <input type="number" step="0.01" min="0" class="e-avant-salaire"></label>
          <label>quotité <input type="number" step="0.01" min="0" class="e-avant-quotite"></label></span>
      </div>
      <div class="e-faits">
        <div class="e-fait-groupe">
          <div class="e-fait-tete">Congés pris
            <span class="discret">— en jours ouvrables, le compteur suit</span></div>
          <div class="e-conges"></div>
          <button class="ajouter-fait discret" data-quoi="conges">+ des congés</button>
        </div>
        <div class="e-fait-groupe">
          <div class="e-fait-tete">Primes</div>
          <div class="e-primes"></div>
          <button class="ajouter-fait discret" data-quoi="primes">+ une prime</button>
        </div>
      </div>
    </div>
    <!-- LES TROIS NOMBRES D'UNE PAIE — ET ON ÉCRIT DANS LES TROIS.
         « J'aimerais voir pour chaque engagement le net pour le salarié, le
         brut et le coût entreprise. Et qu'une fois les cases remplies, on
         puisse mettre par exemple le coût pour l'entreprise et il recalcule
         le brut du cachet » (l'éditeur, 22/09/2026).

         Les voir, d'abord : un brut tout seul ne dit ni ce que le salarié
         touche, ni ce que ça coûte — et ce sont les deux nombres dont on a
         besoin pour décider. Les trois sont donc là, côte à côte, sous les
         lignes de paie.

         Y écrire, ensuite, et c'est la paie prise à l'envers : on connaît
         l'enveloppe (une subvention, une cession, ce qui reste en caisse) et
         on cherche le taux qui la remplit. Taper 500 dans « Coût entreprise »
         d'un engagement à deux cachets de 130 €, c'est demander le taux du
         cachet qui fait que l'employeur sort 500 € — 158,66 €, pas 250.

         CE N'EST PAS UNE DIVISION, et la page ne la fait pas : plafond de la
         Sécurité sociale, tranches de retraite, abattement, réduction
         générale — le taux effectif dépend du montant lui-même. C'est le
         moteur qui cherche, en rejouant la paie entière (route « ajuster »,
         moteur/src/routes/mois.rs), et la note dit ce qui est ATTEINT,
         jamais ce qui a été demandé : deux cachets ne tombent pas toujours
         sur 500,00 € pile, et à égalité c'est le taux qui ne dépasse PAS
         l'enveloppe qui gagne. -->
    <!-- LA SAISIE, PENSÉE DANS L'ORDRE OÙ L'ON REMPLIT (23/09/2026). Qui et
         quel spectacle en tête ; les lignes de paie, jours d'abord ; puis le
         RÉSULTAT en une ligne — le net, ce qu'on vire. Viser un montant est
         un outil, pas une lecture : il se replie. Le métier, la case cadre,
         les bornes et le détail du contrat servent rarement : « Plus
         d'options », ouvert d'office quand l'un d'eux est déjà rempli. -->
    <div class="e-pied">
      <span class="e-net"></span>
      <span class="e-suite"></span>
      <button type="button" class="lien e-viser-btn">Viser un montant</button>
      <button type="button" class="lien e-options-btn">Plus d'options</button>
      <!-- LA PORTE DES IMPRÉVUS (15/10/2026) : un arrêt de travail, une
           absence, une rupture avant terme, pour le salarié de cette carte.
           Discrète : les imprévus sont rares. js/imprevus.js. -->
      <button type="button" class="lien e-imprevu-btn" hidden>Un imprévu…</button>
    </div>
    <div class="e-chiffres" hidden>
      <label title="ce que le salarié touche sur son compte">Net salarié
        <input type="number" step="0.01" min="0" class="e-vise" data-cible="net"></label>
      <label title="le brut du bulletin">Brut
        <input type="number" step="0.01" min="0" class="e-vise" data-cible="brut"></label>
      <label title="brut + cotisations patronales : ce que l'employeur sort">Coût entreprise
        <input type="number" step="0.01" min="0" class="e-vise" data-cible="cout"></label>
      <span class="e-chiffres-note discret">Écrire dans une case : les taux se
        recalculent pour y tomber.</span>
    </div>
    <div class="e-options" hidden>
      <div class="e-options-ligne">
      <!-- LE MÉTIER DE CE CONTRAT. Vide : celui de la fiche (le cas général).
           Choisi : le moteur pose d'un coup code emploi, PCS, libellé, statut
           et classement DSN (emplois.rs). La case « cadre » suit le
           défaut du catalogue et se décoche. -->
      <label class="e-metier-bloc" title="le métier de ce contrat, s'il diffère de la fiche">Métier
        <select class="e-metier"></select></label>
      <label class="coche e-cadre-bloc" title="cadre pour la retraite complémentaire (DSN 40.003 = 01)">
        <input type="checkbox" class="e-cadre"> cadre</label>
      </div>
      <div class="e-options-ligne e-rares" hidden></div>
    <!-- LE CONTRAT, replié : « du 12/10/2026 au 20/12/2026 », préréglé sur
         le premier et le dernier jour joué. On l'ouvre pour une seule chose :
         allonger la date de fin quand le contrat se poursuit le mois suivant
         (contrats sur plusieurs mois, 15/09/2026). Le numéro y apparaît, en
         lecture seule, une fois le mois produit. -->
    <details class="e-contrat">
      <summary></summary>
      <div class="e-contrat-champs">
        <label class="e-borne">Début du contrat <input type="date" class="e-debut"></label>
        <label class="e-borne">Fin du contrat <input type="date" class="e-fin"></label>
        <span class="e-numero discret"></span>
      </div>
      <!-- LE MOTIF DE RECOURS (S21.G00.40.021). Seul un CDD ordinaire en a
           un, et il est obligatoire : c'est lui qui rend le contrat licite
           au regard de L1242-2, et la plate-forme le contrôle (SIG-11). Un
           CDD d'usage n'apparaît pas dans cette liste et c'est délibéré — il
           se saisit comme un CDD d'usage, avec tout ce qui va avec. -->
      <label class="e-recours-bloc" hidden title="pourquoi ce CDD est conclu : la DSN le déclare (motif de recours)">Motif du CDD
        <select class="e-recours"></select></label>
      <!-- CE QUE LE CONTRAT ÉCRIT DE CET ENGAGEMENT DIT EN PLUS (le modèle
           le remplit avec le reste). Le moteur dit quelles cases cet
           engagement appelle (\`contrat.ecrits\` de l'aperçu,
           routes/mois.rs) : la période d'essai et les clauses de tout
           contrat écrit, la personne remplacée d'un remplacement, la
           répartition des horaires d'un temps partiel. -->
      <div class="e-ecrit">
        <label data-ecrit="periode_essai_jours" hidden title="la durée de la période d'essai que le contrat prévoit ; vide : aucune">Période d'essai (jours)
          <input type="number" min="0" step="1" class="e-ecrit-champ" data-cle="periode_essai_jours" data-type="number"></label>
        <label data-ecrit="personne_remplacee" hidden title="le contrat d'un remplacement nomme la personne remplacée et sa qualification (L. 1242-12, 1°)">Personne remplacée
          <input type="text" class="e-ecrit-champ" data-cle="personne_remplacee" placeholder="nom, qualification"></label>
        <label data-ecrit="repartition_horaire" hidden title="à temps partiel, le contrat dit la répartition des heures entre les jours de la semaine ou les semaines du mois (L. 3123-6)">Répartition des horaires
          <input type="text" class="e-ecrit-champ" data-cle="repartition_horaire" placeholder="ex. lundi, mardi et jeudi, de 9 h à 17 h"></label>
        <label data-ecrit="clauses_particulieres" hidden title="à la place des clauses particulières de la compagnie (Configuration › Modèles de contrat)">Clauses particulières de ce contrat
          <textarea rows="2" class="e-ecrit-champ" data-cle="clauses_particulieres" placeholder="vide : celles de la compagnie"></textarea></label>
      </div>
      <!-- LA FIN D'UN CONTRAT QUI SE SIGNALE (19/09/2026). Trois faits que
           le moteur ne devine pas, et il a raison de ne pas les deviner :
           une date de notification ne se déduit pas d'un salaire, et le
           motif de rupture change les droits du salarié à l'assurance
           chômage. Le bloc S21.G00.63 est OBLIGATOIRE dans un signalement
           (cardinalité 1,*) : même sans préavis, il se déclare — c'est le
           type « 90 - pas de clause de préavis applicable », et ses deux
           dates deviennent alors interdites.

           Un CDDU du spectacle n'en montre que le motif : arrivé à son terme
           (031), sa fin part dans la DSN mensuelle par le circuit
           dérogatoire ; rompu avant (036, 037…), elle se signale, et le
           reste du bloc paraît (fiche 2062 ; le moteur le dit motif par
           motif, signalement_par_motif). -->
      <div class="e-rupture" hidden>
        <div class="e-fait-tete">La fin de ce contrat
          <span class="discret e-rupture-dit">— ce que le signalement (nature 07) déclarera</span></div>
        <label class="e-motif-l">Motif de la rupture
          <select class="e-motif-rupture"></select></label>
        <p class="discret e-motif-dit" hidden></p>
        <!-- Les dates et la transaction selon le motif (30/09/2026,
             permanents.faits_de_rupture) : notifiée pour une rupture que
             l'un décide, engagée pour un licenciement, signée pour une
             rupture conventionnelle, la transaction après un litige. -->
        <label data-fait="engagement_licenciement" hidden
               title="la convocation à l'entretien préalable (L1232-2 ; L1332-2 pour une faute)">Procédure engagée le
          <input type="date" class="e-engagement-licenciement"></label>
        <label data-fait="notification_rupture" hidden>Notifiée le
          <input type="date" class="e-notification"></label>
        <label data-fait="signature_convention" hidden
               title="la date de signature de la convention de rupture (L1237-13)">Convention signée le
          <input type="date" class="e-signature-convention"></label>
        <label>Préavis
          <select class="e-preavis-type"></select></label>
        <label class="e-preavis-dates">du
          <input type="date" class="e-preavis-debut"></label>
        <label class="e-preavis-dates">au
          <input type="date" class="e-preavis-fin"></label>
        <label data-fait="transaction" hidden>Transaction
          <select class="e-transaction"></select></label>
        <label title="le dernier jour payé au salaire habituel — la fin du contrat, sauf préavis non effectué">Dernier jour payé
          <input type="date" class="e-dernier-paye" placeholder="la fin du contrat"></label>
        <!-- LES FAITS DU MOTIF (30/09/2026) : le moteur dit, motif par motif,
             ce qu'il doit savoir (permanents.faits_de_rupture) ; la page ne
             montre que ceux-là. Sans eux, une rupture conventionnelle était
             refusée sans qu'on puisse répondre d'ici. -->
        <label data-fait="en_droit_a_la_retraite" hidden
               title="CGI 80 duodecies 6° : l'indemnité n'est imposable que si le salarié est en droit de bénéficier d'une pension de retraite d'un régime obligatoire">En droit à une pension de retraite
          <select class="e-en-droit"><option value="">— à dire —</option>
            <option value="non">non</option><option value="oui">oui</option></select></label>
        <label data-fait="preavis_non_effectue" hidden class="case"
               title="L1234-5 : le préavis dont l'employeur dispense le salarié est payé en indemnité compensatrice">
          <input type="checkbox" class="e-preavis-non-effectue"> préavis non effectué, payé en indemnité</label>
        <label data-fait="preavis_non_effectue" hidden class="e-preavis-mois-l">durée du préavis, en mois
          <input type="number" class="e-preavis-mois" min="0" step="0.5" placeholder="celle de la loi et de la convention"></label>
        <label data-fait="precarite_due" hidden class="case"
               title="L1243-10 : deux cas ne se lisent pas dans le contrat — l'étudiant pendant ses vacances, le salarié qui refuse un CDI">
          <input type="checkbox" class="e-precarite-ecartee"> pas d'indemnité de fin de contrat</label>
        <label data-fait="precarite_due" hidden class="e-precarite-pourquoi-l">pourquoi
          <input class="e-precarite-pourquoi" placeholder="refus d'un CDI, emploi de vacances…"></label>
      </div>
      <!-- LES BORNES NE SE SAISISSENT PLUS POUR UN CONTRAT D'USAGE
           (22/09/2026). L'éditeur : « il y a marqué date de début et fin de
           contrat, mais en fait c'est plus nécessaire puisqu'on clique dans
           le calendrier les dates pour lesquelles on fait les heures ou les
           cachets » — et, sur le contrat qui déborde du mois : « on pourrait
           cliquer sur des jours du mois d'après ».

           Il a raison, et le moteur le fait déjà : Engagement.fin est le
           dernier jour coché, PLANNING COMPRIS (engagement.rs : la fin vaut le
           max du dernier jour joué et du dernier jour de planning). Un CDDU
           d'octobre à décembre se dit donc entièrement au calendrier. Deux
           cases qui ne servent à rien sont deux cases à relire, et une à se
           tromper.

           Elles restent pour les natures payées AU MOIS — un CDI, un CDD
           ordinaire n'ont aucun jour coché, et la date de fin d'un CDD est
           ce qui déclenche le signalement de rupture — et pour un engagement
           qui en porte DÉJÀ : ce qui a été saisi ne devient jamais invisible
           et non modifiable. -->
      <p class="note e-contrat-note"></p>
    </details>
    </div>`);
  // (Sous le formulaire, la coque : `.resolu`, `.e-resume`, `.e-imprevus`,
  // `.avis`, `.etats` — voir ligneEngagement.)
  poserQui(div.querySelector(".e-qui-liste"), voulus);

  // --- la nature du contrat, et la forme du bloc qu'elle décide
  const selNat = div.querySelector(".e-nature");
  poserListe(selNat, "natures", [ETAT.natures], () => (ETAT.natures || []).map(
    (n) => `<option value="${echapper(n.cle)}" title="${echapper(n.aide || "")}">${echapper(n.libelle)}</option>`).join(""));
  // Le nom court se lit ; ce qu'il veut dire est dans l'aide du champ, nature par nature.
  div.querySelector(".e-nature-bloc").title = (ETAT.natures || []).filter((n) => n.aide).map((n) => `${n.libelle} : ${n.aide}`).join("\n");
  selNat.value = e.nature || ETAT.nature_defaut || "";
  selNat.onchange = () => { viderBornes(div); majNature(div); rafraichirApercu(); };
  const selSpec = div.querySelector(".e-spectacle");
  poserSpectacles(selSpec);
  selSpec.value = e.spectacle || "";
  selSpec.onchange = rafraichirApercu;
  // --- le métier
  const selMet = div.querySelector(".e-metier");
  poserMetiers(selMet, e.metier);
  const cadre = div.querySelector(".e-cadre");
  const majCadre = (depuisCatalogue) => {
    const m = metierDe(selMet.value);
    div.querySelector(".e-cadre-bloc").hidden = !m;
    if (m && depuisCatalogue) cadre.checked = !!m.cadre_par_defaut;
  };
  cadre.checked = e.cadre === true || (e.cadre == null && !!(metierDe(e.metier) || {}).cadre_par_defaut);
  majCadre(false);
  selMet.onchange = () => { majCadre(true); rafraichirApercu(); };
  cadre.onchange = rafraichirApercu;
  // --- les lignes de paie
  const cible = div.querySelector(".rubriques");
  lignesDe(e).forEach((l) => cible.appendChild(ligneRubrique(l, natureDe(selNat.value))));
  div.querySelector(".ajouter-rubrique").onclick = (ev) => {
    ev.preventDefault();
    cible.appendChild(ligneRubrique({}, natureDe(selNat.value)));
    rafraichirApercu();
  };
  div.querySelectorAll(".ligne1 input").forEach((c) => {
    c.oninput = rafraichirApercu;
  });
  div.querySelectorAll(".e-qui-case").forEach((c) => {
    c.onchange = () => { majQui(div); rafraichirApercu(); };
  });
  majQui(div);
  // --- les trois nombres, et le total qu'on y vise. `onchange` et non
  // `oninput` : on ne relance pas un calcul de paie à chaque touche d'un
  // montant qu'on est en train de taper.
  div.querySelectorAll(".e-vise").forEach((c) => {
    c.onchange = () => ajusterBloc(div, c);
  });
  // --- le contrat
  div.querySelector(".e-debut").value = e.debut || "";
  div.querySelector(".e-fin").value = e.fin || "";
  div.dataset.forme = formeDePaie(div);
  div.querySelectorAll(".e-debut, .e-fin").forEach((c) => {
    c.onchange = () => { majContrat(div); majRupture(div); rafraichirApercu(); };
  });
  // --- le mois payé d'un permanent, et ses faits
  div.querySelector(".e-salaire").value = e.salaire_mensuel ?? "";
  div.querySelector(".e-quotite").value = e.quotite_contrat ?? "";
  // Le salaire et la quotité tapés recalculent et s'enregistrent (un
  // permanent repris d'un autre logiciel n'a pas d'autre case qui fasse sa
  // paie). `onchange` : pas un calcul par touche d'un montant.
  div.querySelectorAll(".e-salaire, .e-quotite").forEach((c) => { c.onchange = rafraichirApercu; });
  div.querySelector(".e-date-modification").value = e.date_modification || "";
  poserAvant(div, e);
  poserRares(div, e);
  poserEcrit(div, e);
  remplirFaits(div, e);
  div.querySelectorAll(".ajouter-fait").forEach((b) => {
    b.onclick = (ev) => {
      ev.preventDefault();
      ajouterFait(div, b.dataset.quoi);
      rafraichirApercu();
    };
  });
  // --- la fin du contrat, quand elle se signale
  // Un déroulant sans valeur choisie montre sa PREMIÈRE option, et la page
  // l'écrirait comme si on l'avait choisie. Le préavis s'ouvre donc sur un
  // « à choisir » vide — le moteur dira qu'il manque —, et la transaction
  // sur le défaut de la norme (02, « non »), jamais sur la première venue.
  poserListe(div.querySelector(".e-preavis-type"), "preavis", [ETAT.types_preavis], () =>
    `<option value="">— à choisir —</option>`
    + (ETAT.types_preavis || []).map(
      (t) => `<option value="${echapper(t.code)}" data-dates="${t.dates ? 1 : 0}">${
        echapper(t.code)} — ${echapper(t.libelle)}</option>`).join(""));
  poserListe(div.querySelector(".e-transaction"), "transaction", [ETAT.transaction], () =>
    (ETAT.transaction || []).map(
      (t) => `<option value="${echapper(t.code)}"${t.defaut ? " selected" : ""}>${
        echapper(t.libelle)}</option>`).join(""));
  const preavis = e.preavis || {};
  div.dataset.motifRupture = e.motif_rupture || "";
  div.dataset.motifRecours = e.motif_recours || "";
  div.querySelector(".e-preavis-type").value = preavis.type || "";
  div.querySelector(".e-preavis-debut").value = preavis.debut || "";
  div.querySelector(".e-preavis-fin").value = preavis.fin || "";
  div.querySelector(".e-notification").value = e.notification_rupture || "";
  div.querySelector(".e-engagement-licenciement").value = e.engagement_licenciement || "";
  div.querySelector(".e-signature-convention").value = e.signature_convention || "";
  div.querySelector(".e-dernier-paye").value = e.dernier_jour_paye || "";
  if (e.transaction) div.querySelector(".e-transaction").value = e.transaction;
  div.querySelector(".e-en-droit").value = e.en_droit_a_la_retraite === true ? "oui"
    : e.en_droit_a_la_retraite === false ? "non" : "";
  div.querySelector(".e-preavis-non-effectue").checked = !!e.preavis_non_effectue;
  div.querySelector(".e-preavis-mois").value = e.preavis_mois ?? "";
  div.querySelector(".e-precarite-ecartee").checked = e.precarite_due === false;
  div.querySelector(".e-precarite-pourquoi").value = e.precarite_pourquoi || "";
  div.querySelector(".e-preavis-type").onchange = () => { majRupture(div); rafraichirApercu(); };
  div.querySelectorAll(".e-rupture input, .e-motif-rupture, .e-transaction, .e-recours, .e-en-droit")
    .forEach((c) => { c.onchange = () => { majRupture(div); rafraichirApercu(); }; });
  // UN CONTRAT EN COURS DEPUIS UN MOIS PRÉCÉDENT ne se modifie pas d'ici :
  // il a été signé et déclaré avec son premier mois. Salarié, spectacle,
  // métier et bornes sont verrouillés ; seules les lignes du mois se
  // saisissent. `verrou` : fermerEdition ne les rouvre pas. (La classe
  // `en-cours` est posée dès la coque, ligneEngagement.)
  if (div.classList.contains("en-cours")) {
    div.querySelectorAll(".e-debut, .e-fin, .e-spectacle, .e-metier, .e-cadre, .e-qui-case, .e-nature")
      .forEach((c) => { c.disabled = true; c.classList.add("verrou"); });
  }
  majContrat(div);
  majNature(div);
  // La Nature ne se montre que s'il y a un choix : sans permanents, la
  // compagnie n'a que le contrat d'usage, et une case à un seul choix n'en
  // est pas une.
  div.querySelector(".e-nature-bloc").hidden = (ETAT.natures || []).length <= 1;
  div.querySelector(".e-viser-btn").onclick = (ev) => {
    ev.preventDefault();
    const c = div.querySelector(".e-chiffres");
    c.hidden = !c.hidden;
  };
  div.querySelector(".e-imprevu-btn").onclick = (ev) => {
    ev.preventDefault();
    ev.stopPropagation();
    menuImprevu(ev.currentTarget, { div });
  };
  div.querySelector(".e-options-btn").onclick = (ev) => {
    ev.preventDefault();
    const o = div.querySelector(".e-options");
    o.hidden = !o.hidden;
  };
  // Ouvert d'office quand ce qu'il cache est déjà rempli : rien de saisi ne
  // devient invisible.
  const nat = natureDe(selNat.value);
  div.querySelector(".e-options").hidden = !(e.metier || e.cadre != null
    || CHAMPS_RARES.some((c) => aUneValeur(e[c.cle]))
    || ["periode_essai_jours", "personne_remplacee", "repartition_horaire", "clauses_particulieres"].some((c) => aUneValeur(e[c]))
    || nat.mensuelle || e.debut && e.debut.slice(0, 7) < (document.getElementById("mois").value || "")
    || e.motif_recours || e.motif_rupture);
  return div;
}

/** La nature d'un engagement, telle que `contrats-base.json` la décrit.
 *  Un engagement muet est un CDDU du spectacle : c'est le défaut du moteur,
 *  et la page prend le même — elle ne le redéfinit pas. */
function natureDe(cle) {
  const natures = ETAT.natures || [];
  return natures.find((n) => n.cle === (cle || ETAT.nature_defaut))
      || natures.find((n) => n.cle === ETAT.nature_defaut)
      || { cle: "", libelle: "", mensuelle: false, spectacle: true,
           signalement_fctu: false, signalement_par_motif: {}, motifs_rupture: [] };
}

/** La forme de paie de la nature choisie : au mois, ou à l'usage (cachets,
 *  heures, jours joués). */
function formeDePaie(div) {
  return natureDe(div.querySelector(".e-nature").value).mensuelle ? "mois" : "usage";
}

/** UN CONTRAT AU MOIS ET UN CONTRAT D'USAGE SONT DEUX CONTRATS. Les cases
 *  « Début » et « Fin » servent aux deux : quand la nature passe d'une forme
 *  de paie à l'autre, les bornes de la forme quittée ne sont pas celles du
 *  nouveau contrat, elles se vident. */
function viderBornes(div) {
  const forme = formeDePaie(div);
  const quittee = div.dataset.forme;
  if (quittee && quittee !== forme) {
    div.querySelector(".e-debut").value = "";
    div.querySelector(".e-fin").value = "";
    majContrat(div);
    majRupture(div);
  }
  div.dataset.forme = forme;
}

/** Ce que la nature change à l'écran. Un permanent n'a ni spectacle (le
 *  numéro d'objet lui est INTERDIT, CCH-11) ni cachets : il a un salaire au
 *  mois et des faits. Cacher plutôt que griser, parce qu'une case qui ne
 *  peut pas servir est une case qu'on essaie quand même de remplir. */
function majNature(div) {
  const nat = natureDe(div.querySelector(".e-nature").value);
  div.classList.toggle("bloc-permanent", !!nat.mensuelle);
  div.querySelector(".e-spectacle-bloc").hidden = !nat.spectacle;
  div.querySelector(".e-permanent").hidden = !nat.mensuelle;
  div.querySelector(".rubriques").hidden = !!nat.mensuelle;
  div.querySelector(".ajouter-rubrique").hidden = !!nat.mensuelle;
  // Le déroulant des rubriques suit la nature : ce qui ne s'applique pas à
  // ce contrat ne s'y propose plus (la rubrique choisie reste).
  // (Rien à refaire quand la forme de paie est celle de la liste posée.)
  const forme = nat.mensuelle ? "mensuelle" : "activite";
  div.querySelectorAll(".r-code").forEach((s) => {
    if (s.dataset.forme !== forme) poserRubriques(s, s.value, nat);
  });
  // Les bornes : saisies pour un contrat payé au mois, déduites du
  // calendrier pour un contrat d'usage — sauf si elles sont déjà écrites.
  const ecrites = !!(div.querySelector(".e-debut").value
                     || div.querySelector(".e-fin").value);
  const bornes = !!nat.mensuelle || ecrites;
  div.querySelectorAll(".e-borne").forEach((l) => { l.hidden = !bornes; });
  // UN CONTRAT AU MOIS DIT SES BORNES : son début (et la fin
  // d'un CDD) ne se déduit d'aucun jour coché, et le moteur refuse sans lui.
  // Tant qu'il manque, le contrat s'ouvre de lui-même — un testeur ne va pas
  // chercher sous « Plus d'options » ce qu'on exige de lui.
  const sansBornes = nat.mensuelle && (!div.querySelector(".e-debut").value
    || (!nat.duree_indeterminee && !div.querySelector(".e-fin").value));
  if (sansBornes) {
    div.querySelector(".e-options").hidden = false;
    div.querySelector(".e-contrat").open = true;
  }
  if (nat.mensuelle) {
    majContrat(div);
    majAvenant(div);
  }
  div.querySelector(".e-contrat-note").textContent = bornes
    ? "Un contrat payé au mois n'a pas de jour coché : ses bornes se disent ici."
      + " La date de fin d'un CDD est celle qui déclenche le signalement de"
      + " fin de contrat (nature 07) : son délai et sa date butoir se lisent sous « Fin de contrat »."
    : "Le contrat va du premier au dernier jour coché — planning compris,"
      + " quand on a coché des jours dans les mois suivants depuis le"
      + " calendrier d'une ligne. Ses cachets suivants se paient dans le mois"
      + " suivant (le mois venu, le contrat y est repris, ses jours cochés) ;"
      + " la DPAE et le contrat imprimé ne partent qu'avec ce mois-ci, le"
      + " certificat Congés Spectacles à la fin du contrat.";
  // LA DURÉE DU CONTRAT se tape, elle ne se devine pas : la durée de
  // référence d'un temps plein que le moteur donne (`quotite_defaut`) se
  // propose d'un clic, jamais en grisé dans la case, où elle passerait pour
  // une valeur saisie.
  const quotite = div.querySelector(".e-quotite");
  const pleine = div.querySelector(".e-temps-plein");
  pleine.hidden = nat.quotite_defaut == null;
  if (nat.quotite_defaut != null) {
    pleine.textContent = `temps plein : ${String(nat.quotite_defaut).replace(".", ",")} h`;
    pleine.onclick = (ev) => {
      ev.preventDefault();
      quotite.value = nat.quotite_defaut;
      quotite.dispatchEvent(new Event("change"));
      quotite.dispatchEvent(new Event("input", { bubbles: true }));
    };
  }
  // Le motif de recours : proposé seulement par les natures qui en ont un.
  const recours = nat.motifs_recours || [];
  div.querySelector(".e-recours-bloc").hidden = !recours.length;
  const sel = div.querySelector(".e-recours");
  const voulu = sel.value || div.dataset.motifRecours || "";
  poserListe(sel, "recours:" + nat.cle, [recours], () => `<option value="">— à choisir —</option>`
    + recours.map((m) => `<option value="${echapper(m.code)}">${echapper(m.code)} — ${
      echapper(m.libelle)}</option>`).join(""));
  sel.value = recours.some((m) => m.code === voulu) ? voulu : "";
  // UN CDD DIT SON MOTIF, et le moteur refuse sans lui : tant qu'il manque,
  // le contrat s'ouvre de lui-même, comme pour ses bornes.
  if (recours.length && !sel.value) {
    div.querySelector(".e-options").hidden = false;
    div.querySelector(".e-contrat").open = true;
  }
  nommerLesCases(div);
  majRupture(div);
}

/** La fin d'un contrat, et ce que son signalement déclarera. Un CDDU du
 *  spectacle n'en montre que le motif : à son terme (031, le défaut), sa fin
 *  part dans la DSN mensuelle par le circuit dérogatoire et rien d'autre ne
 *  se demande ; rompu avant (036, 037, 084…), elle se signale, et le reste du
 *  bloc paraît. C'est le moteur qui le dit, motif par motif
 *  (`finSeSignale`) : la page ne fait que le montrer.
 *
 *  Les dates de préavis suivent le type choisi : le « 90 - pas de clause de
 *  préavis applicable » les INTERDIT (CCH-11 de S21.G00.63.002), et le
 *  moteur refuserait le fichier. Autant ne pas les proposer. */
function majRupture(div) {
  const nat = natureDe(div.querySelector(".e-nature").value);
  const bloc = div.querySelector(".e-rupture");
  // Un contrat aux cachets n'écrit pas sa fin : le moteur la donne (`contrat.rupture`).
  const fin = div.querySelector(".e-fin").value || (div._rupture && div._rupture.fin) || "";
  const motifs = nat.motifs_rupture || [];
  // LA FIN NE SE DEMANDE QUE LE MOIS OÙ ELLE A LIEU : un CDD de janvier à
  // juin n'a rien à dire de sa fin en janvier. Elle paraît quand la fin du
  // contrat tombe dans le mois affiché (ou avant : une fin avancée), ou
  // quand un motif est déjà dit.
  const moisAffiche = (document.getElementById("mois") || {}).value || "";
  const finCeMois = fin && (!moisAffiche || fin.slice(0, 7) <= moisAffiche);
  const dejaDit = !!(div.querySelector(".e-motif-rupture").value || div.dataset.motifRupture);
  bloc.hidden = !(fin && (finCeMois || dejaDit) && (nat.signalement_fctu || motifs.length));
  // Les motifs sont ceux de CETTE nature : un CDD peut finir à son terme
  // (031), un CDI ne le peut pas — il ne finit jamais tout seul. Le choix
  // vide d'une nature dont la fin par défaut ne se signale pas (le CDDU)
  // est ce défaut : la fin à son terme.
  const sel = div.querySelector(".e-motif-rupture");
  const voulu = sel.value || div.dataset.motifRupture || "";
  const vide = !nat.signalement_fctu && nat.motif_rupture_defaut
    ? `— ${echapper(nat.motif_rupture_defaut)} : à son terme —` : "— à choisir —";
  poserListe(sel, "rupture:" + nat.cle, [nat.motifs_rupture, vide], () => `<option value="">${vide}</option>`
    + (nat.motifs_rupture || []).map(
      (m) => `<option value="${echapper(m.code)}">${echapper(m.code)} — ${echapper(m.libelle)}</option>`).join(""));
  sel.value = (nat.motifs_rupture || []).some((m) => m.code === voulu) ? voulu : "";
  // Ce que le moteur dit du motif choisi (ce qu'il ouvre pour le salarié), sous la liste.
  const motifDit = div.querySelector(".e-motif-dit");
  const dit = ((nat.motifs_rupture || []).find((m) => m.code === sel.value) || {}).dit || "";
  motifDit.textContent = dit;
  motifDit.hidden = !dit;
  const faits = (nat.faits_de_rupture || {})[sel.value] || [];
  div.querySelectorAll(".e-rupture [data-fait]").forEach((l) => {
    l.hidden = !faits.includes(l.dataset.fait);
  });
  // La durée et le pourquoi ne se montrent qu'avec leur case.
  div.querySelector(".e-preavis-mois-l").hidden ||=
    !div.querySelector(".e-preavis-non-effectue").checked;
  div.querySelector(".e-precarite-pourquoi-l").hidden ||=
    !div.querySelector(".e-precarite-ecartee").checked;
  const type = div.querySelector(".e-preavis-type");
  const choisi = type.selectedOptions[0];
  const avecDates = !!(choisi && choisi.dataset.dates === "1");
  div.querySelectorAll(".e-preavis-dates").forEach((l) => { l.hidden = !avecDates; });
  if (!avecDates) {
    div.querySelector(".e-preavis-debut").value = "";
    div.querySelector(".e-preavis-fin").value = "";
  }
  // Une fin qui ne se signale pas (le CDDU à son terme) ne montre que son
  // motif : le reste ne partirait nulle part.
  const signale = finSeSignale(nat, sel.value);
  div.querySelector(".e-rupture-dit").textContent = signale
    ? "— ce que le signalement (nature 07) déclarera"
    : "— elle part dans la DSN du mois, par le circuit dérogatoire : rien à signaler";
  if (!signale) {
    bloc.querySelectorAll(":scope > label:not(.e-motif-l)").forEach((l) => { l.hidden = true; });
  } else {
    bloc.querySelectorAll(":scope > label:not([data-fait]):not(.e-preavis-dates):not(.e-preavis-mois-l):not(.e-precarite-pourquoi-l)")
      .forEach((l) => { l.hidden = false; });
  }
}

/* LES FAITS D'UN MOIS DE PERMANENT — congés, primes. Deux listes
   de la même famille : une ligne se remplit, un « + » en ajoute une, une
   croix la retire. Ce qu'elles écrivent, ce sont les faits du mois, jamais
   des lignes de paie : le calcul appartient au moteur. */
function ligneFait(quoi, v = {}) {
  const div = document.createElement("div");
  div.className = "fait fait-" + quoi;
  if (quoi === "conges") {
    div.innerHTML = `
      <label>du <input type="date" class="f-debut" value="${echapper(v.debut || "")}"></label>
      <label>au <input type="date" class="f-fin" value="${echapper(v.fin || "")}"></label>
      <button class="retirer-fait lien" title="retirer">&#x2715;</button>`;
  } else {
    div.innerHTML = `
      <label>rubrique <input type="text" class="f-rubrique"
        value="${echapper(v.rubrique || "")}" placeholder="prime"></label>
      <label>montant <input type="number" step="0.01" class="f-montant"
        value="${echapper(v.montant ?? "")}"></label>
      ${v.libelle ? `<label>libellé <input class="f-libelle" value="${echapper(v.libelle)}"></label>` : ""}
      <button class="retirer-fait lien" title="retirer">&#x2715;</button>`;
  }
  div.querySelector(".retirer-fait").onclick = (ev) => {
    ev.preventDefault();
    div.remove();
    rafraichirApercu();
  };
  div.querySelectorAll("input").forEach((c) => { c.onchange = rafraichirApercu; });
  return div;
}

const FAITS = { conges: ".e-conges", primes: ".e-primes" };

function remplirFaits(div, e) {
  (e.conges_pris || []).forEach((c) => div.querySelector(FAITS.conges).appendChild(ligneFait("conges", c)));
  (e.primes || []).forEach((p) => div.querySelector(FAITS.primes).appendChild(ligneFait("primes", p)));
}

function ajouterFait(div, quoi) {
  div.querySelector(FAITS[quoi]).appendChild(ligneFait(quoi));
}

/** Les faits tapés dans un bloc, tels qu'ils s'écrivent dans la saisie. Une
 *  ligne dont les deux dates manquent n'est pas un fait : on ne l'écrit pas.
 *  Ce qui est incomplet, en revanche, part tel quel — c'est le moteur qui
 *  dit ce qui manque, pas la page. */
function faitsDuBloc(div) {
  const val = (n, s) => (n.querySelector(s) || {}).value || "";
  const conges = [...div.querySelectorAll(".fait-conges")]
    .map((f) => ({ debut: val(f, ".f-debut"), fin: val(f, ".f-fin") }))
    .filter((c) => c.debut || c.fin);
  const primes = [...div.querySelectorAll(".fait-primes")].map((f) => {
    const p = { montant: +val(f, ".f-montant") };
    if (val(f, ".f-rubrique")) p.rubrique = val(f, ".f-rubrique");
    if (val(f, ".f-libelle")) p.libelle = val(f, ".f-libelle");
    return p;
  }).filter((p) => p.montant);
  return { conges, primes };
}

/** La ligne repliée du contrat : ses bornes, son numéro, et ce qu'il fait
 *  par rapport au mois — « se poursuit en novembre », « en cours depuis
 *  octobre ». Les bornes vides se lisent sur les jours joués, comme le
 *  moteur le fait (`Engagement.debut`/`fin`). */
function majContrat(div) {
  const d = div.querySelector(".e-contrat");
  if (!d) return;
  const mois = document.getElementById("mois").value;
  const jours = joursDuBloc(div, mois);
  const debut = div.querySelector(".e-debut").value || jours[0] || "";
  const fin = div.querySelector(".e-fin").value || jours[jours.length - 1] || "";
  const numero = div.dataset.numero || "";
  div.querySelector(".e-numero").textContent = numero
    ? "n° " + numero + " — attribué à la production, il ne change plus" : "";
  const s = d.querySelector("summary");
  const enCours = !!(debut && mois && debut.slice(0, 7) < mois);
  const sePoursuit = !!(fin && mois && fin.slice(0, 7) > mois);
  d.classList.toggle("a-cheval", enCours || sePoursuit);
  // UN CONTRAT SANS TERME (un CDI) n'a pas de dernier jour joué : il se
  // dit par son début (26/09/2026 — la phrase des cachets s'y affichait).
  if (debut && !fin && natureDe(div.querySelector(".e-nature")?.value).mensuelle) {
    s.textContent = `Contrat depuis le ${jjmmaaaa(debut)}, sans terme${
      numero ? ` · n° ${numero}` : ""}`;
    return;
  }
  if (!debut && natureDe(div.querySelector(".e-nature")?.value).mensuelle) {
    // Un contrat au mois n'a pas de jour joué d'où tirer son début.
    s.innerHTML = `Contrat <span class="discret">— sa date de début, celle du contrat signé</span>`;
    return;
  }
  if (!debut || !fin) {
    s.innerHTML = `Contrat <span class="discret">— du premier au dernier jour joué</span>`;
    return;
  }
  let texte = `Contrat du ${jjmmaaaa(debut)} au ${jjmmaaaa(fin)}`;
  if (numero) texte += ` · n° ${numero}`;
  if (enCours) texte += ` — en cours depuis ${moisEnClair(debut.slice(0, 7))}`;
  if (sePoursuit) texte += ` — se poursuit en ${moisEnClair(moisSuivant(mois))}`;
  s.textContent = texte;
}

/** Les listes de salariés et de spectacles de TOUS les blocs, refaites sur
 *  l'ETAT courant, sans perdre ce qui est coché ou choisi. Appelé après
 *  qu'une fiche a été créée : le bloc d'où l'on est parti doit proposer le
 *  nouveau venu sans qu'on recharge le mois — recharger relirait le disque
 *  par-dessus une frappe pas encore enregistrée. */
function rafraichirReferentiels() {
  document.querySelectorAll(".engagement").forEach((div) => {
    // Une carte fermée n'a pas de listes : elle les prendra de l'ETAT du
    // moment où elle s'ouvrira.
    if (carteFermee(div)) return;
    poserQui(div.querySelector(".e-qui-liste"), new Set(salariesDuBloc(div)));
    div.querySelectorAll(".e-qui-case").forEach((c) => {
      c.onchange = () => { majQui(div); rafraichirApercu(); };
    });
    majQui(div);
    const sel = div.querySelector(".e-spectacle");
    const v = sel.value;
    poserSpectacles(sel);
    sel.value = v;
  });
}

/** Une fiche neuve : le formulaire complet de Configuration, avec ses
 *  vérifications. Une seule façon de créer une fiche, et c'est là. */
function nouvelleFiche(nom) {
  // Les onglets redessinent APRES coup (leurs gestionnaires attendent
  // `quitterLaFiche`) : sans cette marque, `dessinerConfig` effacait la
  // fiche neuve qu'on venait d'ouvrir.
  const onglet = document.querySelector('.onglets button[data-vue="configuration"]');
  const sous = document.querySelector(`.sous-onglets button[data-fiche="${nom}"]`);
  if (!onglet.classList.contains("actif")) onglet.click();
  if (!sous.classList.contains("actif")) sous.click();
  CLE_ACTIVE = null;
  FICHE_NEUVE = true;
  dessinerConfig();
  // L'ancienneté d'une fiche neuve : celle de son embauche, que le moteur
  // pose à la validation de son premier engagement (à changer, dans Avancé).
  const donnees = { actif: true };
  dessinerFormulaire({ nom, cle: null, donnees, anomalies: [] });
  document.getElementById("config-formulaire").scrollIntoView({ behavior: "smooth" });
}

/** Les salariés cochés d'un bloc, dans l'ordre de la liste. */
function salariesDuBloc(div) {
  if (carteFermee(div)) return engagementsLus(div).map((e) => e.salarie);   // une carte fermée : les siens
  return [...div.querySelectorAll(".e-qui-case:checked")].map((c) => c.value);
}

/** Le résumé de la liste refermée : les noms cochés, ou l'invitation. */
function majQui(div) {
  const resume = div.querySelector(".e-qui summary");
  const noms = [...div.querySelectorAll(".e-qui-case:checked")]
    .map((c) => c.parentNode.textContent.replace(/\s+/g, " ").trim());
  resume.textContent = noms.length ? noms.join(" + ") : "— cocher —";
  resume.classList.toggle("vide", !noms.length);
  // Un imprévu se dit d'UN salarié : la porte ne s'ouvre que sur une carte qui n'en porte qu'un.
  majPorteImprevu(div);
}

/** Une liste ouverte se referme quand on clique ailleurs — un <details> ne
 *  le fait pas tout seul. */
document.addEventListener("click", (ev) => {
  document.querySelectorAll(".e-qui[open]").forEach((d) => {
    if (!d.contains(ev.target)) d.open = false;
  });
});

/** Plusieurs salariés cochés : le bloc devient un bloc par salarié.
 *
 *  Appelé APRÈS la validation (validerPlusieurs), pas avant : tant qu'on
 *  saisit, un seul bloc pour tous — c'est ça qui va vite. Le moteur, lui, ne
 *  connaît qu'un salarié par engagement, un contrat, un bulletin ; ici on ne
 *  fait que montrer à l'écran ce que le fichier contient déjà. Rien à faire
 *  pour zéro ou une case. */
function dedoubler(div) {
  const cles = salariesDuBloc(div);
  if (cles.length < 2) return;
  // Chaque bloc né ici part de ce que l'écran écrit pour SON salarié — les
  // lignes, lieux et jours compris (les salariés jouent les mêmes dates aux
  // mêmes endroits), la nature, le métier, tout ce que le formulaire dit.
  const engagements = engagementsDuBloc(div, document.getElementById("mois").value);
  const parent = div.parentNode;
  const suivant = div.nextSibling;
  div.remove();
  engagements.forEach((e) => parent.insertBefore(ligneEngagement(e), suivant));
}

/** Les lignes de paie d'un bloc, telles qu'elles sont saisies. */
/* CE QU'UNE LIGNE DIT PART DANS LA SAISIE, même incomplète : ses jours, sa
 * quantité, son taux ou ses heures. Le moteur l'enregistre en saisie et dit
 * ce qui lui manque. Pour « Viser un montant », seules partent les lignes
 * qui disent leur quantité : le taux est ce qu'on cherche, et le moteur le
 * trouve seul quand il n'y a qu'une ligne de salaire. */
let POUR_VISER = false;
const ligneEnvoyee = (r) => {
  const dit = (s) => r.querySelector(s).value.trim() !== "";
  if (POUR_VISER) return dit(".r-quantite");
  return dit(".r-quantite") || dit(".r-taux") || dit(".r-heures")
    || joursDe(r.querySelector(".r-jours")).length > 0;
};

function lignesDuBloc(div, mois) {
  mois = mois || document.getElementById("mois").value;
  return [...div.querySelectorAll(".rubrique")].map((r) => {
    const q = r.querySelector(".r-quantite").value;
    const t = r.querySelector(".r-taux").value;
    const h = r.querySelector(".r-heures").value;
    if (!ligneEnvoyee(r)) return null;
    const rr = rubriqueDe(r.querySelector(".r-code").value);
    // Une ligne en pourcentage s'écrit comme le moteur la lit : sa base et
    // son pourcentage (`calcul._ligne_en_pourcentage`).
    if (rr && rr.saisie.includes("taux_majoration")) {
      return { rubrique: rr.code, base: q === "" ? "" : +q, taux_majoration: t === "" ? "" : +t };
    }
    const l = { rubrique: r.querySelector(".r-code").value, quantite: q === "" ? "" : +q,
                taux: t === "" ? "" : +t };
    // Les heures s'écrivent dès qu'elles sont dites : leur champ se montre
    // alors, quelle que soit la rubrique (`majAffichage`).
    if (h !== "") l.heures = +h;
    // Le genre d'un avantage en nature, quand la rubrique en demande un et
    // qu'on en a choisi un : le moteur refuse un genre sur toute autre.
    const genre = r.querySelector(".r-genre").value;
    if (genre && rr && (rr.genres || []).length) l.genre = genre;
    // Le lieu et les jours de CETTE ligne. Absents du fichier quand ils sont
    // vides : une ligne qui ne dit rien de son lieu n'écrit pas `"lieu": ""`.
    const ou = r.querySelector(".r-lieu").value.trim();
    if (ou) l.lieu = ou;
    // Un jour qui dit son propre lieu s'écrit {"jour", "lieu"}, les autres nus.
    const propres = new Map([...r.querySelectorAll(".r-lieu-jour")]
      .filter((c) => c.value.trim()).map((c) => [c.dataset.iso, c.value.trim()]));
    const date = (iso) => (propres.has(iso) ? { jour: iso, lieu: propres.get(iso) } : iso);
    const jours = joursDe(r.querySelector(".r-jours"));
    if (jours.length) l.dates = jours.map((j) => date(mois + "-" + String(j).padStart(2, "0")));
    // Le planning de la ligne : les jours cochés dans les mois suivants.
    const plan = planningDe(r);
    if (plan.length) l.planning = plan.map(date);
    return l;
  }).filter(Boolean);
}

/** Les jours d'un engagement, toutes lignes confondues — c'est ce que le
 *  moteur appelle `dates_travail`, et il le recompose de la même façon
 *  (`operations.dates_engagement`). */
function joursDuBloc(div, mois) {
  const vus = new Set();
  // Une carte fermée : les jours des lignes de ses engagements (un bloc porte
  // les mêmes lignes pour tous ses salariés).
  const lignes = carteFermee(div) ? ((engagementsLus(div)[0] || {}).lignes || []) : lignesDuBloc(div, mois);
  lignes.forEach((l) => (l.dates || []).forEach((j) => vus.add(jourDe(j))));
  return [...vus].sort();
}

document.getElementById("ajouter-engagement").onclick = () => {
  document.getElementById("engagements").appendChild(ligneEngagement());
  rafraichirApercu();
};

/* LES CONTRATS EN COURS SE POSENT D'EUX-MÊMES. Un engagement commencé un
   mois précédent est déjà dans le mois (le moteur le montre, `deja_repris`) ;
   ce que le moteur rend en plus (`contrats-en-cours`), ce sont les contrats
   repris d'un autre logiciel qui courent encore. Le mois ouvert les montre
   tout de suite, préremplis et verrouillés ; il reste à poser les jours, ou
   rien si aucun cachet ce mois-ci (la DSN porte le contrat, sans bulletin).
   */
let CONTRATS_EN_COURS = [];

async function poserContratsEnCours(mois) {
  try {
    const r = await api("contrats-en-cours", { mois });
    CONTRATS_EN_COURS = r.contrats || [];
  } catch (e) { CONTRATS_EN_COURS = []; return; }
  if (MOIS_AFFICHE !== mois) return;
  const restants = CONTRATS_EN_COURS.filter((c) => !c.deja_repris);
  if (!restants.length) return;
  const cible = document.getElementById("engagements");
  restants.forEach((c) => {
    // L'engagement vient du moteur (`engagement_de_reprise`) : bornes,
    // numéro, métier, et les lignes déjà datées quand le premier mois a
    // coché son planning. Sinon une ligne de cachets, à dater.
    const e = { ...(c.engagement || {}), salarie: c.salarie, spectacle: c.spectacle,
                debut: c.debut, fin: c.fin, numero: c.numero };
    // UN CONTRAT PAYÉ AU MOIS N'A PAS DE CACHET : le moteur le
    // reprend avec sa nature et ce qui la définit, sans lignes — il compose
    // chaque mois les siennes (`conges::lignes_du_mois`), et une ligne écrite
    // remplacerait son salaire mensuel.
    if (!natureDe(e.nature).mensuelle) {
      // Le moteur rend toujours au moins une ligne de cachets, avec le taux
      // du contrat ou le dernier saisi (`engagement_de_reprise`).
      e.lignes = (e.lignes || []).map((l) => (l.quantite === 0 ? { ...l, quantite: "" } : l));
    }
    // Ce que l'engagement repris porte d'autre (la durée de référence,
    // l'emploi et les statuts déclarés…) se montre dans son formulaire
    // (`CHAMPS_RARES`), et s'écrit de là.
    cible.appendChild(ligneEngagement(e));
  });
  rafraichirApercu();
  // Un permanent n'a pas de jours à poser : il est payé au mois (son salaire
  // se dit dans son bloc).
  const auMois = restants.every((c) => natureDe((c.engagement || {}).nature).mensuelle);
  dire(restants.length === 1
    ? `Contrat en cours repris : ${restants[0].nom}${auMois ? "" : ` — poser ses jours de ${moisEnClair(mois)}`}.`
    : `${restants.length} contrats en cours repris (${restants.map((c) => c.nom).join(", ")})`
      + (auMois ? "." : ` — poser leurs jours de ${moisEnClair(mois)}.`));
}

/** Ce qu'un engagement dit de lui-même en une ligne : son spectacle, ou — quand
 *  il n'en a pas, parce que c'est un permanent — la nature de son contrat.
 *  « — » tout seul derrière un nom, c'est un écran qui a l'air cassé. */
function pourQuoi(ligne) {
  return ligne.spectacle || ligne.nature || "";
}

/* UN CONTRAT SUR PLUSIEURS MOIS SE VOIT : « c'est
 * un contrat sur plusieurs mois, on pourrait faire un signe de ça, par
 * exemple 3 petits points ou une flèche ». Une pastille : les mois du
 * contrat, et une flèche du côté où il déborde du mois affiché — à gauche
 * s'il a commencé avant, à droite s'il se poursuit après ; ses dates en
 * infobulle. Tout vient du moteur, `contrat` d'un engagement de l'aperçu ou d'une
 * ligne des états du mois (`premier_mois`, `dernier_mois`, `depuis`,
 * `jusqu_a`), avec le début et la fin du contrat : la page ne compare aucune
 * date. Un contrat sans terme (un CDI) n'en porte pas : il dure, il n'est
 * pas « sur plusieurs mois ». */
/* UN CONTRAT SUR PLUSIEURS MOIS : un chevron au bord de la case de l'engagement,
 * à gauche quand le contrat a commencé avant ce mois, à droite quand il se
 * poursuit après (un permanent en CDI aussi) ; cliquer va au mois voisin.
 * Tout vient du moteur (`contrat.premier_mois`, `dernier_mois`, `depuis`,
 * `jusqu_a`) : la page ne compare aucune date. */
function marquePlurimensuel(contrat, debut, fin) {
  if (!contrat || typeof contrat.premier_mois !== "boolean" || typeof contrat.dernier_mois !== "boolean") return "";
  const avant = !contrat.premier_mois, apres = !contrat.dernier_mois;
  if (!avant && !apres) return "";
  const x = moisLu(document.getElementById("mois").value);
  const voisin = (pas) => {
    if (!x) return "";
    const d = new Date(x[0], x[1] - 1 + pas, 1);
    return moisEnTexte(d.getFullYear(), d.getMonth() + 1);
  };
  const bornes = `Contrat du ${jjmmaaaa(debut)}${fin ? ` au ${jjmmaaaa(fin)}` : ", sans date de fin"}`;
  const un = (sens, texte, pas) => {
    const m = voisin(pas);
    const titre = `${bornes} — ${texte}${m ? ` · voir ${moisEnClair(m)}` : ""}`;
    return `<button type="button" class="chevron-pluri ${sens}" data-mois="${echapper(m)}"
      title="${echapper(titre)}" aria-label="${echapper(titre)}">${sens === "avant" ? "&#x2039;" : "&#x203A;"}</button>`;
  };
  return (avant ? un("avant", `commencé en ${moisEnClair(contrat.depuis)}`, -1) : "")
    + (apres ? un("apres", fin && contrat.jusqu_a ? `se poursuit jusqu'en ${moisEnClair(contrat.jusqu_a)}` : "se poursuit", 1) : "");
}

/** Pose (ou remplace) les chevrons d'un contrat sur plusieurs mois au bord de la case `div`. */
function poserChevrons(div, contrat, debut, fin) {
  div.querySelectorAll(":scope > .chevron-pluri").forEach((c) => c.remove());
  div.insertAdjacentHTML("beforeend", marquePlurimensuel(contrat, debut, fin));
}

// Le chevron mène au mois voisin, dans l'un ou l'autre état de la case.
document.addEventListener("click", (e) => {
  const c = e.target.closest && e.target.closest(".chevron-pluri[data-mois]");
  if (!c || !c.dataset.mois || typeof allerAuMois !== "function") return;
  e.preventDefault();
  allerAuMois(c.dataset.mois);
});


