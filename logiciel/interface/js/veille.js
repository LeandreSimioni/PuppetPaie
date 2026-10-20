/* La veille. */
"use strict";

/* =======================================================================
   LA VEILLE
   =======================================================================
   Ce que le serveur fait sans qu'on le lui demande : sauvegarder le dossier
   prive dans le Drive, relever les retours. La page ne declenche rien
   d'elle-meme au chargement -- le serveur a deja lance le premier tour a
   l'ouverture. Elle ne fait que demander ou ca en est.

   Elle ne calcule rien non plus : tout ce qui s'affiche ici est rendu par
   /api/veille, comme le reste. */
let VEILLE = null;

/** Un passage de veille a pu changer ce que la page montre : un contrat
 *  signé d'un clic (pastille, colonne de droite, fiche), un retour rattaché
 *  à un dépôt. « Il devrait se mettre à jour tout en même temps » : quand
 *  le dernier passage n'est plus celui qu'on a vu, on repeint les états et
 *  la colonne des actes — ce sont des lectures locales, elles ne coûtent
 *  rien. */
async function rafraichirApresVeille(v) {
  const avant = VEILLE?.dernier?.quand;
  const apres = v?.dernier?.quand;
  if (!apres || apres === avant || v.en_cours) return;
  const mois = document.getElementById("mois").value;
  try {
    await dessinerEtatsDesPaies(mois);
    await rafraichirActions();
  } catch (e) { /* le bandeau dit déjà l'essentiel */ }
}

/* LE BANDEAU, REPENSÉ (24/09/2026). On y cherche trois choses, dans cet
   ordre : est-ce que tout va bien ? est-ce qu'il est arrivé quelque chose ?
   est-ce qu'il y a quelque chose à FAIRE ? Il enfilait tout à la suite
   (« dernier passage … — 285 retour(s), dont 2 nouveau(x), 1 archive(s). —
   Mot de passe Urssaf (DPAE) absent du coffre […] `python -m moteur coffre
   poser urssaf` »), en orange dès qu'une ligne manquait.

   Maintenant : une phrase calme quand tout va (« À jour — relevé à 16:28 »,
   le détail au survol) ; les nouvelles en clair (retours, signatures,
   version) ; ce qui est à faire en orange, dit en mots, avec le lien vers
   l'endroit où le faire. Jamais une commande de terminal : une fenêtre
   n'en a pas. */
const A_POSER = {
  "net-entreprises": ["Les retours de DSN ne se relèvent pas seuls : le mot de passe net-entreprises n'est pas posé.", "Poser le mot de passe"],
  urssaf: ["Les retours de DPAE ne se relèvent pas seuls : le mot de passe Urssaf (DPAE) n'est pas posé.", "Poser le mot de passe"],
  google: ["La sauvegarde ne part pas : aucun Drive n'est lié.", "Lier le Drive"],
};

/** « à 16:28 » aujourd'hui, « le 23/09 à 16:28 » un autre jour. */
function heureDuPassage(q) {
  const m = String(q || "").match(/^(\d{4})-(\d{2})-(\d{2})[ T](\d{2}:\d{2})/)
    || String(q || "").match(/^(\d{2})\/(\d{2})\/(\d{4}) (\d{2}:\d{2})/);
  if (!m) return q ? `le ${q}` : "";
  const [an, mo, jo] = m[1].length === 4 ? [m[1], m[2], m[3]] : [m[3], m[2], m[1]];
  const auj = new Date();
  const memeJour = +an === auj.getFullYear() && +mo === auj.getMonth() + 1 && +jo === auj.getDate();
  return memeJour ? `à ${m[4]}` : `le ${jo}/${mo} à ${m[4]}`;
}

/** Aller aux mots de passe, d'où qu'on soit -- et y ouvrir la ligne de ce
 *  qui manque, le curseur dans la case : il ne reste qu'à taper. */
let OUVRIR_SERVICE = null;
function allerAuxMotsDePasse(service) {
  OUVRIR_SERVICE = service || null;
  document.querySelector('.onglets button[data-vue="configuration"]').click();
  document.querySelector('.sous-onglets button[data-fiche="coffre"]').click();
}

function dessinerVeille(v) {
  VEILLE = v;
  const zone = document.getElementById("veille");
  const texte = document.getElementById("veille-texte");
  if (!v || (!v.minutes && !v.dernier?.quand && !v.en_cours)) { zone.hidden = true; return; }
  zone.hidden = false;
  zone.classList.remove("souci", "neuf");

  if (v.en_cours) {
    texte.innerHTML = "Relevé des retours en cours…";
    return;
  }
  const d = v.dernier || {};

  // 1. L'ÉTAT, en une phrase. Le détail des relevés passe au survol : on
  //    le cherche rarement, et il reste à portée.
  const detail = [d.sauvegarde, d.retours, d.signatures, d.fiches, d.sources,
                  d.maj_exigee ? "" : d.maj].filter(Boolean).join("\n");
  const morceaux = [`<span class="veille-etat" title="${echapper(detail)}">${d.hors_ligne
    ? "<b>Hors connexion</b> — sauvegarde et retours au prochain passage"
    : `À jour — relevé ${echapper(heureDuPassage(d.quand))}${v.sauvegarde_en_cours
        ? " · sauvegarde en cours…" : ""}`}</span>${enFictive()
    ? ` · <button class="lien" data-fictive-explique="coulisses">compagnie fictive : pas de
        sauvegarde — pourquoi ?</button>` : ""}`];

  // 2. LES NOUVELLES : ce qui est arrivé depuis le dernier regard.
  const nouvelles = [];
  // Les retours se LISENT dans l'Historique (20/10/2026) : le bandeau les annonce et y mène.
  const retours = d.nouveaux ? ` <button class="lien" data-vers-historique>${d.nouveaux} retour${d.nouveaux > 1 ? "s" : ""}
    nouveau${d.nouveaux > 1 ? "x" : ""} — les lire dans l'Historique</button>` : "";
  (d.signatures_recues || []).forEach((x) => nouvelles.push(`contrat signé par ${x.nom}`));
  if (d.maj && !d.maj_exigee) nouvelles.push(d.maj);
  // L'HORLOGE DE LA COMPAGNIE FICTIVE : ce que l'administrateur fictif a fait
  // pendant l'absence du visiteur, chaque geste à sa date limite.
  const horloge = enFictive() && (d.horloge || []).length
    ? ` <button class="lien" id="fictive-horloge">pendant votre absence, la compagnie a
        travaillé (${d.horloge.length}) — voir</button>` : "";
  if (retours) morceaux.push(retours);
  if (nouvelles.length) morceaux.push(`<b>${nouvelles.map(echapper).join(" · ")}</b>`);
  if (horloge) morceaux.push(horloge);
  if (d.maj_note) morceaux.push(`<b>${echapper(d.maj_note)}</b>`);
  // La version prete qui attend une DSN : la page laisse passer outre,
  // en le disant. La bascule reste au prochain demarrage, fenetre fermee.
  if (d.maj_attend && d.maj_attend.length) {
    morceaux.push(`<button class="lien" id="maj-malgre-calcul">Appliquer quand
      même au prochain démarrage</button>`);
    setTimeout(() => {
      const b = document.getElementById("maj-malgre-calcul");
      if (!b) return;
      b.onclick = async () => {
        if (!confirm("Cette version change un calcul. Les bulletins déjà produits "
          + "pour " + d.maj_attend.join(", ") + " pourraient différer de la DSN "
          + "qui sera déposée. L'appliquer quand même au prochain démarrage ?")) return;
        try { await api("maj-malgre-calcul"); b.replaceWith("s'appliquera au prochain démarrage"); }
        catch (e) { alert(e.message); }
      };
    }, 0);
  }

  // 3. CE QUI EST À FAIRE : en mots, avec le chemin. Le premier se lit, les
  //    suivants se déplient.
  const afaire = [];
  if (d.maj_exigee) afaire.push(`<b>${echapper(d.maj)}</b>`);
  (d.a_poser || []).forEach((code) => {
    const [phrase, geste] = A_POSER[code] || [code, "Mots de passe"];
    afaire.push(`${echapper(phrase)} <button class="lien" data-aller="${echapper(code)}">${echapper(geste)}</button>`);
  });
  // LES TAUX DE PAS QUI MANQUENT. Ni incident ni panne : la paie sortira, et
  // la loi dit quoi déclarer sans eux (la grille de l'article 204 H). C'est
  // un travail à faire, et le bandeau est le seul endroit où il arrive à
  // temps — trois semaines avant la DSN, pas cinq minutes avant le dépôt.
  if (d.taux_pas && d.taux_pas.length) {
    afaire.push(`<span title="${echapper(d.taux_pas.join(" · "))}">${d.taux_pas.length}
      salarié${d.taux_pas.length > 1 ? "s" : ""} sans taux de PAS valide : la grille
      par défaut s'appliquera</span>${enFictive()
      ? ` <button class="lien" data-fictive-explique="pas">pourquoi ?</button>` : ""}`);
  }
  [...(d.manquants || []), ...(d.incidents || []), ...(v.erreur ? [v.erreur] : [])]
    .forEach((x) => afaire.push(echapper(x)));
  if (afaire.length) {
    zone.classList.add("souci");
    morceaux.push(afaire.length === 1 ? afaire[0]
      : `<details class="ennuis"><summary>${afaire[0]}
           <b>+ ${afaire.length - 1} autre${afaire.length > 2 ? "s" : ""}</b></summary>
           ${afaire.slice(1).map((x) => `<div>${x}</div>`).join("")}</details>`);
  } else if (nouvelles.length) {
    zone.classList.add("neuf");
  }
  texte.innerHTML = morceaux.join(" — ");
  texte.querySelectorAll("[data-vers-historique]").forEach((b) => {
    b.onclick = () => document.querySelector('.onglets button[data-vue="historique"]').click();
  });
  texte.querySelectorAll("[data-aller]").forEach((b) => {
    b.onclick = () => allerAuxMotsDePasse(b.dataset.aller);
  });
  const bh = texte.querySelector("#fictive-horloge");
  if (bh) bh.onclick = () => ouvrirBoite(`
    <div class="fictive-fenetre">
      <h3>Compagnie fictive — Pendant votre absence</h3>
      <p class="intro">L'administrateur fictif a fait ce qui tombait, chacun à sa date
        limite : le 1er du mois, le mois s'écrit ; le jour d'une embauche, sa DPAE part ;
        à l'échéance, la DSN. Tout est joué, rien n'a quitté le poste — et le journal
        de chaque mois le montre.</p>
      <ul>${d.horloge.map((x) => `<li>${echapper(x)}</li>`).join("")}</ul>
      <div class="actions"><button class="primaire" onclick="fermerBoite()">Compris</button></div>
    </div>`);
  texte.querySelectorAll("[data-fictive-explique]").forEach((b) => {
    b.onclick = (ev) => { ev.preventDefault(); fenetreExplicationFictive(b.dataset.fictiveExplique); };
  });
}

async function suivreVeille() {
  if (SERVEUR_PERDU) return;          // le bandeau dit déjà l'essentiel
  try {
    const v = await api("veille");
    await rafraichirApresVeille(v);
    dessinerVeille(v);
    dessinerPastilleReponses();
  } catch (e) { /* déjà signalé */ }
}

document.getElementById("veille-maintenant").onclick = async () => {
  dessinerVeille({ ...(VEILLE || {}), en_cours: true });
  try {
    const v = await api("veille-maintenant");
    // Le relevé manuel repeint TOUJOURS : c'est ce qu'on vient demander.
    VEILLE = { ...(VEILLE || {}), dernier: {} };
    await rafraichirApresVeille(v);
    dessinerVeille(v);
    dire("Relevé terminé.");
  } catch (e) { dire("Relevé impossible : " + e.message, "erreur"); }
};

