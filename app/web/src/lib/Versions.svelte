<script>
  import * as api from './api.js';
  import History from './History.svelte';

  let { project, store, onchanged = () => {} } = $props();

  let links = $state([]);
  let published = $state([]);
  let loading = $state(false);
  let busyId = $state('');
  let error = $state('');
  let rehostErrors = $state({});
  let pushBusy = $state(false);
  let pushError = $state('');

  // Inline per-link action forms (Pin… and Follow branch)
  let pinFor = $state(null);
  let pinReason = $state('');
  let followFor = $state(null);
  let followBranch = $state('main');

  // Add link form
  let adding = $state(false);
  let newId = $state('');
  let newKind = $state('waffle');
  let newBoard = $state('');
  let newRemote = $state('');
  let newPath = $state('');
  let pathTouched = $state(false);
  let newRefType = $state('Branch');
  let newRefValue = $state('main');
  let addError = $state('');
  let addBusy = $state(false);

  const defaultBoard = $derived(project?.boards?.[0]?.board ?? project?.board ?? project?.name ?? '');

  function openAdd() {
    newId = '';
    newKind = 'waffle';
    newBoard = defaultBoard;
    newRemote = '';
    newPath = defaultBoard ? `interface/${defaultBoard}/board-shape.json` : 'interface/board/board-shape.json';
    pathTouched = false;
    newRefType = 'Branch';
    newRefValue = 'main';
    addError = '';
    adding = true;
  }

  function onBoardInput(e) {
    newBoard = e.currentTarget.value;
    if (!pathTouched) {
      const b = newBoard.trim() || 'board';
      newPath = `interface/${b}/board-shape.json`;
    }
  }

  async function load(forceRefresh = false) {
    if (!project) return;
    loading = true;
    error = '';
    try {
      const [nextLinks, nextPublished] = await Promise.all([
        api.links(project, forceRefresh),
        api.published(project),
      ]);
      links = nextLinks;
      published = nextPublished;
    } catch (e) {
      error = e.message;
    } finally {
      loading = false;
    }
  }

  // Reload when the selected project changes or when a bus update refreshes `project`.
  let lastProjectId = null;
  $effect(() => {
    const p = project;
    if (!p) {
      links = [];
      published = [];
      lastProjectId = null;
      return;
    }
    if (p.id !== lastProjectId) {
      lastProjectId = p.id;
      pushError = '';
      rehostErrors = {};
    }
    load(false);
  });

  const short = (sha) => (sha ? String(sha).slice(0, 7) : '—');
  const mm = (nm) => (Math.abs(nm) / 1_000_000).toFixed(2);
  const release = $derived(project?.release ?? null);
  const releaseLinks = $derived(release?.links ?? []);
  const linkById = $derived(Object.fromEntries(links.map((l) => [l.id, l])));

  function shortRev(rev) {
    if (!rev) return '—';
    if (rev.startsWith('rev:sha256:') && rev.length > 18) return `rev:${rev.slice(11, 18)}`;
    if (rev.startsWith('sha256:') && rev.length > 14) return rev.slice(0, 14);
    return rev.length > 20 ? `${rev.slice(0, 18)}…` : rev;
  }

  function dirName(p) {
    if (!p) return 'checkout';
    const trimmed = String(p).replace(/[\\/]+$/, '');
    const idx = Math.max(trimmed.lastIndexOf('/'), trimmed.lastIndexOf('\\'));
    return idx >= 0 ? trimmed.slice(idx + 1) || trimmed : trimmed;
  }

  function unpushedWords(u) {
    if (!u) return '';
    const name = dirName(u.project_path);
    const n = u.commits ?? 0;
    if (n > 0) {
      const noun = n === 1 ? 'commit' : 'commits';
      const dirty = u.uncommitted ? ' and uncommitted changes' : '';
      return `${name} has ${n} unpushed outline ${noun} on ${u.branch} here${dirty}: push it to accept it.`;
    }
    if (u.uncommitted) {
      return `${name} has uncommitted outline changes on ${u.branch} here: push it to accept it.`;
    }
    return '';
  }

  function publishedTone(state) {
    switch (state) {
      case 'Pushed':
        return 'quiet';
      case 'NotPushed':
      case 'Uncommitted':
      case 'NoUpstream':
      case 'NotInGit':
        return 'notice';
      default:
        return 'info';
    }
  }

  function publishedWords(pub) {
    switch (pub.state) {
      case 'Pushed':
        return 'Pushed';
      case 'NotPushed':
        return pub.upstream_revision
          ? `Committed, not pushed: other machines still see ${shortRev(pub.upstream_revision)}`
          : 'Committed, not pushed: other machines do not see this outline yet';
      case 'Uncommitted':
        return 'Uncommitted changes';
      case 'NoUpstream':
        return 'No upstream branch';
      case 'NotInGit':
        return 'Not tracked in git';
      default:
        return pub.state;
    }
  }

  function driverName(driver) {
    if (driver === 'waffle') return 'Waffle Iron';
    if (driver === 'kicad') return 'KiCad';
    if (driver === 'composer') return 'Circuit Composer';
    return driver;
  }

  function lockedText(link) {
    if (!link.locked_by) return '';
    return `Outline controlled by ${driverName(link.locked_by)}`;
  }

  function refSummary(ref) {
    if (!ref) return '';
    if (ref.type === 'Branch') return `follows branch ${ref.name}`;
    if (ref.type === 'Tag') return `follows tag ${ref.name}`;
    if (ref.type === 'Commit') return `pinned at ${short(ref.commit)}`;
    return '';
  }

  function stateTone(state) {
    switch (state) {
      case 'Current':
      case 'NewRevisionNoInterfaceChange':
        return 'quiet';
      case 'PinnedBehind':
        return 'info';
      case 'UpdateAvailable':
      case 'Unavailable':
        return 'notice';
      case 'ChangedOutsideSuite':
      case 'CompetingTakeOvers':
      case 'StaleTakeOver':
      case 'Tampered':
        return 'alert';
      default:
        return 'quiet';
    }
  }

  function stateWords(link) {
    const rev = short(link.resolved?.commit);
    const tip = short(link.latest?.commit);
    const pinned = link.locator?.ref?.type === 'Commit';
    switch (link.state) {
      case 'Current':
        return pinned ? `Pinned at ${rev} · up to date` : `Up to date (${rev})`;
      case 'NewRevisionNoInterfaceChange':
        return `Up to date · new commit ${rev}, no outline changes`;
      case 'UpdateAvailable':
        return `Update available · ${rev} → ${tip}`;
      case 'PinnedBehind':
        return `Pinned at ${rev}, latest is ${tip}`;
      case 'ChangedOutsideSuite':
        return 'Changed outside the suite: import it?';
      case 'CompetingTakeOvers':
        return `Two take overs of ${link.board}'s outline: pick one`;
      case 'StaleTakeOver':
        return `${link.board}'s outline was taken over from an older revision while the other side kept driving: pick one`;
      case 'Unavailable':
        return `Unavailable · last accepted version (${rev}) keeps working`;
      case 'Tampered':
        return `Stored interface hash does not match accepted commit (${rev})`;
      default:
        return link.state;
    }
  }

  // KiCad board frame: +X right, -X left, +Y down, -Y up; 1 mm = 1_000_000 nm.
  function formatChange(c) {
    const subject = c.id ? `${c.role} ${c.id}` : c.role;
    const k = c.kind;
    if (!k) return `${subject} changed`;
    if (k.type === 'added') return `${subject} added`;
    if (k.type === 'removed') return `${subject} removed`;
    if (k.type === 'changed') return `${subject} changed`;
    if (k.type === 'moved') {
      const parts = [];
      if (k.dx_nm) parts.push(`${mm(k.dx_nm)} mm ${k.dx_nm > 0 ? 'right' : 'left'}`);
      if (k.dy_nm) parts.push(`${mm(k.dy_nm)} mm ${k.dy_nm > 0 ? 'down' : 'up'}`);
      return `${subject} moved ${parts.length ? parts.join(', ') : '0.00 mm'}`;
    }
    return `${subject} changed`;
  }

  function changeLines(link) {
    const out = [];
    if (Array.isArray(link.thickness_changed) && link.thickness_changed.length === 2) {
      const [fromNm, toNm] = link.thickness_changed;
      out.push(`board thickness ${mm(fromNm)} mm → ${mm(toNm)} mm`);
    }
    for (const c of link.changes ?? []) {
      out.push(formatChange(c));
    }
    for (const r of link.rematched ?? []) {
      out.push(`re-matched ${r.role} #${r.theirs_index + 1} to ${r.base_id}`);
    }
    for (const u of link.uncertain ?? []) {
      out.push(`uncertain ${u.role} match (${u.candidate_base_ids.join(', ')}): ${u.reason}`);
    }
    return out;
  }

  function canUpdate(link) {
    return (
      link.state === 'UpdateAvailable' ||
      link.state === 'ChangedOutsideSuite' ||
      link.state === 'CompetingTakeOvers' ||
      link.state === 'StaleTakeOver'
    );
  }

  async function runLinkOp(linkId, fn) {
    busyId = linkId;
    error = '';
    const { [linkId]: _, ...rest } = rehostErrors;
    rehostErrors = rest;
    try {
      await fn();
      await load(false);
      onchanged();
    } catch (e) {
      error = e.message;
    } finally {
      busyId = '';
    }
  }

  async function submitRehost(link) {
    busyId = link.id;
    error = '';
    const { [link.id]: _, ...rest } = rehostErrors;
    rehostErrors = rest;
    try {
      await api.rehostLink(project.id, link.id);
      await load(false);
      onchanged();
    } catch (e) {
      rehostErrors = { ...rehostErrors, [link.id]: e.message };
    } finally {
      busyId = '';
    }
  }

  async function submitPush() {
    pushBusy = true;
    pushError = '';
    try {
      await api.pushProject(project.id);
      await load(false);
      onchanged();
    } catch (e) {
      pushError = e.message;
    } finally {
      pushBusy = false;
    }
  }

  function startPin(link) {
    followFor = null;
    pinFor = link.id;
    pinReason = link.pin_reason ?? '';
  }

  async function submitPin(link) {
    const reason = pinReason.trim();
    await runLinkOp(link.id, async () => {
      await api.pinLink(project.id, link.id, reason);
      pinFor = null;
      pinReason = '';
    });
  }

  function startFollow(link) {
    pinFor = null;
    followFor = link.id;
    followBranch = link.locator?.ref?.type === 'Branch' ? link.locator.ref.name : 'main';
  }

  async function submitFollow(link) {
    const branch = followBranch.trim();
    if (!branch) return;
    await runLinkOp(link.id, async () => {
      await api.followLink(project.id, link.id, branch);
      followFor = null;
    });
  }

  async function submitAdd(e) {
    e.preventDefault();
    addError = '';
    addBusy = true;
    const val = newRefValue.trim();
    const ref =
      newRefType === 'Commit'
        ? { type: 'Commit', commit: val }
        : { type: newRefType, name: val || 'main' };
    try {
      await api.addLink(project.id, {
        id: newId.trim(),
        kind: newKind.trim() || 'waffle',
        board: newBoard.trim(),
        remote: newRemote.trim(),
        path: newPath.trim(),
        ref,
      });
      adding = false;
      await load(false);
      onchanged();
    } catch (err) {
      addError = err.message;
    } finally {
      addBusy = false;
    }
  }
</script>

<div class="versions">
  {#if release}
    <section class="built-section" aria-label="Built against">
      <div class="sec-head">
        <h3 class="label">Built against (gate 10)</h3>
        {#if release.judged_at}
          <span class="muted release-date">{release.judged_at.replace('T', ' ').replace(/Z$/, ' UTC')}</span>
        {/if}
      </div>
      {#if releaseLinks.length}
        <div class="card built-card">
          <ul class="built-list">
            {#each releaseLinks as rl (rl.id)}
              {@const live = linkById[rl.id]}
              {@const resolvedNow = live?.resolved?.commit ?? rl.resolved_commit ?? null}
              <li class="built-row" data-release-link={rl.id}>
                <span class="built-id">{rl.id}</span>
                <span class="mono" title={rl.commit}>commit {short(rl.commit)}</span>
                {#if rl.revision}
                  <span class="muted mono" title={rl.revision}>· {shortRev(rl.revision)}</span>
                {/if}
                {#if live?.state === 'PinnedBehind'}
                  <span class="built-flag info">pinned behind latest ({short(live.latest?.commit)})</span>
                {/if}
                {#if resolvedNow === null}
                  <span class="built-flag warn">removed from links.toml</span>
                {:else if resolvedNow !== rl.commit}
                  <span class="built-flag warn">links.toml is now at {short(resolvedNow)}</span>
                {/if}
              </li>
            {/each}
          </ul>
        </div>
      {:else}
        <p class="muted empty-links">Built against local project files (no linked projects at gate 10).</p>
      {/if}
    </section>
  {/if}

  {#if published.length}
    <section class="published-section" aria-label="Your outline">
      <div class="sec-head">
        <h3 class="label">Your outline</h3>
      </div>

      {#if pushError}
        <pre class="err push-err">{pushError}</pre>
      {/if}

      <div class="link-list">
        {#each published as pub (pub.board)}
          {@const pTone = publishedTone(pub.state)}
          <article
            class="card link-card {pTone}"
            data-published-board={pub.board}
            data-published-state={pub.state}
          >
            <div class="link-top">
              <div class="link-title">
                <span class="link-id">{pub.board}</span>
                <span class="pill mono">{pub.path}</span>
                {#if pub.revision}
                  <span class="muted mono ref-meta" title={pub.revision}>{shortRev(pub.revision)}</span>
                {/if}
              </div>
              {#if pub.upstream}
                <span class="muted ref-meta">upstream {pub.upstream}</span>
              {/if}
            </div>

            <div class="pub-state-row">
              <p class="state-line {pTone}">{publishedWords(pub)}</p>
              {#if !api.STATIC && pub.state === 'NotPushed'}
                <button class="btn small primary" disabled={pushBusy} onclick={submitPush}>
                  Push
                </button>
              {/if}
            </div>
          </article>
        {/each}
      </div>
    </section>
  {/if}

  {#if !api.STATIC || links.length}
    <section class="links-section">
      <div class="sec-head">
        <h3 class="label">Linked projects</h3>
        {#if !api.STATIC}
          <div class="sec-actions">
            {#if links.length}
              <button class="btn small" disabled={loading || !!busyId} onclick={() => load(true)}>
                Check remotes
              </button>
            {/if}
            {#if !adding}
              <button class="btn small" onclick={openAdd}>+ Add link</button>
            {/if}
          </div>
        {/if}
      </div>

      {#if error}<p class="err">{error}</p>{/if}

      {#if adding}
        <form class="card link-form" onsubmit={submitAdd}>
          <div class="form-grid">
            <label>
              <span class="label">Link ID</span>
              <input bind:value={newId} placeholder="enclosure" required />
            </label>
            <label>
              <span class="label">Kind</span>
              <input bind:value={newKind} placeholder="waffle" required />
            </label>
            <label>
              <span class="label">Board</span>
              <input value={newBoard} oninput={onBoardInput} placeholder="easy2350" required />
            </label>
            <label class="full">
              <span class="label">Remote repository (URL or local folder)</span>
              <input bind:value={newRemote} placeholder="/path/to/enclosure or https://…" required />
            </label>
            <label>
              <span class="label">Interface JSON path</span>
              <input
                bind:value={newPath}
                oninput={() => (pathTouched = true)}
                placeholder="interface/easy2350/board-shape.json"
                required
              />
            </label>
            <label>
              <span class="label">Follow</span>
              <select
                bind:value={newRefType}
                onchange={() => {
                  if (newRefType === 'Branch' && !newRefValue) newRefValue = 'main';
                }}
              >
                <option value="Branch">Branch</option>
                <option value="Tag">Tag</option>
                <option value="Commit">Commit (pinned)</option>
              </select>
            </label>
            <label>
              <span class="label">{newRefType === 'Commit' ? 'Commit SHA' : `${newRefType} name`}</span>
              <input
                bind:value={newRefValue}
                placeholder={newRefType === 'Commit' ? 'a1b2c3d…' : 'main'}
                required
              />
            </label>
          </div>
          {#if addError}<p class="err">{addError}</p>{/if}
          <div class="form-buttons">
            <button class="btn primary" disabled={addBusy || !newId.trim() || !newBoard.trim() || !newRemote.trim() || !newPath.trim()}>
              Add link
            </button>
            <button type="button" class="btn" disabled={addBusy} onclick={() => ((adding = false), (addError = ''))}>
              Cancel
            </button>
          </div>
        </form>
      {/if}

      {#if links.length}
        <div class="link-list">
          {#each links as link (link.id)}
            {@const lines = changeLines(link)}
            {@const tone = stateTone(link.state)}
            {@const locked = lockedText(link)}
            {@const unpushed = unpushedWords(link.unpushed)}
            <article class="card link-card {tone}" data-link-id={link.id} data-link-state={link.state}>
              <div class="link-top">
                <div class="link-title">
                  <span class="link-id">{link.id}</span>
                  <span class="pill">{link.kind} · {link.board}</span>
                  {#if locked}
                    <span class="locked-chip">{locked}</span>
                  {/if}
                  {#if link.local_only}
                    <span
                      class="local-chip"
                      title={link.hosted_remote ? `Hosted origin: ${link.hosted_remote}` : 'Remote is a filesystem path'}
                    >
                      This machine only
                    </span>
                  {/if}
                </div>
                <span class="muted ref-meta">{refSummary(link.locator?.ref)}</span>
              </div>

              <p class="state-line {tone}">{stateWords(link)}</p>

              {#if link.pin_reason}
                <p class="pin-note muted">Pinned reason: {link.pin_reason}</p>
              {/if}

              {#if link.error}
                <p class="err detail">{link.error}</p>
              {/if}

              {#if link.state === 'Unavailable' && link.hint}
                <p class="hint-line">{link.hint}</p>
              {/if}

              {#if unpushed}
                <p class="unpushed-note">{unpushed}</p>
              {/if}

              {#if rehostErrors[link.id]}
                <p class="err detail" data-rehost-error={link.id}>{rehostErrors[link.id]}</p>
              {/if}

              {#if lines.length}
                <ul class="changes">
                  {#each lines as line}
                    <li>{line}</li>
                  {/each}
                </ul>
              {/if}

              <div class="locator-meta muted mono">
                <span>{link.locator?.remote}</span>
                <span>· {link.locator?.path}</span>
              </div>

              {#if !api.STATIC}
                <div class="link-actions">
                  <button
                    class="btn small"
                    class:primary={canUpdate(link)}
                    disabled={busyId === link.id || !canUpdate(link)}
                    title={canUpdate(link) ? 'Accept the latest interface version' : 'No unaccepted update on a followed ref'}
                    onclick={() => runLinkOp(link.id, () => api.updateLink(project.id, link.id))}
                  >
                    Update
                  </button>
                  <button
                    class="btn small"
                    disabled={busyId === link.id}
                    onclick={() => startPin(link)}
                  >
                    Pin…
                  </button>
                  <button
                    class="btn small"
                    disabled={busyId === link.id}
                    onclick={() => startFollow(link)}
                  >
                    Follow branch
                  </button>
                  {#if link.local_only && link.hosted_remote}
                    <button
                      class="btn small"
                      disabled={busyId === link.id}
                      title="Switch remote to {link.hosted_remote}"
                      onclick={() => submitRehost(link)}
                    >
                      Use hosted remote
                    </button>
                  {/if}
                  <button
                    class="btn small ghost-danger"
                    disabled={busyId === link.id}
                    onclick={() => runLinkOp(link.id, () => api.removeLink(project.id, link.id))}
                  >
                    Remove
                  </button>
                </div>

                {#if pinFor === link.id}
                  <form
                    class="inline-form"
                    onsubmit={(e) => {
                      e.preventDefault();
                      submitPin(link);
                    }}
                  >
                    <input
                      bind:value={pinReason}
                      placeholder="Reason for pinning (e.g. hold outline for gate 4)"
                      aria-label="Reason for pinning"
                    />
                    <button class="btn small primary" disabled={busyId === link.id}>Pin</button>
                    <button type="button" class="btn small" onclick={() => (pinFor = null)}>Cancel</button>
                  </form>
                {/if}

                {#if followFor === link.id}
                  <form
                    class="inline-form"
                    onsubmit={(e) => {
                      e.preventDefault();
                      submitFollow(link);
                    }}
                  >
                    <input
                      bind:value={followBranch}
                      placeholder="main"
                      aria-label="Branch name"
                      required
                    />
                    <button class="btn small primary" disabled={busyId === link.id || !followBranch.trim()}>
                      Follow
                    </button>
                    <button type="button" class="btn small" onclick={() => (followFor = null)}>Cancel</button>
                  </form>
                {/if}
              {/if}
            </article>
          {/each}
        </div>
      {:else if !adding && !loading}
        <p class="muted empty-links">
          No linked projects yet. Link a Waffle Iron project's <span class="mono">board-shape.json</span> to track outline revisions.
        </p>
      {/if}
    </section>
  {/if}

  <section class="history-section">
    <History {store} />
  </section>
</div>

<style>
  .versions {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .built-section,
  .published-section,
  .links-section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .pub-state-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px 12px;
    flex-wrap: wrap;
  }

  .push-err {
    padding: 8px 10px;
    background: var(--bad-soft);
    border: 1px solid var(--bad);
    border-radius: 6px;
    font: 12px/1.4 ui-monospace, SFMono-Regular, Menlo, monospace;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .release-date {
    font-size: 12px;
  }

  .built-card {
    padding: 10px 14px;
  }

  .built-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .built-row {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 4px 8px;
    font-size: 13px;
    overflow-wrap: anywhere;
  }

  .built-row + .built-row {
    padding-top: 6px;
    border-top: 1px solid var(--line);
  }

  .built-id {
    font-weight: 600;
  }

  .built-flag {
    font-size: 11.5px;
    font-weight: 600;
    padding: 1px 8px;
    border-radius: 10px;
  }

  .built-flag.info {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .built-flag.warn {
    background: var(--warn-soft);
    border: 1px solid var(--stale);
    color: var(--warn);
  }

  .sec-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    flex-wrap: wrap;
  }

  .sec-actions {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .link-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .link-card {
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .link-card.notice {
    border-color: var(--accent);
  }

  .link-card.alert {
    border-color: var(--bad);
  }

  .link-top {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px 12px;
    flex-wrap: wrap;
  }

  .link-title {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .link-id {
    font-weight: 600;
    font-size: 15px;
  }

  .pill {
    font-size: 11.5px;
    padding: 1px 8px;
    border-radius: 10px;
    background: var(--panel-2);
    border: 1px solid var(--line);
    color: var(--muted);
  }

  .locked-chip {
    font-size: 11.5px;
    font-weight: 600;
    padding: 1px 8px;
    border-radius: 10px;
    background: var(--accent-soft);
    color: var(--accent);
  }

  .local-chip {
    font-size: 11.5px;
    font-weight: 600;
    padding: 1px 8px;
    border-radius: 10px;
    background: var(--warn-soft);
    border: 1px solid var(--stale);
    color: var(--warn);
  }

  .hint-line {
    margin: 0;
    padding: 6px 10px;
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: 6px;
    font-size: 13px;
    color: var(--text);
    overflow-wrap: anywhere;
  }

  .unpushed-note {
    margin: 0;
    padding: 6px 10px;
    background: var(--warn-soft);
    border: 1px solid var(--stale);
    border-radius: 6px;
    font-size: 13px;
    color: var(--warn);
    overflow-wrap: anywhere;
  }

  .ref-meta {
    font-size: 12.5px;
  }

  .state-line {
    margin: 0;
    font-size: 13.5px;
  }

  .state-line.quiet {
    color: var(--muted);
  }

  .state-line.info {
    color: var(--text);
  }

  .state-line.notice {
    font-weight: 600;
    color: var(--text);
  }

  .state-line.alert {
    font-weight: 600;
    color: var(--bad);
  }

  .pin-note {
    margin: 0;
    font-size: 12.5px;
  }

  .changes {
    margin: 0;
    padding: 6px 10px 6px 24px;
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: 6px;
    font-size: 13px;
  }

  .changes li + li {
    margin-top: 2px;
  }

  .locator-meta {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    font-size: 11.5px;
    overflow-wrap: anywhere;
  }

  .link-actions {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    padding-top: 2px;
  }

  .small {
    padding: 3px 9px;
    font-size: 12.5px;
  }

  .ghost-danger {
    color: var(--muted);
  }

  .ghost-danger:hover:not(:disabled) {
    border-color: var(--bad);
    color: var(--bad);
  }

  .inline-form {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    align-items: center;
    padding-top: 4px;
  }

  .inline-form input,
  .link-form input,
  .link-form select {
    font: inherit;
    color: inherit;
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 5px 8px;
  }

  .inline-form input {
    flex: 1 1 220px;
    min-width: 0;
  }

  .link-form {
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .form-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px 10px;
  }

  .form-grid label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .form-grid label.full {
    grid-column: 1 / -1;
  }

  .form-buttons {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .empty-links {
    margin: 0;
  }

  .err {
    color: var(--bad);
    margin: 0;
  }

  .err.detail {
    font-size: 12.5px;
    overflow-wrap: anywhere;
  }

  @media (max-width: 760px) {
    .form-grid {
      grid-template-columns: 1fr;
    }

    .form-grid label.full {
      grid-column: auto;
    }

    .link-card {
      padding: 10px 12px;
    }

    .inline-form input {
      flex-basis: 100%;
    }
  }
</style>
