// The store (app/store/store.js) in Chrome, on a made-up origin served from here, so it needs no
// running server:
//
//     npm run test:store
//
// The browser backend: reads fall through to the snapshot, writes are revisions, a stale write is
// a conflict, a reload keeps the tab's workspace and a new tab starts fresh, other windows on the
// same workspace hear of a change, Restore, Reset, Download changes (checked with Python's
// zipfile), and old workspaces are swept. The server backend against a stand-in for Boardroom's
// file API: ETags, If-Match, 412.
import { chromium } from 'playwright-core';
import { readFileSync, writeFileSync, mkdtempSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const ORIGIN = 'http://store.test';
const STORE = readFileSync(new URL('../../store/store.js', import.meta.url), 'utf8');
const SNAPSHOT = { 'hw/b/a/b.kicad_pcb': '(kicad_pcb published)', 'design/status.json': '{"v":1}' };
// The stand-in server: path -> bytes, and the revision each has (its length and first byte).
const disk = new Map([['hw/b/a/b.kicad_pcb', '(kicad_pcb on disk)']]);
const rev = (s) => `r${s.length}-${s.charCodeAt(0)}`;

const browser = await chromium.launch({ executablePath: process.env.CHROME ?? '/usr/bin/google-chrome' });
const context = await browser.newContext();
await context.route(`${ORIGIN}/**`, async (route) => {
  const req = route.request();
  const url = new URL(req.url());
  if (url.pathname === '/store.js') return route.fulfill({ body: STORE, contentType: 'text/javascript' });
  if (url.pathname.startsWith('/snapshot/p/')) {
    const body = SNAPSHOT[decodeURIComponent(url.pathname.slice('/snapshot/p/'.length))];
    return body === undefined ? route.fulfill({ status: 404, body: 'no' }) : route.fulfill({ body });
  }
  if (url.pathname.startsWith('/api/projects/p/files/')) {
    const path = decodeURIComponent(url.pathname.slice('/api/projects/p/files/'.length));
    const now = disk.get(path);
    const etag = now === undefined ? {} : { etag: `"${rev(now)}"` };
    if (req.method() === 'GET') return now === undefined ? route.fulfill({ status: 404, body: 'no' }) : route.fulfill({ body: now, headers: etag });
    const h = req.headers();
    const ok = h['if-match'] ? now !== undefined && h['if-match'] === `"${rev(now)}"` : h['if-none-match'] === '*' ? now === undefined : true;
    if (!ok) return route.fulfill({ status: 412, body: 'changed', headers: etag });
    const body = req.postDataBuffer().toString();
    disk.set(path, body);
    return route.fulfill({ body: JSON.stringify({ rev: rev(body) }), headers: { etag: `"${rev(body)}"`, 'content-type': 'application/json' } });
  }
  if (url.pathname === '/api/projects/p/history') return route.fulfill({ body: '[]', contentType: 'application/json' });
  return route.fulfill({ body: '<!doctype html><title>store test</title>', contentType: 'text/html' });
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
const eq = (a, b, what) => {
  if (JSON.stringify(a) !== JSON.stringify(b)) throw new Error(`${what}: ${JSON.stringify(a)}, not ${JSON.stringify(b)}`);
};

async function tab(query = '') {
  const page = await context.newPage();
  page.on('pageerror', (e) => console.log(`page error: ${e.message}`));
  await page.goto(`${ORIGIN}/${query}`);
  await page.evaluate(async () => {
    window.S = await import('/store.js');
    window.store = window.S.browserStore({ project: 'p', snapshot: '/snapshot/p/' });
  });
  return page;
}

const PCB = 'hw/b/a/b.kicad_pcb';
const a = await tab();

await check('a read falls through to the published snapshot', async () => {
  eq(await a.evaluate(async (p) => { const f = await store.read(p); return [new TextDecoder().decode(f.bytes), f.rev]; }, PCB), ['(kicad_pcb published)', 'published'], 'read');
  eq(await a.evaluate(() => store.read('nope.txt')), null, 'a missing file');
});

await check('a write is a revision, and a stale one is a conflict', async () => {
  const r = await a.evaluate(async (p) => {
    const one = await store.write(p, '(kicad_pcb moved)', { app: 'composer', message: 'Placement', base: 'published' });
    let conflict = null;
    try {
      await store.write(p, '(kicad_pcb other)', { app: 'route', base: 'published' });
    } catch (e) {
      conflict = [e.name, e.current];
    }
    return [one.rev, conflict, await store.text(p)];
  }, PCB);
  eq(r, ['r1', ['Conflict', 'r1'], '(kicad_pcb moved)'], 'write');
});

await check('a reload keeps the tab\'s workspace', async () => {
  const ws = await a.evaluate(() => store.ws);
  await a.reload();
  await a.evaluate(async () => {
    window.S = await import('/store.js');
    window.store = window.S.browserStore({ project: 'p', snapshot: '/snapshot/p/' });
  });
  eq(await a.evaluate(() => store.ws), ws, 'workspace');
  eq(await a.evaluate((p) => store.text(p), PCB), '(kicad_pcb moved)', 'after reload');
});

await check('a new tab starts from the published project', async () => {
  const b = await tab();
  eq(await b.evaluate((p) => store.text(p), PCB), '(kicad_pcb published)', 'new tab');
  await b.close();
});

await check('a window given the workspace hears of a change, and its stale write is refused', async () => {
  const ws = await a.evaluate(() => store.ws);
  const c = await tab(`?ws=${ws}`);
  const base = await c.evaluate(async (p) => (await store.read(p)).rev, PCB);
  await c.evaluate(() => {
    window.heard = new Promise((resolve) => store.watch(resolve));
  });
  await a.evaluate((p) => store.write(p, '(kicad_pcb routed)', { app: 'route', message: 'Routed 83/83', base: 'r1' }), PCB);
  eq(await c.evaluate(() => window.heard), { paths: [PCB], app: 'route' }, 'heard');
  eq(await c.evaluate(async ([p, base]) => { try { await store.write(p, 'x', { app: 'composer', base }); return 'written'; } catch (e) { return e.name; } }, [PCB, base]), 'Conflict', 'stale write');
  await c.close();
});

await check('history, changes and Restore', async () => {
  const r = await a.evaluate(async (p) => {
    const before = (await store.history()).map((h) => [h.rev, h.app, h.message]);
    await store.restore('r1');
    return [before, (await store.changes()).map((c) => [c.path, c.rev]), await store.text(p), (await store.history(p))[0].message];
  }, PCB);
  eq(r[0], [['r2', 'route', 'Routed 83/83'], ['r1', 'composer', 'Placement']], 'history');
  eq(r[1], [[PCB, 'r3']], 'changes');
  eq(r[2], '(kicad_pcb moved)', 'restored');
  eq(r[3], 'Back to r1 (composer: Placement)', 'message');
});

await check('Download changes is a zip of the changed files at their paths', async () => {
  await a.evaluate(() => store.write('compose/b/placement.json', '{"parts":{}}', { app: 'composer' }));
  const b64 = await a.evaluate(async () => {
    const buf = new Uint8Array(await (await store.download()).arrayBuffer());
    let s = '';
    for (const x of buf) s += String.fromCharCode(x);
    return btoa(s);
  });
  const file = join(mkdtempSync(join(tmpdir(), 'store-')), 'changes.zip');
  writeFileSync(file, Buffer.from(b64, 'base64'));
  const out = execFileSync('python3', ['-c', 'import sys, zipfile, json; z = zipfile.ZipFile(sys.argv[1]); assert z.testzip() is None; print(json.dumps({n: z.read(n).decode() for n in z.namelist()}))', file]).toString();
  eq(JSON.parse(out), { 'compose/b/placement.json': '{"parts":{}}', [PCB]: '(kicad_pcb moved)' }, 'zip');
});

await check('Reset goes back to the published project', async () => {
  const r = await a.evaluate(async (p) => {
    const heard = new Promise((resolve) => store.watch(resolve));
    await store.reset();
    return [(await heard).paths.sort(), await store.text(p), await store.changes(), await store.history()];
  }, PCB);
  eq(r, [['compose/b/placement.json', PCB], '(kicad_pcb published)', [], []], 'reset');
});

await check('a workspace not opened for a week is swept', async () => {
  const left = await a.evaluate(async () => {
    const old = S.browserStore({ project: 'p', snapshot: '/snapshot/p/', ws: 'old-tab' });
    await old.write('x.txt', 'x', { app: 't' });
    const db = await new Promise((r) => { const q = indexedDB.open('boardroom-store'); q.onsuccess = () => r(q.result); });
    await new Promise((r) => { const tx = db.transaction('workspaces', 'readwrite'); const s = tx.objectStore('workspaces'); const g = s.get('old-tab'); g.onsuccess = () => s.put({ ...g.result, seen: Date.now() - 8 * 86400e3 }); tx.oncomplete = r; });
    db.close();
    const fresh = S.browserStore({ project: 'p', snapshot: '/snapshot/p/', ws: 'new-tab' });
    await fresh.read('x.txt');
    await new Promise((r) => setTimeout(r, 300));
    return [await S.browserStore({ project: 'p', snapshot: '/snapshot/p/', ws: 'old-tab' }).text('x.txt')];
  });
  eq(left, [null], 'old workspace');
});

await check('the server backend: ETags, If-Match and 412', async () => {
  const r = await a.evaluate(async (p) => {
    const s = S.serverStore('/api/projects/p/');
    const f = await s.read(p);
    const w = await s.write(p, '(kicad_pcb placed)', { app: 'composer', message: 'Placement ✓', base: f.rev });
    let conflict = null;
    try {
      await s.write(p, '(kicad_pcb late)', { app: 'route', base: f.rev });
    } catch (e) {
      conflict = [e.name, e.current];
    }
    let created = null;
    try {
      await s.write('new.txt', 'a', { base: null });
      await s.write('new.txt', 'b', { base: null });
    } catch (e) {
      created = e.name;
    }
    return [f.rev, w.rev, conflict, await s.text(p), created];
  }, PCB);
  eq(r, [rev('(kicad_pcb on disk)'), rev('(kicad_pcb placed)'), ['Conflict', rev('(kicad_pcb placed)')], '(kicad_pcb placed)', 'Conflict'], 'server');
});

await check('an app opened with ?store= finds it', async () => {
  const r = await a.evaluate(() => {
    const b = S.browserStore({ project: 'p', snapshot: '/snapshot/p/', ws: 'w1' });
    const q = S.query(b);
    const back = S.fromLocation(new URL(`/composer/?board=b&${q}`, location.href));
    const srv = S.fromLocation(new URL(`/composer/?${S.query(S.serverStore('/api/projects/p/'))}`, location.href));
    return [back.kind, back.project, back.ws, back.root, srv.kind, srv.root, S.fromLocation(new URL('/composer/', location.href))];
  });
  eq(r, ['browser', 'p', 'w1', `${ORIGIN}/snapshot/p/`, 'server', `${ORIGIN}/api/projects/p/`, null], 'fromLocation');
});

await browser.close();
console.log(failed ? `${failed} failed` : 'all passed');
process.exit(failed ? 1 : 0);
