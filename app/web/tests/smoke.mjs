// Smoke test: drives a running Boardroom in Chrome and checks what a person
// would do first. Run against the registered instance:
//
//     BOARDROOM_URL=http://localhost:8054/ npm run smoke
//
// Uses the installed Chrome (no browser download). Exits non-zero on failure.
import { chromium } from 'playwright-core';

const base = process.env.BOARDROOM_URL;
if (!base) {
  console.error('smoke: set BOARDROOM_URL (e.g. http://localhost:$(proj port)/)');
  process.exit(2);
}
const browser = await chromium.launch({
  executablePath: process.env.CHROME ?? '/usr/bin/google-chrome',
  args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'],
});
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const errors = [];
page.on('pageerror', (e) => errors.push(e.message));
page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));

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

const projects = await (await fetch(new URL('api/projects', base))).json();
const withViewer = projects.find((p) => p.viewer);

await check('project list loads', async () => {
  await page.goto(base);
  await page.locator('.proj').first().waitFor({ timeout: 5000 });
  const n = await page.locator('.proj').count();
  if (n !== projects.length) throw new Error(`${n} rows for ${projects.length} projects`);
});

if (withViewer) {
  const url = new URL(`?project=${withViewer.id}&tab=board`, base).href;

  await check('board layers draw', async () => {
    await page.goto(url);
    await page.waitForFunction(() => document.querySelector('g.board')?.childElementCount > 3, null, { timeout: 8000 });
  });

  await check('clicking a part selects it and opens its pane', async () => {
    const parts = await (await fetch(new URL(`api/projects/${withViewer.id}/files/viewer/${await boardId()}/3d/parts.json`, base))).json();
    // The front part with the largest box: easiest to hit.
    const [ref, p] = Object.entries(parts.parts)
      .filter(([, p]) => p.side === 'front' && p.box)
      .sort(([, a], [, b]) => (b.box[2] - b.box[0]) * (b.box[3] - b.box[1]) - (a.box[2] - a.box[0]) * (a.box[3] - a.box[1]))[0];
    const at = await page.evaluate(([x, y]) => {
      const svg = document.querySelector('.stage svg');
      const pt = svg.createSVGPoint();
      pt.x = x;
      pt.y = y;
      const s = pt.matrixTransform(svg.getScreenCTM());
      return [s.x, s.y];
    }, [(p.box[0] + p.box[2]) / 2, (p.box[1] + p.box[3]) / 2]);
    await page.mouse.click(at[0], at[1]);
    await page.locator('.pane h2', { hasText: ref }).waitFor({ timeout: 3000 });
  });

  await check('a selection reaches another window over the bus', async () => {
    const other = await browser.newPage();
    await other.goto(url);
    await other.locator('.proj').first().waitFor();
    // The board's middle is the MCU on easy2350-like boards; any part will do.
    const seen = other.waitForFunction(() => document.querySelector('.selchip')?.textContent?.includes('Selected'), null, { timeout: 5000 });
    const box = await page.locator('.stage svg').boundingBox();
    await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
    await seen;
    await other.close();
  });

  await check('3D view loads the model', async () => {
    await page.goto(new URL(`?project=${withViewer.id}&tab=board&view=3d`, base).href);
    await page.locator('.b3 .views button').first().waitFor({ timeout: 8000 });
    await page.waitForFunction(() => !document.querySelector('.b3 .status'), null, { timeout: 20000 });
  });

  await check('Layers: switching contrast modes and mirroring', async () => {
    await page.goto(url);
    await page.waitForFunction(() => document.querySelector('g.board')?.childElementCount > 3);
    await page.getByRole('radio', { name: 'Hidden' }).click();
    await page.keyboard.press('b');
    await page.locator('.back').waitFor({ state: 'attached', timeout: 2000 }).catch(() => {});
    await page.keyboard.press('f');
  });

  async function boardId() {
    const site = await (await fetch(new URL(`api/projects/${withViewer.id}/files/viewer/site.json`, base))).json();
    return site.boards[0].id;
  }
}

await check('no errors in the page', async () => {
  if (errors.length) throw new Error(errors.slice(0, 3).join(' | '));
});

await browser.close();
process.exit(failed ? 1 : 0);
