// End-to-end Playwright test for the embedded Terminal tab against a real
// Boardroom server bound on 127.0.0.1:0 (--port 0) with term_command = ["/bin/sh"]:
//
//     npm run build && npm run build:pages && npm run test:terminal
//
// Verifies: starting a project terminal, sidebar live marker, typing into xterm
// and seeing /bin/sh output come back, reconnecting with scrollback replay after
// a dropped WebSocket, phone width (375 px) with no horizontal overflow, no
// `.stale` collision, and Terminal tab hidden in the static Pages build.
import { chromium } from 'playwright-core';
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { extname, join } from 'node:path';

const DIST = new URL('../dist', import.meta.url).pathname;
const DIST_PAGES = new URL('../dist-pages', import.meta.url).pathname;
const STORE = readFileSync(new URL('../store/store.js', import.meta.url), 'utf8');

if (!existsSync(`${DIST}/index.html`)) {
  console.error('test:terminal: build web/ first (npm run build)');
  process.exit(2);
}

function findServerBin() {
  if (process.env.BOARDROOM_BIN && existsSync(process.env.BOARDROOM_BIN)) {
    return process.env.BOARDROOM_BIN;
  }
  const rel = new URL('../../target/release/boardroom', import.meta.url).pathname;
  const dbg = new URL('../../target/debug/boardroom', import.meta.url).pathname;
  const candidates = [rel, dbg].filter((p) => existsSync(p));
  if (!candidates.length) {
    console.error('test:terminal: build the server first (cargo build or ./build.sh)');
    process.exit(2);
  }
  candidates.sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
  return candidates[0];
}

const root = mkdtempSync(join(tmpdir(), 'boardroom-web-term-'));
const xdgConfig = join(root, 'config');
const xdgState = join(root, 'state');
const xdgCache = join(root, 'cache');
const projDir = join(root, 'easy2350');

mkdirSync(join(xdgConfig, 'boardroom'), { recursive: true });
mkdirSync(xdgState, { recursive: true });
mkdirSync(xdgCache, { recursive: true });
mkdirSync(join(projDir, 'design'), { recursive: true });

writeFileSync(
  join(projDir, 'design/status.json'),
  JSON.stringify({ schema: 1, board: 'easy2350', phase: 4, needs_user: [] }, null, 2) + '\n',
);
writeFileSync(join(xdgConfig, 'boardroom/config.toml'), 'term_command = ["/bin/sh"]\n');
writeFileSync(
  join(xdgConfig, 'boardroom/projects.toml'),
  `[[projects]]\npath = ${JSON.stringify(projDir)}\npinned = false\n`,
);

const serverBin = findServerBin();
const serverProc = spawn(serverBin, ['serve', '--port', '0', '--web', DIST], {
  env: {
    ...process.env,
    XDG_CONFIG_HOME: xdgConfig,
    XDG_STATE_HOME: xdgState,
    XDG_CACHE_HOME: xdgCache,
  },
  stdio: ['ignore', 'pipe', 'pipe'],
});

let serverStderr = '';
serverProc.stderr.on('data', (d) => (serverStderr += d.toString()));

const baseUrl = await new Promise((resolve, reject) => {
  let buf = '';
  const timer = setTimeout(() => reject(new Error(`server startup timed out; stderr: ${serverStderr}`)), 10000);
  serverProc.stdout.on('data', (d) => {
    buf += d.toString();
    const m = buf.match(/Boardroom:\s+(http:\/\/localhost:\d+\/)/);
    if (m) {
      clearTimeout(timer);
      resolve(m[1].replace('localhost', '127.0.0.1'));
    }
  });
  serverProc.on('exit', (code) => {
    clearTimeout(timer);
    reject(new Error(`server exited early (${code}): ${serverStderr}`));
  });
});

let browser;
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

try {
  browser = await chromium.launch({
    executablePath: process.env.CHROME ?? '/usr/bin/google-chrome',
  });
  const context = await browser.newContext({ viewport: { width: 1280, height: 820 } });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));

  let activeTermWs = null;
  let wsConnections = 0;
  await page.routeWebSocket(/\/api\/term\/t-/, (ws) => {
    wsConnections++;
    ws.connectToServer();
    activeTermWs = ws;
  });

  await page.goto(baseUrl);
  await page.locator('.proj').first().waitFor({ timeout: 5000 });

  await check('Terminal tab is listed and sidebar has no live dot yet', async () => {
    await page.locator('.tabs button', { hasText: /^Terminal$/ }).waitFor({ timeout: 3000 });
    assert((await page.locator('.proj .term-dot').count()) === 0, 'unexpected .term-dot before starting');
  });

  await check('opening Terminal starts /bin/sh, echoes output, and lights sidebar marker', async () => {
    await page.locator('.tabs button', { hasText: /^Terminal$/ }).click();
    await page.locator('.term-wrap[data-term-state="live"]').waitFor({ timeout: 5000 });
    await page.locator('.proj .term-dot').waitFor({ state: 'attached', timeout: 5000 });

    // Focus xterm and run a command whose output string differs from the typed command.
    await page.locator('.term-mount .xterm-helper-textarea').focus();
    await page.keyboard.type("printf '%s-%s\\n' pty echo_ok");
    await page.keyboard.press('Enter');

    await page.waitForFunction(
      () => (document.querySelector('.xterm-rows')?.textContent ?? '').includes('pty-echo_ok'),
      null,
      { timeout: 5000 },
    );
    assert((await page.locator('.stale').count()) === 0, 'Terminal tab must not render .stale elements');
  });

  await check('dropped WebSocket reconnects automatically and replays scrollback', async () => {
    const beforeCount = wsConnections;
    assert(activeTermWs != null, 'expected intercepted WebSocket');
    await activeTermWs.close();

    // Wait until a new WebSocket connection is established and reaches live state.
    const deadline = Date.now() + 6000;
    while (wsConnections <= beforeCount && Date.now() < deadline) {
      await page.waitForTimeout(100);
    }
    assert(wsConnections > beforeCount, `expected reconnect (wsConnections=${wsConnections})`);
    await page.waitForFunction(() => document.querySelector('.term-wrap')?.dataset.termState === 'live', null, {
      timeout: 6000,
    });
    await page.waitForFunction(
      () => (document.querySelector('.xterm-rows')?.textContent ?? '').includes('pty-echo_ok'),
      null,
      { timeout: 5000 },
    );

    // Confirm the reconnected socket is interactive too.
    await page.locator('.term-mount .xterm-helper-textarea').focus();
    await page.keyboard.type("printf '%s:%s\\n' after drop_ok");
    await page.keyboard.press('Enter');
    await page.waitForFunction(
      () => (document.querySelector('.xterm-rows')?.textContent ?? '').includes('after:drop_ok'),
      null,
      { timeout: 5000 },
    );
  });

  await check('phone width (375 px) fits without horizontal overflow', async () => {
    await page.setViewportSize({ width: 375, height: 812 });
    await page.waitForTimeout(300);
    const metrics = await page.evaluate(() => {
      const host = document.querySelector('.term-wrap')?.getBoundingClientRect();
      return {
        scrollWidth: document.documentElement.scrollWidth,
        innerWidth: window.innerWidth,
        hostWidth: host?.width ?? 0,
      };
    });
    assert(
      metrics.scrollWidth <= metrics.innerWidth,
      `horizontal overflow at 375px: scrollWidth=${metrics.scrollWidth} > innerWidth=${metrics.innerWidth}`,
    );
    assert(metrics.hostWidth >= 360 && metrics.hostWidth <= 375, `unexpected terminal width ${metrics.hostWidth}`);
  });

  if (existsSync(`${DIST_PAGES}/index.html`)) {
    await check('Terminal tab is hidden in the static Pages build', async () => {
      const TYPES = {
        '.html': 'text/html',
        '.js': 'text/javascript',
        '.css': 'text/css',
        '.json': 'application/json',
      };
      const ORIGIN = 'http://pages-term.test';
      const snapProj = {
        id: 'p',
        name: 'b',
        board: 'b',
        path: 'b',
        boards: [{ board: 'b', variant: 'a', kicad_pro: null }],
        exists: true,
        is_project: true,
        error: null,
        phase: 4,
        gates: [],
        checks: [],
        needs_user: [],
        docs: [],
        links: [],
        apps: {},
        pinned: false,
        updated: '2026-11-06',
        thumbnail: null,
        viewer: null,
      };
      const staticCtx = await browser.newContext({ viewport: { width: 1280, height: 800 } });
      await staticCtx.route(`${ORIGIN}/**`, (route) => {
        const path = decodeURIComponent(new URL(route.request().url()).pathname);
        const type = { contentType: TYPES[extname(path)] ?? 'application/octet-stream' };
        if (path === '/store.js') return route.fulfill({ body: STORE, contentType: 'text/javascript' });
        if (path === '/snapshot/projects.json') {
          return route.fulfill({ body: JSON.stringify([snapProj]), contentType: 'application/json' });
        }
        const file = DIST_PAGES + (path === '/' ? '/index.html' : path);
        return existsSync(file)
          ? route.fulfill({ body: readFileSync(file), ...type })
          : route.fulfill({ status: 404, body: 'no' });
      });
      const sp = await staticCtx.newPage();
      sp.on('pageerror', (e) => errors.push(e.message));
      await sp.goto(`${ORIGIN}/index.html?project=p&tab=terminal`);
      await sp.locator('.overview').waitFor({ timeout: 5000 });
      assert(
        (await sp.locator('.tabs button', { hasText: /^Terminal$/ }).count()) === 0,
        'Terminal tab should be hidden when api.STATIC is true',
      );
      await staticCtx.close();
    });
  }

  await check('no page errors', async () => assert(!errors.length, errors.join('; ')));
} finally {
  await browser?.close().catch(() => {});
  serverProc.kill('SIGTERM');
  await new Promise((r) => {
    serverProc.once('exit', r);
    setTimeout(r, 2000);
  });
  rmSync(root, { recursive: true, force: true });
}

process.exit(failed ? 1 : 0);
