/* LE PARCOURS COURT À L'ÉCRAN DE LA FICTIVE DE RÉFÉRENCE DU FIL DE FER (`atelier/fictives/fictive-fil-de-fer/`), le pendant
 * dans la page de son rejeu en ligne de commande (`moteur/tests/rejeu.rs`) : la compagnie livrée telle que « Publier » la
 * livre le 2 novembre 2026 (`scenario livrer … --jusqu-au 2026-11-02` : les mois d'avant faits, les cartes de décembre
 * écrites, non validées), ouverte ce jour-là ; l'accueil ; le panneau des étapes d'octobre (la DSN sans salarié à
 * déposer) ; la DSN d'octobre déposée en test puis en réel (simulé) ; novembre ; décembre : la carte de Solène Marchetot
 * à Dijon, déjà saisie, validée, et le panneau qui le suit. Un seul navigateur, une seule ouverture, aucune attente fixe :
 * chaque geste attend un état (la page n'a plus rien en route, une case, un bouton).
 *
 *   PUPPETPAIE_ATELIER=<cible>-atelier/essais/moteur-atelier node atelier/outils/banc/fil-de-fer.js <banc neuf>
 *
 * Les captures vont dans `<banc>/captures/`. Le script livre la fictive dans `fictives/` du dépôt (non suivie), lance le
 * banc, joue, arrête le banc. */
"use strict";
const path = require("path");
const fs = require("fs");
const { execFileSync } = require("child_process");
const banc = require("./banc.js");

const DEPOT = path.resolve(__dirname, "../..", "..");
const B = path.resolve(process.argv[2] || "/tmp/banc-fil-de-fer");
const C = path.join(B, "captures");
const NOM = "Compagnie du Fil de Fer";
const MOTEUR = process.env.PUPPETPAIE_ATELIER;
const debut = Date.now();
const dire = (t) => console.log(`[${((Date.now() - debut) / 1000).toFixed(1)} s] ${t}`);

/* « La page a fini » : plus aucun appel en route (corps lus) et trois tours de boucle sans nouvel appel. */
const COMPTEUR = `(() => { if (window.__calme) return; const c = (window.__calme = { n: 0, en: 0 });
  const f = window.fetch.bind(window);
  window.fetch = function (u, o) { c.n++; c.en++; const fini = () => { c.en--; };
    return f(u, o).then((r) => { try { r.clone().arrayBuffer().then(fini, fini); } catch (e) { fini(); } return r; }, (e) => { fini(); throw e; }); }; })();`;
async function calme(page) {
  await page.evaluate(async () => {
    const c = window.__calme, tour = () => new Promise((r) => setTimeout(r, 0));
    for (let calmes = 0; calmes < 3;) { const n = c.n; await tour(); if (c.en === 0 && c.n === n) calmes++; else calmes = 0; }
  });
}
/* Les minuteurs de la page (l'enregistrement une seconde après la frappe, l'attente du bilan) partent quand on avance son
 * horloge : jamais d'attente réelle. */
async function avancer(page, ms) { await page.clock.fastForward(ms); await calme(page); }
async function capture(page, nom) { await banc.capture(page, path.join(C, nom)); dire(`capture ${nom}`); }
const panneau = (page) => page.innerText("#actions-mois");
const caseDe = (page, etape) => page.locator("#actions-mois .kase", { hasText: etape }).first();

(async () => {
  if (!MOTEUR) throw new Error("PUPPETPAIE_ATELIER : le binaire moteur-atelier.");
  if (fs.existsSync(B)) throw new Error(`${B} existe : le parcours part d'un banc neuf.`);
  execFileSync(MOTEUR, ["scenario", "livrer", path.join(DEPOT, "atelier/fictives/fictive-fil-de-fer"),
    path.join(DEPOT, "fictives/fictive-fil-de-fer"), "--jusqu-au", "2026-11-02"], { stdio: "inherit" });
  await banc.lancer(B, { jour: "2026-11-02", moteur: MOTEUR });
  dire("banc lancé au 02/11/2026");
  const { page, fermer } = await banc.ouvrir(B);
  page.on("pageerror", (e) => { throw new Error("erreur de la page : " + e.message); });
  await page.context().addInitScript(COMPTEUR);
  await page.evaluate(COMPTEUR);

  // 1. L'accueil : la tuile de la fictive livrée ; puis la compagnie, dont le bienvenu dit ce qui est à faire ce jour-là.
  const tuile = page.locator(".tuile", { hasText: NOM }).first();
  await tuile.waitFor();
  await capture(page, "01-accueil.png");
  dire("tuile : " + (await tuile.innerText()).replace(/\n/g, " | "));
  await tuile.click();
  await page.waitForSelector("#actions-mois .etapes", { timeout: 60000 });
  await page.evaluate(COMPTEUR);
  await calme(page);
  await capture(page, "02-ouverture.png");
  if (await page.isVisible("#rappels-fermer")) await page.click("#rappels-fermer");
  await page.waitForSelector("#voile", { state: "hidden" });

  // 2. Le panneau des étapes d'octobre : rien à déclarer d'embauche, la DSN sans salarié à déposer.
  const vers = async (mois) => {
    for (let i = 0; i < 12 && (await page.inputValue("#mois")) !== mois; i++) {
      await page.click((await page.inputValue("#mois")) < mois ? "#mois-apres" : "#mois-avant");
      await calme(page);
    }
    if ((await page.inputValue("#mois")) !== mois) throw new Error(`la page ne va pas à ${mois}`);
  };
  dire("la page s'ouvre sur " + await page.inputValue("#mois"));
  await vers("2026-10");
  const octobre = await panneau(page);
  if (!/DSN\s+à déposer, sans salarié/.test(octobre)) throw new Error("la DSN d'octobre n'est pas à déposer : " + octobre);
  await capture(page, "03-etapes-octobre.png");

  // 3. La DSN d'octobre : la case ouvre la boîte ; le test, son bilan (attendu dans la boîte), puis le réel (simulé).
  await caseDe(page, "DSN").click();
  await page.waitForSelector("#d-test:not([disabled])");
  await calme(page);
  await page.click("#d-test");
  await calme(page);
  for (let i = 0; i < 40 && await page.isDisabled("#d-reel"); i++) await avancer(page, 10000);
  if (await page.isDisabled("#d-reel")) throw new Error("le bilan du test n'arrive pas : " + await page.innerText("#d-bilan"));
  await capture(page, "04-dsn-test-conforme.png");
  // La DSN d'octobre n'a pas de salarié : aucune boîte d'avis ne suit (le moteur n'en rend pas, `avis` de `deposer`).
  await page.click("#d-reel");
  await page.waitForSelector("#voile", { state: "hidden" });
  await calme(page);
  // Le suivi du dépôt repeint le panneau quand le bilan est tranché (son minuteur, avancé).
  for (let i = 0; i < 10 && !/✓\s*DSN/.test(await panneau(page)); i++) await avancer(page, 5000);
  const apres = await panneau(page);
  if (!/✓\s*DSN/.test(apres)) throw new Error("la DSN d'octobre n'est pas faite : " + apres);
  await capture(page, "05-dsn-octobre-faite.png");

  // 4. Novembre, le mois en cours : aucun engagement.
  await vers("2026-11");
  await capture(page, "06-novembre.png");
  dire("panneau de novembre :\n" + await panneau(page));

  // 5. Décembre : les cartes du Grand Fil à Dijon sont là, non validées ; celle de Solène Marchetot se valide.
  await vers("2026-12");
  const carte = page.locator(".engagement", { has: page.locator(".e-qui-case[value=MARCHETOT]:checked") }).first();
  await carte.waitFor();
  await capture(page, "07-decembre-cartes.png");
  await carte.locator(".v-basculer").click();
  await calme(page);
  await page.waitForFunction(() => /DPAE/.test(document.getElementById("actions-mois").innerText));
  const dec = await panneau(page);
  if (!/DPAE/.test(dec)) throw new Error("décembre ne dit pas sa DPAE : " + dec);
  await capture(page, "08-engagement-valide.png");
  dire("panneau de décembre :\n" + dec);

  await fermer();
  banc.arreter(B);
  dire("fini");
})().catch((e) => { console.error(e); try { banc.arreter(B); } catch (x) { /* déjà arrêté */ } process.exit(1); });
