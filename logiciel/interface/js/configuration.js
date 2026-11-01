/* La configuration : la liste, l'importation, metiers, organismes, AT/MP, sante. */
"use strict";

/* =======================================================================
   CONFIGURATION
   ===================================================================== */
let FICHE_ACTIVE = "salarie";
let CLE_ACTIVE = null;
/* Une fiche neuve est ouverte : `dessinerConfig` ne l'efface pas. */
let FICHE_NEUVE = false;

document.querySelectorAll(".sous-onglets button").forEach((b) => {
  b.onclick = async () => {
    if (!(await quitterLaFiche())) return;
    document.querySelectorAll(".sous-onglets button").forEach((x) => x.classList.remove("actif"));
    b.classList.add("actif");
    if (FICHE_ACTIVE !== b.dataset.fiche) FICHE_NEUVE = false;
    FICHE_ACTIVE = b.dataset.fiche;
    CLE_ACTIVE = null;
    dessinerConfig();
  };
});

const ARCHIVES_VISIBLES = { salarie: false, spectacle: false };
/* Ce qu'on cherche dans chaque liste ; gardé quand la liste se redessine. */
const RECHERCHE = { salarie: "", spectacle: "" };

function fichesDe(nom) {
  const c = ETAT.configuration;
  if (nom === "entreprise") return [c.entreprise];
  return nom === "salarie" ? c.salaries : c.spectacles;
}

function dessinerConfig() {
  const liste = document.getElementById("config-liste");
  dessinerSuppression();
  // Avancé : la place du sous-onglet ouvert (`data-place`), ses routes sans écran dessiné.
  const avance = document.getElementById("config-avance");
  avance.dataset.auto = (document.querySelector(`.sous-onglets button[data-fiche="${FICHE_ACTIVE}"]`) || {}).dataset?.place || "";
  dessinerAvance(avance);
  if (FICHE_ACTIVE === "importer") {
    document.getElementById("config-formulaire").innerHTML = "";
    dessinerImportation(liste);
    return;
  }
  if (FICHE_ACTIVE === "coffre") {
    document.getElementById("config-formulaire").innerHTML = "";
    dessinerCoffre(liste);
    return;
  }
  if (FICHE_ACTIVE === "identite") {
    document.getElementById("config-formulaire").innerHTML = "";
    dessinerIdentite(liste);
    return;
  }
  if (FICHE_ACTIVE === "metiers") {
    document.getElementById("config-formulaire").innerHTML = "";
    dessinerMetiers(liste);
    return;
  }
  if (FICHE_ACTIVE === "organismes") {
    document.getElementById("config-formulaire").innerHTML = "";
    dessinerOrganismes(liste);
    return;
  }
  // Le modèle du bulletin : `modeles-bulletin.js`.
  if (FICHE_ACTIVE === "bulletin") {
    document.getElementById("config-formulaire").innerHTML = "";
    dessinerModelesBulletin(liste);
    return;
  }
  // Les modèles de contrat et les clauses particulières : `modeles-contrat.js`.
  if (FICHE_ACTIVE === "contrats") {
    document.getElementById("config-formulaire").innerHTML = "";
    dessinerModelesContrat(liste);
    return;
  }
  const fiches = fichesDe(FICHE_ACTIVE);
  if (FICHE_ACTIVE === "entreprise") {
    liste.innerHTML = "";
    peindreDemarrage(liste);
    CLE_ACTIVE = null;
    dessinerFormulaire(fiches[0]);
    return;
  }
  // Les archives en dernier, jamais melangees aux courantes. Elles restent
  // trouvables : une fiche qu'on ne voit plus passe pour perdue, et on la
  // ressaisit -- avec un deuxieme code salarie, ce qui serait pire.
  const courantes = fiches.filter((f) => f.donnees.actif !== false);
  // Les spectacles par titre.
  if (FICHE_ACTIVE === "spectacle") {
    courantes.sort((a, b) => String(a.donnees.titre || "").localeCompare(
      String(b.donnees.titre || ""), "fr", { sensitivity: "base" }));
  }
  // Les numéros d'objet archivés, du plus récent au plus ancien.
  const archivees = fiches.filter((f) => f.donnees.actif === false)
    .sort((a, b) => String(b.donnees.numero_objet || "").localeCompare(
      String(a.donnees.numero_objet || "")));
  const rangees = [...courantes, ...archivees];
  const salarie = FICHE_ACTIVE === "salarie";

  // UN SPECTACLE, PAS UNE FICHE PAR NUMÉRO. Une fiche archivée du
  // même titre qu'un spectacle actif (un ancien numéro) se montre sous lui,
  // pas dans les archives.
  const titreDe = (d) => String(d.titre || "").normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "").toLowerCase().replace(/\s+/g, " ").trim();
  const sansTitre = (d) => titreDe(d) === "a completer";
  const passees = {};
  const sousUnActif = new Set();
  if (!salarie) {
    const titresActifs = new Set(courantes.map((f) => titreDe(f.donnees)));
    archivees.forEach((f) => {
      const t = titreDe(f.donnees);
      if (sansTitre(f.donnees) || !titresActifs.has(t)) return;
      (passees[t] = passees[t] || []).push(f);
      sousUnActif.add(f.cle);
    });
  }
  // Les archives qui restent : les spectacles qui ne se jouent plus, par
  // titre ; puis les numéros sans titre, groupés -- à nommer si on le sait.
  const archiveesSeules = archivees.filter((f) => !sousUnActif.has(f.cle))
    .sort((a, b) => (sansTitre(a.donnees) - sansTitre(b.donnees))
      || titreDe(a.donnees).localeCompare(titreDe(b.donnees))
      || String(b.donnees.numero_objet || "").localeCompare(String(a.donnees.numero_objet || "")));

  // UNE LIGNE PAR FICHE : le nom, ce qui
  // l'identifie, son état, puis les gestes -- « Archiver » dit en toutes
  // lettres, là où le clic sur toute la carte archivait sans prévenir.
  const rangHtml = (f, trouve) => {
    const d = f.donnees;
    const archive = d.actif === false;
    const titre = salarie
      ? `${String(d.nom_affichage || d.nom || "").toUpperCase()} ${d.prenom_affichage || d.prenom || ""}`.trim()
      : String(d.titre || "").trim();
    // Un numero d'objet importe de CoolPaie n'a pas de titre : le moteur
    // pose « A COMPLETER », que la ligne dit en minuscules.
    const aCompleter = !salarie && titre.toUpperCase() === "A COMPLETER";
    const quoi = salarie
      ? [d.emploi_affichage || d.emploi || "", d.code_salarie ? "matricule " + d.code_salarie : ""]
      : [d.numero_objet || "", d.code_analytique ? "analytique " + d.code_analytique : ""];
    const autresNumeros = !salarie && !archive && !trouve ? (passees[titreDe(d)] || [])
      .slice().sort((a, b) => String(b.donnees.numero_objet || "").localeCompare(
        String(a.donnees.numero_objet || ""))) : [];
    const etat = pastilleDeLEtat(f.pastille);
    return `<div class="rang fiche${archive ? " archivee" : ""}${f.cle === CLE_ACTIVE ? " selectionnee" : ""}"
        data-cle="${echapper(f.cle)}">
      <div class="rang-infos">
        <b class="rang-nom">${aCompleter ? `<span class="a-completer">titre à compléter</span>`
          : echapper(titre || f.cle)}</b>
        ${echapper(quoi.filter(Boolean).join(" · "))}
        ${trouve ? `<span class="trouve">trouvé dans ${echapper(trouve.ou)} « ${echapper(trouve.quoi)} »</span>` : ""}
        ${autresNumeros.length ? `<span class="annees">anciens numéros
          ${autresNumeros.map((p) => `<button class="lien" data-ouvrir="${echapper(p.cle)}">${
            echapper(p.donnees.numero_objet || p.cle)}</button>`).join(" · ")}</span>` : ""}
      </div>
      <div class="rang-etat">${etat}</div>
      <div class="rang-actions">
        <button class="lien" data-activer="${archive ? "oui" : "non"}">${archive ? "Réactiver" : "Archiver"}</button>
      </div>
    </div>`;
  };

  // CACHÉES PAR DÉFAUT : un spectacle archivé ne
  // se joue plus. Une case les montre ; elle reste
  // cochée tant qu'on ne la décoche pas, et une fiche archivée ouverte les
  // montre d'office.
  const voirArchives = ARCHIVES_VISIBLES[FICHE_ACTIVE]
    || archivees.some((f) => f.cle === CLE_ACTIVE);
  // UNE BARRE, DEUX GESTES : chercher, ajouter. Le titre qui répétait le
  // sous-onglet (« SALARIÉS » sous Salariés) est parti.
  liste.innerHTML = `<div class="barre-liste">
      <input type="search" id="config-recherche" class="recherche" autocomplete="off"
        value="${echapper(RECHERCHE[FICHE_ACTIVE] || "")}"
        placeholder="Rechercher">
      <button class="secondaire" id="config-nouvelle">+ ${salarie
        ? "un salarié" : "un spectacle"}</button>
    </div>
    <div id="config-rangs"></div>`
    + (salarie ? fichesAttenduesHtml() : "");

  // LA RECHERCHE DANS TOUS LES CHAMPS. Chaque valeur
  // de la fiche, les anciennes valeurs datées comprises (un courriel changé
  // retrouve encore la personne), sans accents, sans majuscules, sans
  // espaces (« 2 93 08 » trouve le NIR) ; une date se cherche aussi comme
  // on l'écrit (23/07/1990). Plusieurs mots : chacun doit se trouver
  // quelque part dans la fiche. Elle fouille les archivées aussi -- c'est
  // souvent là qu'on cherche.
  const libelles = {};
  (ETAT.configuration.description[FICHE_ACTIVE] || []).forEach((c) => { libelles[c.cle] = c.libelle; });
  const libelleDe = (cle) => libelles[cle] || libelles[cle.replace(/_affichage$/, "")]
    || cle.replace(/_/g, " ");
  const plat = (t) => String(t).normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
  const valeurs = (f) => {
    const out = [];
    const ajouter = (ou, v) => {
      if (v === null || v === undefined || v === "" || typeof v === "boolean") return;
      if (typeof v === "object") {
        Object.entries(v).forEach(([k, x]) => { if (!k.startsWith("_")) ajouter(ou, x); });
        return;
      }
      const t = String(v);
      out.push({ ou, quoi: t });
      if (/^\d{4}-\d{2}-\d{2}$/.test(t)) out.push({ ou, quoi: jjmmaaaa(t) });
    };
    // La forme lisible d'abord (« Avallon »), la forme déclarée ensuite
    // (« AVALLON ») : c'est la première trouvée que la ligne cite.
    Object.entries(f.donnees)
      .sort(([a], [b]) => b.endsWith("_affichage") - a.endsWith("_affichage"))
      .forEach(([k, v]) => {
        if (k.startsWith("_") || k === "historique" || k === "actif") return;
        ajouter(libelleDe(k), v);
      });
    (f.donnees.historique || []).forEach((h) =>
      ajouter(libelleDe(h.champ) + " (ancienne valeur)", h.valeur));
    return out.map((x) => ({ ...x, cle: plat(x.quoi), serre: plat(x.quoi).replace(/\s+/g, "") }));
  };
  const chercher = (f, mots) => {
    const vs = valeurs(f);
    let premier = null;
    for (const m of mots) {
      const serre = m.replace(/\s+/g, "");
      const v = vs.find((x) => x.cle.includes(m) || x.serre.includes(serre));
      if (!v) return null;
      premier = premier || v;
    }
    return premier || {};
  };

  // Les numéros sans titre (importés des DSN, « A COMPLETER ») en un groupe,
  // sous une phrase qui dit d'où ils viennent et ce qu'on peut en faire.
  const archivesHtml = () => {
    const nommees = archiveesSeules.filter((f) => salarie || !sansTitre(f.donnees));
    const anonymes = salarie ? [] : archiveesSeules.filter((f) => sansTitre(f.donnees));
    return `${nommees.length ? `<div class="rangs">${nommees.map((f) => rangHtml(f)).join("")}</div>` : ""}
      ${anonymes.length ? `<p class="note">${echapper(ETAT.configuration.numeros_sans_titre)}</p>
        <div class="rangs">${anonymes.map((f) => rangHtml(f)).join("")}</div>` : ""}`;
  };

  const peindre = () => {
    const q = plat(RECHERCHE[FICHE_ACTIVE] || "").trim();
    const cible = document.getElementById("config-rangs");
    if (q) {
      const mots = q.split(/\s+/);
      const trouvees = rangees.map((f) => ({ f, t: chercher(f, mots) })).filter((x) => x.t);
      // Où l'on a trouvé, sauf si c'est le nom même qu'on lit sur la ligne.
      cible.innerHTML = trouvees.length
        ? `<div class="rangs">${trouvees.map(({ f, t }) =>
            rangHtml(f, t.ou && !/^(nom|prénom|titre)/i.test(t.ou) ? t : null)).join("")}</div>`
        : `<p class="discret">Aucune fiche trouvée</p>`;
    } else {
      cible.innerHTML = `${courantes.length
          ? `<div class="rangs">${courantes.map((f) => rangHtml(f)).join("")}</div>`
          : `<p class="discret">${salarie ? "Aucun salarié actif" : "Aucun spectacle actif"}</p>`}
        ${archiveesSeules.length ? `<label class="coche montrer-archives">
            <input type="checkbox" id="config-archives"${voirArchives ? " checked" : ""}>
            ${salarie ? "Afficher les salariés archivés" : "Afficher les spectacles archivés"}
            <span class="discret">(${archiveesSeules.length})</span></label>
          ${voirArchives ? archivesHtml() : ""}` : ""}`;
      const caseArchives = document.getElementById("config-archives");
      if (caseArchives) caseArchives.onchange = () => {
        ARCHIVES_VISIBLES[FICHE_ACTIVE] = caseArchives.checked;
        dessinerConfig();
      };
    }
    cible.querySelectorAll(".rang.fiche").forEach((r) => {
      const cle = r.dataset.cle;
      r.querySelector("[data-activer]").onclick = async (e) => {
        const activer = e.currentTarget.dataset.activer === "oui";
        try {
          const rep = await api("activer-fiche", { nom: FICHE_ACTIVE, cle, actif: activer });
          poserEtat(rep.etat);
          dire(rep.phrase);
          dessinerConfig();
        } catch (err) { dire(err.message, "erreur"); }
      };
      const ouvrir = async (c) => {
        if (c !== CLE_ACTIVE && !(await quitterLaFiche())) return;
        CLE_ACTIVE = c;
        FICHE_NEUVE = false;
        dessinerConfig();
        dessinerFormulaire(rangees.find((f) => f.cle === c));
        document.getElementById("config-formulaire").scrollIntoView({ behavior: "smooth" });
      };
      // LA LIGNE S'OUVRE D'UN CLIC SUR ELLE.
      rendreOuvrable(r, () => ouvrir(cle), "Ouvrir la fiche");
      r.querySelectorAll("[data-ouvrir]").forEach((b) => { b.onclick = () => ouvrir(b.dataset.ouvrir); });
    });
  };
  peindre();
  const recherche = document.getElementById("config-recherche");
  recherche.oninput = () => { RECHERCHE[FICHE_ACTIVE] = recherche.value; peindre(); };
  document.getElementById("config-nouvelle").onclick = () => nouvelleFiche(FICHE_ACTIVE);
  brancherFichesAttendues();
  if (!CLE_ACTIVE && !FICHE_NEUVE) document.getElementById("config-formulaire").innerHTML = "";
}

/* ---------------------------------------------------- reprendre l'historique
   Deux temps, et jamais un seul : l'aperçu dit ce qui serait créé et ce que
   les DSN disaient autrement que les fiches ; le bouton n'écrit qu'après.
   Une fiche existante reçoit seulement ce qui y est vide ou « A COMPLETER »
   (`import::completer`) ; tout champ rempli fait foi, l'écart
   est signalé, un changement se date à la main.

   LA BOÎTE AUX LETTRES. Une seule
   entrée : le dossier `import/` de la compagnie, où l'on met tout ce que
   l'ancien logiciel a laissé — DSN, AEM, bulletins de paie en PDF, archives
   zip. Le moteur trie par le contenu (`importation-apercu` avec `dossier`) et
   rend un compte rendu par fichier : la page le montre en tableau, le sort en
   pastille, la raison en clair. Elle ne décide rien : ce qui est repris, ce
   qui ne l'est pas et pourquoi, c'est le moteur qui le dit. */
let IMPORT_DOSSIER = "";

/* Le sort d'un fichier ou d'une page (`import::bulletins::SORTS`) : sa pastille,
   telle que la réponse du moteur la donne (`pastilles`). */
function pastilleSort(sort, pastilles) {
  return pastilleDeLEtat((pastilles || {})[sort] || { couleur: "gris", mots: String(sort || "?") });
}

/* « Ouvrir le dossier » n'a de sens que si la page tourne sur le poste,
   servie par sa fenêtre : c'est elle qui ouvre l'explorateur (`ouvrir`, `interface/poste.rs`). Sans elle, le bouton
   n'apparaît pas. */
// Dans la fenêtre PuppetPaie (Tauri, `cadre.js`), et non dans un navigateur : le poste peut ouvrir son dossier.
const fenetreDuPoste = () => !!window.fenetrePuppetPaie;

/* REPRENDRE L'HISTORIQUE : une
   logique simple : ils mettent tout dans import/, ou ils choisissent un
   dossier avec tout dedans. La page ne sert qu'une fois : le jour où une
   compagnie arrive avec l'historique d'un autre logiciel. Un dossier, celui
   de la compagnie prérempli ; « Parcourir… » pour en prendre un autre ;
   « Ouvrir le dossier » pour y déposer les fichiers ; puis l'aperçu. Aucun
   logiciel n'est nommé. Si l'historique est repris, l'état le dit en une
   ligne, et « Relire un dossier » reste pour ce qui manquait. */
async function dessinerImportation(cible) {
  let e = null;
  try { e = await api("importation-etat"); } catch (err) { e = null; }
  if (!IMPORT_DOSSIER && e && e.dossier) IMPORT_DOSSIER = e.dossier;
  const deja = e && ((e.reprises && e.reprises.length) || e.contrats);
  const dossier = `<div class="e-contrat-champs">
      <label style="flex:1">Le dossier
        <span style="display:flex;gap:6px">
          <input type="text" id="import-dossier" style="flex:1" placeholder="vide : le dossier import/ de la compagnie"
                 value="${echapper(IMPORT_DOSSIER)}">
          <button type="button" class="secondaire" id="import-parcourir">Parcourir…</button>
          ${fenetreDuPoste() ? `<button type="button" class="secondaire" id="import-ouvrir">Ouvrir le dossier</button>` : ""}
        </span>
      </label>
      ${deja ? `<button class="secondaire" id="import-apercu">Voir ce qui serait repris</button>` : ""}
    </div>`;
  if (deja) {
    cible.innerHTML = `
    <div class="carte">
      <div id="import-etat">${vueEtatImportation(e)}</div>
      <details class="relire"><summary>Relire un dossier — un fichier ajouté depuis, un
          salarié ou une année qui manquait</summary>
          <p class="note">Les fiches qui existent déjà ne sont pas touchées ; ce que les DSN ou les
            bulletins disaient autrement est seulement signalé. Ce qui est déjà repris ne se double pas.</p>
          <p class="intro">Mettez dans ce dossier ce qui manquait, puis « Voir ce qui serait repris ».
            Rien n'est écrit avant « Reprendre ».</p>
          ${dossier}
          <div id="import-resultat"></div>
        </details>
    </div>`;
  } else {
    // RIEN N'EST ENCORE REPRIS : le guide (`importation-etat`, `guide`, du moteur), le dossier, et « Importer », qui
    // reprend tout d'un geste. Ce qui reste à dire (`import-etat`, `import-demarrage`) se peint au-dessus après lui.
    cible.innerHTML = `
    <div class="carte">
      <div id="import-etat">${vueEtatImportation(e)}</div>
      <div id="import-demarrage"></div>
      <div id="import-guide">${vueGuideImport(e && e.guide)}
        ${dossier}
        <p><button class="primaire" id="import-tout">Importer</button>
          <button type="button" class="lien" id="import-apercu">ou voir d'abord ce qui serait repris</button></p>
      </div>
      <div id="import-resultat"></div>
    </div>`;
    document.getElementById("import-tout").onclick = () => importerDepuisLeGuide();
  }
  document.getElementById("import-apercu").onclick = () => apercuImportation();
  // L'EXPLORATEUR : la boîte du système, ouverte par la fenêtre.
  document.getElementById("import-parcourir").onclick = async () => {
    try {
      const r = await fenetre("choisir_dossier", "Le dossier à reprendre");
      if (r.dossier) document.getElementById("import-dossier").value = r.dossier;
    } catch (err) { dire(err.message, "erreur"); }
  };
  const ouvrir = document.getElementById("import-ouvrir");
  // Le dossier choisi, sinon la boîte import/ de la compagnie (`dossier` de `importation-etat`), créée si elle manque.
  if (ouvrir) ouvrir.onclick = () => ouvrirEtDire(document.getElementById("import-dossier").value.trim() || (e && e.dossier) || "", "", true);
}

/* LE GUIDE DE L'IMPORT (`import::guide`, du moteur) : ce qu'on met dans le
   dossier, ce que le logiciel lit de chaque pièce, ce qui restera à dire. La
   page le montre tel quel. */
function vueGuideImport(g) {
  if (!g) return "";
  const pieces = (g.pieces || []).map((p) => `<div class="guide-piece">
      <b>${echapper(p.quoi)}</b>
      <div class="note">${echapper(p.ou)}</div>
      ${(p.lu || []).length ? `<ul>${p.lu.map((x) => `<li>${echapper(x)}</li>`).join("")}</ul>` : ""}
    </div>`).join("");
  return `<h3>Reprendre votre passé</h3>
    <p class="intro">Mettez dans le dossier ci-dessous tout ce que votre ancien logiciel vous a laissé, puis
      « Importer » : PuppetPaie crée la fiche de la compagnie, ses salariés, ses spectacles et leurs contrats
      avec ce que les pièces disent.</p>
    ${(g.a_quoi_sert || []).length ? `<ul>${g.a_quoi_sert.map((x) => `<li>${echapper(x)}</li>`).join("")}</ul>` : ""}
    <p class="note">${echapper(g.accepte || "")}</p>
    <div class="guide-pieces">${pieces}</div>
    <p class="note">${echapper(g.reste || "")}</p>
    <p class="note">${echapper(g.rien_n_est_perdu || "")}</p>`;
}

/* « IMPORTER », D'UN GESTE : le moteur reprend tout (`importer`), puis la page
   montre, au-dessus, ce qui reste à dire — la nature des contrats repris, les
   congés à reprendre (`importation-etat`) et ce qui manque encore pour
   démarrer (`pour-demarrer`) — et, dessous, ce qui a été repris. */
async function importerDepuisLeGuide() {
  IMPORT_DOSSIER = document.getElementById("import-dossier").value.trim();
  const rendre = occupe(document.getElementById("import-tout"), "Import en cours : les fiches, les contrats, puis chaque bulletin…");
  await ecrireImportation();
  rendre();
  // Rien de repris (un dossier vide, des pièces illisibles) : le guide reste, avec ce que le moteur en a dit.
  let etat = null;
  try { etat = await api("importation-etat"); } catch (e) { return; }
  if (!((etat.reprises || []).length || etat.contrats)) return;
  const guide = document.getElementById("import-guide");
  if (guide) guide.hidden = true;
  const demarrage = document.getElementById("import-demarrage");
  if (demarrage) await peindreDemarrage(demarrage);
  document.getElementById("import-etat").scrollIntoView({ block: "start" });
}

/* L'ÉTAT (`importation-etat`), en une ligne et sans nommer de logiciel :
   « Historique repris le … : N bulletins, N déclarations, N contrats, de …
   à … ». Les bulletins : les paies qu'un bulletin a écrites ou complétées ;
   les déclarations : une DSN par mois déclaré (`declarations` de la reprise
   DSN) et une AEM par contrat qu'elle atteste ; les contrats : tous ceux de
   l'historique (`contrats`). Puis ce qui attend l'éditeur (`editeur`). */
function vueEtatImportation(e) {
  if (!e) return "";
  const reprises = e.reprises || [];
  const de = (source) => reprises.find((r) => r.source === source) || {};
  const compte = (n, mot) => (n ? `${n} ${mot}${n > 1 ? "s" : ""}` : "");
  const quoi = [
    compte(de("bulletins").nombre || 0, "bulletin"),
    compte((de("DSN").declarations || 0) + (de("AEM").nombre || 0), "déclaration"),
    compte(de("certificats").nombre || 0, "certificat"),
    compte(e.contrats || 0, "contrat"),
  ].filter((x) => x).join(", ");
  const bornes = [e.du, e.au, ...reprises.flatMap((r) => [r.du, r.au])]
    .filter((x) => x).map((x) => String(x).slice(0, 7)).sort();
  const [du, au] = [bornes[0], bornes[bornes.length - 1]];
  const periode = !du ? "" : du === au ? `, en ${moisEnClair(du)}` : `, ${deMois(du)} à ${moisEnClair(au)}`;
  const le = [e.importe_le, ...reprises.map((r) => r.le)].filter((x) => x).sort().pop();
  const repris = quoi ? `<div class="rang">
      <div class="rang-infos"><b class="rang-nom">Historique repris${le ? ` le ${echapper(jjmmaaaa(le))}` : ""}</b>
        ${echapper(quoi + periode)}</div>
      <div class="rang-etat">${pastilleDeLEtat(e.pastille_repris)}</div>
    </div>` : "";
  const editeur = (e.editeur || []).map((x) => `<div class="rang">
      <div class="rang-infos"><b class="rang-nom">${x.canal === "signalement" ? "Message à l'éditeur" : "Incident"}${
        x.quand ? ` du ${echapper(quand(x.quand))}` : ""}</b>${echapper(x.resume || "")}</div>
      <div class="rang-etat">${pastilleDeLEtat(x.pastille)}</div>
    </div>`).join("");
  return repris + vueNaturesADire(e.a_dire)
    + (editeur ? `<h3>Ce qui attend l'éditeur</h3>${editeur}` : "");
}

/* LA NATURE DES CONTRATS REPRIS QU'AUCUN BULLETIN NE DIT
   (`import::nature`). Les bulletins d'un permanent ne disent pas s'il
   est en CDI ou en CDD : le moteur les nomme (`a_dire`), la page le
   demande, une fois par salarié, sans rien choisir d'avance. */
function vueNaturesADire(liste) {
  if (!liste || !liste.length) return "";
  // Ce que chaque nature demande avec elle (`demande`, du moteur) : pour un
  // contrat qui court encore, son début, sa fin s'il a un terme, son numéro.
  const CASES = {
    debut: (x) => `<label>Début du contrat <input type="date" data-nature-champ="debut"></label>`,
    fin: (x) => `<label>Fin prévue <input type="date" data-nature-champ="fin"></label>`,
    numero: (x) => `<label title="rubrique S21.G00.40.009 de la DSN">Numéro du contrat
        <input type="text" data-nature-champ="numero" placeholder="sur la DSN ${echapper(deMois(String(x.au).slice(0, 7)))}"></label>`,
    salaire: (x) => `<label>Salaire mensuel brut <input type="number" step="0.01" min="0" data-nature-champ="salaire"
        placeholder="celui du contrat signé"></label>`,
    quotite: (x) => `<label title="rubrique S21.G00.40.013 de la DSN">Durée du travail (heures par mois)
        <input type="number" step="0.01" min="0" data-nature-champ="quotite" placeholder="celle du contrat signé"></label>`,
  };
  const lignes = liste.map((x) => `<div class="rang" data-nature-rang="${echapper(x.salarie)}">
      <div class="rang-infos"><b class="rang-nom">${echapper(x.nom)}</b>
        ${x.contrats} contrat${x.contrats > 1 ? "s" : ""} repris${x.du ? `, du ${echapper(jjmmaaaa(x.du))}
          au ${echapper(jjmmaaaa(x.au))}` : ""} — ses bulletins n'ont pas de Congés Spectacles ni de
        numéro d'objet : c'est un contrat de permanent.
        ${x.en_cours ? `<div class="note">Payé jusqu'en ${echapper(moisEnClair(String(x.au).slice(0, 7)))} : son contrat
          continue en ${echapper(moisEnClair(x.suite))}, et sa DSN le déclare avec la date de début et le
          numéro que votre ancien logiciel lui a donnés. Ils sont sur sa dernière DSN (bloc Contrat), pas sur
          le bulletin.</div>` : ""}
        <div class="e-contrat-champs" data-nature-cases hidden>${["debut", "fin", "numero", "salaire", "quotite"].map((c) =>
          `<span data-nature-case="${c}" hidden>${CASES[c](x)}</span>`).join("")}</div></div>
      <div class="rang-etat" style="display:flex;gap:6px;align-items:center">
        <select data-nature-choix aria-label="Nature du contrat de ${echapper(x.nom)}">
          <option value="">— choisir —</option>
          ${(x.natures || []).map((n) => `<option value="${echapper(n.cle)}"
             data-demande="${echapper((n.demande || []).join(" "))}">${echapper(n.libelle)}</option>`).join("")}
        </select>
        <button type="button" class="petit" data-nature-salarie="${echapper(x.salarie)}">Enregistrer</button>
      </div>
    </div>`).join("");
  return `<h3>La nature de ces contrats</h3>
    <p class="note">Un bulletin de paie ne dit pas si le contrat est un CDI ou un CDD. Dites-le pour chacun :
      la DSN et les documents de fin de contrat en dépendent.</p>${lignes}`;
}

/* La nature choisie montre les cases qu'elle demande, et elles seules. */
document.addEventListener("change", (ev) => {
  const sel = ev.target.closest && ev.target.closest("[data-nature-choix]");
  if (!sel) return;
  const rang = sel.closest("[data-nature-rang]");
  const opt = sel.selectedOptions[0];
  const demande = ((opt && opt.dataset.demande) || "").split(" ").filter((x) => x);
  rang.querySelectorAll("[data-nature-case]").forEach((c) => { c.hidden = !demande.includes(c.dataset.natureCase); });
  rang.querySelector("[data-nature-cases]").hidden = !demande.length;
});

document.addEventListener("click", async (ev) => {
  const ouvrir = ev.target.closest && ev.target.closest("[data-ouvrir-fiche]");
  if (ouvrir) {
    ev.preventDefault();
    ouvrirFicheImportee(ouvrir.dataset.ouvrirFiche, ouvrir.dataset.ouvrirCle);
    return;
  }
  const b = ev.target.closest && ev.target.closest("[data-nature-salarie]");
  if (!b) return;
  ev.preventDefault();
  const rang = b.closest("[data-nature-rang]");
  const nature = rang ? rang.querySelector("[data-nature-choix]").value : "";
  // Une nature vide part telle quelle : c'est le moteur qui la refuse (règle 2).
  b.disabled = true;
  try {
    const charge = { salarie: b.dataset.natureSalarie, nature };
    rang.querySelectorAll("[data-nature-case]:not([hidden]) [data-nature-champ]").forEach((c) => {
      charge[c.dataset.natureChamp] = c.value;
    });
    const r = await api("importation-nature", charge);
    dire(r.phrase, r.couleur);
    // Les natures au mois s'ouvrent alors dans la saisie (`etat.natures`).
    await relireEtat();
    rafraichirReferentiels();
    rafraichirEtatImportation();
  } catch (err) {
    dire(err.message, "erreur");
    b.disabled = false;
  }
});

/* Ouvrir, dans Configuration, la fiche que l'import vient de créer. */
async function ouvrirFicheImportee(nom, cle) {
  if (!(await quitterLaFiche())) return;
  document.querySelectorAll(".sous-onglets button").forEach((x) => x.classList.toggle("actif", x.dataset.fiche === nom));
  FICHE_ACTIVE = nom;
  CLE_ACTIVE = nom === "entreprise" ? null : cle;
  FICHE_NEUVE = false;
  dessinerConfig();
  if (nom !== "entreprise") {
    const f = fichesDe(nom).find((x) => x.cle === cle);
    if (f) dessinerFormulaire(f);
  }
  document.getElementById("config-formulaire").scrollIntoView({ behavior: "smooth" });
}

/* D'où vient une valeur : la DSN d'un mois, un bulletin (fichier, page), ou
   un certificat Congés Spectacles (son mois, son fichier). */
function origineImport(e) {
  if (e.source === "certificat" || e.source === "AEM") {
    return `${e.source === "AEM" ? "l'AEM" : "le certificat Congés Spectacles"}${e.mois ? ` de ${echapper(moisEnClair(e.mois))}` : ""}${
      e.fichier ? ` (${echapper(e.fichier)}${e.page ? `, page ${e.page}` : ""})` : ""}`;
  }
  return e.source === "bulletin" || "bulletin" in e
    ? `le bulletin${e.fichier ? ` (${echapper(e.fichier)}${e.page ? `, page ${e.page}` : ""})` : ""}`
    : `la DSN ${echapper(deMois(e.mois))}`;
}

/* Ce que l'import met dans une case vide ou « A COMPLETER » d'une fiche
   existante (`completer`) : le moteur l'a décidé, la page le dit. */
function valeurImportee(v) {
  if (v && typeof v === "object") return Array.isArray(v)
    ? v.map((x) => (x && typeof x === "object" ? Object.values(x).join(" ") : x)).join(", ")
    : Object.entries(v).map(([k, x]) => `${k} ${x}`).join(", ");
  return v;
}

/* Une fiche que l'import crée, complète, laisse, ou sur laquelle il a à
   redire. Un écart vient d'une DSN (`dsn`, `mois`), d'un bulletin ou d'un certificat
   (`bulletin`, `fichier`, `page`) : la fiche fait foi, il est seulement
   signalé. Une case vide est complétée, et la page dit d'où. */
function fichesImportees(fiches, titre) {
  const lignes = (fiches || []).filter((f) => f).map((f) => {
    const etat = pastilleDeLEtat(f.pastille);
    // Une fiche que seuls des certificats ou des AEM créent : d'où elle vient (`lu_dans`).
    const luDans = f.lu_dans ? `<div class="acte-dernier">Lu dans ${origineImport(f.lu_dans)}</div>` : "";
    const completes = Object.entries(f.completer || {}).map(([champ, e]) =>
      `<div class="acte-dernier"><b>${echapper(champ)}</b> ${f.etat === "completee" ? "a reçu" : "recevra"}
         « ${echapper(valeurImportee(e.valeur))} », lu dans ${origineImport(e)} — la case était vide</div>`).join("");
    const ecarts = Object.entries(f.ecarts || {}).map(([champ, e]) => {
      const ou = "certificat" in e || "aem" in e
        ? `${"aem" in e ? "l'AEM" : "le certificat"} (${echapper(e.fichier || "")}${e.page ? `, page ${e.page}` : ""}) disait`
        : "bulletin" in e
        ? `le bulletin (${echapper(e.fichier || "")}${e.page ? `, page ${e.page}` : ""}) disait`
        : `la DSN ${echapper(deMois(e.mois))} disait`;
      const lu = "certificat" in e ? e.certificat : "aem" in e ? e.aem : "bulletin" in e ? e.bulletin : e.dsn;
      return `<div class="acte-dernier">${ou} <b>${echapper(champ)}</b> = « ${echapper(lu)} »,
         la fiche dit « ${echapper(e.fiche)} » — la fiche fait foi</div>`;
    }).join("");
    // UNE VALEUR FAUSSE DES PIÈCES se dit, avec le libellé de sa case ; une
    // case vide n'est pas une faute (ce qu'un acte exige, « Pour démarrer »
    // le dit après l'import).
    const fautes = (f.anomalies || []).filter((a) => a.gravite === "erreur").map((a) =>
      `<div class="acte-dernier faute">${echapper(a.libelle || a.champ)} : ${echapper(a.message)}</div>`).join("");
    const ouvrable = f.etat === "creee" && f.cle && f.nom;
    const reste = fautes ? `<div class="acte-dernier">${ouvrable ? `<button type="button" class="petit" data-ouvrir-fiche="${echapper(f.nom)}"
            data-ouvrir-cle="${echapper(f.cle)}">Compléter la fiche</button>` : ""}</div>${fautes}` : "";
    return `<div class="rang import">
      <span class="rang-nom">${echapper(f.libelle)}</span>
      ${etat}
      <span class="rang-infos">${luDans}${completes}${ecarts}${reste}</span></div>`;
  });
  if (!lignes.length) return "";
  return `<h3>${titre}</h3>${lignes.join("")}`;
}

/* Le compte rendu, fichier par fichier (`tri.fichiers`) : le chemin dans le
   dossier, le sort, la raison. Un PDF de plusieurs pages déplie ses pages. */
function compteRenduImport(tri, pastilles) {
  if (!tri || !tri.fichiers) return "";
  const comptes = Object.entries(tri.comptes || {}).filter(([, n]) => n > 0)
    .map(([s, n]) => `${pastilleSort(s, pastilles)} ${n}`).join(" &nbsp; ");
  const lignes = tri.fichiers.map((f) => {
    const pages = f.pages && f.pages.length > 1
      ? `<details class="relire"><summary>${f.pages.length} pages</summary>${f.pages.map((p) =>
          `<div class="acte-dernier">page ${p.page}${p.mois ? ` (${echapper(moisEnClair(p.mois))})` : ""}
             ${pastilleSort(p.sort, pastilles)} ${echapper(p.raison || "")}</div>`).join("")}</details>` : "";
    const de = f.doublon_de ? ` <span class="discret">(copie de ${echapper(f.doublon_de)})</span>`
      : f.archive ? ` <span class="discret">(dans ${echapper(f.archive)})</span>` : "";
    return `<tr><td>${echapper(f.chemin)}${de}</td><td style="white-space:nowrap">${pastilleSort(f.sort, pastilles)}</td>
      <td>${echapper(f.raison || "")}${pages}</td></tr>`;
  }).join("");
  return `<h3>Le dossier, fichier par fichier</h3>
    <p class="note">${tri.total} fichier${tri.total > 1 ? "s" : ""} dans
      <code>${echapper(tri.dossier || "")}</code> &nbsp; ${comptes}</p>
    <table class="tableau-detail"><thead><tr><th>Fichier</th><th>Sort</th><th>Raison</th></tr></thead>
      <tbody>${lignes}</tbody></table>`;
}

/* Les bulletins lus : combien, combien repris, de quand à quand, et ce
   qu'ils ajoutent (`bulletins`). Pas le logiciel qui les a faits. */
function bulletinsImportes(b, ecrit) {
  if (!b || !b.pages) return "";
  const periode = b.du ? ` du ${echapper(jjmmaaaa(b.du))} au ${echapper(jjmmaaaa(b.au))}` : "";
  const ajouts = [
    b.paies_nouvelles ? `${b.paies_nouvelles} paie${b.paies_nouvelles > 1 ? "s" : ""} qu'aucune DSN ne déclare` : "",
    b.paies_completees ? `${b.paies_completees} paie${b.paies_completees > 1 ? "s" : ""} déclarée${b.paies_completees > 1 ? "s" : ""} complétée${b.paies_completees > 1 ? "s" : ""} (cotisations, coût employeur, lignes)` : "",
    b.contrats_nouveaux ? `${b.contrats_nouveaux} contrat${b.contrats_nouveaux > 1 ? "s" : ""} attesté${b.contrats_nouveaux > 1 ? "s" : ""}` : "",
    b.fiches_a_creer ? `${b.fiches_a_creer} fiche${b.fiches_a_creer > 1 ? "s" : ""}${ecrit ? (b.fiches_a_creer > 1 ? " créées" : " créée") : " à créer"}` : "",
  ].filter((x) => x);
  return `<div class="encart"><b>Bulletins de paie</b>
      ${b.lus} bulletin${b.lus > 1 ? "s" : ""} lu${b.lus > 1 ? "s" : ""} sur ${b.pages} page${b.pages > 1 ? "s" : ""}${
        periode} ; ${b.repris} ${ecrit ? `repris` : "à reprendre"}${
        ajouts.length ? ` : ${ajouts.join(", ")}` : ""}.</div>`;
}

/* Les certificats Congés Spectacles (`certificats`) ou les AEM en PDF
   (`aem_pdf`) lus : combien, de quand à quand, ce qu'ils ajoutent, ce
   qu'ils confirment, et, pour une fiche qu'ils créent, ce qu'aucune de ces
   pièces ne porte (le moteur le nomme). `titre`, `piece` : leurs mots. */
function piecesImportees(c, ecrit, titre, piece, pieces, aucune) {
  if (!c || !c.lus) return "";
  const periode = c.du ? ` du ${echapper(jjmmaaaa(c.du))} au ${echapper(jjmmaaaa(c.au))}` : "";
  const ajouts = [
    c.contrats_nouveaux ? `${c.contrats_nouveaux} contrat${c.contrats_nouveaux > 1 ? "s" : ""} attesté${c.contrats_nouveaux > 1 ? "s" : ""}` : "",
    c.confirmes ? `${c.confirmes} déjà attesté${c.confirmes > 1 ? "s" : ""} par une autre pièce` : "",
    c.fiches_a_creer ? `${c.fiches_a_creer} fiche${c.fiches_a_creer > 1 ? "s" : ""}${ecrit ? (c.fiches_a_creer > 1 ? " créées" : " créée") : " à créer"}` : "",
  ].filter((x) => x);
  const reste = (c.reste_a_saisir || []).length
    ? `<div class="note">${aucune}, et reste à saisir dans chaque fiche :
         ${c.reste_a_saisir.map((x) => echapper(x)).join(" ; ")}.</div>` : "";
  return `<div class="encart"><b>${titre}</b>
      ${c.lus} ${c.lus > 1 ? pieces : piece} lu${c.lus > 1 ? "s" : ""}${periode}${
        ajouts.length ? ` : ${ajouts.join(", ")}` : ""}.${reste}</div>`;
}

/* Les écarts entre un bulletin et ce que la DSN (ou l'historique) disait
   de la même paie : le bulletin fait foi sur les montants, l'écart est
   noté (`tri.fichiers[].pages[].ecarts`). */
const CHAMPS_PAIE = { brut: "brut", net_imposable: "net imposable", net_a_payer: "net à payer", pas: "prélèvement à la source" };
function ecartsBulletinDsn(tri) {
  const lignes = [];
  for (const f of (tri && tri.fichiers) || []) {
    for (const p of f.pages || []) {
      for (const e of p.ecarts || []) {
        lignes.push(`<tr><td>${echapper(f.chemin)}${f.pages.length > 1 ? `, page ${p.page}` : ""}</td>
          <td>${echapper(moisEnClair(p.mois || ""))}</td>
          <td>${echapper(CHAMPS_PAIE[e.champ] || e.champ)}</td>
          <td class="nombre">${echapper(e.valeur == null ? "" : e.valeur)}</td>
          <td class="nombre">${echapper(e.bulletin == null ? "" : e.bulletin)}</td>
          <td>${pastilleDeLEtat(e.pastille)}</td></tr>`);
      }
    }
  }
  if (!lignes.length) return "";
  return `<h3>Ce que les bulletins disent autrement</h3>
    <p class="note">Le bulletin fait foi : c'est son montant qui s'écrit, l'écart reste noté ici.</p>
    <table class="tableau-detail"><thead><tr><th>Bulletin</th><th>Mois</th><th>Champ</th>
      <th class="nombre">DSN ou historique</th><th class="nombre">Bulletin</th><th></th></tr></thead>
      <tbody>${lignes.join("")}</tbody></table>`;
}

/* Les documents qui ne sont pas repris, et pourquoi (`tri.non_repris`, du
   moteur), avec la phrase qui dit que l'import se rejoue. */
function vueNonRepris(nr, pastilles) {
  if (!nr || !(nr.fichiers || []).length) return "";
  return `<div class="encart attention"><b>Ces documents ne sont pas repris.</b>
      ${nr.fichiers.map((f) => `<div class="acte-dernier">${echapper(f.chemin)} ${pastilleSort(f.sort, pastilles)}
        ${echapper(f.raison || "")}</div>`).join("")}
      <p>${echapper(nr.phrase || "")}</p>
    </div>`;
}

async function rafraichirEtatImportation() {
  const cible = document.getElementById("import-etat");
  if (!cible) return;
  try { cible.innerHTML = vueEtatImportation(await api("importation-etat")); } catch (e) { /* l'état se relira à la prochaine visite */ }
}

/* L'aperçu, ou ce qui a été repris. Aucun logiciel n'y est nommé : les
   déclarations sont des DSN et des AEM, les bulletins des bulletins. */
function vueImportation(r) {
  const periode = r.premier_mois ? ` (${moisEnClair(r.premier_mois)} → ${moisEnClair(r.dernier_mois)})` : "";
  const contrats = r.contrats
    ? `${r.contrats} contrat(s) attesté(s) du ${jjmmaaaa(r.contrats_du)} au ${jjmmaaaa(r.contrats_au)} :
       <strong>${r.contrats_nouveaux} ${r.ecrit ? "ajouté(s)" : "à ajouter"}</strong> à l'historique,
       ${r.contrats - r.contrats_nouveaux - r.contrats_sans_fiche} déjà connu(s)${
         r.contrats_sans_fiche ? `, ${r.contrats_sans_fiche} sans fiche` : ""}`
    : "aucun contrat attesté par une AEM";
  const declarations = r.dsn || r.aem
    ? `${r.dsn} DSN${periode}, ${r.aem} fichier(s) AEM — ${contrats}`
    : "aucune DSN ni fichier d'AEM déposé dans le dossier";
  const avert = (r.avertissements || []).map((a) => `<div class="encart attention">${echapper(a)}</div>`).join("");
  const b = r.bulletins || {};
  const aEcrire = (f) => f && (f.etat === "a_creer" || (f.etat === "existante" && Object.keys(f.completer || {}).length));
  const ns = (r.salaries || []).filter(aEcrire).length, nsp = (r.spectacles || []).filter(aEcrire).length;
  const nc = (r.contrats_nouveaux || 0) + (b.contrats_nouveaux || 0);
  const np = (b.paies_nouvelles || 0) + (b.paies_completees || 0);
  const quoi = [
    aEcrire(r.entreprise) ? "la fiche de la compagnie" : "",
    ns ? `${ns} salarié${ns > 1 ? "s" : ""}` : "",
    nsp ? `${nsp} spectacle${nsp > 1 ? "s" : ""}` : "",
    nc ? `${nc} contrat${nc > 1 ? "s" : ""}` : "",
    np ? `${np} bulletin${np > 1 ? "s" : ""}` : "",
  ].filter((x) => x);
  // La couleur de l'encart, et ce que l'import a posé de la configuration,
  // viennent du moteur (`encart`, `parametrage`).
  const couleur = ENCART_DE_COULEUR[(r.encart || {}).couleur] || "";
  const pose = (r.parametrage || []).length
    ? `<p class="note">Posé dans la configuration :</p><ul>${r.parametrage.map((p) => `<li>${echapper(p)}</li>`).join("")}</ul>` : "";
  const bouton = r.ecrit
    ? `<div class="encart ${couleur}"><b>Repris.</b> ${(r.fichiers || []).map((f) => echapper(f)).join("<br>")}
        ${pose}
        <p class="note">Les fichiers lus sont copiés dans l'historique de la compagnie ; vous pouvez vider
          le dossier <code>import/</code>.</p></div>`
    : r.rien_a_faire
      ? `<div class="encart ${couleur}"><b>Rien à reprendre</b> : tout est déjà là.</div>`
      : `<p><button class="primaire" id="import-ecrire">Reprendre${quoi.length ? " " + quoi.join(", ") : ""}</button>
         <span class="note">Rien n'est écrit avant ce clic.</span></p>`;
  return `${vueNonRepris((r.tri || {}).non_repris, r.pastilles)}
    ${compteRenduImport(r.tri, r.pastilles)}
    <h3>${r.ecrit ? "Ce qui a été repris" : "Ce qui serait repris"}</h3>
    <div class="encart"><b>Déclarations</b>
      ${declarations}</div>
    ${bulletinsImportes(r.bulletins, r.ecrit)}
    ${piecesImportees(r.certificats, r.ecrit, "Certificats Congés Spectacles", "certificat", "certificats", "Aucun certificat ne porte")}
    ${piecesImportees(r.aem_pdf, r.ecrit, "AEM en PDF", "attestation", "attestations", "Aucune AEM ne porte")}
    ${avert}
    ${fichesImportees([r.entreprise], "Entreprise")}
    ${fichesImportees(r.salaries, "Salariés")}
    ${fichesImportees(r.spectacles, "Spectacles")}
    ${ecartsBulletinDsn(r.tri)}
    ${bouton}`;
}

function brancherImportation() {
  const b = document.getElementById("import-ecrire");
  if (b) b.onclick = () => ecrireImportation();
}

async function apercuImportation() {
  IMPORT_DOSSIER = document.getElementById("import-dossier").value.trim();
  const cible = document.getElementById("import-resultat");
  // LE GESTE QUI ATTEND SE VOIT : le bouton dit qu'il lit, la page qu'elle travaille.
  const rendre = occupe(document.getElementById("import-apercu"), "Lecture des fichiers…");
  cible.innerHTML = `<p class="discret">Lecture du dossier, fichier par fichier…</p>`;
  try {
    const r = await api("importation-apercu", { dossier: IMPORT_DOSSIER });
    cible.innerHTML = vueImportation(r);
    brancherImportation();
  } catch (e) {
    cible.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`;
  } finally {
    rendre();
  }
}

async function ecrireImportation() {
  const cible = document.getElementById("import-resultat");
  cible.classList.add("attente");
  occupe(document.getElementById("import-ecrire"), "Reprise en cours : les fiches, les contrats, puis chaque bulletin…");
  try {
    const r = await api("importer", { dossier: IMPORT_DOSSIER });
    cible.innerHTML = vueImportation(r);
    brancherImportation();
    // Les fiches créées doivent apparaître dans Salariés et Spectacles, et
    // dans les déroulants de la saisie, sans recharger la page.
    await relireEtat();
    rafraichirReferentiels();
    // CE QUI RESTE À DIRE : la nature des contrats repris se demande en tête de la page ; on y va.
    await rafraichirEtatImportation();
    const aDire = document.querySelector("[data-nature-rang]");
    if (aDire) aDire.scrollIntoView({ block: "center" });
    dire(r.phrase, r.couleur);
  } catch (e) {
    cible.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`;
  } finally {
    cible.classList.remove("attente");
  }
}

/*  LA FICHE ENTREPRISE SE LIT
 *
 *  Elle etait une liste plate de trente cases, des accolades JSON a taper
 *  pour le signataire et la banque, une etiquette « DÉCLARÉ » sous chacune,
 *  et le SIREN, le NIC et le SIRET demandes trois fois de suite. L'editeur :
 *  « tout est un peu mélangé (...) pas très friendly pour ceux qui
 *  comprennent pas le code ».
 *
 *  Quatre decisions, et aucune ne touche au moteur :
 *
 *   - des SECTIONS (SECTIONS_FICHE) : on cherche « ma banque », pas « le
 *     champ 27 » ;
 *   - les structures se saisissent case par case (`structureDecrite`), une case
 *     par valeur, un « + » pour une liste. Les clefs qu'on ne montre pas
 *     (`_note`, `nom_aem`...) sont relues telles quelles a l'enregistrement
 *     — voir `lireFormulaire` : on ne perd pas ce qu'on n'affiche pas ;
 *   - ce que la machine DEDUIT ne se saisit pas : le SIREN et le NIC sont
 *     les 9 et les 5 chiffres du SIRET, poses en cases cachees (CACHES) ;
 *   - l'aide tient derriere un « ? » : presente pour qui la cherche,
 *     absente pour qui ne la cherche pas. Et l'etiquette « déclaré » ne
 *     s'affiche plus : savoir ce qui part en DSN est le travail du moteur.
 *
 *  `donnees/champs-base.json` (lu par `champs.rs`) reste la seule description des champs : ces tables ne
 *  disent que l'ORDRE et la MISE EN PAGE. Un champ nouveau qui n'y figure
 *  pas s'affiche quand meme, dans la derniere section.
 */
const SECTIONS_FICHE = {
  // LA FICHE ENTREPRISE : on demande le minimum d'information aux gens, puis on traite l'information pour qu'elle rentre
  // dans la DSN. Six blocs, dans l'ordre ou l'on cherche : qui
  // l'on est, ou l'on est, sous quelle convention, ses numeros, qui signe,
  // le compte. Les cases que le moteur deduit sont nommees ici pour que
  // leur case cachee soit posee dans le bloc de la valeur dont elles
  // viennent (voir CACHES).
  entreprise: [
    // La date de la première embauche sort de l'« Avancé » :
    // pour une compagnie de moins de cinq ans, sans elle l'effectif est
    // inconnu et le premier mois ne se calcule pas.
    ["La compagnie", ["nom_affichage", "siret", "siren", "nic", "naf",
                      "licence_spectacle", "premiere_embauche"]],
    ["Adresse", ["adresse_affichage", "code_postal", "commune_affichage"]],
    ["Convention collective", ["idcc",
                               "annexe_convention",
                               "jauge_annexe_1", "type_radio_1922",
                               "cinquante_salaries_3241"]],
    // LE CURSEUR DES PERMANENTS SORT DE L'« AVANCÉ ».
    // C'est la question qui commande tout ce qui ne vaut que pour les
    // salariés au mois (natures CDI/CDD, santé et prévoyance, organismes) :
    // elle se pose en clair. Le moteur le lit (`contexte::emploie_des_permanents`).
    // Les conseils « Le saviez-vous ? » :
    // une option qui parle de vos salariés, elle se pose avec eux.
    ["Vos salariés", ["emploie_des_permanents", "conseils"]],
    // Le taux AT/MP se pose en tête de ce bloc (`ATMP_HTML`).
    // LE CODE DU SERVICE DE SANTÉ AU TRAVAIL N'EST PAS DANS L'« AVANCÉ » (le tiroir
    // replié) : chaque DPAE l'exige (`dpae::medecine_de`) et
    // « Pour démarrer » le demande. Un champ exigé est visible là où on l'exige.
    ["Numéros chez les organismes",
     ["urssaf_compte", "congesspectacles_adherent",
      "affiliation_france_travail",
      "france_travail_dossier_recouvrement", "medecine_travail"], { atmp: true }],
    // Le contact de la médecine du travail interne ne se montre que si le code du
    // service de santé au travail vaut MT01 (`si` de champs-base.json) ; la DSN
    // l'écrit alors en S20.G00.07 de type 17.
    ["Qui signe, qui répond", ["signataire", "declarant_dsn", "contact_sante_travail"]],
    ["Paiement des cotisations", ["banque"]],
    // LA TAXE SUR LES SALAIRES : la DSN de décembre
    // déclare chaque année si la compagnie la doit (bloc S21.G00.44), et ne
    // se fait pas sans réponse. Le moteur ne connaît pas sa TVA.
    ["Taxe sur les salaires", ["taxe_sur_les_salaires"]],
    // L'AVANCE. L'editeur : « si on peut deviner que c'est le
    // truc de base, on le met en truc de base ; si les gens veulent changer,
    // ils cliquent sur avancé ». Chacun de ces champs a une valeur par
    // defaut qui est le cas ordinaire d'une compagnie : pas de salarie au
    // mois, pas de syndicat, pas de CSE, pas d'exemption, et un effectif lu
    // dans les retours de l'Urssaf plutot qu'une date de premiere embauche.
    // Le bloc s'ouvre de lui-meme si l'un d'eux s'ecarte du cas ordinaire.
    // Chaque case suit ce qui la commande : les fériés chômés
    // et la prévoyance ne se montrent qu'avec des permanents, la date
    // d'adhésion qu'avec un syndicat, le CSE qu'en CCNEAC, l'exemption
    // CASC-SVP qu'en 3090, l'article 45 qu'au cinéma.
    ["Avancé — cas particuliers", ["jours_feries_chomes", "plafond_temps_partiel",
      "deduction_forfaitaire_option",
      "prevoyance_permanents", "sante_permanents",
      "organisation_patronale", "organisation_patronale_depuis",
      "representation_personnel", "casc_svp_exemptee",
      "prevoyance_techniciens_3097",
      "contribution_supplementaire_apprentissage",
      "taxe_apprentissage_motif_non_assujettissement",
      "code_urssaf", "retraite_complementaire", "etablissements"],
     { avance: true }],
    // Sans titre : l'historique se replie derriere un lien (`histoireDuChamp`).
    ["", ["historique"]],
  ],
  // LA FICHE SALARIE, sur le modele de l'entreprise.
  // Dans l'ordre ou on la remplit, papiers en main : la carte Vitale, la
  // piece d'identite, l'adresse ; puis ce que l'employeur decide, l'emploi.
  // Chaque champ est lu par le moteur : le taux de
  // PAS de la fiche ne se montre pas ; le sexe
  // ne sert que sans NIR, il ne se montre que dans ce cas (`brancherSexe`).
  salarie: [
    // Le numero et sa cle cote a cote, comme sur la carte Vitale.
    ["Identité", ["prenom_affichage", "nom_affichage", "nir", "nir_cle",
                  "date_naissance", "sexe"]],
    ["Naissance et nationalité", ["lieu_naissance_affichage",
      "departement_naissance", "pays_naissance", "pays_nationalite",
      "pays_nationalite_hors_liste", "code_nationalite_dsn"]],
    ["Coordonnées", ["adresse_affichage", "code_postal", "commune_affichage",
                     "email"]],
    // LE COMPTE OÙ LE SALAIRE EST VIRÉ : l'IBAN, dont le moteur contrôle la clé, et le BIC.
    ["Paiement du salaire", ["banque"]],
    // LE MÉTIER SE CHOISIT, SES CODES SE DÉDUISENT. La liste
    // des métiers cochés (Configuration › Métiers) se pose en tête du bloc
    // (`METIER_HTML`) ; l'intitulé reste à portée de main — c'est lui que
    // lit le contrat. Statut, cadre, PCS-ESE et code France Travail se
    // remplissent au choix du métier : ils passent dans l'« Avancé ».
    ["Emploi", ["emploi_affichage"], { metier: true }],
    ["Intermittence", ["num_conges_spectacles", "anniversaire_droits"]],
    // Rien a remplir tant que la compagnie n'emploie pas de salarie au mois.
    ["Salarié au mois", ["jours_travailles", "sante_dispense", "sante_dispense_depuis",
                         "sante_dispense_jusqu_au", "prevoyance_branche_depuis",
                         "categorie_conventionnelle", "conges_reprise",
                         "forfait_jours"],
     { sante: true }],
    // L'avance : ce qui a une valeur par defaut juste pour presque tout le
    // monde. Le tiroir s'ouvre seul si l'une s'en ecarte (`ORDINAIRE`).
    // La déduction forfaitaire spécifique : un tiroir à elle, qui dit ce que
    // l'entreprise en a décidé (`DFS_HTML`).
    ["Déduction forfaitaire spécifique", ["deduction_forfaitaire",
      "deduction_forfaitaire_accord"], { avance: true, dfs: true }],
    ["Avancé — cas particuliers", ["statut", "statut_categoriel_rc",
      "pcs_ese", "code_emploi_ft", "date_anciennete", "code_salarie",
      "initiales_contrat", "statut_conventionnel", "statut_boeth", "echelon_ccneac", "classement_conventionnel",
      "poste_depuis", "experimente_3241", "stagiaire_ccneac",
      "actif"], { avance: true }],
    ["", ["historique"]],
  ],
};

/* Les jours de la semaine, tels que la fiche d'un permanent les ecrit (`droit::conges::JOURS_DE_LA_SEMAINE`). */
const JOURS_DE_LA_SEMAINE = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];

/* Ce qui, dans le tiroir « Avancé », vaut le cas ordinaire et ne l'ouvre
 * donc pas. Une case absente d'ici ouvre le tiroir des qu'elle est remplie. */
const ORDINAIRE = {
  code_urssaf: (v) => String(v) === String(ETAT.urssaf_deduit || ""),
  retraite_complementaire: (v) => String(v).trim().toUpperCase() === "AUDIENS",
  // Toujours remplis, et c'est normal : un matricule, un prefixe, une date.
  code_salarie: () => true,
  initiales_contrat: () => true,
  date_anciennete: () => true,
  actif: (v) => v !== false,
  statut_conventionnel: (v) => String(v) === "05",
  // Ce que le métier remplit : toujours là, et l'« Avancé » n'a pas à
  // s'ouvrir pour eux.
  statut: () => true,
  statut_categoriel_rc: () => true,
  pcs_ese: () => true,
  code_emploi_ft: () => true,
};

/* Les champs que la machine deduit d'un autre, ou qu'elle connait sans
 * qu'on les lui dise : ils restent dans le formulaire — donc ils repartent
 * a l'enregistrement avec leur valeur — mais en case cachee.
 *
 *  - `siren`, `nic` : le SIRET (le moteur, `routes::fiches::deriver`).
 *
 *  Le code Urssaf et la caisse de retraite, eux, sont de VRAIS choix : ils
 *  vont dans l'avance, vides par defaut -- le moteur prend alors l'Urssaf
 *  du code postal et Audiens. L'editeur : « il ne faut pas enlever de
 *  l'interface les choix que peut faire l'entreprise ». */
const CACHES = {
  entreprise: ["siren", "nic"],
  // `sante_svp` se remplit par les trois cases de la complementaire sante : gardé tel quel, jamais demandé.
  salarie: ["sante_svp"],
};

/* Ou aller chercher une valeur qu'on n'a pas sous la main : une phrase et
 * un lien, derriere le « ? » du champ. VIDE : les liens menaient a la page d'accueil d'un organisme, ce qui n'apprend rien.
 * L'aide de chaque champ (`donnees/champs-base.json`) dit ou lire la
 * valeur. Un lien ne revient ici que s'il mene a la page qui la donne. */
const OU_TROUVER = {};

/* Les structures telles qu'elles etaient AVANT la saisie : `lireFormulaire`
 * en repart pour ne pas perdre les clefs qu'on n'affiche pas. */
let STRUCTURES_ORIGINE = {};

/* L'aide derriere un « ? » : le bouton dans le libelle, le texte dessous,
 * replie. `OU_TROUVER` y ajoute une phrase et un lien quand la valeur se
 * demande a quelqu'un d'autre qu'a soi-meme. */
function aideHtml(cle, aide) {
  return (aide || OU_TROUVER[cle])
    ? `<button type="button" class="aide-bouton" data-aide="${echapper(cle)}"
         title="aide">?</button>` : "";
}

function aideCorps(cle, aide) {
  const ou = OU_TROUVER[cle];
  if (!aide && !ou) return "";
  return `<div class="aide" data-aide-corps="${echapper(cle)}" hidden>
    ${ou ? `<p class="ou-trouver">${echapper(ou[0])}
        <a href="${echapper(ou[1])}" target="_blank" rel="noopener">${echapper(ou[1])}</a></p>` : ""}
    ${aide ? `<p>${echapper(aide)}</p>` : ""}
  </div>`;
}

/*  UNE STRUCTURE SE SAISIT CASE PAR CASE, PAS EN ACCOLADES.
 *
 *  Deux formes : un objet (le signataire, la banque) rendu en une rangee de
 *  cases ; une liste (les licences, les etablissements) rendue en lignes,
 *  avec un « + » pour en ajouter une et une croix pour en retirer une. Ce
 *  que la table ne nomme pas n'est pas affiche, et n'est pas perdu.
 */
function structureHtml(c, v, sous, a, avecPermanents) {
  const cache = c.si_permanents && !avecPermanents;
  const tete = `<label class="titre-structure">${echapper(c.libelle)}
      ${aideHtml(c.cle, c.aide)}</label>
    ${aideCorps(c.cle, c.aide)}`;
  let corps;
  if (sous.liste) {
    const lignes = Array.isArray(v) ? v : (v ? [v] : []);
    corps = `<div class="liste-structure" data-liste="${echapper(c.cle)}">
        ${lignes.map((l, i) => ligneStructure(c.cle, sous, l, i)).join("")}
      </div>
      <button type="button" class="secondaire ajouter-ligne"
        data-ajouter="${echapper(c.cle)}">+ Ajouter un ${echapper(sous.ligne || "élément")}</button>`;
  } else {
    const o = (v && typeof v === "object" && !Array.isArray(v)) ? v : {};
    corps = `<div class="sous-champs">${sous.champs.map(([k, lib, aide, genre = {}]) => {
      const bouton = aide ? `<button type="button" class="aide-bouton"
             data-aide="${echapper(c.cle + "." + k)}" title="aide">?</button>` : "";
      // Des boutons à cocher (`boutons`) : chaque réponse est une phrase
      // entière, qu'une liste déroulante couperait. Le libellé
      // n'est pas un <label> : il en contiendrait plusieurs.
      const haut = genre.boutons
        ? `<div class="libelle-boutons">${echapper(lib)} ${bouton}</div>
           ${caseSousChamp(c.cle, k, valeurEn(o, k), genre)}`
        : `<label>${echapper(lib)}
          ${bouton}
          ${caseSousChamp(c.cle, k, valeurEn(o, k), genre)}</label>`;
      return `
      <div class="sous-champ${genre.boutons ? " sous-champ-large" : ""}">
        ${haut}
        ${aide ? `<div class="aide" data-aide-corps="${echapper(c.cle + "." + k)}" hidden>
            <p>${echapper(aide)}</p></div>` : ""}
      </div>`;
    }).join("")}</div>`;
  }
  return `<div class="champ pleine structure${c.si_permanents ? " champ-permanents" : ""}"${
      cache ? " hidden" : ""}>
    ${tete}
    ${corps}
    ${a ? `<div class="anomalie ${a.gravite}">${echapper(a.message)}</div>` : ""}
  </div>`;
}

/* Une valeur dans un objet, par son chemin : « taux.t1_cadre ». */
function valeurEn(o, chemin) {
  return chemin.split(".").reduce((x, k) => (x && typeof x === "object") ? x[k] : undefined, o);
}

/* La case d'un sous-champ : une liste de choix, un nombre ou du texte. Les
 * deux premiers portent `data-sous-type` : vides, ils sont retires a
 * l'enregistrement (`lireFormulaire`). */
function caseSousChamp(parent, k, val, genre) {
  const attrs = `data-sous-de="${echapper(parent)}" data-sous-cle="${echapper(k)}"`;
  if (genre.choix && genre.boutons) {
    // Les boutons à cocher : la valeur est dans une case cachée, que tout
    // le reste de la page lit comme une liste de choix (`lireFormulaire`) ;
    // cocher l'écrit (`cocherUnBouton`). Le choix vide (« à dire ») n'est
    // pas un bouton : rien de coché.
    const type = genre.booleen ? "booleen" : "choix";
    const nom = `bouton-${parent}-${k}`;
    return `<div class="boutons-choix" role="radiogroup">
      <input type="hidden" ${attrs} data-sous-type="${type}" value="${echapper(String(val ?? ""))}">
      ${Object.entries(genre.choix).filter(([v]) => v !== "").map(([v, lib]) => `
      <label class="coche"><input type="radio" name="${echapper(nom)}" value="${echapper(v)}"
        ${String(val ?? "") === v ? "checked" : ""}>
        <span data-choix-libelle="${echapper(v)}">${echapper(lib)}</span></label>`).join("")}
    </div>`;
  }
  if (genre.choix) {
    // Un oui/non (`booleen`) se choisit comme un choix et s'enregistre en
    // vrai booleen (`lireFormulaire`).
    const type = genre.booleen ? "booleen" : "choix";
    return `<select ${attrs} data-sous-type="${type}">${Object.entries(genre.choix).map(([v, lib]) =>
      `<option value="${echapper(v)}" ${String(val ?? "") === v ? "selected" : ""}>${echapper(lib)}</option>`).join("")}</select>`;
  }
  if (genre.nombre) {
    return `<input type="number" step="0.01" min="0" ${attrs} data-sous-type="nombre"
      value="${echapper(val ?? "")}">`;
  }
  // Une date se choisit dans un calendrier, comme partout ; la valeur
  // reste AAAA-MM-JJ. Une valeur ancienne d'une autre forme reste un texte.
  if (genre.date && /^(\d{4}-\d{2}-\d{2})?$/.test(String(val ?? ""))) {
    return `<input type="date" ${attrs} value="${echapper(val ?? "")}">`;
  }
  return `<input ${attrs} value="${echapper(val ?? "")}">`;
}

/* Cocher un bouton écrit sa valeur dans la case cachée du groupe, et la
 * case le dit comme si on l'avait tapée (`input`). En capture : la case est
 * à jour avant que le bouton lui-même ne relise le formulaire. */
function cocherUnBouton(e) {
  const r = e.target;
  if (!(r instanceof HTMLInputElement) || r.type !== "radio") return;
  const groupe = r.closest(".boutons-choix");
  const caseCachee = groupe && groupe.querySelector('input[type="hidden"][data-sous-cle]');
  if (!caseCachee) return;
  caseCachee.value = r.value;
  caseCachee.dispatchEvent(new Event("input", { bubbles: true }));
}
document.addEventListener("input", cocherUnBouton, true);

function ligneStructure(cle, sous, ligne, i) {
  const o = ligne && typeof ligne === "object" ? ligne : {};
  return `<div class="ligne-structure" data-index="${i}">
    ${sous.champs.filter(([, , , genre = {}]) => !genre.si || (ETAT.configuration || {})[genre.si]).map(([k, lib, aide, genre = {}]) => {
      const val = valeurEn(o, k);
      const attrs = `data-liste-de="${echapper(cle)}" data-liste-index="${i}"
          data-sous-cle="${echapper(k)}" ${aide ? `placeholder="${echapper(aide)}"` : ""}`;
      const v = echapper(val ?? "");
      const champ = genre.date && /^(\d{4}-\d{2}-\d{2})?$/.test(String(val ?? ""))
        ? `<input type="date" ${attrs} value="${v}">`
        : genre.codesAtmp ? `<input ${attrs} list="codes-atmp" maxlength="7" value="${v}">`
        : genre.decimal ? `<input ${attrs} inputmode="decimal" value="${v}">`
        : `<input ${attrs} value="${v}">`;
      return `<label>${echapper(lib)} ${champ}</label>`;
    }).join("")}
    <button type="button" class="retirer-ligne" title="retirer cette ligne">✕</button>
    ${o.nic ? `<p class="aide ligne-dit" data-ligne-nic="${echapper(o.nic)}"></p>` : ""}
  </div>`;
}

/* La fiche que le formulaire montre en ce moment : `quitterLaFiche` en a
 * besoin pour pouvoir l'enregistrer avant de partir. */
let FICHE_COURANTE = null;

/* LE MÉTIER, en tête du bloc « Emploi ». Un seul
 * déroulant, qui montre le métier de la fiche (son code France Travail). La liste est celle des métiers cochés ;
 * on en ajoute dans l'onglet Métiers, pas ici. */
function METIER_HTML(fiche) {
  const code = ((fiche || {}).donnees || {}).code_emploi_ft || "";
  return `<div class="champ" id="config-metier">
    <label>Métier
      <select id="config-metier-choix">${optionsMetiers(code, "— choisir —")}</select></label>
    <div class="discret petit"><button type="button" class="lien" id="aller-metiers">Ajouter un métier</button></div>
  </div>`;
}

/* CE QU'UN SPECTACLE, OU UN SALARIÉ, A COÛTÉ DANS L'ANNÉE.
 * L'éditeur : « si un administrateur veut savoir combien un de ses
 * spectacles lui a coûté dans l'année, il faut qu'il le trouve
 * facilement ». La réponse était dans Exports, un état parmi vingt-deux ;
 * elle est en tête de la fiche. Le calcul est celui du livre de
 * paie (route « couts ») : une fois par année et par séance, oublié dès
 * qu'une saisie s'enregistre. */
const COUTS = {};
function coutsDe(annee) {
  if (!COUTS[annee]) COUTS[annee] = api("couts", { annee }).catch(() => null);
  return COUTS[annee];
}
function oublierCouts() { Object.keys(COUTS).forEach((k) => delete COUTS[k]); }

async function peindreCoutAnnee() {
  const el = document.querySelector("#config-formulaire .cout-annee");
  if (!el) return;
  const an = anneeDuJour();
  const ligne = (r, annee) => {
    const d = r && (r[el.dataset.cote] || {})[el.dataset.cle];
    if (!d || (!d.bulletins && !Number(d.prevu))) return "";
    return `<div><b>${annee}</b> : ${echapper(d.phrase)}</div>`;
  };
  el.innerHTML = `<span class="discret">Coût…</span>`;
  const [cette, avant] = await Promise.all([coutsDe(an), coutsDe(an - 1)]);
  if (!el.isConnected) return;
  el.innerHTML = ligne(cette, an) + ligne(avant, an - 1);
}

/* L'ONGLET MÉTIERS. L'éditeur : « les métiers, c'est vraiment
 * un onglet à part : on a les métiers de base de notre convention, on peut
 * en rajouter » — et les cocher ou les décocher. Ce qui est coché se
 * propose dans la fiche d'un salarié et dans un engagement ; rien d'autre
 * ne change, aucun calcul. Par famille, repliée, avec son compte ; la
 * recherche ouvre ce qu'elle trouve. Les métiers des autres conventions ne
 * se montrent que si on le demande. */
let METIERS_TOUS = false;
let METIERS_FILTRE = "";

async function dessinerMetiers(cible) {
  let r;
  try { r = await api("metiers-catalogue"); }
  catch (e) { cible.innerHTML = `<p class="discret">${echapper(e.message)}</p>`; return; }
  const sansAccent = (t) => String(t || "").normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
  const peindre = () => {
    const filtre = sansAccent(METIERS_FILTRE.trim());
    const vus = r.metiers.filter((m) => (METIERS_TOUS || m.de_la_convention || m.retenu)
      && (!filtre || sansAccent(m.libelle).includes(filtre) || sansAccent(m.code).includes(filtre)));
    const familles = [...new Set(vus.map((m) => m.famille))];
    const retenus = r.metiers.filter((m) => m.retenu).length;
    document.getElementById("metiers-compte").textContent =
      `${retenus} métier${retenus > 1 ? "s" : ""} coché${retenus > 1 ? "s" : ""}`;
    document.getElementById("metiers-liste").innerHTML = familles.length ? familles.map((f) => {
      const dedans = vus.filter((m) => m.famille === f);
      const n = dedans.filter((m) => m.retenu).length;
      return `<details class="metiers-famille"${filtre ? " open" : ""}>
        <summary><b>${echapper(f)}</b> <span class="discret">${n} coché${n > 1 ? "s" : ""} sur ${dedans.length}</span></summary>
        <div class="metiers-colonnes">${dedans.map((m) => `
          <label class="coche" title="${echapper(m.code)}${m.de ? " — " + echapper(m.de) : ""}">
            <input type="checkbox" data-code="${echapper(m.code)}"${m.retenu ? " checked" : ""}>
            ${echapper(m.libelle)}${m.de ? ` <span class="discret">· autre convention</span>` : ""}
          </label>`).join("")}</div>
      </details>`;
    }).join("") : `<p class="discret">Aucun métier</p>`;
    document.querySelectorAll("#metiers-liste input[data-code]").forEach((c) => {
      c.onchange = async () => {
        try {
          const x = await api("metier-retenu", { code: c.dataset.code, retenu: c.checked });
          ETAT.emplois = x.emplois;
          ETAT.emplois_autres = x.emplois_autres;
          const m = r.metiers.find((y) => y.code === c.dataset.code);
          if (m) m.retenu = c.checked;
          const garde = [...document.querySelectorAll("#metiers-liste details[open] summary b")].map((b) => b.textContent);
          peindre();
          document.querySelectorAll("#metiers-liste details").forEach((d) => {
            if (garde.includes(d.querySelector("summary b").textContent)) d.open = true;
          });
        } catch (e) { c.checked = !c.checked; dire(e.message, "erreur"); }
      };
    });
  };
  cible.innerHTML = `
    <div class="carte">
      <h2>Métiers de la compagnie</h2>
      <p class="note">${echapper(r.phrase)}</p>
      <div class="metiers-outils">
        <input type="search" id="metiers-filtre" placeholder="Chercher"
               value="${echapper(METIERS_FILTRE)}">
        <label class="coche"><input type="checkbox" id="metiers-tous"${METIERS_TOUS ? " checked" : ""}>
          autres conventions aussi</label>
        <span id="metiers-compte" class="discret"></span>
      </div>
      <div id="metiers-liste"></div>
    </div>`;
  document.getElementById("metiers-filtre").oninput = (e) => { METIERS_FILTRE = e.target.value; peindre(); };
  document.getElementById("metiers-tous").onchange = (e) => { METIERS_TOUS = e.target.checked; peindre(); };
  peindre();
}

/* LES ORGANISMES ET LES CONTRATS DE LA DSN. Une ligne par
 * contrat que la convention exige ; le moteur (`organismes-contrats`) dit ce
 * qui est fixé par la branche et ce que la compagnie saisit. Enregistrer
 * écrit ce que la compagnie saisit (`organismes-enregistrer`) : la page ne
 * décide rien. */
async function dessinerOrganismes(cible) {
  let r;
  // Les contrats de la periode regardee : le mois de l'onglet Paie, sinon le moteur prend celui du jour.
  const mois = typeof MOIS_AFFICHE === "string" && MOIS_AFFICHE ? { mois: MOIS_AFFICHE } : {};
  try { r = await api("organismes-contrats", mois); }
  catch (e) { cible.innerHTML = `<p class="discret">${echapper(e.message)}</p>`; return; }
  const champ = (c, k, fixe, place) => fixe
    ? `<span>${echapper(c[k]) || "—"}</span>`
    : `<input type="text" data-contrat="${echapper(c.cle)}" data-champ="${k}" value="${echapper(c[k])}"
         placeholder="${place}" maxlength="${k === "libelle" ? 30 : 9}">`;
  const ligne = (c) => `
    <tr>
      <td><b>${echapper(c.quoi)}</b>
        ${c.periode ? `<div class="note">${echapper(c.periode)}</div>` : ""}
        ${c.note ? `<div class="note">${echapper(c.note)}</div>` : ""}</td>
      <td>${echapper(c.beneficiaires)}</td>
      <td>${champ(c, "organisme", c.organisme_fixe, "P0983")}</td>
      <td>${champ(c, "libelle", c.libelle_fixe, "référence du contrat")}</td>
    </tr>`;
  cible.innerHTML = `
    <div class="carte">
      <h2>Organismes et contrats DSN</h2>
      ${encarts(r.encarts)}
      <table class="tableau">
        <thead><tr><th>Contrat</th><th>Bénéficiaires</th><th>Organisme</th><th>Référence du contrat</th></tr></thead>
        <tbody>${r.contrats.map(ligne).join("")}</tbody>
      </table>
      <div class="actions"><button id="organismes-enregistrer">Enregistrer</button></div>
    </div>`;
  document.getElementById("organismes-enregistrer").onclick = async () => {
    const contrats = {};
    cible.querySelectorAll("input[data-contrat]").forEach((i) => {
      (contrats[i.dataset.contrat] ||= {})[i.dataset.champ] = i.value.trim();
    });
    try {
      const x = await api("organismes-enregistrer", { contrats, ...mois });
      dire(x.phrase);
      dessinerOrganismes(cible);
    } catch (e) { dire(e.message, "erreur"); }
  };
}

/* LA DÉDUCTION FORFAITAIRE SPÉCIFIQUE, vue depuis la fiche d'un salarié : ce
 * que la fiche de l'entreprise en dit (l'option est celle de l'employeur), et
 * le chemin pour la régler. La page nomme l'option, le moteur décide
 * (`calcul::deduction_forfaitaire`). */
function DFS_HTML() {
  return `<div class="encart" id="config-dfs">
    <p>${echapper((ETAT.configuration || {}).deduction_forfaitaire)}</p>
    <div class="actions"><button type="button" class="petit secondaire" onclick="ouvrirOptionDfs()">Fiche de l'entreprise</button></div>
  </div>`;
}

/* Ouvre la fiche de l'entreprise sur l'option de la déduction forfaitaire. */
async function ouvrirOptionDfs() {
  await ouvrirFicheImportee("entreprise");
  const champ = document.querySelector('[data-cle="deduction_forfaitaire_option"]');
  if (!champ) return;
  const tiroir = champ.closest("details");
  if (tiroir) tiroir.open = true;
  champ.scrollIntoView({ behavior: "smooth", block: "center" });
  champ.focus();
}

/* LE CODE RISQUE ET LE TAUX AT/MP, en tête du bloc « Vos numéros chez les
 * organismes ». Ils ne sont pas des champs de la fiche : ils vivent dans le
 * barème de la compagnie (`config/baremes/<année>.json`). Le moteur
 * (`taux-atmp`) dit ce qui est posé, le risque tel que l'arrêté l'écrit,
 * pourquoi, où le lire, où le changer, et les codes qu'il connaît ; la page
 * les affiche. « Avancé » les change : le moteur refuse ce qui ne va pas. */
function ATMP_HTML() {
  const an = anneeDuJour();
  return `<div class="encart" id="config-atmp" data-annee="${an}">
    <h4>Accidents du travail (AT/MP) ${an}</h4>
    <p id="atmp-resume"></p>
    <p class="note" id="atmp-libelle"></p>
    <p class="aide" id="atmp-explication"></p>
    <p class="aide" id="atmp-ou"></p>
    <details id="atmp-avance"><summary>Avancé</summary>
      <div class="ligne-champs">
        <label>Code risque notifié
          <select id="atmp-choix" class="hors-fiche"></select></label>
        <label id="atmp-autre-l">Autre code
          <input id="atmp-code" class="hors-fiche" maxlength="7"></label>
        <label>Taux notifié (%)
          <input id="atmp-valeur" class="hors-fiche" inputmode="decimal"></label>
      </div>
      <div class="aide" id="atmp-aide"></div>
      <div class="actions"><button class="petit" id="atmp-enregistrer">Enregistrer</button>
        <span id="atmp-etat"></span></div>
    </details>
  </div>`;
}

async function brancherAtmp() {
  const bloc = document.getElementById("config-atmp");
  if (!bloc) return;
  const annee = Number(bloc.dataset.annee);
  const $ = (id) => document.getElementById(id);
  const [choix, code, valeur, etat] = ["atmp-choix", "atmp-code", "atmp-valeur", "atmp-etat"].map($);
  // Ces cases ne sont pas la fiche : elles ne la marquent pas modifiée.
  [choix, code, valeur].forEach((c) => { c.oninput = null; });
  const peindre = (r) => {
    if (!r) return;
    const t = r.taux || {};
    $("atmp-resume").innerHTML = r.resume ? `<b>${echapper(r.resume)}</b>` : "";
    $("atmp-libelle").textContent = r.risque ? `« ${r.risque.libelle} »` : "";
    $("atmp-explication").textContent = r.explication || "";
    $("atmp-aide").textContent = r.aide || "";
    $("atmp-ou").textContent = [r.ou_le_lire, r.ou_le_changer].filter(Boolean).join(" ");
    const codes = r.codes || [];
    const connu = codes.some((c) => c.code === t.code_risque);
    choix.innerHTML = codes.map((c) =>
      `<option value="${echapper(c.code)}">${echapper(c.code_arrete || c.code)} — ${echapper(c.libelle)}</option>`).join("")
      + `<option value="">Un autre code notifié</option>`;
    choix.value = connu ? t.code_risque : "";
    if (document.activeElement !== code) code.value = connu ? "" : (t.code_risque || "");
    if (document.activeElement !== valeur) valeur.value = "";
    // Les codes que les autres établissements proposent, et ce que le
    // moteur applique à chacun, sous sa ligne.
    let liste = document.getElementById("codes-atmp");
    if (!liste) {
      liste = document.createElement("datalist");
      liste.id = "codes-atmp";
      document.body.appendChild(liste);
    }
    liste.innerHTML = codes.map((c) =>
      `<option value="${echapper(c.code)}">${echapper(c.code_arrete || c.code)} — ${echapper(c.libelle)}</option>`).join("");
    (r.etablissements || []).forEach((e) => {
      const p = document.querySelector(`[data-ligne-nic="${CSS.escape(String(e.nic || ""))}"]`);
      if (p) p.innerHTML = `<b>${echapper(e.resume || "")}</b>${e.risque ? ` — « ${echapper(e.risque.libelle)} »` : ""}. ${echapper(e.explication || "")}`;
    });
  };
  $("atmp-enregistrer").onclick = async (ev) => {
    ev.preventDefault();
    try {
      const r = await api("taux-atmp", { annee, code_risque: choix.value || code.value,
                                         valeur: valeur.value });
      peindre(r);
      etat.textContent = r.phrase;
      valeur.classList.remove("faute");
    } catch (e) {
      etat.textContent = e.message;
      valeur.classList.add("faute");
    }
  };
  // La fiche quittée avant la réponse n'a plus ces cases : rien à peindre.
  try { const r = await api("taux-atmp", { annee }); if (bloc.isConnected) peindre(r); } catch (e) { /* rien à montrer */ }
}

/* LA COMPLÉMENTAIRE SANTÉ D'UN PERMANENT, en tete du bloc
 * « Salarié au mois ». Elle n'est pas un taux de barème : c'est un forfait
 * en pourcentage du PMSS, et il dépend de deux choses que seul le bulletin
 * d'adhésion du salarié dit. Trois cases plutôt qu'un JSON à taper ; le
 * champ `sante_svp` reste dans la fiche, en case cachée, et ces cases
 * l'écrivent (`brancherSante`). */
function SANTE_HTML() {
  // Sous une convention qui ne la fait pas choisir sur la fiche, le moteur dit
  // d'où elle vient (`sante_hors_fiche`) : rien à remplir ici.
  if (ETAT.sante_hors_fiche) {
    return `<div class="encart" id="config-sante">
      <div class="aide">${echapper(ETAT.sante_hors_fiche)}</div>
    </div>`;
  }
  return `<div class="encart" id="config-sante">
    <label>Complémentaire santé
      <select id="config-sante-niveau">
        <option value="">— pas encore dite —</option>
        ${(ETAT.niveaux_sante || []).map((n) =>
          `<option value="${echapper(n.niveau)}">${echapper(n.libelle)}</option>`).join("")}
      </select></label>
    <label class="coche"><input type="checkbox" id="config-sante-conjoint"> conjoint couvert</label>
    <label>Enfants couverts
      <input type="number" min="0" step="1" id="config-sante-enfants" value="0"></label>
    <div class="aide">${echapper(ETAT.sante_aide)}</div>
  </div>`;
}

/* LE HAUT D'UNE FICHE SALARIE : son nom en clair, et deux
 * gestes. « Exporter ses documents » mene a l'onglet Exports, le dossier de
 * ce salarie deja choisi ; une fiche neuve propose de la faire remplir par
 * la personne elle-meme, par courriel (`demanderFiche`). */
function enteteSalarie(fiche) {
  const d = fiche.donnees || {};
  const nom = `${d.prenom_affichage || d.prenom || ""} ${d.nom_affichage || d.nom || ""}`.trim();
  if (fiche.cle) {
    return `<div class="entete-fiche">
      <h2>${echapper(nom || fiche.cle)}
        ${d.code_salarie ? `<span class="discret">— matricule ${echapper(d.code_salarie)}</span>` : ""}</h2>
      <button type="button" class="secondaire petit" id="config-exporter">Exporter ses documents</button>
    </div>
    <div class="cout-annee" data-cote="salaries" data-cle="${echapper(fiche.cle)}"></div>
    <!-- SES IMPRÉVUS : ses arrêts et ses absences, passés et en
         cours, ce qui en est parti, et la porte « Un imprévu… ». js/imprevus.js. -->
    <div class="fiche-imprevus" id="fiche-imprevus"></div>
    <!-- CE QUE VOS CONTRATS FONT POUR SES DROITS : les conseils sur
         l'historique, quand l'option « conseils » est cochée. js/conseils.js. -->
    <div class="fiche-conseils" id="fiche-conseils"></div>
    ${d.fiche_recue && fiche.anomalies.some((a) => a.gravite === "erreur")
      ? `<div class="encart">${echapper(d.fiche_recue)}</div>` : ""}`;
  }
  const lie = !!(ETAT.configuration || {}).boite_liee;
  const inv = (ETAT.configuration || {}).invitation || {};
  return `<h2>Nouveau salarié</h2>
    <div class="encart invitation">
      <h3>À faire remplir</h3>
      <p class="aide">${echapper(inv.aide)}</p>
      ${lie ? `<div class="champs">
        <div class="champ"><label>Son courriel
          <input type="email" id="invitation-courriel" placeholder="prenom.nom@exemple.fr"></label></div>
        <div class="champ"><label>Son prénom
          <input id="invitation-prenom" placeholder="pour dire bonjour"></label></div>
      </div>
      <div class="actions">
        <button type="button" class="primaire" id="invitation-envoyer">Envoyer la demande</button>
      </div>` : `<p class="note">${echapper(inv.sans_boite)}</p>`}
    </div>
    <p class="note">${echapper(inv.ou)}</p>`;
}


/* SUPPRIMER LA COMPAGNIE , sous la fiche Entreprise. Le moteur dit si elle se supprime
   (`supprimable` de la route `etat` : jamais une fictive livrée, qui se remet à zéro) et ce que la boîte demande
   (`compagnie-supprimer`, sans `confirme`) : ce qui sera effacé ; une compagnie réelle, le rappel de la conservation des
   bulletins, la sauvegarde si un Drive est lié, puis le mot de passe du logiciel (le popup de chaque acte qui engage,
   `demanderMotDePasse`). La même route, confirmée, supprime ; la page revient à l'accueil. */
function dessinerSuppression() {
  const zone = document.getElementById("config-suppression");
  if (!zone) return;
  if (FICHE_ACTIVE !== "entreprise" || !ETAT || !ETAT.supprimable) { zone.innerHTML = ""; return; }
  zone.innerHTML = `<div class="encart attention suppression-compagnie">
      <b>Supprimer la compagnie</b>
      ${echapper(ETAT.suppression)}
      <div class="actions"><button type="button" class="secondaire alerte" id="supprimer-compagnie">Supprimer la compagnie</button></div>
    </div>`;
  document.getElementById("supprimer-compagnie").onclick = boiteSuppression;
}

async function boiteSuppression() {
  let q;
  try { q = await api("compagnie-supprimer"); }
  catch (e) { dire(e.message, "erreur"); return; }
  // Une compagnie qui ne se supprime pas : le moteur dit pourquoi.
  if (!q.genre) { dire(q.phrase, "erreur"); return; }
  const reelle = q.genre === "reelle";
  const s = q.sauvegarde || {};
  const sauvegarde = reelle && s.liee
    ? `<p>${echapper(s.phrase)}</p>
       <p><button type="button" class="secondaire" id="sup-sauvegarder">Sauvegarder maintenant</button>
       <span class="discret" id="sup-sauvegarde-etat"></span></p>`
    : "";
  ouvrirBoite(`<h3>Supprimer ${echapper(q.nom)} ?</h3>
    <p>${echapper(q.avertissement)}</p>
    ${reelle ? `<p>${echapper(q.conservation)}</p>` : ""}
    ${sauvegarde}
    <p class="erreur" id="sup-erreur"></p>
    <div class="actions">
      <button type="button" class="secondaire" id="sup-annuler">Annuler</button>
      <button type="button" class="secondaire alerte" id="sup-confirmer">Supprimer définitivement</button>
    </div>`);
  document.getElementById("sup-annuler").onclick = fermerBoite;
  const bSauve = document.getElementById("sup-sauvegarder");
  if (bSauve) bSauve.onclick = async () => {
    const etat = document.getElementById("sup-sauvegarde-etat");
    bSauve.disabled = true;
    etat.textContent = "…";
    try {
      const v = await api("sauvegarde-maintenant");
      etat.textContent = v.phrase;
    } catch (e) { etat.textContent = e.message; }
    bSauve.disabled = false;
  };
  document.getElementById("sup-confirmer").onclick = async () => {
    let mdp = "";
    if (q.mot_de_passe) {
      mdp = await demanderMotDePasse("Supprimer définitivement");
      if (mdp === null) return;
    }
    try {
      await api("compagnie-supprimer", { confirme: true, ...(mdp ? { motdepasse: mdp } : {}) });
      location.replace("/");
    } catch (e) { document.getElementById("sup-erreur").textContent = e.message; }
  };
}
