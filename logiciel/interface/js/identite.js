/* L'identite de la compagnie : signature et tampon. */
"use strict";

/* Configuration → Signature & tampon.

   L'éditeur : « je peux pas ajouter la signature dans l'interface, pourquoi ? ».
   Parce qu'il n'y avait AUCUNE route de fichier dans le logiciel. C'est la
   première, et elle sert la règle 0 : une compagnie qui installe PuppetPaie
   doit pouvoir poser SA signature, pas hériter de celle d'une autre.

   TROIS PARTIS PRIS, ET ILS TIENNENT ENSEMBLE :

   - ON MONTRE TOUJOURS CE QUI EST EN PLACE. « les gens doivent voir ce qu'ils
     ont mis » : poser une signature à l'aveugle, c'est ne jamais savoir ce
     qui part sur les attestations.
   - LE TAMPON EST DESSINÉ PAR DÉFAUT, et on le montre AUSSI — « que les gens
     le trouvent beau et ne passent pas trop de temps à faire le leur ». Il
     vient de la fiche Entreprise et la suit ; il n'y a rien à fournir.
   - RETIRER, C'EST REVENIR AU DÉFAUT. Un tampon importé qu'on retire rend la
     main au dessin. Rien à reconfigurer, rien à se rappeler. */
async function dessinerIdentite(cible) {
  const lu = await api("identite");
  const etat = lu.identite;
  const t = Date.now();          // l'image vient d'être remplacée : pas de cache

  const apercu = (src, alt) => `<div class="identite-apercu">
    <img src="${src}" alt="${alt}"></div>`;

  // UNE LIGNE CHACUN, comme les mots de passe : l'image en
  // place à gauche -- on voit toujours ce qui part --, ce qu'elle est et
  // d'où elle vient, puis les gestes. Le champ de fichier du navigateur
  // (« Choisir un fichier — aucun fichier choisi ») se cache derrière un
  // bouton qui dit ce qu'il fait.
  const rang = (quoi) => {
    const e = etat[quoi] || { pose: false };
    const titre = echapper(e.titre || "");
    // Le tampon a une image même quand rien n'est posé : celle que le
    // logiciel dessine. C'est tout l'objet de la case « automatique ».
    const vue = e.pose
      ? apercu(`/identite/${quoi}?t=${t}`, titre)
      : e.automatique ? apercu(`/identite/cachet-automatique?t=${t}`, titre)
      : `<div class="identite-apercu vide">rien de posé</div>`;
    const dit = e.pose
      ? `${pastilleDeLEtat(e.pastille)} <span class="discret">${e.format.toUpperCase()},
           ${Math.round((e.octets || 0) / 1024)} Ko, posé le ${jjmmaaaa((e.quand || "").slice(0, 10))}</span>`
      : e.automatique
        ? `${pastilleDeLEtat(e.pastille)} <span class="discret">${echapper(e.automatique_dit)}</span>`
        : pastilleDeLEtat(e.pastille);
    return `<div class="rang rang-image">
      ${vue}
      <div class="rang-infos">
        <b class="rang-nom">${titre}</b>
        <span class="discret">${echapper(e.dit || "")}</span>${quoi === "signature" ? `
          <button class="lien" data-aller-fiche="entreprise">Qui signe</button>` : ""}
        <div>${dit}</div>
      </div>
      <div class="rang-actions">
        ${e.pose ? `<button class="lien" data-retirer="${quoi}">${
          e.automatique ? "Tampon du logiciel" : "Retirer"}</button>` : ""}
        <label class="${e.pose ? "modifier" : "bouton-fichier"}">${
          e.pose ? CRAYON + "Modifier…" : "Importer…"}
          <input type="file" accept="image/png,image/jpeg,image/svg+xml"
                 data-quoi="${quoi}"></label>
      </div>
    </div>`;
  };

  // OÙ VA CHAQUE IMAGE, CE QU'ELLE ENGAGE, et ce qui la contredit : le moteur le dit (`titre`, `dit` de chaque image).

  cible.innerHTML = `
    <p class="intro">${echapper(lu.intro)}</p>
    <div class="rangs">
      ${rang("signature")}
      ${rang("cachet")}
      ${rang("logo")}
    </div>`;
  cible.querySelectorAll("[data-aller-fiche]").forEach((b) => {
    b.onclick = () => document.querySelector(
      `.sous-onglets button[data-fiche="${b.dataset.allerFiche}"]`).click();
  });

  cible.querySelectorAll('input[type="file"]').forEach((ch) => {
    ch.onchange = async () => {
      const f = ch.files && ch.files[0];
      if (!f) return;
      ch.disabled = true;
      try {
        // On envoie les octets tels quels : le moteur reconnaît le format sur
        // l'en-tête du fichier, pas sur son extension — qui ne prouve rien.
        const contenu = await new Promise((ok, non) => {
          const l = new FileReader();
          l.onload = () => ok(String(l.result).split(",").pop());
          l.onerror = () => non(l.error);
          l.readAsDataURL(f);
        });
        await api("poser-identite", { quoi: ch.dataset.quoi, contenu });
        await dessinerIdentite(cible);
      } catch (e) {
        // Un format refusé se dit : il partait en exception, sans un mot.
        ch.disabled = false;
        ch.value = "";
        dire(e.message, "erreur");
      }
    };
  });

  cible.querySelectorAll("[data-retirer]").forEach((b) => {
    b.onclick = async () => {
      b.disabled = true;
      try {
        await api("retirer-identite", { quoi: b.dataset.retirer });
        await dessinerIdentite(cible);
      } catch (e) { b.disabled = false; dire(e.message, "erreur"); }
    };
  });
}
