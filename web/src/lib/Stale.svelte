<script>
  import { STATIC } from './api.js';
  import { modified } from './stale.js';

  // The bar over a drawn tab once the board has changed since it was drawn. `change` is the
  // board's newest change (stale.js); `from` is the file the tab is drawn from, whose time says
  // when it was drawn on this machine. On Pages every tab shows the published board, so any
  // change in the tab makes it stale.
  let { project, change, from, fix = '', onhistory } = $props();

  const APPS = { route: 'Route', composer: 'Composer', boardroom: 'Boardroom' };

  let drawn = $state(null);
  $effect(() => {
    const [p, f, c] = [project, from, change];
    drawn = null;
    if (STATIC || !f || !c) return;
    let live = true;
    modified(p.id, f).then((t) => live && (drawn = t)).catch(() => {});
    return () => (live = false);
  });

  // A couple of seconds' slack: a build that reads the board writes its output just after.
  const shown = $derived(!!change && (STATIC || (!!drawn && new Date(change.time) - drawn > 2000)));
  const file = $derived(change?.path.split('/').pop());

  function when(t) {
    const d = new Date(t);
    const today = d.toDateString() === new Date().toDateString();
    return today
      ? d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' })
      : d.toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' });
  }
</script>

{#if shown}
  <div class="stale" role="status">
    {#if STATIC}
      <span>
        Showing the published board; <span class="mono">{file}</span> changed in {APPS[change.app] ?? change.app}
        at {when(change.time)}{#if change.message}<span class="muted">{` · ${change.message}`}</span>{/if}
      </span>
    {:else}
      <span>
        Drawn before the board's last change: <span class="mono">{file}</span> changed at {when(change.time)},
        after this was made at {when(drawn)}.{#if fix}<span class="muted">{` ${fix}`}</span>{/if}
      </span>
    {/if}
    {#if onhistory}<button class="btn" onclick={onhistory}>History</button>{/if}
  </div>
{/if}

<style>
  .stale {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
    padding: 6px 14px;
    font-size: 13px;
    color: var(--stale);
    background: color-mix(in srgb, var(--stale) 12%, transparent);
    border-bottom: 1px solid color-mix(in srgb, var(--stale) 35%, transparent);
    flex-shrink: 0;
  }

  /* In a padded tab (Overview, Renders) it sits as a card above the content. */
  :global(.body:not(.flush)) > .stale {
    border: 1px solid color-mix(in srgb, var(--stale) 35%, transparent);
    border-radius: 6px;
    margin-bottom: 12px;
  }

  .stale .btn {
    padding: 2px 10px;
    font-size: 12.5px;
  }
</style>
