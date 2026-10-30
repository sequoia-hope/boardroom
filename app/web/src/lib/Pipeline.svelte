<script>
  // The phase pipeline: one cell per phase, 0 to 10, coloured by its gate.
  let { gates = [], current = null, onpick = () => {} } = $props();

  const words = {
    passed: 'passed',
    current: 'in progress',
    open: 'gate file, not passed',
    stale: 'stale',
    not_started: 'not started',
  };
</script>

<ol class="pipe">
  {#each gates as g (g.phase)}
    <li>
      <button
        class="cell {g.state}"
        class:here={g.phase === current}
        title="{g.phase} {g.name}: {words[g.state] ?? g.state}{g.signoff ? ' · you sign off' : ''}{g.stale_because ? ` · ${g.stale_because}` : ''}"
        onclick={() => onpick(g)}
        disabled={!g.file}
      >
        <span class="n">{g.phase}</span>
        {#if g.signoff}<span class="sign" aria-label="you sign off">✎</span>{/if}
      </button>
      <span class="name">{g.name}</span>
    </li>
  {/each}
</ol>

<style>
  .pipe {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(11, minmax(0, 1fr));
    gap: 4px;
  }

  li {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  .cell {
    position: relative;
    height: 34px;
    border-radius: 5px;
    border: 1px solid var(--line);
    background: var(--panel-2);
    display: grid;
    place-items: center;
    padding: 0;
  }

  .cell:disabled {
    opacity: 1;
  }

  .n {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .sign {
    position: absolute;
    top: 1px;
    right: 4px;
    font-size: 10px;
    opacity: 0.7;
  }

  .passed {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--bg);
  }

  .current {
    border: 2px solid var(--accent);
    background: var(--accent-soft);
  }

  .open {
    border-color: var(--warn);
    background: var(--warn-soft);
  }

  .stale {
    background: repeating-linear-gradient(-45deg, var(--warn-soft) 0 5px, transparent 5px 10px);
    border-color: var(--stale);
  }

  .not_started {
    color: var(--muted);
  }

  .here {
    outline: 2px solid var(--text);
    outline-offset: 2px;
  }

  .name {
    font-size: 10.5px;
    line-height: 1.2;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  @media (max-width: 640px) {
    .name {
      display: none;
    }
  }
</style>
