<script>
  import { fileUrl } from '../api.js';
  import Layers2D from './Layers2D.svelte';
  // three.js is most of the page's weight: load it only when 3D is opened.
  const load3D = () => import('./Board3D.svelte');

  // The Board tab: pcbview's data (viewer/site.json and each board's layers/
  // and 3d/) drawn by Boardroom itself, as layers or in 3D, with the part
  // pane both share. A selection here goes to every other window, and one
  // made elsewhere shows here.

  let { project, selected = [], onselect = () => {} } = $props();

  let site = $state(null);
  let boardId = $state(null);
  // ?view=3d opens the 3D view first.
  let mode = $state(new URLSearchParams(location.search).get('view') === '3d' ? '3d' : 'layers');
  let parts = $state({});
  let error = $state('');

  $effect(() => {
    const p = project;
    error = '';
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

<div class="bv">
  <div class="bar">
    <div class="seg" role="tablist">
      <button class:on={mode === 'layers'} role="tab" aria-selected={mode === 'layers'} onclick={() => (mode = 'layers')}>Layers</button>
      <button class:on={mode === '3d'} role="tab" aria-selected={mode === '3d'} onclick={() => (mode = '3d')}>3D</button>
    </div>
    {#if site?.boards.length > 1}
      <select bind:value={boardId} aria-label="Board">
        {#each site.boards as b}<option value={b.id}>{b.name}</option>{/each}
      </select>
    {:else if site}
      <span class="muted">{site.boards[0]?.name}</span>
    {/if}
    {#if site?.built}<span class="muted built">viewer data built {site.built.replace('T', ' ')}</span>{/if}
    {#if selected.length}
      <span class="sel">Selected: {selected.join(', ')} <button class="x" onclick={() => onselect([])} aria-label="Clear the selection">✕</button></span>
    {/if}
  </div>

  {#if error}
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
