// The Board tab drawing a board straight from its .kicad_pcb (docs/demo-spec.md, milestone 5), in
// the Pages build, in Chrome, on a made-up origin whose snapshot is the worked example's board:
//
//     scripts/draw.sh && npm run build:pages && npm run test:board
//
// It draws easy2350 (its parts, tracks and vias counted), a click picks a part, a save of the
// board from another app redraws it, and it has no stale bar.
import { chromium } from 'playwright-core';
import { readFileSync, existsSync } from 'node:fs';
import { extname } from 'node:path';

const ORIGIN = 'http://pages.test';
const DIST = new URL('../dist-pages', import.meta.url).pathname;
const EXAMPLE = new URL('../../../examples/easy2350/', import.meta.url).pathname;
const STORE = readFileSync(new URL('../../store/store.js', import.meta.url), 'utf8');
const PCB = 'hw/easy2350/a/easy2350.kicad_pcb';
const PRO = 'hw/easy2350/a/easy2350.kicad_pro';
const project = {
  id: 'p', name: 'easy2350', board: 'easy2350', path: 'easy2350', boards: [{ board: 'easy2350', variant: 'a', kicad_pro: PRO }],
  exists: true, is_project: true, error: null, phase: null, gates: [], checks: [], needs_user: [], docs: [], links: [],
  apps: {}, pinned: false, updated: '2026-10-31', thumbnail: null, viewer: null, listings: {},
};
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.wasm': 'application/wasm' };

if (!existsSync(`${DIST}/index.html`)) {
  console.error('test:board: build the Pages page first: scripts/draw.sh && npm run build:pages');
  process.exit(2);
}
const browser = await chromium.launch({
  executablePath: process.env.CHROME ?? '/usr/bin/google-chrome',
  args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'],
});
const context = await browser.newContext({ viewport: { width: 1400, height: 900 } });
await context.route(`${ORIGIN}/**`, (route) => {
  const path = decodeURIComponent(new URL(route.request().url()).pathname);
  const type = { contentType: TYPES[extname(path)] ?? 'application/octet-stream' };
  if (path === '/store.js') return route.fulfill({ body: STORE, contentType: 'text/javascript' });
  if (path === '/snapshot/projects.json') return route.fulfill({ body: JSON.stringify([project]), ...type });
  if (path.startsWith('/snapshot/p/')) {
    const file = EXAMPLE + path.slice('/snapshot/p/'.length);
    return existsSync(file) ? route.fulfill({ body: readFileSync(file), ...type }) : route.fulfill({ status: 404, body: 'no' });
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
const stats = page.locator('.live .stats');
const counted = async () => Object.fromEntries([...(await stats.textContent()).matchAll(/(\d+) (parts|tracks|vias)/g)].map((m) => [m[2], +m[1]]));
await page.goto(`${ORIGIN}/index.html?project=p&tab=board`);

await check('draws the board from its .kicad_pcb', async () => {
  await stats.waitFor({ timeout: 30000 });
  const n = await counted();
  assert(n.parts === 58 && n.tracks > 500 && n.vias > 100, JSON.stringify(n));
  // WebGL's buffer is gone after each frame, so look at a screenshot: an empty canvas is one
  // colour and compresses to almost nothing, a drawn board doesn't.
  await page.waitForTimeout(300);
  const png = await page.locator('.live canvas').screenshot();
  assert(png.length > 50000, `the canvas looks empty (${png.length} bytes of PNG)`);
});

await check('no stale bar over it', async () => assert((await page.locator('.stale').count()) === 0, 'a bar is shown'));

await check('a click on a part selects it', async () => {
  const box = await page.locator('.live canvas').boundingBox();
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await page.locator('.selchip').waitFor({ timeout: 2000 });
  assert(/Selected: \w+/.test(await page.locator('.selchip').textContent()), 'no selection');
});

await check("another app's save redraws it", async () => {
  await page.evaluate(async (pcb) => {
    const { browserStore } = await import('/store.js');
    const s = browserStore({ project: 'p', snapshot: './snapshot/p/' });
    const cur = await s.read(pcb);
    const text = new TextDecoder().decode(cur.bytes).replace(/\n\t\(segment[\s\S]*?\n\t\)/g, '');
    await s.write(pcb, text, { app: 'route', message: 'tracks gone', base: cur.rev });
  }, PCB);
  await page.waitForFunction(() => /\b0 tracks/.test(document.querySelector('.live .stats')?.textContent ?? ''), null, { timeout: 15000 });
  const n = await counted();
  assert(n.parts === 58 && n.tracks === 0, JSON.stringify(n));
});

await check('no page errors', async () => assert(!errors.length, errors.join('; ')));

await browser.close();
process.exit(failed ? 1 : 0);
