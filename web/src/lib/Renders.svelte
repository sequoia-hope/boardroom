<script>
  import { listing, fileUrl } from './api.js';

  // design/renders/ as a gallery; click one to see it full size.
  let { project } = $props();

  let images = $state([]);
  let others = $state([]);
  let big = $state(null);

  const IMAGE = /\.(png|jpe?g|svg|webp|gif)$/i;

  $effect(() => {
    const p = project;
    const stamp = p.updated;
    load(p).catch(() => (images = []));
  });

  async function load(p) {
    const top = await listing(p, 'design/renders');
    const found = [];
    const rest = [];
    for (const e of top) {
      const path = `design/renders/${e.name}`;
      if (e.dir) {
        for (const s of await listing(p, path)) {
          if (!s.dir && IMAGE.test(s.name)) found.push(`${path}/${s.name}`);
        }
      } else if (IMAGE.test(e.name)) found.push(path);
      else rest.push(path);
    }
    images = found;
    others = rest;
  }

  const name = (p) => p.replace('design/renders/', '');
</script>

{#if images.length}
  <div class="grid">
    {#each images as img (img)}
      <button class="tile card" onclick={() => (big = img)}>
        <img src={fileUrl(project.id, img)} alt={name(img)} loading="lazy" />
        <span class="mono">{name(img)}</span>
      </button>
    {/each}
  </div>
  {#if others.length}
    <p class="muted">
      Also: {#each others as o, i}<a href={fileUrl(project.id, o)} target="_blank">{name(o)}</a>{i < others.length - 1 ? ', ' : ''}{/each}
    </p>
  {/if}
{:else}
  <p class="muted">No renders yet. The skill writes them to design/renders/ at each gate.</p>
{/if}

{#if big}
  <button class="lightbox" onclick={() => (big = null)} aria-label="Close">
    <img src={fileUrl(project.id, big)} alt={name(big)} />
    <span class="mono">{name(big)} · click to close</span>
  </button>
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
    gap: 12px;
  }

  .tile {
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    text-align: left;
  }

  .tile:hover {
    border-color: var(--accent);
  }

  .tile img {
    width: 100%;
    aspect-ratio: 4 / 3;
    object-fit: contain;
    background: var(--panel-2);
    border-radius: 4px;
  }

  .tile span {
    font-size: 11.5px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .lightbox {
    position: fixed;
    inset: 0;
    z-index: 20;
    border: none;
    background: #000d;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 24px;
    color: #ddd;
  }

  .lightbox img {
    max-width: 100%;
    max-height: calc(100% - 30px);
    object-fit: contain;
  }
</style>
