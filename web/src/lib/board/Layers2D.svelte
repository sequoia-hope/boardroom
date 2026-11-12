<script>
  import { onMount } from 'svelte';

  // The board's layers, as pcbview plots them: one SVG per layer, all in
  // KiCad's own millimetre coordinates, so they stack in register. They are
  // drawn into one <svg>; panning and zooming move its viewBox, so the copper
  // stays sharp at any zoom. Each layer's colours are swapped for a CSS
  // variable when it loads, which is what lets the contrast modes recolour it.

  let { base, parts = {}, selected = [], onselect = () => {} } = $props();

  let meta = $state(null);
  let error = $state('');
  let layers = $state([]); // [{name, slug, kind, color, note, stats…, visible, own}]
  let active = $state(null); // slug
  // ?side=back, ?others=grey|solo, ?bg=light|black open the view that way.
  const params = new URLSearchParams(location.search);
  let mode = $state(['dim', 'grey', 'solo'].includes(params.get('others')) ? params.get('others') : 'dim'); // dim | solo | grey
  let dim = $state(0.3);
  let mirror = $state(params.get('side') === 'back');
  let background = $state(['dark', 'black', 'light'].includes(params.get('bg')) ? params.get('bg') : 'dark');
  let showBody = $state(true);
  let vb = $state(null); // {x, y, w, h}
  let cursor = $state(null); // [x, y] in board mm
  let hover = $state(null); // ref under the cursor

  let svg;
  let stage;
  const groups = new Map(); // slug -> <g>
  let bodyGroup = null;

  const BACKGROUNDS = { dark: '#0d1418', black: '#000000', light: '#f4f4f0' };
  const GREY = '#8b949a';
  const SVG_NS = 'http://www.w3.org/2000/svg';

  onMount(() => {
    load().catch((e) => (error = e.message));
    const onKey = (e) => {
      if (e.target.closest?.('input, select, textarea')) return;
      if (e.key === 'f') fit();
      else if (e.key === 'b') mirror = !mirror;
      else if (/^[1-9]$/.test(e.key)) {
        const L = layers[Number(e.key) - 1];
        if (L) activate(L.slug);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  async function load() {
    const res = await fetch(`${base}layers/layers.json`);
    if (!res.ok) throw new Error(`layers.json: ${res.status}`);
    meta = await res.json();
    layers = meta.layers.map((L) => ({
      ...L,
      visible: L.kind === 'copper' || L.kind === 'outline' || L.slug === 'silk_f',
      own: L.color,
    }));
    active = layers.find((L) => L.kind === 'copper')?.slug ?? layers[0]?.slug;
    fit();
    const texts = await Promise.all(
      [{ slug: '__body', file: meta.body }, ...layers].map(async (L) => {
        if (!L.file) return [L.slug, null];
        const r = await fetch(`${base}layers/${L.file}`);
        return [L.slug, r.ok ? await r.text() : null];
      }),
    );
    for (const [slug, text] of texts) {
      if (!text) continue;
      const g = toGroup(text);
      if (slug === '__body') bodyGroup = g;
      else groups.set(slug, g);
    }
    draw();
  }

  // An SVG file's drawing as one <g>, its colours replaced by var(--lc).
  function toGroup(text) {
    const recoloured = text
      .replace(/\b(fill|stroke)\s*:\s*#[0-9a-fA-F]{3,8}/g, '$1:var(--lc)')
      .replace(/\b(fill|stroke)="#[0-9a-fA-F]{3,8}"/g, '$1="var(--lc)"');
    const doc = new DOMParser().parseFromString(recoloured, 'image/svg+xml');
    const g = document.createElementNS(SVG_NS, 'g');
    for (const child of [...doc.documentElement.childNodes]) g.appendChild(document.importNode(child, true));
    return g;
  }

  // Layers in drawing order, as a board is seen: the far side's overlays,
  // the copper far to near with the active copper layer on top of the rest,
  // the near side's overlays over the copper, the outline over everything.
  // An active overlay comes to the top of the overlays.
  const ordered = $derived.by(() => {
    const raise = (list) => {
      const a = list.find((L) => L.slug === active);
      return a ? [...list.filter((L) => L !== a), a] : list;
    };
    const copper = layers.filter((L) => L.kind === 'copper');
    const rest = layers.filter((L) => L.kind !== 'copper' && L.kind !== 'outline');
    const outline = layers.filter((L) => L.kind === 'outline');
    const far = mirror ? copper : [...copper].reverse();
    const near = rest.filter((L) => (mirror ? L.slug.endsWith('_b') : !L.slug.endsWith('_b')));
    const farRest = rest.filter((L) => !near.includes(L));
    return [...raise(farRest), ...raise(far), ...raise(near), ...outline];
  });

  function styleFor(L) {
    const isActive = L.slug === active;
    const outline = L.kind === 'outline';
    let shown = L.visible;
    let colour = L.color;
    let opacity = 1;
    // Dimmed and Grey act on the other copper; overlays (silkscreen, fab)
    // stay readable. Hidden leaves only the active layer and the outline.
    if (!isActive && !outline && (mode === 'solo' || L.kind === 'copper')) {
      if (mode === 'solo') shown = false;
      else if (mode === 'grey') {
        colour = GREY;
        opacity = Math.max(dim, 0.15);
      } else opacity = dim;
    }
    if (isActive) shown = true;
    return { shown, colour, opacity };
  }

  // Puts the groups into the <svg> in order with their colours and opacities.
  function draw() {
    if (!svg) return;
    const root = svg.querySelector('g.board');
    if (!root) return;
    if (bodyGroup) {
      bodyGroup.style.setProperty('--lc', background === 'light' ? '#d9dcc8' : '#1f3a2c');
      bodyGroup.style.display = showBody ? '' : 'none';
      root.appendChild(bodyGroup);
    }
    for (const L of ordered) {
      const g = groups.get(L.slug);
      if (!g) continue;
      const s = styleFor(L);
      g.style.setProperty('--lc', s.colour);
      g.style.opacity = String(s.opacity);
      g.style.display = s.shown ? '' : 'none';
      root.appendChild(g);
    }
  }

  $effect(() => {
    // Re-run whenever anything draw() reads changes.
    ordered;
    mode;
    dim;
    background;
    showBody;
    layers.map((L) => [L.visible, L.color]);
    draw();
  });

  // ---- view: fit, zoom, pan --------------------------------------------------
  function fit() {
    if (!meta) return;
    const [x, y, w, h] = meta.view_mm;
    const r = stage?.getBoundingClientRect();
    const aspect = r && r.height ? r.width / r.height : 1;
    const pad = 1.06;
    let W = w * pad;
    let H = h * pad;
    if (W / H < aspect) W = H * aspect;
    else H = W / aspect;
    vb = { x: x + w / 2 - W / 2, y: y + h / 2 - H / 2, w: W, h: H };
  }

  // Screen point -> drawing coordinates (before the mirror).
  function toSvg(ev) {
    const pt = svg.createSVGPoint();
    pt.x = ev.clientX;
    pt.y = ev.clientY;
    return pt.matrixTransform(svg.getScreenCTM().inverse());
  }

  const centreX = $derived(meta ? meta.view_mm[0] + meta.view_mm[2] / 2 : 0);
  // Drawing x -> board x: the mirror reflects about the board's centre.
  const boardX = (x) => (mirror ? 2 * centreX - x : x);

  function onWheel(ev) {
    ev.preventDefault();
    if (!vb) return;
    const p = toSvg(ev);
    const k = Math.exp((ev.deltaMode === 1 ? ev.deltaY * 16 : ev.deltaY) * 0.0015);
    const w = Math.min(Math.max(vb.w * k, 0.05), 2000);
    const h = (vb.h * w) / vb.w;
    vb = { x: p.x - ((p.x - vb.x) * w) / vb.w, y: p.y - ((p.y - vb.y) * h) / vb.h, w, h };
  }

  let drag = null;
  const pointers = new Map();
  function onDown(ev) {
    stage.setPointerCapture(ev.pointerId);
    pointers.set(ev.pointerId, [ev.clientX, ev.clientY]);
    drag = { x: ev.clientX, y: ev.clientY, vb: { ...vb }, moved: false, pinch: pinchSpan() };
  }
  function pinchSpan() {
    if (pointers.size < 2) return null;
    const [a, b] = [...pointers.values()];
    return Math.hypot(a[0] - b[0], a[1] - b[1]);
  }
  function onMove(ev) {
    const p = toSvg(ev);
    cursor = [boardX(p.x), p.y];
    if (pointers.has(ev.pointerId)) pointers.set(ev.pointerId, [ev.clientX, ev.clientY]);
    if (!drag) {
      hover = partAt(boardX(p.x), p.y);
      return;
    }
    const r = stage.getBoundingClientRect();
    const span = pinchSpan();
    if (span && drag.pinch) {
      const k = drag.pinch / span;
      const w = drag.vb.w * k;
      const h = drag.vb.h * k;
      vb = { x: drag.vb.x + (drag.vb.w - w) / 2, y: drag.vb.y + (drag.vb.h - h) / 2, w, h };
      drag.moved = true;
      return;
    }
    const dx = ((ev.clientX - drag.x) / r.width) * drag.vb.w;
    const dy = ((ev.clientY - drag.y) / r.height) * drag.vb.h;
    if (Math.abs(ev.clientX - drag.x) + Math.abs(ev.clientY - drag.y) > 3) drag.moved = true;
    // The viewBox and the stage share an aspect (fit keeps it), so one scale serves both axes.
    vb = { ...drag.vb, x: drag.vb.x - dx, y: drag.vb.y - dy };
  }
  function onUp(ev) {
    pointers.delete(ev.pointerId);
    if (drag && !drag.moved && pointers.size === 0) {
      const p = toSvg(ev);
      const ref = partAt(boardX(p.x), p.y);
      onselect(ref ? [ref] : []);
    }
    if (pointers.size === 0) drag = null;
  }

  // The smallest part whose box holds the point, on the side being looked at first.
  function partAt(x, y) {
    let best = null;
    for (const [ref, p] of Object.entries(parts)) {
      const b = p.box;
      if (!b || x < b[0] || x > b[2] || y < b[1] || y > b[3]) continue;
      const near = (p.side === 'back') === mirror ? 0 : 1;
      const area = (b[2] - b[0]) * (b[3] - b[1]);
      if (!best || near < best.near || (near === best.near && area < best.area)) best = { ref, near, area };
    }
    return best?.ref ?? null;
  }

  $effect(() => {
    if (!stage) return;
    const ro = new ResizeObserver(() => {
      if (!vb || !meta) return fit();
      const r = stage.getBoundingClientRect();
      if (!r.height) return;
      // Keep the centre and the scale; change only the extent.
      const scale = vb.w / (lastWidth || r.width);
      const w = r.width * scale;
      const h = r.height * scale;
      vb = { x: vb.x + vb.w / 2 - w / 2, y: vb.y + vb.h / 2 - h / 2, w, h };
      lastWidth = r.width;
    });
    let lastWidth = stage.getBoundingClientRect().width;
    ro.observe(stage);
    return () => ro.disconnect();
  });

  function activate(slug) {
    active = slug;
    const L = layers.find((x) => x.slug === slug);
    if (L) L.visible = true;
  }

  function preset(which) {
    for (const L of layers) {
      if (which === 'all') L.visible = true;
      else if (which === 'none') L.visible = L.kind === 'outline';
      else if (which === 'copper') L.visible = L.kind === 'copper' || L.kind === 'outline';
    }
  }

  const fmt = (n) => (Math.abs(n) >= 100 ? n.toFixed(2) : n.toFixed(3));
  const scaleBar = $derived.by(() => {
    if (!vb || !stage) return null;
    const pxPerMm = stage.clientWidth / vb.w;
    const target = 110 / pxPerMm;
    const step = [0.1, 0.2, 0.5, 1, 2, 5, 10, 20, 50, 100].find((s) => s >= target) ?? 100;
    return { mm: step, px: step * pxPerMm };
  });

  const selBoxes = $derived(selected.map((r) => parts[r]).filter((p) => p?.box));
  const hoverPart = $derived(hover ? parts[hover] : null);
  // Listed nearest first: from the back, B.Cu leads.
  const copper = $derived.by(() => {
    const list = layers.filter((L) => L.kind === 'copper');
    return mirror ? list.reverse() : list;
  });
  const others = $derived(layers.filter((L) => L.kind !== 'copper'));
</script>

<div class="l2d" style="--bgc: {BACKGROUNDS[background]}">
  <aside class="panel">
    {#if meta}
      <div class="facts muted">
        {meta.size_mm[0]} × {meta.size_mm[1]} mm · {copper.length} layers · {meta.thickness} mm<br />
        {meta.footprints} parts · {meta.nets} nets · {meta.vias} vias · {meta.length_mm} mm of track
      </div>
    {/if}

    <div class="label">Copper, {mirror ? 'back to front' : 'front to back'}</div>
    {#each copper as L, i (L.slug)}
      {@render row(L, i)}
    {/each}
    <div class="label">Overlays</div>
    {#each others as L, i (L.slug)}
      {@render row(L, copper.length + i)}
    {/each}

    <div class="presets">
      <button class="btn" onclick={() => preset('all')}>All</button>
      <button class="btn" onclick={() => preset('copper')}>Copper</button>
      <button class="btn" onclick={() => preset('none')}>None</button>
    </div>

    <div class="label">Other copper</div>
    <div class="seg" role="radiogroup" aria-label="How the other layers are drawn">
      {#each [['dim', 'Dimmed'], ['grey', 'Grey'], ['solo', 'Hidden']] as [m, t]}
        <button class:on={mode === m} role="radio" aria-checked={mode === m} onclick={() => (mode = m)}>{t}</button>
      {/each}
    </div>
    {#if mode !== 'solo'}
      <label class="slider">
        <span class="muted">Opacity</span>
        <input type="range" min="0.05" max="1" step="0.05" bind:value={dim} />
        <span class="mono">{Math.round(dim * 100)}%</span>
      </label>
    {/if}

    <label class="check"><input type="checkbox" bind:checked={mirror} /> View from the back <kbd>b</kbd></label>
    <label class="check"><input type="checkbox" bind:checked={showBody} /> Board body</label>
    <label class="bg">
      <span class="muted">Background</span>
      <select bind:value={background}>
        <option value="dark">Dark</option>
        <option value="black">Black</option>
        <option value="light">Light</option>
      </select>
    </label>
    <p class="hint muted">
      Click a layer to bring it forward (<kbd>1</kbd>–<kbd>9</kbd>). Drag to pan, wheel or pinch to zoom,
      <kbd>f</kbd> to fit. Click a part to select it.
    </p>
  </aside>

  {#snippet row(L, i)}
    <div class="row" class:active={L.slug === active}>
      <input type="checkbox" bind:checked={L.visible} aria-label="Show {L.name}" disabled={L.slug === active} />
      <input class="swatch" type="color" bind:value={L.color} aria-label="{L.name} colour" />
      <button class="name" onclick={() => activate(L.slug)} title={L.note}>
        <span>{L.name}</span>
        {#if L.kind === 'copper'}
          <span class="stats mono">
            {#if L.tracks}{L.tracks} tracks · {Math.round(L.length_mm)} mm{:else if L.zones?.length}plane {L.zones.join(', ')}{:else}{L.pads} pads{/if}
          </span>
        {/if}
      </button>
      {#if i < 9}<kbd class="key">{i + 1}</kbd>{/if}
    </div>
  {/snippet}

  <div
    class="stage"
    bind:this={stage}
    onwheel={onWheel}
    onpointerdown={onDown}
    onpointermove={onMove}
    onpointerup={onUp}
    onpointercancel={onUp}
    onpointerleave={() => ((cursor = null), (hover = null))}
    ondblclick={fit}
    role="application"
    aria-label="Board layers"
  >
    {#if error}
      <p class="err">{error}</p>
    {/if}
    <svg
      bind:this={svg}
      viewBox={vb ? `${vb.x} ${vb.y} ${vb.w} ${vb.h}` : '0 0 1 1'}
      preserveAspectRatio="xMidYMid meet"
      class:hovering={!!hover}
    >
      <g transform={mirror ? `translate(${2 * centreX} 0) scale(-1 1)` : ''}>
        <g class="board"></g>
        {#each selBoxes as p}
          <rect class="sel" x={p.box[0]} y={p.box[1]} width={p.box[2] - p.box[0]} height={p.box[3] - p.box[1]} />
        {/each}
        {#if hoverPart && !selected.includes(hover)}
          <rect class="hov" x={hoverPart.box[0]} y={hoverPart.box[1]} width={hoverPart.box[2] - hoverPart.box[0]} height={hoverPart.box[3] - hoverPart.box[1]} />
        {/if}
      </g>
    </svg>

    {#if hoverPart || cursor}<div class="readout mono">
      {#if hoverPart}<strong>{hover}</strong> {hoverPart.value} · {/if}
      {#if cursor}x {fmt(cursor[0])} y {fmt(cursor[1])} mm{/if}
      {#if mirror}<span class="back">from the back</span>{/if}
    </div>{/if}
    {#if scaleBar}
      <div class="scale mono"><span style="width: {scaleBar.px}px"></span>{scaleBar.mm} mm</div>
    {/if}
    <div class="zoom">
      <button class="btn" onclick={() => onWheel({ preventDefault() {}, deltaY: -300, deltaMode: 0, clientX: stage.getBoundingClientRect().left + stage.clientWidth / 2, clientY: stage.getBoundingClientRect().top + stage.clientHeight / 2 })} aria-label="Zoom in">+</button>
      <button class="btn" onclick={() => onWheel({ preventDefault() {}, deltaY: 300, deltaMode: 0, clientX: stage.getBoundingClientRect().left + stage.clientWidth / 2, clientY: stage.getBoundingClientRect().top + stage.clientHeight / 2 })} aria-label="Zoom out">−</button>
      <button class="btn" onclick={fit}>Fit</button>
    </div>
  </div>
</div>

<style>
  .l2d {
    display: grid;
    grid-template-columns: 260px minmax(0, 1fr);
    height: 100%;
    min-height: 0;
  }

  .panel {
    overflow: auto;
    padding: 12px;
    border-right: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    gap: 6px;
    background: var(--panel);
  }

  .panel > :global(*) {
    flex-shrink: 0;
  }

  .facts {
    font-size: 12px;
    margin-bottom: 4px;
  }

  .label {
    margin-top: 8px;
  }

  .row {
    display: grid;
    grid-template-columns: auto auto 1fr auto;
    align-items: center;
    gap: 6px;
    padding: 2px 4px;
    border-radius: 6px;
  }

  .row.active {
    background: var(--accent-soft);
  }

  .swatch {
    width: 18px;
    height: 18px;
    padding: 0;
    border: 1px solid var(--line);
    border-radius: 4px;
    background: none;
    cursor: pointer;
  }

  .swatch::-webkit-color-swatch-wrapper {
    padding: 0;
  }

  .swatch::-webkit-color-swatch {
    border: none;
    border-radius: 3px;
  }

  .name {
    border: none;
    background: none;
    text-align: left;
    padding: 3px 0;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .row.active .name span:first-child {
    font-weight: 700;
  }

  .stats {
    font-size: 11px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  kbd,
  .key {
    font-family: ui-monospace, monospace;
    font-size: 10.5px;
    border: 1px solid var(--line);
    border-radius: 3px;
    padding: 0 4px;
    color: var(--muted);
  }

  .presets {
    display: flex;
    gap: 6px;
    margin-top: 4px;
  }

  .seg {
    display: flex;
    border: 1px solid var(--line);
    border-radius: 6px;
    overflow: hidden;
  }

  .seg button {
    flex: 1;
    border: none;
    background: var(--panel-2);
    padding: 5px 0;
  }

  .seg button.on {
    background: var(--accent);
    color: var(--bg);
    font-weight: 600;
  }

  .slider,
  .bg {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 8px;
  }

  .bg {
    grid-template-columns: auto 1fr;
  }

  select {
    font: inherit;
    color: inherit;
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 3px 6px;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .hint {
    font-size: 12px;
    margin-top: auto;
  }

  .stage {
    position: relative;
    background: var(--bgc);
    overflow: hidden;
    touch-action: none;
    cursor: grab;
    min-height: 0;
  }

  .stage:active {
    cursor: grabbing;
  }

  svg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }

  svg.hovering {
    cursor: pointer;
  }

  .sel,
  .hov {
    fill: none;
    vector-effect: non-scaling-stroke;
  }

  .sel {
    stroke: #3d8bff;
    stroke-width: 2.5;
    fill: #3d8bff22;
  }

  .hov {
    stroke: #ffffffaa;
    stroke-width: 1.2;
    stroke-dasharray: 4 3;
  }

  .readout {
    position: absolute;
    left: 10px;
    bottom: 10px;
    font-size: 12px;
    padding: 3px 8px;
    border-radius: 5px;
    background: #000a;
    color: #e6eef2;
    pointer-events: none;
  }

  .back {
    margin-left: 8px;
    color: #ffcf70;
  }

  .scale {
    position: absolute;
    right: 12px;
    bottom: 12px;
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
    color: #e6eef2;
    background: #000a;
    padding: 3px 8px;
    border-radius: 5px;
    pointer-events: none;
  }

  .scale span {
    display: block;
    height: 6px;
    border: 1.5px solid #e6eef2;
    border-top: none;
  }

  .zoom {
    position: absolute;
    top: 10px;
    right: 10px;
    display: flex;
    gap: 4px;
  }

  .err {
    position: absolute;
    top: 10px;
    left: 10px;
    color: var(--bad);
  }

  @media (max-width: 760px) {
    .l2d {
      grid-template-columns: 1fr;
      grid-template-rows: auto 60vh;
    }

    .panel {
      border-right: none;
      border-bottom: 1px solid var(--line);
      max-height: 40vh;
    }
  }
</style>
