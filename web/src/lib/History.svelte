<script>
  // What changed in the project, through the store. In the browser alone: this tab's changes to
  // the published project, each revision kept, with Restore, Download changes and Reset. On this
  // machine: what isn't committed yet, then git's log.
  let { store } = $props();

  let entries = $state([]);
  let changes = $state([]);
  let error = $state('');
  let busy = $state(false);
  const browser = $derived(store?.kind === 'browser');

  async function load() {
    error = '';
    try {
      entries = await store.history();
      changes = browser ? await store.changes() : [];
    } catch (e) {
      error = e.message;
    }
  }

  $effect(() => {
    if (!store) return;
    load();
    return store.watch(() => load());
  });

  // The newest revision of each file is what it is now; older ones can be restored.
  const latest = $derived(new Set(changes.map((c) => c.rev)));

  async function act(fn) {
    busy = true;
    try {
      await fn();
      await load();
    } catch (e) {
      error = e.message;
    } finally {
      busy = false;
    }
  }

  async function download() {
    const blob = await store.download();
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = `${store.project}-changes.zip`;
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 1000);
  }

  function reset() {
    if (confirm('Drop every change made in this tab and go back to the published project?')) act(() => store.reset());
  }

  const when = (t) => (t ? new Date(t).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' }) : '');
</script>

{#if browser}
  <div class="head">
    <p class="muted">
      Changes are kept in this browser tab, and a reload keeps them. A new tab starts from the
      published project.
      {#if store.persisted === false}The browser may clear them if space runs low; download them to keep them.{/if}
    </p>
    <div class="actions">
      <button class="btn" onclick={() => act(download)} disabled={busy || !changes.length} title="A zip of the changed files at their places in the project, to unpack over a clone">Download changes</button>
      <button class="btn" onclick={reset} disabled={busy || !changes.length}>Reset to the published project</button>
    </div>
  </div>
  {#if changes.length}
    <p class="label">Changed files</p>
    <ul class="files card">
      {#each changes as c (c.path)}
        <li><span class="mono">{c.path}</span> <span class="muted">· {c.app}, {when(c.time)}</span></li>
      {/each}
    </ul>
  {/if}
{/if}

{#if error}<p class="err">{error}</p>{/if}

{#if entries.length}
  <p class="label">{browser ? 'Every revision' : 'History'}</p>
  <ol class="log card">
    {#each entries as e (e.rev + e.path)}
      <li>
        <div class="line">
          <span class="msg">{e.message || 'Saved'}</span>
          {#if browser && !latest.has(e.rev)}
            <button class="btn small" disabled={busy} onclick={() => act(() => store.restore(e.rev))} title="Make this revision the current one again (a new revision; nothing is lost)">Restore</button>
          {/if}
        </div>
        <div class="muted meta">
          {[e.app, when(e.time)].filter(Boolean).concat('').join(' · ')}<span class="mono">{e.paths?.length > 1 ? `${e.paths.length} files` : e.path}</span>
          {#if e.rev !== 'uncommitted'}<span class="mono rev">{e.rev.slice(0, 8)}</span>{/if}
        </div>
      </li>
    {/each}
  </ol>
{:else if !error}
  <p class="muted">{browser ? 'Nothing changed in this tab yet. Saving in Composer or Route shows up here.' : 'No history: the project folder is not in git, and nothing has been saved yet.'}</p>
{/if}

<style>
  .head {
    display: flex;
    gap: 16px;
    align-items: flex-start;
    justify-content: space-between;
    flex-wrap: wrap;
    margin-bottom: 12px;
  }

  .head p {
    margin: 0;
    max-width: 60ch;
  }

  .actions {
    display: flex;
    gap: 8px;
  }

  .label {
    margin: 16px 0 6px;
  }

  ul,
  ol {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .files li {
    padding: 6px 12px;
    border-top: 1px solid var(--line);
  }

  .log li {
    padding: 8px 12px;
    border-top: 1px solid var(--line);
  }

  .files li:first-child,
  .log li:first-child {
    border-top: 0;
  }

  .line {
    display: flex;
    gap: 12px;
    align-items: center;
    justify-content: space-between;
  }

  .meta {
    font-size: 12.5px;
  }

  .rev {
    margin-left: 6px;
    opacity: 0.7;
  }

  .small {
    padding: 2px 8px;
    font-size: 12.5px;
  }

  .err {
    color: #c0392b;
  }
</style>
