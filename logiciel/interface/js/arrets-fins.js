/* La reprise du journal, les arrets de travail, les fins de contrat, les embauches. */
"use strict";

/* ------------------------------------------- la reprise du journal
   Le journal des dépôts est né le 11/09/2026 au soir. Tout ce qui est parti
   avant — par un autre logiciel, par le site, ou par nos propres dépôts de la veille —
   n'y figure pas, et l'écran affiche donc « rien n'est parti » sur des actes
   qui ont bel et bien eu lieu. Devant la liste des embauches :

     « je comprends pas pourquoi il veut que je renvoie ça alors qu'on l'a
       déjà envoyé et on a les traces de dépôt il me semble »

   Ce bloc lui rend ce qu'il savait. Il n'apparaît QUE s'il y a quelque chose
   à reprendre : une fois le journal à jour, il disparaît et ne revient que si
   un nouveau dépôt atteste d'un chemin qui n'est pas passé par ici.

   DEUX TEMPS, comme partout ailleurs : on montre d'abord ce qui serait écrit,
   ligne par ligne, et on n'écrit que sur un second clic. Une reprise qui part
   au premier appel est une reprise qu'on lance « pour voir » et qui laisse
   des lignes que personne n'a relues. */
/* ------------------------------------------------- LES ARRÊTS DE TRAVAIL
   Un intermittent tombe malade pendant ses dates. Le contrat est signé, il
   est suspendu, et DEUX déclarations le disent :

     - le SIGNALEMENT d'arrêt de travail (nature 04), dû dans les cinq jours
       qui suivent la prise de connaissance par l'employeur ;
     - le REPORT dans la DSN mensuelle, obligatoire (cahier technique §2.5.2),
       qui se fait tout seul à la production du mois.

   L'écran montre les deux, parce qu'un écran qui n'en montrerait qu'une
   laisserait croire que l'autre est faite. Rien ne part d'ici : produire
   écrit un fichier, et c'est la boîte de dépôt — avec son mot de passe —
   qui l'envoie, exactement comme une DSN. */
let ARRETS = null;          // le dernier /api/arrets lu

/* Le mois principal déclaré présente-t-il une période commune avec l'arrêt ?
   La même règle que le moteur (`signalement.Arret.couvre`), parce que
   l'écran doit montrer d'un mois ce que sa DSN portera — ni plus, ni moins. */
function croiseLeMois(a, mois) {
  if (!mois) return true;
  const derniere = a.reprise || a.fin_previsionnelle;
  return a.dernier_jour_travaille <= mois + "-31" && derniere >= mois + "-01";
}

async function dessinerArrets(mois) {
  const cible = document.getElementById("arrets");
  if (!cible) return;
  try { ARRETS = await api("arrets"); }
  catch (e) { cible.innerHTML = `<p class="discret">${echapper(e.message)}</p>`; return; }
  const duMois = (ARRETS.arrets || []).filter((a) => croiseLeMois(a, mois));
  cible.innerHTML = `
    ${duMois.length ? duMois.map((a) => `
      <div class="rang">
        <span class="rang-nom">${echapper(a.salarie)}</span>
        <span class="pastille ${a.signale ? "bien" : "alerte"}">${a.signale ? "signalé" : "à signaler"}</span>
        <span class="rang-infos">${echapper(a.motif_libelle)} —
          dernier jour travaillé le ${echapper(jjmmaaaa(a.dernier_jour_travaille))},
          ${a.reprise ? `reprise le ${echapper(jjmmaaaa(a.reprise))}`
                      : `fin prévue le ${echapper(jjmmaaaa(a.fin_previsionnelle))}`}
          · contrat ${echapper(a.contrat)}${a.subrogation === "01" ? " · subrogation" : ""}
          ${a.reporte_par.length ? ` · reporté par la DSN ${a.reporte_par.map(deMois).join(", ")}`
                                 : " · à reporter dans la DSN du mois"}</span>
        <button class="secondaire" data-signaler="${echapper(a.salarie)}:${echapper(a.dernier_jour_travaille)}">Signaler…</button>
        <button class="lien" data-reprise="${echapper(a.salarie)}:${echapper(a.dernier_jour_travaille)}">Reprise…</button>
        ${a.signale ? "" : `<button class="lien" data-oublier="${echapper(a.salarie)}:${echapper(a.dernier_jour_travaille)}">Retirer</button>`}
      </div>`).join("")
      : ""}
    <button class="lien" id="ajouter-arret">+ un arrêt de travail</button>`;
  // Le titre ne parle que s'il y a un arrêt : un bloc qui dit « aucun »
  // tous les mois finit par ne plus se lire.
  const titre = document.getElementById("titre-arrets");
  if (titre) titre.hidden = !duMois.length;

  document.getElementById("ajouter-arret").onclick = () => boiteArret(mois);
  cible.querySelectorAll("[data-signaler]").forEach((b) => {
    const [salarie, jour] = b.dataset.signaler.split(":");
    b.onclick = () => boiteSignalerArret(salarie, jour);
  });
  cible.querySelectorAll("[data-reprise]").forEach((b) => {
    const [salarie, jour] = b.dataset.reprise.split(":");
    b.onclick = () => boiteReprise(salarie, jour);
  });
  cible.querySelectorAll("[data-oublier]").forEach((b) => {
    b.onclick = async () => {
      const [salarie, jour] = b.dataset.oublier.split(":");
      try {
        await api("arret-oublier", { salarie, dernier_jour: jour });
        dire("Arrêt retiré — il n'avait jamais été déclaré.");
        await dessinerArrets(MOIS_AFFICHE);
      } catch (e) { dire(e.message, "erreur"); }
    };
  });
}

/** Saisir un arrêt. Le contrat suspendu n'est pas choisi par la page : elle
 *  demande au moteur ceux qui couvrent ce jour-là, et n'en propose pas
 *  d'autre. Sans contrat, il n'y a rien à suspendre, et on le dit. */
async function boiteArret(mois) {
  const motifs = (ARRETS && ARRETS.motifs) || [];
  const salaries = (ETAT && ETAT.salaries) || [];
  ouvrirBoite(`
    <h3>Un arrêt de travail</h3>
    <p class="note">Il suspend un contrat en cours. Deux déclarations
      suivront : le <b>signalement</b>, dû dans les cinq jours, et le report
      dans la <b>DSN du mois</b>, qui se fait à la production.</p>
    <label class="champ-boite">Salarié
      <select id="ar-salarie">
        ${salaries.map((s) => `<option value="${echapper(s.cle)}">${echapper(s.libelle)}</option>`).join("")}
      </select></label>
    <label class="champ-boite">Dernier jour travaillé
      <input type="date" id="ar-dernier"></label>
    <div id="ar-contrats" class="note"></div>
    <label class="champ-boite">Fin prévisionnelle <span class="discret">— celle
      que le médecin a prescrite ; elle est due même sans reprise</span>
      <input type="date" id="ar-fin"></label>
    <label class="champ-boite">Motif
      <select id="ar-motif">
        ${motifs.map((m) => `<option value="${echapper(m.code)}"${m.date_accident ? ' data-accident="1"' : ""}>${echapper(m.libelle)}</option>`).join("")}
      </select></label>
    <label class="champ-boite" id="ar-bloc-accident" hidden>Date de l'accident
      ou de la première constatation
      <input type="date" id="ar-accident"></label>
    <label class="champ-boite">Subrogation
      <select id="ar-subrogation">
        <option value="02">non — le salarié touche ses indemnités journalières</option>
        <option value="01">oui — la compagnie les perçoit et maintient le salaire</option>
      </select></label>
    <div id="ar-bloc-subrogation" hidden>
      <label class="champ-boite">Subrogation du
        <input type="date" id="ar-sub-debut"></label>
      <label class="champ-boite">au
        <input type="date" id="ar-sub-fin"></label>
      <p class="note">L'IBAN et le BIC de la compagnie partiront avec : ils se
        lisent dans la fiche entreprise, ils ne se saisissent pas ici.</p>
    </div>
    <label class="champ-boite">Note <span class="discret">— pour vous, ne part nulle part</span>
      <input type="text" id="ar-note"></label>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="ar-ok">Enregistrer</button>
    </div>
    <p class="note">Rien ne part : l'arrêt est écrit, et le signalement se
      produit ensuite.</p>`);

  const motif = document.getElementById("ar-motif");
  const sub = document.getElementById("ar-subrogation");
  const majMotif = () => {
    const choisi = motif.selectedOptions[0];
    document.getElementById("ar-bloc-accident").hidden = !(choisi && choisi.dataset.accident);
  };
  const majSub = () => {
    document.getElementById("ar-bloc-subrogation").hidden = sub.value !== "01";
  };
  motif.onchange = majMotif;
  sub.onchange = majSub;
  majMotif(); majSub();

  // Les contrats que cet arrêt suspendrait : relus à chaque changement de
  // salarié ou de date, parce que ce sont eux qui décident si l'arrêt peut
  // exister.
  let contrats = [];
  const relire = async () => {
    const salarie = document.getElementById("ar-salarie").value;
    const jour = document.getElementById("ar-dernier").value;
    const bloc = document.getElementById("ar-contrats");
    if (!salarie || !jour) { bloc.innerHTML = ""; return; }
    try {
      const r = await api("arret-contrats", { salarie, jour });
      contrats = r.contrats || [];
    } catch (e) { bloc.innerHTML = `<span class="erreur">${echapper(e.message)}</span>`; return; }
    bloc.innerHTML = contrats.length
      ? (contrats.length === 1
          ? `Contrat suspendu : <b>${echapper(contrats[0].numero)}</b>, du
             ${echapper(jjmmaaaa(contrats[0].debut))} au ${echapper(jjmmaaaa(contrats[0].fin))}.`
          : `Deux contrats couvrent ce jour-là — dire lequel :
             <select id="ar-contrat">${contrats.map((c) =>
               `<option value="${echapper(c.numero)}">${echapper(c.numero)}, du ${echapper(jjmmaaaa(c.debut))} au ${echapper(jjmmaaaa(c.fin))}</option>`).join("")}</select>`)
      : `<span class="erreur">Aucun contrat ne couvre ce jour-là : il n'y a rien
         à suspendre. Vérifier la date, ou saisir le contrat d'abord.</span>`;
  };
  document.getElementById("ar-salarie").onchange = relire;
  document.getElementById("ar-dernier").onchange = relire;
  relire();

  document.getElementById("ar-ok").onclick = async () => {
    const choisi = document.getElementById("ar-contrat");
    try {
      await api("arret-enregistrer", {
        salarie: document.getElementById("ar-salarie").value,
        dernier_jour: document.getElementById("ar-dernier").value,
        fin: document.getElementById("ar-fin").value,
        motif: document.getElementById("ar-motif").value,
        subrogation: document.getElementById("ar-subrogation").value,
        subrogation_debut: document.getElementById("ar-sub-debut").value,
        subrogation_fin: document.getElementById("ar-sub-fin").value,
        date_accident: document.getElementById("ar-accident").value,
        contrat: choisi ? choisi.value : (contrats[0] && contrats[0].numero) || "",
        note: document.getElementById("ar-note").value,
      });
      fermerBoite();
      dire("Arrêt enregistré. Le signalement reste à produire.");
      await dessinerArrets(MOIS_AFFICHE);
    } catch (e) { dire(e.message, "erreur"); }
  };
}

/** La reprise. Elle ne se signale que si elle est ANTICIPÉE ; sinon elle part
 *  avec la DSN du mois, et le moteur le dit plutôt que de laisser produire un
 *  signalement qui n'a pas lieu d'être. */
function boiteReprise(salarie, jour) {
  const motifs = (ARRETS && ARRETS.motifs_reprise) || [];
  ouvrirBoite(`
    <h3>La reprise</h3>
    <p class="note">La date de reprise <b>réelle</b> — le premier jour
      retravaillé. Un signalement de reprise ne s'émet que si elle est
      <b>anticipée</b> par rapport à la fin prévue de l'arrêt ; sinon la DSN
      du mois suffit.</p>
    <label class="champ-boite">Reprise le
      <input type="date" id="re-date"></label>
    <label class="champ-boite">Motif
      <select id="re-motif">
        ${motifs.map((m) => `<option value="${echapper(m.code)}">${echapper(m.libelle)}</option>`).join("")}
      </select></label>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="re-ok">Noter la reprise</button>
    </div>`);
  document.getElementById("re-ok").onclick = async () => {
    try {
      await api("arret-reprise", {
        salarie, dernier_jour: jour,
        reprise: document.getElementById("re-date").value,
        motif_reprise: document.getElementById("re-motif").value,
      });
      fermerBoite();
      dire("Reprise notée.");
      await dessinerArrets(MOIS_AFFICHE);
    } catch (e) { dire(e.message, "erreur"); }
  };
}

/** Produire le signalement, puis le déposer. Deux gestes et deux boîtes :
 *  écrire un fichier n'engage rien, le déposer engage tout. */
function boiteSignalerArret(salarie, jour) {
  const a = ((ARRETS && ARRETS.arrets) || []).find(
    (x) => x.salarie === salarie && x.dernier_jour_travaille === jour) || {};
  const dejaSignale = (a.signalements || []).length;
  ouvrirBoite(`
    <h3>Signaler l'arrêt de ${echapper(salarie)}</h3>
    <p class="discret">${echapper(a.motif_libelle || "")} — dernier jour travaillé le
      ${echapper(jjmmaaaa(jour))}, fin prévue le ${echapper(jjmmaaaa(a.fin_previsionnelle))}.</p>
    <div class="encart">
      <b>Le signalement est dû dans les cinq jours</b>
      Il informe l'Assurance Maladie au plus près de l'événement, pour les
      indemnités journalières. La DSN du mois le répétera : ce n'est pas la
      même déclaration, et l'une ne remplace pas l'autre.
    </div>
    ${a.reprise ? `<label class="champ-boite">
      <input type="checkbox" id="si-reprise"> Signaler la <b>reprise anticipée</b>
      (nature 05) plutôt que l'arrêt</label>` : ""}
    ${dejaSignale ? `<div class="encart attention">
      <b>Déjà signalé ${dejaSignale} fois</b>
      Un nouveau signalement normal ne remplace rien : pour corriger ce qui est
      parti, il faut un annule-et-remplace, qui vise l'identifiant rendu par le
      point de dépôt (il est sur le certificat de conformité).
      <label class="champ-boite">Identifiant de la déclaration remplacée
        <input type="text" id="si-remplace" placeholder="vide pour un signalement normal"></label>
    </div>` : ""}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="si-ok">Écrire le signalement</button>
    </div>`);
  document.getElementById("si-ok").onclick = async () => {
    const remplace = document.getElementById("si-remplace");
    const reprise = document.getElementById("si-reprise");
    let r;
    try {
      r = await api("arret-signaler", {
        salarie, dernier_jour: jour,
        nature: reprise && reprise.checked ? "05" : "04",
        remplace: remplace ? remplace.value.trim() : "",
      });
    } catch (e) { dire(e.message, "erreur"); return; }
    fermerBoite();
    // Les avis du signalement suivent le message, sur la même ligne : un
    // par un en rouge, le suivant effaçait le précédent (24/09/2026).
    dire(["Signalement écrit : " + r.nom + ". Rien n'est parti."]
      .concat(r.avertissements || []).join("  ·  "));
    await dessinerArrets(MOIS_AFFICHE);
    boiteDepot("dsn", r.fichier);
  };
}

/* ------------------------------------------ LA FIN D'UN CONTRAT (nature 07)
   Un CDI ou un CDD ordinaire se termine : sa fin ne part pas dans la DSN du
   mois, elle fait un SIGNALEMENT à part, dû dans les cinq jours ouvrés. Un
   CDD d'usage du spectacle arrivé à son terme (031), lui, n'en émet pas — le
   circuit dérogatoire porte sa fin dans la mensuelle ; rompu avant son terme
   (036, 037…), il se signale comme les autres (fiche 2062). C'est le moteur
   qui tranche (`Nature::signalement_fctu`, route `fins-de-contrat`), jamais
   cet écran.

   Ce bloc ne fait pas saisir : il MONTRE ce qui manque et renvoie au bloc
   d'engagement, où les faits se tapent. Un signalement qui partirait sans
   motif de rupture serait refusé, et proposer un bouton qui refuse est pire
   que ne rien proposer. */
async function dessinerFins(mois) {
  const cible = document.getElementById("fins-de-contrat");
  if (!cible) return;
  let r;
  try { r = await api("fins-de-contrat", { mois }); }
  catch (e) { cible.innerHTML = ""; return; }
  if (MOIS_AFFICHE !== mois) return;          // on a changé de mois entre-temps
  const fins = r.fins || [];
  if (!fins.length) { cible.innerHTML = ""; return; }
  cible.innerHTML = `
    <h2>Fin de contrat <span class="discret">— à signaler sous 5 jours ouvrés</span></h2>
    ${fins.map((f) => `
      <div class="rang">
        <span class="rang-nom">${echapper(f.salarie)}</span>
        <span class="pastille ${f.a_saisir.length ? "alerte" : "bien"}">${
          f.a_saisir.length ? "saisie incomplète" : "prêt"}</span>
        <span class="rang-infos">${echapper(f.nature)} — contrat ${echapper(f.numero || "?")},
          fin le ${echapper(jjmmaaaa(f.fin))}</span>
        ${f.a_saisir.length ? `<div class="encart attention">
          <b>Ce que le signalement exige et que la saisie ne dit pas</b>
          ${f.a_saisir.map((m) => `<div>${echapper(m)}</div>`).join("")}
          Ça se tape sous « Contrat », dans le bloc de cet engagement.
        </div>` : ""}
        <button class="secondaire" data-signaler-fin="${echapper(f.salarie)}"
          ${f.a_saisir.length ? "disabled" : ""}>Signaler…</button>
        <button class="lien" data-papiers="${echapper(f.salarie)}">Les deux papiers…</button>
      </div>`).join("")}
    <p class="note">Le signalement informe France Travail au plus près de la
      rupture — c'est lui qui ouvre les droits. Rien ne part d'ici : le
      fichier s'écrit, et le dépôt est un acte à part.</p>`;

  cible.querySelectorAll("[data-signaler-fin]").forEach((b) => {
    b.onclick = () => boiteSignalerFin(mois, b.dataset.signalerFin,
                                       fins.find((f) => f.salarie === b.dataset.signalerFin));
  });
  cible.querySelectorAll("[data-papiers]").forEach((b) => {
    b.onclick = () => boitePapiersDeFin(mois, b.dataset.papiers);
  });
}

/** Écrire le signalement, puis le déposer. Deux gestes, comme pour un
 *  arrêt : écrire un fichier n'engage rien, le déposer engage tout. */
function boiteSignalerFin(mois, salarie, f = {}) {
  ouvrirBoite(`
    <h3>Signaler la fin du contrat de ${echapper(salarie)}</h3>
    <p class="discret">${echapper(f.nature || "")} — contrat ${echapper(f.numero || "?")},
      fin le ${echapper(jjmmaaaa(f.fin))}.</p>
    <div class="encart">
      <b>Le signalement est dû dans les cinq jours ouvrés</b>
      Il est adressé à France Travail, qui en tire l'attestation employeur du
      salarié : c'est ce document-là qui ouvre ses droits. La DSN du mois ne
      le remplace pas — elle en reprend la fin ensuite, trop tard pour lui.
    </div>
    <label class="champ-boite">Identifiant de la déclaration remplacée
      <input type="text" id="fi-remplace" placeholder="vide pour un signalement normal"></label>
    <p class="note">Un annule-et-remplace vise l'identifiant du signalement
      déjà valide : <b>JJMMAAAA suivi du rang de l'envoi</b>, lu sur son
      certificat de conformité. Ce n'est pas l'identifiant de flux.</p>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="fi-ok">Écrire le signalement</button>
    </div>`);
  document.getElementById("fi-ok").onclick = async () => {
    let r;
    try {
      r = await api("fin-contrat-signaler", {
        mois, salarie,
        remplace: document.getElementById("fi-remplace").value.trim(),
      });
    } catch (e) { dire(e.message, "erreur"); return; }
    fermerBoite();
    dire(["Signalement écrit : " + r.nom + " (préavis : " + r.preavis + "). Rien n'est parti."]
      .concat(r.avertissements || []).join("  ·  "));
    await dessinerFins(MOIS_AFFICHE);
    boiteDepot("dsn", r.fichier);
  };
}

/** Les deux papiers dus au salarié qui part. Ils ne partent nulle part :
 *  ils se REMETTENT, et le reçu se signe à la main. */
async function boitePapiersDeFin(mois, salarie) {
  let r;
  try { r = await api("fin-contrat-documents", { mois, salarie }); }
  catch (e) { dire(e.message, "erreur"); return; }
  ouvrirBoite(`
    <h3>Le certificat de travail et le reçu pour solde de tout compte</h3>
    <p class="discret">Écrits pour ${echapper(salarie)} :
      ${r.noms.map((n) => echapper(n)).join(", ")}.</p>
    ${(r.avertissements || []).map((m) => `<div class="encart attention">${echapper(m)}</div>`).join("")}
    <div class="actions">
      <button class="secondaire" id="fp-dossier">Ouvrir le dossier</button>
      <button class="primaire" onclick="fermerBoite()">Fermer</button>
    </div>`);
  document.getElementById("fp-dossier").onclick = async () => {
    try {
      const r = await api("ouvrir-dossier", { mois });
      if (r && r.dossier) dire(`Dossier ouvert : ${r.dossier}`);
    } catch (e) { dire(e.message, "erreur"); }
  };
}

async function dessinerReprise() {
  const cible = document.getElementById("reprise");
  if (!cible) return;
  let r;
  try {
    r = await api("journal-reprise", {});
  } catch (e) {
    cible.innerHTML = "";
    return;
  }
  if (!r.ajoutes.length) { cible.innerHTML = ""; return; }

  const reels = r.ajoutes.filter((v) => v.mode === "reel" && v.accepte);
  // Replié en une ligne, sauf s'il y a un dépôt RÉEL accepté à reprendre :
  // des essais retrouvés sont un historique à ranger, pas une urgence.
  cible.innerHTML = `
    <details class="reprise" ${reels.length ? "open" : ""}>
    <summary>${r.ajoutes.length} dépôt${r.ajoutes.length > 1 ? "s" : ""} retrouvé${r.ajoutes.length > 1 ? "s" : ""}
      dans les retours, absent${r.ajoutes.length > 1 ? "s" : ""} du journal${reels.length
        ? ` — <b>dont ${reels.length} réel${reels.length > 1 ? "s" : ""}</b>` : ""}</summary>
    <div class="encart">
      <b>${r.ajoutes.length} dépôt(s) attestés par les retours, absents du journal</b>
      Le journal des dépôts est né le 11/09/2026 ; ce qui est parti avant ne
      s'y est pas écrit. Les accusés d'enregistrement reçus des organismes,
      eux, s'en souviennent. Les reprendre ne dépose
      rien et n'envoie rien : ça remet au journal ce qui a déjà eu lieu.
      ${reels.length ? `<br><b>${reels.length} sont des dépôts RÉELS acceptés.</b>` : ""}
    </div>
    <div class="liste-choix">
      ${r.ajoutes.map((v) => `
        <div class="choix"><span>
          <b>${echapper(quand(v.quand))}</b>
          — ${echapper(v.mois ? moisEnClair(v.mois) : "mois inconnu")}
          <span class="pastille ${v.mode === "reel" ? (v.accepte ? "bien" : "erreur") : "alerte"}">
            ${v.mode === "reel" ? "dépôt réel" : "test"}${v.accepte ? "" : " — refusé"}</span>
          <br><span class="discret">déposé par ${echapper(v.logiciel || "logiciel inconnu")}
            · flux ${echapper(v.flux)}</span>
        </span></div>`).join("")}
    </div>
    <button class="secondaire" id="reprendre">Reprendre ces ${r.ajoutes.length} dépôt(s)</button>
    <p class="note">Le nom du logiciel n'est pas une annotation : il est lu
      dans le certificat de conformité. C'est lui qui distingue le dépôt d'un
      autre logiciel d'un dépôt PuppetPaie.</p>
    </details>`;

  document.getElementById("reprendre").onclick = async () => {
    document.getElementById("reprendre").disabled = true;
    try {
      const f = await api("journal-reprise", { ecrire: true });
      dire(f.ajoutes.length + " dépôt(s) remis au journal"
           + (f.rattaches ? ", " + f.rattaches + " retour(s) rattachés." : "."));
      await rafraichirActions();
      await dessinerReprise();
    } catch (e) {
      document.getElementById("reprendre").disabled = false;
      dire(e.message, "erreur");
    }
  };
}

/* --------------------------------- les embauches sans trace d'envoi
   La demande était « un bouton qui envoie les DPAE des contrats pas encore
   envoyés ». Ce qu'on peut faire honnêtement aujourd'hui, c'est la LISTE et
   le chemin vers le dépôt : le fichier DPAE d'un mois porte tout le mois, et
   il n'existe pas encore de production qui ne reprendrait qu'une partie des
   engagements. Le récapitulatif du dépôt dit alors, ligne par ligne, ce qui a
   déjà une trace — c'est là que la nuance se joue, pas ici.

   DU MOIS AFFICHÉ, ET DE LUI SEUL, depuis le 12/09/2026 :

     « je vois octobre dans septembre »
     « chaque mois doit être autonome »

   La liste balayait tous les mois connus. Une saisie d'essai laissée en
   octobre s'affichait donc dans la colonne de septembre, avec son bouton
   « Voir la DPAE d'octobre… » — un dépôt d'un autre mois à un clic, dans une
   colonne qui ne parle que de celui-ci. Le moteur savait déjà filtrer
   (`embauches_a_declarer(ctx, mois)`) ; on lui passe enfin le mois. */
async function dessinerEmbauches(mois) {
  const cible = document.getElementById("embauches");
  const titre = document.getElementById("titre-embauches");
  // Plus de bloc dans la page depuis le 13/09/2026 : la pastille DPAE de
  // chaque paie et la boîte DPAE disent déjà tout. Le code reste pour le
  // jour où on voudrait la liste ailleurs.
  if (!cible) return;
  if (titre) {
    titre.innerHTML = `Embauches sans trace d'envoi
      <span class="discret">${mois ? "— " + echapper(moisEnClair(mois)) : ""}</span>`;
  }
  if (!mois) { cible.innerHTML = ""; return; }
  let liste;
  try {
    liste = (await api("embauches-a-declarer", { mois })).embauches;
  } catch (e) {
    cible.innerHTML = `<p class="discret">${echapper(e.message)}</p>`;
    return;
  }
  if (!liste.length) {
    cible.innerHTML = `<p class="discret">Toutes les embauches de
      ${echapper(moisEnClair(mois))} ont une trace d'envoi.</p>`;
    return;
  }
  cible.innerHTML = `
    <div class="encart info">
      <b>« Pas de trace d'envoi », et non « pas déclarée »</b>
      Ce journal ne connaît que ce que <i>ce</i> logiciel a déposé. Une
      embauche déclarée par un autre logiciel, ou à la main sur le site de l'Urssaf,
      figure ici quand même — et déclarer deux fois la même embauche n'est pas
      anodin.
    </div>
    <div class="acte">
      <ul class="embauches-liste">
        ${liste.map((x) => `
          <li>${echapper(x.nom)} — ${echapper(x.spectacle)}, du
            ${echapper(jjmmaaaa(x.debut))} au
            ${echapper(jjmmaaaa(x.fin))}</li>`).join("")}
      </ul>
      <button class="secondaire" data-embauche="${echapper(mois)}">
        Voir la DPAE ${echapper(deMois(mois))}…</button>
    </div>
    <p class="note">Le fichier DPAE d'un mois porte <b>tout</b> le mois.
      Le récapitulatif du dépôt dit, pour chaque embauche, si elle a déjà une
      trace.</p>`;
  cible.querySelectorAll("[data-embauche]").forEach((b) => {
    b.onclick = () => ouvrirDpaeDuMois(b.dataset.embauche);
  });
}

async function ouvrirDpaeDuMois(mois) {
  return boiteDpae(mois);
}

