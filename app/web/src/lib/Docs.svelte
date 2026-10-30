<script>
  import { marked } from 'marked';
  import { docs as listDocs, text, fileUrl } from './api.js';

  // A project's markdown: a list on the left, the chosen file rendered.
  // `filter` picks which files this tab shows; `selected` can be set from
  // outside (a click on a gate cell or a link).
  let { project, filter = () => true, selected = $bindable(null), titles = {} } = $props();

  let files = $state([]);
  let html = $state('');
  let error = $state('');

  $effect(() => {
    const p = project;
    listDocs(p)
      .then((all) => {
        files = all.filter(filter);
        if (!selected || !files.includes(selected)) selected = files[0] ?? null;
      })
      .catch((e) => (error = e.message));
  });

  $effect(() => {
    const p = project;
    const path = selected;
    const stamp = p.updated; // re-render when the project changes
    if (!path) {
      html = '';
      return;
    }
    error = '';
    text(p.id, path)
      .then((md) => (html = render(md, p.id, path)))
      .catch((e) => (error = e.message));
  });

  // Relative links and images inside a document point at project files.
  function render(md, id, path) {
    const dir = path.split('/').slice(0, -1);
    const resolve = (href) => {
      if (/^([a-z]+:|#|\/)/i.test(href)) return null;
      const parts = [...dir];
      for (const seg of href.split('/')) {
        if (seg === '..') parts.pop();
        else if (seg !== '.' && seg !== '') parts.push(seg);
      }
      return parts.join('/');
    };
    return marked.parse(md, {
      gfm: true,
      walkTokens(token) {
        if (token.type === 'image' || token.type === 'link') {
          const rel = resolve(token.href);
          if (rel) token.href = fileUrl(id, rel);
        }
      },
    });
  }

  const label = (f) => titles[f] ?? f.split('/').pop().replace(/\.md$/, '');
</script>

<div class="docs">
  <nav class="card">
    {#each files as f (f)}
      <button class:on={f === selected} onclick={() => (selected = f)}>{label(f)}</button>
    {:else}
      <p class="muted pad">No documents yet.</p>
    {/each}
  </nav>
  <article class="card md">
    {#if error}
      <p class="err">{error}</p>
    {:else if html}
      <div class="path mono muted">{selected}</div>
      {@html html}
    {/if}
  </article>
</div>

<style>
  .docs {
    display: grid;
    grid-template-columns: 210px minmax(0, 1fr);
    gap: 14px;
    align-items: start;
  }

  nav {
    display: flex;
    flex-direction: column;
    padding: 6px;
    position: sticky;
    top: calc(var(--top-h, 62px) + 52px);
    max-height: calc(100vh - var(--top-h, 62px) - 70px);
    overflow: auto;
  }

  nav button {
    text-align: left;
    border: none;
    background: none;
    padding: 5px 8px;
    border-radius: 5px;
  }

  nav button:hover {
    background: var(--panel-2);
  }

  nav button.on {
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }

  .pad {
    padding: 6px 8px;
    margin: 0;
  }

  .md {
    padding: 14px 22px 22px;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .path {
    font-size: 11.5px;
    margin-bottom: 4px;
  }

  .err {
    color: var(--bad);
  }

  .md :global(h1) {
    font-size: 22px;
    margin: 8px 0 12px;
  }

  .md :global(h2) {
    font-size: 17px;
    margin: 22px 0 8px;
  }

  .md :global(h3) {
    font-size: 15px;
    margin: 18px 0 6px;
  }

  .md :global(table) {
    border-collapse: collapse;
    margin: 10px 0;
    display: block;
    overflow-x: auto;
  }

  .md :global(th),
  .md :global(td) {
    border: 1px solid var(--line);
    padding: 4px 8px;
    vertical-align: top;
  }

  .md :global(th) {
    background: var(--panel-2);
  }

  .md :global(pre) {
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 10px 12px;
    overflow-x: auto;
    font-size: 12.5px;
  }

  .md :global(code) {
    font-family: ui-monospace, 'SF Mono', Menlo, monospace;
    font-size: 0.92em;
  }

  .md :global(img) {
    max-width: 100%;
    border-radius: 6px;
    border: 1px solid var(--line);
  }

  .md :global(blockquote) {
    margin: 10px 0;
    padding: 2px 14px;
    border-left: 3px solid var(--accent);
    color: var(--muted);
  }

  @media (max-width: 760px) {
    .docs {
      grid-template-columns: 1fr;
    }

    nav {
      position: static;
      max-height: none;
      flex-direction: row;
      flex-wrap: wrap;
    }
  }
</style>
