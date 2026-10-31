// The bar over Boardroom's drawn tabs once the board changed (docs/demo-spec.md, "What goes
// stale"), in the Pages build, in Chrome, on a made-up origin with a made-up snapshot:
//
//     npm run build:pages && npm run test:stale
//
// No bar on the published board; a save to the board's .kicad_pcb in the tab's workspace puts
// it on the Board tab's Plots, Viewer, Renders and Overview (not the Board tab's own drawing of
// the file, Gates or History), naming the app; a reload
// keeps it, its History button opens History, Reset takes it away, and a new tab has none.
import { chromium } from 'playwright-core';
import { readFileSync, existsSync } from 'node:fs';
import { extname } from 'node:path';

const ORIGIN = 'http://pages.test';
const DIST = new URL('../dist-pages', import.meta.url).pathname;
const STORE = readFileSync(new URL('../../store/store.js', import.meta.url), 'utf8');
const PCB = 'hw/b/a/b.kicad_pcb';
const PNG = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=', 'base64');
const project = {
  id: 'p', name: 'b', board: 'b', path: 'b', boards: [{ board: 'b', variant: 'a', kicad_pro: null }],
  exists: true, is_project: true, error: null, phase: null, gates: [], checks: [], needs_user: [], docs: [], links: [],
  apps: {}, pinned: false, updated: '2026-10-31', thumbnail: 'design/renders/top.png', viewer: 'viewer/index.html',
  listings: { 'design/renders': [{ name: 'top.png', dir: false }] },
};
const SNAPSHOT = {
  'projects.json': JSON.stringify([project]),
  [`p/${PCB}`]: '(kicad_pcb published)',
  'p/design/renders/top.png': PNG,
  'p/viewer/site.json': JSON.stringify({ title: 'b', boards: [{ id: 'b-a', name: 'b, variant A' }] }),
  'p/viewer/index.html': '<!doctype html><title>viewer</title>',
};
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.png': 'image/png' };

if (!existsSync(`${DIST}/index.html`)) {
  console.error('test:stale: build the Pages page first: npm run build:pages');
  process.exit(2);
}
const browser = await chromium.launch({ executablePath: process.env.CHROME ?? '/usr/bin/google-chrome' });
const context = await browser.newContext({ viewport: { width: 1400, height: 900 } });
await context.route(`${ORIGIN}/**`, (route) => {
  const path = decodeURIComponent(new URL(route.request().url()).pathname);
  const type = { contentType: TYPES[extname(path)] ?? 'application/octet-stream' };
  if (path === '/store.js') return route.fulfill({ body: STORE, contentType: 'text/javascript' });
  if (path.startsWith('/snapshot/')) {
    const body = SNAPSHOT[path.slice('/snapshot/'.length)];
    return body === undefined ? route.fulfill({ status: 404, body: 'no' }) : route.fulfill({ body, ...type });
  }
  const file = DIST + (path === '/' ? '/index.html' : path);
  return existsSync(file) ? route.fulfill({ body: readFileSync(file), ...type }) : route.fulfill({ status: 404, body: 'no' });
});

let failed = 0;
async function check(name, fn) {
  try {
    await fn();
    console.log(`ok   ${name}`);
  } catch (e) {
    failed++;
    console.log(`FAIL ${name}: ${e.message.split('\n')[0]}`);
  }
}
const assert = (cond, msg) => {
  if (!cond) throw new Error(msg);
};

const page = await context.newPage();
const errors = [];
page.on('pageerror', (e) => errors.push(e.message));
const bars = () => page.locator('.stale');
const tab = (name) => page.locator('.tabs button', { hasText: new RegExp(`^${name}$`) }).click();
await page.goto(`${ORIGIN}/index.html?project=p&tab=overview`);
await page.locator('.overview').waitFor();

await check('no bar on the published board', async () => {
  await page.waitForTimeout(500);
  assert((await bars().count()) === 0, 'a bar is shown');
});

await check("a save from Route puts the bar up, naming the app and the file", async () => {
  await page.evaluate(async (pcb) => {
    const { browserStore } = await import('/store.js');
    const s = browserStore({ project: 'p', snapshot: './snapshot/p/' });
    await s.write(pcb, '(kicad_pcb routed)', { app: 'route', message: 'Routed b', base: 'published' });
  }, PCB);
  await bars().first().waitFor({ timeout: 3000 });
  const text = (await bars().first().textContent()).replace(/\s+/g, ' ');
  assert(/Showing the published board; b\.kicad_pcb changed in Route at .+ · Routed b/.test(text), text);
});

const plots = () => page.locator('.bv .seg button', { hasText: 'Plots' }).click();
for (const [name, shown] of [['Board', false], ['Plots', true], ['Viewer', true], ['Renders', true], ['Overview', true], ['Gates', false], ['History', false]]) {
  await check(`${shown ? 'a bar' : 'no bar'} on ${name}`, async () => {
    if (name === 'Plots') {
      await tab('Board');
      await plots();
    } else await tab(name);
    await page.waitForTimeout(300);
    assert((await bars().count()) === (shown ? 1 : 0), `${await bars().count()} bars`);
  });
}

await check('a reload keeps it, and its History button opens History', async () => {
  await page.reload();
  await tab('Viewer');
  await bars().first().waitFor({ timeout: 3000 });
  await bars().first().locator('button', { hasText: 'History' }).click();
  assert((await page.locator('.tabs button.on').textContent()) === 'History', 'History is not open');
});

await check('Reset takes it away', async () => {
  page.once('dialog', (d) => d.accept());
  await page.locator('button', { hasText: 'Reset to the published project' }).click();
  await tab('Viewer');
  await page.waitForTimeout(500);
  assert((await bars().count()) === 0, 'still shown');
});

await check('a new tab has no bar', async () => {
  await page.evaluate(async (pcb) => {
    const { browserStore } = await import('/store.js');
    await browserStore({ project: 'p', snapshot: './snapshot/p/' }).write(pcb, '(kicad_pcb again)', { app: 'composer' });
  }, PCB);
  await bars().first().waitFor({ timeout: 3000 });
  const fresh = await context.newPage();
  await fresh.goto(`${ORIGIN}/index.html?project=p&tab=board`);
  await fresh.locator('.bv').waitFor();
  await fresh.waitForTimeout(500);
  assert((await fresh.locator('.stale').count()) === 0, 'a bar in the new tab');
});

await check('no page errors', async () => assert(!errors.length, errors.join('; ')));

await browser.close();
process.exit(failed ? 1 : 0);
