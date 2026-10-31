<script>
  import { onMount } from 'svelte';
  import { Renderer } from 'board-draw';
  import initCore, { Studio } from '../../draw/core/studio_core.js';
  import { kicadFiles } from '../stale.js';

  // The board as its .kicad_pcb is now, read through the store and drawn the way UltraRoute
  // Studio draws it: ultraroute's view() turns the file into Studio's lines (in a worker), Studio's
  // core makes them a board-draw scene, and board-draw's renderer draws its mesh. So a save from
  // Composer or Route shows here as soon as it lands. Copper, pads, vias, outline and pours;
  // silkscreen and fab are in the Plots view (pcbview's) until board-draw draws them.

  let { store, board, selected = [], onselect = () => {} } = $props();

  const BACKGROUND = [0.05, 0.08, 0.09];
  const paths = $derived(kicadFiles(board));

  let canvas;
  let renderer = null;
  let studio = null;
  let worker = null;
  let asked = 0; // the newest read of the file
  let posted = 0; // the read whose lines are awaited from the worker
  let shownRev = null; // the board file's revision drawn, or being drawn
  let status = $state('Reading the board…');
  let error = $state('');
  let layers = $state([]); // [{name, color, hidden, active}]
  let partIds = new Map(); // name -> id
  let counts = $state(null);
  let back = $state(false);
  let pours = $state(true);
  let hover = $state('');

  const view = { cx: 0, cy: 0, zoom: 1 };
  let version = 0;
  let drawn = -1;
  let drawnZoom = 0;
  let drawnBack = false;
  let viewChanged = true;
  let fitted = false;
  let drag = null;
  const touches = new Map();
  let pinch = null;

  onMount(() => {
    let raf = 0;
    let live = true;
    const ro = new ResizeObserver(resize);
    (async () => {
      try {
        await initCore();
        if (!live) return;
        studio = new Studio();
        renderer = new Renderer(canvas);
        worker = new Worker(new URL('./draw-worker.js', import.meta.url), { type: 'module' });
        worker.onmessage = got;
        ro.observe(canvas);
        resize();
        load();
        const frame = () => {
          draw();
          raf = requestAnimationFrame(frame);
        };
        frame();
      } catch (e) {
        error = e.message;
      }
    })();
    return () => {
      live = false;
      cancelAnimationFrame(raf);
      ro.disconnect();
      worker?.terminate();
      studio?.free();
    };
  });

  // A change to the board file from any app or window draws it again. Locally the bus also says
  // only that something in the project changed (no paths); load() then skips an unchanged file.
  $effect(() => {
    const [s, pcb] = [store, paths.pcb];
    return s.watch(({ paths: changed }) => {
      if (!changed || changed.includes(pcb)) load();
    });
  });

  async function load() {
    if (!worker) return;
    const id = ++asked;
    try {
      const file = await store.read(paths.pcb);
      if (!file) throw new Error(`${paths.pcb} isn't in the project`);
      if (id !== asked || (file.rev && file.rev === shownRev)) return;
      const [pro, dru] = await Promise.all([store.text(paths.pro), store.text(paths.dru)]);
      const pcb = new TextDecoder().decode(file.bytes);
      if (id !== asked) return;
      posted = id;
      shownRev = file.rev;
      worker.postMessage({ id, pcb, pro: pro ?? '{}', dru: dru ?? '' });
    } catch (e) {
      error = e.message;
    }
  }

  function got({ data }) {
    if (data.id !== posted) return;
    if (data.error) {
      shownRev = null;
      error = `Can't draw ${paths.pcb}: ${data.error}`;
      return;
    }
    error = '';
    const hidden = new Set(layers.filter((l) => l.hidden).map((l) => l.name));
    for (const line of data.lines) studio.apply(line);
    const st = JSON.parse(studio.status());
    st.layers.forEach((l, i) => hidden.has(l.name) && studio.set_layer_hidden(i, true));
    studio.set_show_pours(pours);
    layers = JSON.parse(studio.status()).layers;
    partIds = new Map(JSON.parse(studio.parts()).map((p) => [p.name, p.id]));
    counts = { parts: partIds.size, traces: st.traces, vias: st.vias };
    highlight();
    status = '';
    version++;
  }

  function highlight() {
    if (!studio) return;
    studio.set_highlight(new Int32Array(selected.map((r) => partIds.get(r)).filter(Boolean)));
    version++;
  }
  // Only when the selection itself changes: the list is a new array on every refresh.
  const selectedKey = $derived(selected.join('\n'));
  $effect(() => {
    selectedKey;
    highlight();
  });

  function resize() {
    const dpr = window.devicePixelRatio || 1;
    const r = canvas.getBoundingClientRect();
    canvas.width = Math.max(1, Math.round(r.width * dpr));
    canvas.height = Math.max(1, Math.round(r.height * dpr));
    if (!fitted) fitted = fit();
    viewChanged = true;
  }

  function fit() {
    const b = studio?.bbox();
    if (!b || !(b[2] > b[0]) || !canvas) return false;
    view.cx = (b[0] + b[2]) / 2;
    view.cy = (b[1] + b[3]) / 2;
    view.zoom = 0.92 * Math.min(canvas.width / (b[2] - b[0]), canvas.height / (b[3] - b[1]));
    viewChanged = true;
    return true;
  }

  function draw() {
    if (!renderer || !studio || !canvas) return;
    if (!fitted && version > 0) fitted = fit();
    if (version !== drawn || Math.abs(drawnZoom - view.zoom) / view.zoom > 0.15 || drawnBack !== back) {
      renderer.upload(0, studio.mesh_view(1 / view.zoom, 0xffffffff, performance.now()));
      [drawn, drawnZoom, drawnBack] = [version, view.zoom, back];
      viewChanged = true;
    }
    if (viewChanged) {
      renderer.clear(BACKGROUND);
      renderer.draw(0, view, { x: 0, y: 0, w: canvas.width, h: canvas.height }, back);
      viewChanged = false;
    }
  }

  // The board point under a pointer (y up, as Studio's lines are).
  function toBoard(e) {
    const r = canvas.getBoundingClientRect();
    const dpr = canvas.width / r.width;
    let x = (e.clientX - r.left) * dpr - canvas.width / 2;
    const y = canvas.height / 2 - (e.clientY - r.top) * dpr;
    if (back) x = -x;
    return [view.cx + x / view.zoom, view.cy + y / view.zoom];
  }

  function zoomAt(e, factor) {
    const at = toBoard(e);
    view.zoom = Math.min(2000, Math.max(0.2, view.zoom * factor));
    const after = toBoard(e);
    view.cx += at[0] - after[0];
    view.cy += at[1] - after[1];
    viewChanged = true;
  }

  const middle = () => {
    const [a, b] = [...touches.values()];
    return { clientX: (a.clientX + b.clientX) / 2, clientY: (a.clientY + b.clientY) / 2, apart: Math.max(1, Math.hypot(a.clientX - b.clientX, a.clientY - b.clientY)) };
  };

  function down(e) {
    canvas.setPointerCapture(e.pointerId);
    if (e.pointerType === 'touch') {
      touches.set(e.pointerId, { clientX: e.clientX, clientY: e.clientY });
      if (touches.size === 2) {
        const m = middle();
        pinch = { at: toBoard(m), apart: m.apart, zoom: view.zoom };
        drag = null;
        return;
      }
    }
    drag = { at: toBoard(e), screen: [e.clientX, e.clientY], moved: false };
  }

  function move(e) {
    if (e.pointerType === 'touch' && touches.has(e.pointerId)) touches.set(e.pointerId, { clientX: e.clientX, clientY: e.clientY });
    if (pinch && touches.size === 2) {
      const m = middle();
      view.zoom = Math.min(2000, Math.max(0.2, (pinch.zoom * m.apart) / pinch.apart));
      const now = toBoard(m);
      view.cx += pinch.at[0] - now[0];
      view.cy += pinch.at[1] - now[1];
      viewChanged = true;
      return;
    }
    if (drag) {
      if (Math.hypot(e.clientX - drag.screen[0], e.clientY - drag.screen[1]) > 4) drag.moved = true;
      if (drag.moved) {
        const now = toBoard(e);
        view.cx += drag.at[0] - now[0];
        view.cy += drag.at[1] - now[1];
        viewChanged = true;
      }
      return;
    }
    if (e.pointerType === 'mouse' && studio) {
      const id = studio.part_at(...toBoard(e));
      hover = id ? (JSON.parse(studio.part(id) || '{}').name ?? '') : '';
    }
  }

  function up(e) {
    touches.delete(e.pointerId);
    if (touches.size < 2) pinch = null;
    if (drag && !drag.moved && studio) {
      const id = studio.part_at(...toBoard(e));
      const name = id ? JSON.parse(studio.part(id) || '{}').name : null;
      onselect(name ? [name] : []);
    }
    drag = null;
  }

  function toggle(i) {
    layers[i].hidden = !layers[i].hidden;
    studio.set_layer_hidden(i, layers[i].hidden);
    version++;
  }

  function activate(i) {
    studio.set_active_layer(i);
    layers = JSON.parse(studio.status()).layers;
    version++;
  }

  function setPours(on) {
    pours = on;
    studio.set_show_pours(on);
    version++;
  }

  const rgb = (c) => `rgb(${c.map((v) => Math.round(v * 255)).join(',')})`;
</script>

<div class="live">
  <div class="side">
    {#if counts}<p class="muted stats">{counts.parts} parts · {counts.traces} tracks · {counts.vias} vias</p>{/if}
    <div class="label">Copper</div>
    {#each layers as l, i (l.name)}
      <div class="layer" class:on={l.active}>
        <input type="checkbox" checked={!l.hidden} onchange={() => toggle(i)} aria-label="Show {l.name}" />
        <button class="lname" onclick={() => activate(i)} title="Draw {l.name} on top">
          <span class="sw" style="background:{rgb(l.color)}"></span>{l.name}
        </button>
      </div>
    {/each}
    <label class="opt"><input type="checkbox" checked={pours} onchange={(e) => setPours(e.currentTarget.checked)} /> Poured zones</label>
    <div class="seg">
      <button class:on={!back} onclick={() => (back = false)}>Top</button>
      <button class:on={back} onclick={() => (back = true)}>Bottom</button>
    </div>
    <button class="btn" onclick={() => (fitted = fit())}>Fit</button>
    <p class="muted note">Drawn from the board file as it is now. Silkscreen and fab are in Plots.</p>
  </div>
  <div class="stage">
    <canvas
      bind:this={canvas}
      onwheel={(e) => (e.preventDefault(), zoomAt(e, Math.exp(-e.deltaY * 0.0015)))}
      onpointerdown={down}
      onpointermove={move}
      onpointerup={up}
      onpointercancel={up}
      onpointerleave={() => (hover = '')}
    ></canvas>
    {#if error}
      <p class="msg err">{error}</p>
    {:else if status}
      <p class="msg muted">{status}</p>
    {/if}
    {#if hover}<span class="hover mono">{hover}</span>{/if}
  </div>
</div>

<style>
  .live {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 200px minmax(0, 1fr);
  }

  .side {
    border-right: 1px solid var(--line);
    background: var(--panel);
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    overflow: auto;
    font-size: 13px;
  }

  .stats,
  .note {
    font-size: 12px;
    margin: 0 0 4px;
  }

  .layer {
    display: flex;
    align-items: center;
    gap: 6px;
    border-radius: 6px;
    padding: 2px 4px;
  }

  .layer.on {
    background: var(--accent-soft);
  }

  .lname {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    border: none;
    background: none;
    text-align: left;
    padding: 4px 0;
  }

  .sw {
    width: 12px;
    height: 12px;
    border-radius: 3px;
  }

  .opt {
    display: flex;
    gap: 6px;
    align-items: center;
    margin: 4px 0;
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
    background: var(--panel);
    padding: 5px 0;
  }

  .seg button.on {
    background: var(--accent);
    color: var(--bg);
    font-weight: 600;
  }

  .stage {
    position: relative;
    min-height: 0;
    min-width: 0;
  }

  canvas {
    display: block;
    width: 100%;
    height: 100%;
    min-height: 420px;
    touch-action: none;
    cursor: grab;
  }

  .msg {
    position: absolute;
    top: 12px;
    left: 14px;
    margin: 0;
  }

  .err {
    color: var(--bad);
  }

  .hover {
    position: absolute;
    bottom: 10px;
    left: 12px;
    font-size: 12px;
    padding: 2px 8px;
    border-radius: 4px;
    background: #000a;
    color: #fff;
    pointer-events: none;
  }

  @media (max-width: 760px) {
    .live {
      grid-template-columns: 1fr;
      grid-template-rows: auto minmax(360px, 1fr);
    }

    .side {
      border-right: none;
      border-bottom: 1px solid var(--line);
      flex-direction: row;
      flex-wrap: wrap;
      align-items: center;
    }

    .side .label,
    .note {
      display: none;
    }

    .seg button {
      padding: 5px 12px;
    }
  }
</style>
