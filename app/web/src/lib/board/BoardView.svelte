<script>
  import { fileUrl } from '../api.js';
  import Layers2D from './Layers2D.svelte';
  import Stale from '../Stale.svelte';
  // three.js is most of the page's weight, and the live board's WASM is 2 MB: each loads only
  // when its view is opened.
  const load3D = () => import('./Board3D.svelte');
  const loadLive = () => import('./LiveBoard.svelte');

  // The Board tab. Board: the .kicad_pcb as it is now, through the store, drawn as Studio draws
  // it. Plots and 3D: pcbview's data (viewer/site.json and each board's layers/ and 3d/), made
  // before publishing, so they carry the stale bar once the board changes. The part pane is
  // shared. A selection here goes to every other window, and one made elsewhere shows here.

  let { project, store, change = null, onhistory, selected = [], onselect = () => {} } = $props();

  let site = $state(null);
  let boardId = $state(null);
  // ?view=3d or ?view=plots opens that view first.
  const asked = new URLSearchParams(location.search).get('view');
  let mode = $state(asked === '3d' ? '3d' : asked === 'plots' || !project.boards.length ? 'layers' : 'board');
  // The board Composer and Route open (the first), or another of the project's.
  let boardIndex = $state(0);
  const kicadBoard = $derived(project.boards[boardIndex] ?? project.boards[0] ?? null);
  let parts = $state({});
  let error = $state('');

  $effect(() => {
    const p = project;
    error = '';
    site = null;
    if (!p.viewer) return;
    fetch(fileUrl(p.id, 'viewer/site.json'))
      .then((r) => (r.ok ? r.json() : Promise.reject(new Error(`viewer/site.json: ${r.status}`))))
      .then((s) => {
        site = s;
        if (!s.boards.some((b) => b.id === boardId)) boardId = s.boards[0]?.id ?? null;
      })
      .catch((e) => (error = e.message));
  });

  const base = $derived(boardId ? fileUrl(project.id, `viewer/${boardId}/`) : null);

  $effect(() => {
    const b = base;
    parts = {};
    if (!b) return;
    fetch(`${b}3d/parts.json`)
      .then((r) => (r.ok ? r.json() : null))
      .then((d) => (parts = d?.parts ?? {}))
      .catch(() => {});
  });

  const part = $derived(selected.length === 1 ? parts[selected[0]] : null);
  const lcsc = (n) => `https://www.lcsc.com/product-detail/${encodeURIComponent(n)}.html`;
</script>

{#snippet pane()}
      {#if part}
        <aside class="pane">
          <h2>{selected[0]} <span class="muted">{part.value}</span></h2>
          {#if part.description}<p class="muted desc">{part.description}</p>{/if}
          <dl>
            <dt>Footprint</dt>
            <dd class="mono">{part.lib ? `${part.lib}:` : ''}{part.footprint}</dd>
            <dt>Side</dt>
            <dd>{part.side}</dd>
            <dt>At</dt>
            <dd class="mono">{part.x}, {part.y} mm · {part.rot}°</dd>
            {#if part.mpn}<dt>MPN</dt><dd>{part.mpn}</dd>{/if}
            {#if part.fields?.Manufacturer}<dt>Maker</dt><dd>{part.fields.Manufacturer}</dd>{/if}
            {#if part.lcsc}<dt>LCSC</dt><dd><a href={lcsc(part.lcsc)} target="_blank" rel="noopener">{part.lcsc}</a></dd>{/if}
            {#if part.datasheet}<dt>Datasheet</dt><dd><a href={part.datasheet} target="_blank" rel="noopener">open</a></dd>{/if}
            {#each Object.entries(part.fields ?? {}).filter(([k]) => !['MPN', 'Manufacturer'].includes(k)) as [k, v]}
              <dt>{k}</dt>
              <dd>{v}</dd>
            {/each}
          </dl>
          {#if part.pins?.length}
            <div class="label">Pins</div>
            <table class="pins mono">
              <tbody>
                {#each part.pins as [pin, net]}
                  <tr><td>{pin}</td><td>{net || '—'}</td></tr>
                {/each}
              </tbody>
            </table>
          {/if}
        </aside>
      {/if}
{/snippet}

<div class="bv">
  <div class="bar">
    <div class="seg" role="tablist">
      {#if kicadBoard}
        <button class:on={mode === 'board'} role="tab" aria-selected={mode === 'board'} onclick={() => (mode = 'board')} title="The board file as it is now">Board</button>
      {/if}
      {#if project.viewer}
        <button class:on={mode === 'layers'} role="tab" aria-selected={mode === 'layers'} onclick={() => (mode = 'layers')} title="pcbview's plots, with silkscreen and fab, as published">Plots</button>
        <button class:on={mode === '3d'} role="tab" aria-selected={mode === '3d'} onclick={() => (mode = '3d')}>3D</button>
      {/if}
    </div>
    {#if mode === 'board' && project.boards.length > 1}
      <select bind:value={boardIndex} aria-label="Board">
        {#each project.boards as b, i}<option value={i}>{b.board} / {b.variant}</option>{/each}
      </select>
    {:else if mode === 'board'}
      <span class="muted">{kicadBoard.board} / {kicadBoard.variant}</span>
    {:else if site?.boards.length > 1}
      <select bind:value={boardId} aria-label="Board">
        {#each site.boards as b}<option value={b.id}>{b.name}</option>{/each}
      </select>
    {:else if site}
      <span class="muted">{site.boards[0]?.name}</span>
    {/if}
    {#if site?.built && mode !== 'board'}<span class="muted built">viewer data built {site.built.replace('T', ' ')}</span>{/if}
    {#if selected.length}
      <span class="sel">Selected: {selected.join(', ')} <button class="x" onclick={() => onselect([])} aria-label="Clear the selection">✕</button></span>
    {/if}
  </div>

  {#if mode !== 'board'}
    <Stale {project} {change} from="viewer/site.json" fix="Rebuild the viewer: scripts/view.sh build pcbview.toml" {onhistory} />
  {/if}
  {#if mode === 'board'}
    <div class="main" class:with-pane={!!part}>
      {#await loadLive()}
        <p class="muted pad">Loading the board drawing…</p>
      {:then m}
        {#key `${store.root} ${boardIndex}`}
          <m.default {store} board={kicadBoard} {selected} {onselect} />
        {/key}
      {:catch e}
        <p class="err">{e.message}</p>
      {/await}
      {@render pane()}
    </div>
  {:else if error}
    <p class="err">{error}. This project has no pcbview build yet: <span class="mono">scripts/view.sh build pcbview.toml</span></p>
  {:else if base}
    <div class="main" class:with-pane={!!part}>
      {#key base + mode}
        {#if mode === 'layers'}
          <Layers2D {base} {parts} {selected} {onselect} />
        {:else}
          {#await load3D()}
            <p class="muted pad">Loading the 3D view…</p>
          {:then m}
            <m.default {base} {selected} {onselect} />
          {:catch e}
            <p class="err">{e.message}</p>
          {/await}
        {/if}
      {/key}
      {@render pane()}
    </div>
  {/if}
</div>

<style>
  .bv {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    width: 100%;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 14px;
    border-bottom: 1px solid var(--line);
    flex-wrap: wrap;
  }

  .seg {
    display: flex;
    border: 1px solid var(--line);
    border-radius: 6px;
    overflow: hidden;
  }

  .seg button {
    border: none;
    background: var(--panel);
    padding: 4px 14px;
  }

  .seg button.on {
    background: var(--accent);
    color: var(--bg);
    font-weight: 600;
  }

  select {
    font: inherit;
    color: inherit;
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 3px 6px;
  }

  .built {
    font-size: 12px;
  }

  .sel {
    margin-left: auto;
    font-size: 12.5px;
    padding: 2px 4px 2px 10px;
    border-radius: 12px;
    background: var(--accent-soft);
    color: var(--accent);
  }

  .x {
    border: none;
    background: none;
    color: inherit;
    padding: 0 6px;
  }

  .main {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
  }

  .main.with-pane {
    grid-template-columns: minmax(0, 1fr) 280px;
  }

  .pane {
    border-left: 1px solid var(--line);
    background: var(--panel);
    padding: 14px;
    overflow: auto;
  }

  .pane h2 {
    font-size: 18px;
  }

  .desc {
    margin: 4px 0 10px;
    font-size: 12.5px;
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 3px 10px;
    margin: 10px 0;
  }

  dt {
    color: var(--muted);
  }

  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .pins {
    border-collapse: collapse;
    font-size: 12px;
    width: 100%;
    margin-top: 4px;
  }

  .pins td {
    border-top: 1px solid var(--line);
    padding: 3px 4px;
  }

  .pad {
    padding: 20px;
  }

  .err {
    padding: 20px;
    color: var(--bad);
  }

  @media (max-width: 900px) {
    .main.with-pane {
      grid-template-columns: 1fr;
    }

    .pane {
      border-left: none;
      border-top: 1px solid var(--line);
    }
  }
</style>
