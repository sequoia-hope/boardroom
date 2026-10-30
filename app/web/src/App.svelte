<script>
  import * as api from './lib/api.js';
  import Overview from './lib/Overview.svelte';
  import Docs from './lib/Docs.svelte';
  import Renders from './lib/Renders.svelte';
  import History from './lib/History.svelte';
  import BoardView from './lib/board/BoardView.svelte';

  // ---- remembered per viewer (window mode, last project, theme) ----
  const remember = (key, fallback) => {
    try {
      return localStorage.getItem(`boardroom.${key}`) ?? fallback;
    } catch {
      return fallback;
    }
  };
  const keep = (key, value) => {
    try {
      localStorage.setItem(`boardroom.${key}`, value);
    } catch {}
  };

  let projects = $state([]);
  let examples = $state([]);
  let loadError = $state('');
  // ?project=<id>&tab=<name> opens straight to a project (windows, links).
  const params = new URLSearchParams(location.search);
  let selId = $state(params.get('project') ?? remember('project', null));
  let view = $state(params.get('view') === 'inbox' ? 'inbox' : 'project'); // project | inbox
  let tab = $state(params.get('tab') ?? 'overview'); // overview | board | gates | documents | renders | history | viewer | composer | studio
  let gateDoc = $state(null);
  let doc = $state(null);
  // Pages has no desktop to open windows on, so it is always single-window.
  let mode = $state(api.STATIC ? 'single' : (params.get('mode') ?? remember('mode', 'windows')));
  let theme = $state(remember('theme', 'auto'));
  let adding = $state(false);
  let addPath = $state('');
  let addError = $state('');
  let toast = $state('');
  let topH = $state(62);
  let selection = $state(null); // {from, board, refs} — the last selection any app reported
  let composerFrame = $state(null);
  // The apps embedded in single-window mode load on first visit and then stay loaded, hidden
  // behind the other tabs, so going back to one finds it as it was left.
  const FRAMES = ['viewer', 'composer', 'studio'];
  let loaded = $state(FRAMES.includes(tab) ? [tab] : []);
  $effect(() => {
    if (FRAMES.includes(tab) && !loaded.includes(tab)) loaded.push(tab);
  });

  const sel = $derived(projects.find((p) => p.id === selId) ?? projects[0] ?? null);
  // The selected project's files, as every app reads and writes them.
  const store = $derived(sel ? api.store(sel.id) : null);
  const sorted = $derived(
    [...projects].sort((a, b) => (b.pinned - a.pinned) || (b.updated ?? '').localeCompare(a.updated ?? '')),
  );
  const inbox = $derived(projects.flatMap((p) => p.needs_user.map((n) => ({ ...n, project: p }))));

  $effect(() => {
    if (sel) keep('project', sel.id);
  });
  $effect(() => {
    keep('mode', mode);
    if (mode === 'windows') {
      if (FRAMES.includes(tab)) tab = 'overview';
      loaded = [];
    }
  });
  $effect(() => {
    keep('theme', theme);
    if (theme === 'auto') delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = theme;
  });
  $effect(() => {
    document.title = sel ? `${sel.board ?? sel.name} · Boardroom` : 'Boardroom';
  });

  async function refresh() {
    try {
      [projects, examples] = await Promise.all([api.projects(), api.examples()]);
      loadError = '';
    } catch (e) {
      loadError = e.message;
    }
  }

  // The bus says which project changed; refetch at most every 400 ms. A
  // selection from another window is shown and passed to an embedded app.
  let pending = null;
  let sendBus = () => {};
  $effect(() => {
    refresh();
    const conn = api.bus((msg) => {
      if (msg.type === 'select') {
        if (msg.project === selId) {
          selection = msg;
          if (msg.from !== 'composer') composerFrame?.contentWindow?.postMessage({ type: 'select', board: msg.board, refs: msg.refs }, '*');
        }
        return;
      }
      clearTimeout(pending);
      pending = setTimeout(refresh, 400);
    });
    sendBus = conn.send;
    return conn.close;
  });

  // Circuit Composer, embedded, posts its selection to its parent; until its
  // server joins the bus, Boardroom relays it there for every other window.
  $effect(() => {
    const onMessage = (e) => {
      const composerOrigin = new URL(api.COMPOSER, location.href).origin;
      if (e.origin !== composerOrigin || e.data?.source !== 'circuit-composer' || e.data.type !== 'select') return;
      const msg = { type: 'select', from: 'composer', project: selId, board: e.data.board, refs: e.data.refs ?? [] };
      selection = msg;
      sendBus(msg);
    };
    window.addEventListener('message', onMessage);
    return () => window.removeEventListener('message', onMessage);
  });

  // A selection made in Boardroom itself (the Board tab): shown, sent to the
  // bus for other windows, and passed to an embedded Composer.
  function select(refs, from = 'boardroom') {
    const msg = { type: 'select', from, project: selId, board: sel?.boards[0]?.board ?? sel?.name, refs };
    selection = msg;
    sendBus(msg);
    composerFrame?.contentWindow?.postMessage({ type: 'select', board: msg.board, refs }, '*');
  }

  const composerBoard = $derived(sel?.boards[0]?.board ?? sel?.board ?? sel?.name);
  const composerUrl = (embed) =>
    `${api.COMPOSER}?board=${encodeURIComponent(composerBoard)}&project=${encodeURIComponent(sel.path)}${embed ? '&embed=1' : ''}&${api.storeQuery(store)}`;

  function openComposer() {
    if (mode === 'single') {
      tab = 'composer';
      return;
    }
    const w = window.open(composerUrl(false), `composer-${sel.id}`, 'popup,width=1500,height=950');
    if (!w) flash('The browser blocked the window; allow pop-ups for Boardroom.');
  }

  const studioUrl = () => {
    const b = sel.boards[0];
    const q = new URLSearchParams({ project: sel.path, ...(b ? { board: b.board, variant: b.variant } : {}) });
    return `${api.STUDIO}?${q}&${api.storeQuery(store)}`;
  };

  function openStudio() {
    if (mode === 'single') {
      tab = 'studio';
      return;
    }
    const w = window.open(studioUrl(), `studio-${sel.id}`, 'popup,width=1500,height=950');
    if (!w) flash('The browser blocked the window; allow pop-ups for Boardroom.');
  }

  function flash(msg) {
    toast = msg;
    setTimeout(() => (toast = ''), 3500);
  }

  function choose(p) {
    if (p.id !== selId) loaded = FRAMES.includes(tab) ? [tab] : [];
    selId = p.id;
    view = 'project';
    if ((tab === 'viewer' || tab === 'board') && !p.viewer) tab = 'overview';
  }

  async function act(what) {
    try {
      await api.open(sel.id, what);
      flash(what === 'kicad' ? 'Opening KiCad…' : 'Opening the folder…');
    } catch (e) {
      flash(e.message);
    }
  }

  function openViewer() {
    if (mode === 'single') {
      tab = 'viewer';
      return;
    }
    const w = window.open(api.fileUrl(sel.id, sel.viewer), `viewer-${sel.id}`, 'popup,width=1400,height=900');
    if (!w) flash('The browser blocked the window; allow pop-ups for Boardroom.');
  }

  function gotoGate(g) {
    if (!g.file) return;
    gateDoc = g.file;
    tab = 'gates';
  }

  // A link to a project document opens in its tab; anything else opens as a file.
  function followLink(e, l) {
    if (/^design\/gates\/.+\.md$/.test(l.href)) {
      e.preventDefault();
      gateDoc = l.href;
      tab = 'gates';
    } else if (/^design\/[^/]+\.md$/.test(l.href)) {
      e.preventDefault();
      doc = l.href;
      tab = 'documents';
    } else if (/^design\/renders\/?$/.test(l.href)) {
      e.preventDefault();
      tab = 'renders';
    } else {
      e.preventDefault();
      window.open(e.currentTarget.href, '_blank');
    }
  }

  async function pick() {
    addError = '';
    try {
      const { path } = await api.pickFolder();
      if (path) addPath = path;
    } catch (e) {
      addError = e.message;
    }
  }

  async function add() {
    addError = '';
    try {
      const { id } = await api.addProject(addPath);
      await refresh();
      selId = id;
      adding = false;
      addPath = '';
    } catch (e) {
      addError = e.message;
    }
  }

  async function removeSel() {
    if (!confirm(`Take ${sel.board ?? sel.name} off the list? The folder is not touched.`)) return;
    await api.removeProject(sel.id);
    selId = null;
    refresh();
  }

  const tabs = $derived([
    ['overview', 'Overview'],
    ...(sel?.viewer ? [['board', 'Board']] : []),
    ['gates', 'Gates'],
    ['documents', 'Documents'],
    ['renders', 'Renders'],
    ['history', 'History'],
    ...(mode === 'single' && sel?.viewer ? [['viewer', 'Viewer']] : []),
    ...(mode === 'single' ? [['composer', 'Composer']] : []),
    ...(mode === 'single' && api.STUDIO ? [['studio', 'Route']] : []),
  ]);

  const gateTitles = $derived(
    Object.fromEntries((sel?.gates ?? []).filter((g) => g.file).map((g) => [g.file, `${g.phase} · ${g.name}`])),
  );
  const docTitles = { 'design/intent.md': 'Requirements', 'design/decisions.md': 'Decisions', 'design/brief.md': 'Brief' };
</script>

<div class="shell">
  <aside>
    <div class="brand">
      <svg viewBox="0 0 32 32" width="22" height="22" aria-hidden="true">
        <rect x="3" y="5" width="26" height="22" rx="3" fill="var(--accent)" />
        <circle cx="10" cy="12" r="2.2" fill="var(--bg)" />
        <circle cx="22" cy="20" r="2.2" fill="var(--bg)" />
        <path d="M10 12h6l6 8" stroke="var(--bg)" stroke-width="2" fill="none" />
      </svg>
      <span>Boardroom</span>
    </div>

    <button class="inbox-btn" class:on={view === 'inbox'} onclick={() => (view = 'inbox')}>
      Inbox
      {#if inbox.length}<span class="badge">{inbox.length}</span>{/if}
    </button>

    <div class="label list-head">Projects</div>
    <ul class="plist">
      {#each sorted as p (p.id)}
        <li>
          <button class="proj" class:on={view === 'project' && sel?.id === p.id} onclick={() => choose(p)}>
            <span class="pname">{p.board ?? p.name}{#if p.example}<span class="extag">example</span>{/if}</span>
            <span class="pmeta">
              {#if !p.exists}missing{:else if p.phase != null}{p.phase} · {p.gates[p.phase]?.name}{:else if !p.is_project}folder{:else}not started{/if}
            </span>
            {#if p.needs_user.length}<span class="badge small">{p.needs_user.length}</span>{/if}
          </button>
        </li>
      {/each}
    </ul>

    {#if examples.some((e) => !e.project)}
      <div class="label list-head">Examples not on this machine</div>
      <ul class="plist">
        {#each examples.filter((e) => !e.project) as e (e.name)}
          <li class="ex">
            <span class="pname">{e.name}</span>
            {#if e.repo}
              <span class="pmeta">Clone it, then add the folder:</span>
              <code class="mono clone">git clone {e.repo}</code>
            {:else}
              <span class="pmeta mono">{e.in_repo}</span>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}

    {#if !api.STATIC}
      {#if adding}
        <form class="add" onsubmit={(e) => (e.preventDefault(), add())}>
          <input bind:value={addPath} placeholder="/path/to/project" aria-label="Project folder" />
          <div class="row">
            <button type="button" class="btn" onclick={pick}>Choose…</button>
            <button class="btn primary" disabled={!addPath.trim()}>Add</button>
            <button type="button" class="btn" onclick={() => ((adding = false), (addError = ''))}>Cancel</button>
          </div>
          {#if addError}<p class="err">{addError}</p>{/if}
        </form>
      {:else}
        <button class="btn add-btn" onclick={() => (adding = true)}>+ Add project</button>
      {/if}
    {/if}

    <div class="foot">
      {#if !api.STATIC}
        <label title="Windows: the viewer and apps open in their own windows. Single window: they open as tabs here.">
          <span class="label">Mode</span>
          <select bind:value={mode}>
            <option value="windows">Windows</option>
            <option value="single">Single window</option>
          </select>
        </label>
      {/if}
      <label>
        <span class="label">Theme</span>
        <select bind:value={theme}>
          <option value="auto">Auto</option>
          <option value="light">Light</option>
          <option value="dark">Dark</option>
        </select>
      </label>
    </div>
  </aside>

  <main style="--top-h: {topH}px">
    {#if api.STATIC}
      <div class="demo">
        A read-only snapshot of the example project. Boardroom itself runs on your desktop, where it
        opens your project folders, KiCad, and the suite's other apps.
        <a href="https://github.com/sequoia-hope/boardroom">github.com/sequoia-hope/boardroom</a>
      </div>
    {/if}

    {#if loadError}
      <p class="err pad">Can't reach the Boardroom server: {loadError}</p>
    {:else if view === 'inbox'}
      <header class="top"><h1>Inbox</h1></header>
      <div class="body">
        {#each inbox as n (n.project.id + n.id)}
          <button class="card inbox-item" onclick={() => choose(n.project)}>
            <span class="label">{n.project.board ?? n.project.name} · {n.kind === 'signoff' ? 'sign-off' : n.kind === 'alert' ? 'problem' : 'question'}</span>
            <span>{n.text}</span>
          </button>
        {:else}
          <p class="muted">Nothing is waiting on you in any project.</p>
        {/each}
      </div>
    {:else if !sel}
      <div class="empty">
        <h1>No projects yet</h1>
        <p class="muted">Add a project folder: any folder with a <span class="mono">design/status.json</span>, made from the template.</p>
        {#if !api.STATIC}<button class="btn primary" onclick={() => (adding = true)}>+ Add project</button>{/if}
      </div>
    {:else}
      <header class="top" bind:clientHeight={topH}>
        <div class="crumbs">
          <h1>{sel.board ?? sel.name}</h1>
          {#each sel.boards as b}<span class="crumb">{b.board} / {b.variant}</span>{/each}
          {#if selection?.refs?.length}
            <span class="selchip" title="Selected in {selection.from ?? 'another window'}">Selected: {selection.refs.join(', ')}</span>
          {/if}
        </div>
        <div class="open-in">
          <span class="label">Open in</span>
          <button class="btn" title="Circuit Composer, board {composerBoard} (prototype)" onclick={openComposer}>Composer</button>
          <button class="btn" disabled={!api.STUDIO} title={api.STUDIO ? `Route in UltraRoute Studio, board ${composerBoard}` : 'Studio opens a project from this machine, not from the Pages site'} onclick={openStudio}>Route</button>
          <button class="btn" disabled={!sel.viewer} title={sel.viewer ? 'pcbview' : 'No viewer built yet (scripts/view.sh build)'} onclick={openViewer}>Viewer</button>
          {#if !api.STATIC}
            <button class="btn" disabled={!sel.boards.some((b) => b.kicad_pro)} onclick={() => act('kicad')}>KiCad</button>
            <button class="btn" onclick={() => act('folder')}>Folder</button>
            <button class="btn ghost" title="Take off the list" onclick={removeSel}>✕</button>
          {/if}
        </div>
      </header>

      <nav class="tabs">
        {#each tabs as [key, title]}
          <button class:on={tab === key} onclick={() => (tab = key)}>{title}</button>
        {/each}
      </nav>

      <div class="body" class:flush={FRAMES.includes(tab) || tab === 'board'}>
        {#if !sel.exists}
          <p class="err">This folder is missing: <span class="mono">{sel.path}</span></p>
        {:else if sel.error}
          <p class="err">{sel.error}</p>
        {/if}
        {#if sel.exists}
          {#if tab === 'overview'}
            <Overview project={sel} ongate={gotoGate} onlink={followLink} />
          {:else if tab === 'gates'}
            <Docs project={sel} filter={(f) => f.startsWith('design/gates/') && !f.endsWith('README.md')} bind:selected={gateDoc} titles={gateTitles} />
          {:else if tab === 'documents'}
            <Docs project={sel} filter={(f) => !f.startsWith('design/gates/')} bind:selected={doc} titles={docTitles} />
          {:else if tab === 'board'}
            <BoardView project={sel} selected={selection?.project === sel.id ? (selection.refs ?? []) : []} onselect={(refs) => select(refs)} />
          {:else if tab === 'renders'}
            <Renders project={sel} />
          {:else if tab === 'history'}
            <History {store} />
          {/if}
          {#if loaded.includes('composer')}
            <iframe class="viewer" hidden={tab !== 'composer'} bind:this={composerFrame} src={composerUrl(true)} title="Circuit Composer" allow="cross-origin-isolated"></iframe>
          {/if}
          {#if loaded.includes('studio') && api.STUDIO}
            <iframe class="viewer" hidden={tab !== 'studio'} src={studioUrl()} title="UltraRoute Studio" allow="cross-origin-isolated"></iframe>
          {/if}
          {#if loaded.includes('viewer') && sel.viewer}
            <iframe class="viewer" hidden={tab !== 'viewer'} src={api.fileUrl(sel.id, sel.viewer)} title="Board viewer"></iframe>
          {/if}
        {/if}
      </div>
    {/if}
  </main>
</div>

{#if toast}<div class="toast" role="status">{toast}</div>{/if}

<style>
  .shell {
    display: grid;
    grid-template-columns: 250px minmax(0, 1fr);
    height: 100%;
  }

  aside {
    border-right: 1px solid var(--line);
    background: var(--panel);
    display: flex;
    flex-direction: column;
    padding: 12px 10px;
    gap: 6px;
    overflow: auto;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 700;
    font-size: 16px;
    padding: 2px 6px 10px;
  }

  .inbox-btn,
  .proj {
    width: 100%;
    border: none;
    background: none;
    text-align: left;
    border-radius: 6px;
    padding: 7px 8px;
  }

  .inbox-btn {
    display: flex;
    justify-content: space-between;
    font-weight: 600;
  }

  .inbox-btn:hover,
  .proj:hover {
    background: var(--panel-2);
  }

  .inbox-btn.on,
  .proj.on {
    background: var(--accent-soft);
  }

  .list-head {
    padding: 10px 8px 2px;
  }

  .plist {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .proj {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 0 6px;
  }

  .pname {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .extag {
    margin-left: 6px;
    font-size: 10.5px;
    font-weight: 600;
    padding: 0 6px;
    border-radius: 8px;
    border: 1px solid var(--line);
    color: var(--muted);
  }

  .ex {
    display: flex;
    flex-direction: column;
    padding: 6px 8px;
    gap: 2px;
  }

  .clone {
    font-size: 11px;
    user-select: all;
    overflow-wrap: anywhere;
    color: var(--text);
  }

  .pmeta {
    grid-column: 1;
    font-size: 12px;
    color: var(--muted);
  }

  .proj .badge {
    grid-row: 1 / span 2;
    grid-column: 2;
    align-self: center;
  }

  .badge {
    background: var(--warn);
    color: var(--bg);
    font-size: 11px;
    font-weight: 700;
    border-radius: 9px;
    padding: 0 7px;
    min-width: 18px;
    text-align: center;
  }

  .add-btn {
    margin: 6px 4px;
  }

  .add {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 6px 4px;
  }

  .add input,
  select {
    font: inherit;
    color: inherit;
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 5px 8px;
  }

  .row {
    display: flex;
    gap: 6px;
  }

  .foot {
    margin-top: auto;
    display: flex;
    gap: 8px;
    padding: 10px 4px 0;
    border-top: 1px solid var(--line);
  }

  .foot label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    flex: 1;
  }

  main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    overflow: auto;
  }

  .demo {
    background: var(--accent-soft);
    border-bottom: 1px solid var(--line);
    padding: 8px 20px;
    font-size: 13px;
  }

  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
    padding: 16px 20px 10px;
  }

  .top h1 {
    font-size: 22px;
  }

  .crumbs {
    display: flex;
    align-items: baseline;
    gap: 10px;
    flex-wrap: wrap;
  }

  .crumb {
    color: var(--muted);
    font-size: 13px;
  }

  .crumb::before {
    content: '▸ ';
  }

  .selchip {
    font-size: 12px;
    padding: 1px 8px;
    border-radius: 10px;
    background: var(--accent-soft);
    color: var(--accent);
  }

  .open-in {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .ghost {
    box-shadow: none;
    background: none;
    border-color: transparent;
    color: var(--muted);
  }

  .demo,
  .top,
  .tabs {
    flex-shrink: 0;
  }

  /* The project's name, Open-in buttons and tabs stay put while the body scrolls. */
  .top {
    position: sticky;
    top: 0;
    z-index: 5;
    background: var(--bg);
  }

  .tabs {
    position: sticky;
    top: var(--top-h, 62px);
    z-index: 5;
    background: var(--bg);
    display: flex;
    gap: 2px;
    padding: 0 16px;
    border-bottom: 1px solid var(--line);
    overflow-x: auto;
  }

  .tabs button {
    border: none;
    background: none;
    padding: 8px 12px;
    border-bottom: 2px solid transparent;
    color: var(--muted);
    white-space: nowrap;
  }

  .tabs button.on {
    color: var(--text);
    border-bottom-color: var(--accent);
    font-weight: 600;
  }

  .body {
    padding: 16px 20px 28px;
    flex: 1;
  }

  .body.flush {
    padding: 0;
    display: flex;
    min-height: 0;
    overflow: hidden;
  }

  .viewer {
    flex: 1;
    border: none;
    width: 100%;
    min-height: 600px;
  }

  .inbox-item {
    display: flex;
    flex-direction: column;
    gap: 4px;
    width: 100%;
    text-align: left;
    padding: 12px 14px;
    margin-bottom: 10px;
  }

  .inbox-item:hover {
    border-color: var(--accent);
  }

  .empty {
    margin: auto;
    text-align: center;
    max-width: 420px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    align-items: center;
  }

  .err {
    color: var(--bad);
    margin: 4px 0 10px;
  }

  .pad {
    padding: 20px;
  }

  .toast {
    position: fixed;
    bottom: 18px;
    left: 50%;
    transform: translateX(-50%);
    background: var(--text);
    color: var(--bg);
    padding: 8px 14px;
    border-radius: 8px;
    font-size: 13px;
    z-index: 30;
  }

  @media (max-width: 760px) {
    .shell {
      grid-template-columns: 1fr;
      grid-template-rows: auto 1fr;
      height: auto;
      min-height: 100%;
    }

    aside {
      border-right: none;
      border-bottom: 1px solid var(--line);
      max-height: 45vh;
    }

    .top,
    .body {
      padding-left: 16px;
      padding-right: 16px;
    }
  }
</style>
