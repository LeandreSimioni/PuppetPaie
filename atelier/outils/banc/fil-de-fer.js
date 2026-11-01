/* LE PARCOURS COURT À L'ÉCRAN DE LA FICTIVE DE RÉFÉRENCE DU FIL DE FER (`atelier/fictives/fictive-fil-de-fer/`), le pendant
 * dans la page de son rejeu en ligne de commande (`moteur/tests/rejeu.rs`) : la compagnie livrée telle qu'elle est au
 * 1er septembre 2026 (`scenario livrer … --jusqu-au 2026-09-01`), ouverte le 2 ; le panneau des étapes d'août (la DSN
 * sans salarié à déposer) ; la DSN d'août déposée en test puis en réel (simulé) ; septembre : l'engagement de Solène
 * Marchetot à Nevers saisi sur sa carte et validé, et le panneau qui le suit. Un seul navigateur, une seule ouverture,
 * aucune attente fixe : chaque geste attend un état (la page n'a plus rien en route, une case, un bouton).
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
    path.join(DEPOT, "fictives/fictive-fil-de-fer"), "--jusqu-au", "2026-09-01"], { stdio: "inherit" });
  await banc.lancer(B, { jour: "2026-09-02", moteur: MOTEUR });
  dire("banc lancé au 02/09/2026");
  const { page, fermer } = await banc.ouvrir(B);
  page.on("pageerror", (e) => { throw new Error("erreur de la page : " + e.message); });
  await page.context().addInitScript(COMPTEUR);
  await page.evaluate(COMPTEUR);

  // 1. L'accueil, puis la compagnie : son bienvenu dit ce qui est à faire ce jour-là.
  await page.locator(".tuile", { hasText: NOM }).first().click();
  await page.waitForSelector("#actions-mois .etapes", { timeout: 60000 });
  await page.evaluate(COMPTEUR);
  await calme(page);
  await capture(page, "01-ouverture.png");
  if (await page.isVisible("#rappels-fermer")) await page.click("#rappels-fermer");
  await page.waitForSelector("#voile", { state: "hidden" });

  // 2. Le panneau des étapes d'août : rien à déclarer d'embauche, la DSN sans salarié à déposer.
  if ((await page.inputValue("#mois")) !== "2026-08") throw new Error("la page ne s'ouvre pas sur août 2026 : " + await page.inputValue("#mois"));
  const aout = await panneau(page);
  if (!/DSN\s+à déposer, sans salarié/.test(aout)) throw new Error("la DSN d'août n'est pas à déposer : " + aout);
  await capture(page, "02-etapes-aout.png");

  // 3. La DSN d'août : la case ouvre la boîte ; le test, son bilan (attendu dans la boîte), puis le réel (simulé).
  await caseDe(page, "DSN").click();
  await page.waitForSelector("#d-test:not([disabled])");
  await calme(page);
  await capture(page, "03-boite-dsn.png");
  await page.click("#d-test");
  await calme(page);
  for (let i = 0; i < 40 && await page.isDisabled("#d-reel"); i++) await avancer(page, 10000);
  if (await page.isDisabled("#d-reel")) throw new Error("le bilan du test n'arrive pas : " + await page.innerText("#d-bilan"));
  await capture(page, "04-dsn-test-conforme.png");
  await page.click("#d-reel");
  await page.waitForSelector("#boite >> text=DSN envoyée");
  await calme(page);
  await capture(page, "05-dsn-envoyee.png");
  await page.click("#boite >> text=Compris");
  await page.waitForSelector("#voile", { state: "hidden" });
  // Le suivi du dépôt repeint le panneau quand le bilan est tranché (son minuteur, avancé).
  for (let i = 0; i < 10 && !/✓\s*DSN/.test(await panneau(page)); i++) await avancer(page, 5000);
  const apres = await panneau(page);
  if (!/✓\s*DSN/.test(apres)) throw new Error("la DSN d'août n'est pas faite : " + apres);
  await capture(page, "06-dsn-aout-faite.png");

  // 4. Septembre : l'engagement de Solène Marchetot à Nevers, trois cachets, saisi sur sa carte, puis validé.
  await page.click("#mois-apres");
  await page.waitForFunction(() => document.getElementById("mois").value === "2026-09");
  await calme(page);
  await page.click("text=+ un engagement");
  await calme(page);
  const carte = page.locator(".engagement").last();
  await carte.locator(".e-qui summary").click();
  await carte.locator(".e-qui-case[value=MARCHETOT]").check();
  await page.mouse.click(5, 300);
  await carte.locator(".e-spectacle").selectOption("objet-256Z00931108");
  await carte.locator(".r-code").first().selectOption("cachet");
  await carte.locator(".r-jours-btn").first().click();
  for (const j of ["18", "19", "20"]) await page.click(`.calendrier .c-jour[data-j='${j}']`);
  await page.click(".calendrier .c-ok");
  await carte.locator(".r-taux").first().fill("140");
  await carte.locator(".r-lieu").first().fill("Maison de la culture, 58000 Nevers");
  await carte.locator(".r-lieu").first().press("Tab");
  await avancer(page, 1000);
  await capture(page, "07-carte-saisie.png");
  await carte.locator(".v-basculer").click();
  await calme(page);
  await page.waitForFunction(() => /DPAE/.test(document.getElementById("actions-mois").innerText));
  const sept = await panneau(page);
  if (!/DPAE\s+1\s/.test(sept.replace(/\n/g, " ") + " ") && !/à déposer|à envoyer/.test(sept)) throw new Error("septembre ne dit pas sa DPAE : " + sept);
  await capture(page, "08-engagement-valide.png");
  dire("panneau de septembre :\n" + sept);

  await fermer();
  banc.arreter(B);
  dire("fini");
})().catch((e) => { console.error(e); try { banc.arreter(B); } catch (x) { /* déjà arrêté */ } process.exit(1); });
