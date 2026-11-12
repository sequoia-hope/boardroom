<script>
  import Pipeline from './Pipeline.svelte';
  import { fileUrl } from './api.js';

  // The headline for one project: where it is, what needs you, what's true.
  let { project: p, ongate = () => {}, onlink = () => {} } = $props();

  const phaseName = $derived(p.phase != null ? p.gates[p.phase]?.name : null);
  const passed = $derived(p.gates.filter((g) => g.state === 'passed').length);

  // Composer and Studio report under apps.<name>.<board> (status.json v1).
  const appRows = $derived(
    Object.entries(p.apps ?? {}).flatMap(([app, boards]) =>
      Object.entries(boards ?? {}).map(([board, v]) => ({ app, board, v })),
    ),
  );

  const KIND = { signoff: 'Sign-off', question: 'Question', alert: 'Problem' };

  const n = (k, word) => `${k} ${word}${k === 1 ? '' : 's'}`;

  // Studio's entry, from ultraroute's run report: "412/414 routed · 2 opens".
  function studioLine(v) {
    const parts = [];
    if (v.connections != null) parts.push(`${v.routed ?? '?'}/${v.connections} routed`);
    if (v.opens != null) parts.push(v.opens ? `${v.opens} open${v.opens === 1 ? '' : 's'}` : 'no opens');
    const f = v.findings ?? {};
    if (f.error || f.warning) parts.push(`${n(f.error ?? 0, 'error')}, ${n(f.warning ?? 0, 'warning')}`);
    return parts.join(' · ');
  }

  function fmt(v) {
    if (v == null) return '';
    if (typeof v === 'object') return Object.entries(v).map(([k, x]) => `${k} ${x}`).join(', ');
    return String(v);
  }
</script>

<div class="overview">
  <section class="head card">
    {#if p.thumbnail}
      <img class="thumb" src={fileUrl(p.id, p.thumbnail)} alt="{p.board ?? p.name}, top render" />
    {/if}
    <div class="state">
      <div class="phase">
        {#if p.phase != null}
          <span class="label">Phase {p.phase}</span>
          <h2>{phaseName}</h2>
        {:else}
          <span class="label">Phase</span>
          <h2 class="muted">Not started</h2>
        {/if}
        <span class="muted count">{passed} of {p.gates.length} gates passed</span>
      </div>
      <Pipeline gates={p.gates} current={p.phase} onpick={ongate} />
      {#if p.doing}
        <p class="doing"><span class="label">Doing</span> {p.doing}</p>
      {/if}
    </div>
  </section>

  <section class="card block needs" class:empty={!p.needs_user.length}>
    <h3 class="label">Needs you</h3>
    {#each p.needs_user as n (n.id)}
      <div class="need">
        <span class="kind {n.kind}">{KIND[n.kind] ?? 'Question'}</span>
        <p>{n.text}</p>
        {#if n.options.length}
          <div class="opts">
            {#each n.options as o}<button class="btn" disabled title="Answering lands with AI jobs (milestone 4)">{o}</button>{/each}
          </div>
        {/if}
      </div>
    {:else}
      <p class="muted">Nothing is waiting on you.</p>
    {/each}
  </section>

  <div class="cols">
    <section class="card block">
      <h3 class="label">Checks</h3>
      {#if p.checks.length}
        <table>
          <tbody>
            {#each p.checks as c (c.name)}
              <tr>
                <td class="mark {c.ok === true ? 'ok' : c.ok === false ? 'fail' : 'unk'}">
                  {c.ok === true ? '✓' : c.ok === false ? '✕' : '·'}
                </td>
                <th>{c.name}</th>
                <td>{c.result}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {:else}
        <p class="muted">No checks recorded yet.</p>
      {/if}
    </section>

    <section class="card block side">
      <h3 class="label">Project</h3>
      <dl>
        {#if p.boards.length}
          <dt>Boards</dt>
          <dd>{p.boards.map((b) => `${b.board} / ${b.variant}`).join(', ')}</dd>
        {/if}
        {#if p.example}
          <dt>Example</dt>
          <dd>{p.example}</dd>
        {/if}
        {#if p.git}
          {#if p.git.remote}
            <dt>Remote</dt>
            <dd>
              {#if p.git.remote.startsWith('https://')}<a href={p.git.remote} target="_blank" rel="noopener">{p.git.remote.replace('https://', '')}</a>{:else}<span class="mono">{p.git.remote}</span>{/if}
            </dd>
          {/if}
          <dt>Git</dt>
          <dd>
            {p.git.branch || 'detached'}
            {#if p.git.ahead}<span class="ahead"> · {p.git.ahead} to push</span>{/if}
            {#if p.git.behind}<span class="dirty"> · {p.git.behind} to pull</span>{/if}
            {#if p.git.ahead === 0 && p.git.behind === 0}<span class="muted"> · in step with its remote</span>{/if}
            {#if p.git.dirty}<span class="dirty"> · {p.git.dirty} uncommitted</span>{/if}
          </dd>
          <dt>Last commit</dt>
          <dd>{p.git.last_commit} <span class="muted">{p.git.last_date}</span></dd>
        {/if}
        {#if p.updated}
          <dt>Status updated</dt>
          <dd>{p.updated}</dd>
        {/if}
        <dt>Folder</dt>
        <dd class="mono path">{p.path}</dd>
      </dl>

      {#if appRows.length}
        <h3 class="label">Apps</h3>
        <dl>
          {#each appRows as r}
            <dt>{r.app} · {r.board}</dt>
            {#if r.app === 'studio'}
              <dd>
                {#if r.v.failed}<span class="dirty">{r.v.failed === 'crash' ? 'crashed' : 'ran out of time'} · </span>{/if}
                {studioLine(r.v)}
                {#if r.v.report}· <a href={fileUrl(p.id, r.v.report)} target="_blank" rel="noopener">run report</a>{/if}
                {#if r.v.last_route}<br /><span class="muted">routed {r.v.last_route.replace('T', ' ').replace(/\.\d+/, '')}{r.v.router ? ` · ultraroute ${String(r.v.router).slice(0, 7)}` : ''}</span>{/if}
              </dd>
            {:else}
              <dd>{Object.entries(r.v).filter(([k]) => k !== 'updated').map(([k, v]) => `${k}: ${fmt(v)}`).join(' · ')}</dd>
            {/if}
          {/each}
        </dl>
      {/if}

      {#if p.links.length}
        <h3 class="label">Links</h3>
        <ul class="links">
          {#each p.links as l}
            <li><a href={fileUrl(p.id, l.href)} onclick={(e) => onlink(e, l)}>{l.label}</a></li>
          {/each}
        </ul>
      {/if}
    </section>
  </div>
</div>

<style>
  .overview {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .head {
    display: grid;
    grid-template-columns: 220px 1fr;
    gap: 18px;
    padding: 14px;
    align-items: start;
  }

  .thumb {
    width: 100%;
    aspect-ratio: 4 / 3;
    object-fit: contain;
    background: var(--panel-2);
    border-radius: 6px;
    border: 1px solid var(--line);
  }

  .state {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
  }

  .phase {
    display: flex;
    align-items: baseline;
    gap: 10px;
    flex-wrap: wrap;
  }

  .phase h2 {
    font-size: 20px;
  }

  .count {
    margin-left: auto;
    font-size: 12.5px;
  }

  .doing {
    margin: 0;
  }

  .doing .label {
    margin-right: 6px;
  }

  .block {
    padding: 12px 14px;
  }

  .block h3 {
    margin-bottom: 8px;
  }

  .needs:not(.empty) {
    border-color: var(--warn);
  }

  .need {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 4px 10px;
    align-items: baseline;
  }

  .need + .need {
    margin-top: 10px;
    padding-top: 10px;
    border-top: 1px solid var(--line);
  }

  .need p {
    margin: 0;
  }

  .kind {
    font-size: 11px;
    font-weight: 600;
    padding: 1px 7px;
    border-radius: 10px;
    background: var(--warn-soft);
    color: var(--warn);
  }

  .kind.alert {
    background: color-mix(in srgb, var(--bad) 15%, transparent);
    color: var(--bad);
  }

  .kind.signoff {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .opts {
    grid-column: 2;
    display: flex;
    gap: 6px;
  }

  .cols {
    display: grid;
    grid-template-columns: minmax(0, 1.6fr) minmax(0, 1fr);
    gap: 14px;
    align-items: start;
  }

  table {
    border-collapse: collapse;
    width: 100%;
  }

  td,
  th {
    text-align: left;
    vertical-align: top;
    padding: 5px 6px;
    border-top: 1px solid var(--line);
  }

  tr:first-child td,
  tr:first-child th {
    border-top: none;
  }

  th {
    font-weight: 600;
    white-space: nowrap;
  }

  .mark {
    width: 1.5em;
    font-weight: 700;
    text-align: center;
  }

  .ok {
    color: var(--accent);
  }

  .fail {
    color: var(--bad);
  }

  .unk {
    color: var(--muted);
  }

  dl {
    margin: 0 0 12px;
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 3px 10px;
  }

  dt {
    color: var(--muted);
  }

  dd {
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .path {
    font-size: 11.5px;
  }

  .dirty {
    color: var(--warn);
  }

  .ahead {
    color: var(--accent);
  }

  .links {
    margin: 0;
    padding: 0;
    list-style: none;
    columns: 2;
  }

  .links li {
    padding: 2px 0;
  }

  @media (max-width: 900px) {
    .cols,
    .head {
      grid-template-columns: 1fr;
    }

    .thumb {
      max-width: 320px;
    }
  }
</style>
