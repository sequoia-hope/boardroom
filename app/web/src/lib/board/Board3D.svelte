<script>
  import { onMount } from 'svelte';
  import * as THREE from 'three';
  import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js';
  import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
  import { RoomEnvironment } from 'three/examples/jsm/environments/RoomEnvironment.js';

  // The board in 3D, from pcbview's board.glb: kicad-cli's export, merged and
  // quantized, in metres, the board flat with its top facing +y. Each part's
  // node carries its reference and side; the board's own meshes carry the
  // layer they came from, which is what the toggles switch.

  let { base, selected = [], onselect = () => {} } = $props();

  let stage;
  let canvas;
  let status = $state('Loading the model…');
  let view = $state('tilt');
  let hoverRef = $state(null);
  let groups = $state([]); // [{key, name, on}]

  // camera direction (board towards eye) and up, per view; as pcbview's
  const VIEWS = {
    top: [[0, 1, 0], [0, 0, -1]],
    bottom: [[0, -1, 0], [0, 0, -1]],
    tilt: [[0, 0.62, 0.78], [0, 1, 0]],
    edge: [[0, 0.06, 1], [0, 1, 0]],
  };
  const GROUPS = [
    ['parts', 'Parts', null],
    ['copper', 'Copper', ['copper', 'pad', 'via']],
    ['soldermask', 'Solder mask', ['soldermask']],
    ['silkscreen', 'Silkscreen', ['silkscreen']],
    ['PCB', 'Laminate', ['PCB']],
  ];
  const SELECT = new THREE.Color(0x3d8bff);
  const HOVER = new THREE.Color(0x777777);

  let renderer, scene, camera, controls, board, radius = 0.05;
  const parts = new Map(); // ref -> node
  const byLayer = {};
  const occluders = [];
  let frame = 0;
  let tween = null;

  const redraw = () => {
    if (!frame) frame = requestAnimationFrame(render);
  };

  function render() {
    frame = 0;
    if (tween) {
      const t = Math.min((performance.now() - tween.start) / 450, 1);
      const e = t * (2 - t);
      camera.position.lerpVectors(tween.fromPos, tween.toPos, e);
      controls.target.lerpVectors(tween.fromTarget, tween.toTarget, e);
      camera.up.lerpVectors(tween.fromUp, tween.toUp, e).normalize();
      camera.lookAt(controls.target);
      if (t < 1) redraw();
      else tween = null;
    }
    controls.update();
    const d = camera.position.distanceTo(controls.target);
    camera.near = Math.max(d / 200, 1e-5);
    camera.far = d + 6 * radius;
    camera.updateProjectionMatrix();
    renderer.render(scene, camera);
  }

  onMount(() => {
    renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.toneMapping = THREE.ACESFilmicToneMapping;
    scene = new THREE.Scene();
    const pmrem = new THREE.PMREMGenerator(renderer);
    scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
    camera = new THREE.PerspectiveCamera(30, 1, 1e-4, 10);
    controls = new OrbitControls(camera, canvas);
    controls.enableDamping = false;
    controls.screenSpacePanning = true;
    controls.addEventListener('change', redraw);

    const ro = new ResizeObserver(resize);
    ro.observe(stage);
    resize();

    new GLTFLoader().load(
      `${base}3d/board.glb`,
      loaded,
      (e) => {
        if (e.total) status = `Loading the model… ${Math.round((100 * e.loaded) / e.total)}%`;
      },
      (e) => (status = `No 3D model (${e.message ?? e}). pcbview builds it: scripts/view.sh build pcbview.toml`),
    );

    return () => {
      ro.disconnect();
      cancelAnimationFrame(frame);
      controls.dispose();
      scene.traverse((o) => {
        o.geometry?.dispose();
        if (o.material) [o.material].flat().forEach((m) => m.dispose());
      });
      pmrem.dispose();
      renderer.dispose();
    };
  });

  function resize() {
    const w = stage.clientWidth;
    const h = stage.clientHeight;
    if (!w || !h) return;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
    redraw();
  }

  function loaded(gltf) {
    board = gltf.scene;
    board.traverse((o) => {
      const u = o.userData;
      if (u.ref) parts.set(u.ref, o);
      if (u.layer) {
        (byLayer[u.layer] ??= []).push(o);
        if (u.layer === 'PCB' || u.layer === 'soldermask') occluders.push(o);
      }
    });
    // Centre on the laminate: a connector may hang over the edge.
    const pcb = byLayer.PCB ? new THREE.Box3().setFromObject(byLayer.PCB[0]) : new THREE.Box3().setFromObject(board);
    board.position.sub(pcb.getCenter(new THREE.Vector3()));
    scene.add(board);
    const size = pcb.getSize(new THREE.Vector3());
    radius = Math.max(size.x, size.z) / 2;
    groups = GROUPS.filter(([, , list]) => (list ? list.some((l) => byLayer[l]) : parts.size)).map(([key, name]) => ({
      key,
      name,
      on: true,
    }));
    status = '';
    go('tilt', true);
    tintAll();
  }

  function fitDistance(r) {
    const fov = THREE.MathUtils.degToRad(camera.fov) / 2;
    return (1.1 * r) / Math.sin(Math.min(fov, Math.atan(Math.tan(fov) * camera.aspect)));
  }

  function fly(dir, up, target, dist, now) {
    const toPos = target.clone().add(dir.clone().normalize().multiplyScalar(dist));
    if (now) {
      camera.position.copy(toPos);
      camera.up.copy(up);
      controls.target.copy(target);
      camera.lookAt(target);
      redraw();
      return;
    }
    tween = {
      start: performance.now(),
      fromPos: camera.position.clone(),
      toPos,
      fromTarget: controls.target.clone(),
      toTarget: target,
      fromUp: camera.up.clone(),
      toUp: up,
    };
    redraw();
  }

  function go(name, now) {
    if (!board) return;
    view = name;
    const [d, u] = VIEWS[name];
    const k = { tilt: 1.15, edge: 0.8 }[name] ?? 1;
    fly(new THREE.Vector3(...d), new THREE.Vector3(...u), new THREE.Vector3(), fitDistance(radius * k), now);
  }

  function focus(ref) {
    const p = parts.get(ref);
    if (!p) return;
    const box = new THREE.Box3().setFromObject(p);
    if (box.isEmpty()) return;
    const c = box.getCenter(new THREE.Vector3());
    let dir = camera.position.clone().sub(controls.target).normalize();
    const back = p.userData.side === 'back';
    if (back ? dir.y > 0 : dir.y < 0) dir = new THREE.Vector3(0, back ? -0.8 : 0.8, 0.6);
    const size = box.getSize(new THREE.Vector3()).length();
    fly(dir, new THREE.Vector3(0, 1, 0), c, Math.max(fitDistance(size * 1.8), 0.012));
    view = '';
  }

  function toggle(g) {
    g.on = !g.on;
    const def = GROUPS.find(([k]) => k === g.key);
    const objs = def[2] ? def[2].flatMap((l) => byLayer[l] ?? []) : [...parts.values()];
    objs.forEach((o) => (o.visible = g.on));
    redraw();
  }

  // ---- picking and highlight -------------------------------------------------
  const ray = new THREE.Raycaster();
  const shown = (o) => {
    for (; o; o = o.parent) if (!o.visible) return false;
    return true;
  };

  function partAt(ev) {
    if (!board) return null;
    const r = canvas.getBoundingClientRect();
    ray.setFromCamera(
      new THREE.Vector2(((ev.clientX - r.left) / r.width) * 2 - 1, -((ev.clientY - r.top) / r.height) * 2 + 1),
      camera,
    );
    for (const h of ray.intersectObjects([...parts.values(), ...occluders].filter(shown), true)) {
      if (!shown(h.object)) continue;
      for (let o = h.object; o; o = o.parent) if (o.userData.ref) return o.userData.ref;
      return null; // the board is in the way
    }
    return null;
  }

  function tint(node, colour) {
    node.traverse((o) => {
      if (!o.isMesh) return;
      if (colour) {
        if (o.userData.own) o.material.dispose();
        else o.userData.own = o.material;
        o.material = o.userData.own.clone();
        o.material.emissive = colour;
        o.material.emissiveIntensity = 0.75;
      } else if (o.userData.own) {
        o.material.dispose();
        o.material = o.userData.own;
        delete o.userData.own;
      }
    });
  }

  function tintAll() {
    for (const [ref, node] of parts) {
      tint(node, selected.includes(ref) ? SELECT : ref === hoverRef ? HOVER : null);
    }
    redraw();
  }

  let lastSelected = [];
  $effect(() => {
    const now = [...selected];
    if (!board) return;
    tintAll();
    // Fly to a part chosen elsewhere (another window, the 2D view).
    const fresh = now.find((r) => !lastSelected.includes(r));
    if (fresh && now.length === 1) focus(fresh);
    lastSelected = now;
  });

  let down = null;
  let pending = 0;
  function onMove(ev) {
    if (ev.buttons || pending) return;
    pending = requestAnimationFrame(() => {
      pending = 0;
      const ref = partAt(ev);
      if (ref !== hoverRef) {
        hoverRef = ref;
        canvas.style.cursor = ref ? 'pointer' : '';
        tintAll();
      }
    });
  }
  function onUp(ev) {
    if (!down || Math.hypot(ev.clientX - down[0], ev.clientY - down[1]) > 4) return;
    const ref = partAt(ev);
    lastSelected = ref ? [ref] : []; // picked here: no need to fly to it
    onselect(ref ? [ref] : []);
  }
</script>

<div class="b3">
  <aside class="panel">
    <div class="label">View</div>
    <div class="views">
      {#each [['top', 'Top'], ['bottom', 'Bottom'], ['tilt', 'Tilted'], ['edge', 'Edge']] as [k, t]}
        <button class="btn" class:on={view === k} onclick={() => go(k)}>{t}</button>
      {/each}
    </div>
    <div class="label">Show</div>
    {#each groups as g (g.key)}
      <label class="check"><input type="checkbox" checked={g.on} onchange={() => toggle(g)} /> {g.name}</label>
    {/each}
    <p class="hint muted">Drag to turn, right-drag or two fingers to move, wheel to zoom. Click a part to select it.</p>
  </aside>
  <div class="stage" bind:this={stage}>
    <canvas
      bind:this={canvas}
      onpointerdown={(e) => (down = [e.clientX, e.clientY])}
      onpointermove={onMove}
      onpointerup={onUp}
      onpointerleave={() => {
        hoverRef = null;
        tintAll();
      }}
    ></canvas>
    {#if status}<p class="status">{status}</p>{/if}
    {#if hoverRef}<div class="readout mono">{hoverRef}</div>{/if}
  </div>
</div>

<style>
  .b3 {
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

  .label {
    margin-top: 8px;
  }

  .views {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }

  .views .on {
    border-color: var(--accent);
    background: var(--accent-soft);
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
    min-height: 0;
    background: radial-gradient(ellipse at 50% 40%, #26323a, #0d1418 75%);
  }

  canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    touch-action: none;
  }

  .status {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: #cfd8dc;
    margin: 0;
    padding: 20px;
    text-align: center;
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

  @media (max-width: 760px) {
    .b3 {
      grid-template-columns: 1fr;
      grid-template-rows: auto 60vh;
    }

    .panel {
      border-right: none;
      border-bottom: 1px solid var(--line);
    }
  }
</style>
