<script>
  import '@xterm/xterm/css/xterm.css';
  import { onMount } from 'svelte';
  import { Terminal } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';
  import * as api from './api.js';

  let { project, theme = 'auto', onchanged = () => {} } = $props();

  let mountEl = $state(null);
  let summary = $state(null);
  // connecting | live | reconnecting | exited | ended | error
  let connState = $state('connecting');
  let errorMsg = $state('');

  const darkQuery = matchMedia('(prefers-color-scheme: dark)');
  let systemDark = $state(darkQuery.matches);
  $effect(() => {
    const onChange = () => (systemDark = darkQuery.matches);
    darkQuery.addEventListener('change', onChange);
    return () => darkQuery.removeEventListener('change', onChange);
  });

  const isDark = $derived(theme === 'dark' || (theme === 'auto' && systemDark));

  function xtermPalette(dark) {
    return dark
      ? {
          background: '#0e1921',
          foreground: '#d8e2e8',
          cursor: '#83c5af',
          cursorAccent: '#0e1921',
          selectionBackground: '#83c5af40',
        }
      : {
          background: '#ffffff',
          foreground: '#15232c',
          cursor: '#2e8a6d',
          cursorAccent: '#ffffff',
          selectionBackground: '#2e8a6d33',
        };
  }

  let termRef = null;
  let fitRef = null;
  let wsRef = null;
  let reconnectTimer = null;
  let disposed = false;
  let intentionalEnd = false;
  let lastCols = 0;
  let lastRows = 0;

  $effect(() => {
    const pal = xtermPalette(isDark);
    if (termRef) termRef.options.theme = pal;
  });

  function sendResize(cols, rows) {
    if (!cols || !rows) return;
    lastCols = cols;
    lastRows = rows;
    if (wsRef?.readyState === WebSocket.OPEN) {
      wsRef.send(JSON.stringify({ type: 'resize', cols, rows }));
    }
  }

  function fitTerminal() {
    if (!fitRef || !termRef || !mountEl) return;
    if (mountEl.clientWidth < 10 || mountEl.clientHeight < 10) return;
    try {
      fitRef.fit();
      if (termRef.cols !== lastCols || termRef.rows !== lastRows) {
        sendResize(termRef.cols, termRef.rows);
      }
    } catch {}
  }

  function clearReconnect() {
    if (reconnectTimer != null) {
      clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
  }

  function closeSocket() {
    clearReconnect();
    if (wsRef) {
      const s = wsRef;
      wsRef = null;
      s.onopen = null;
      s.onmessage = null;
      s.onclose = null;
      s.onerror = null;
      try {
        s.close();
      } catch {}
    }
  }

  function connectSocket(termId, isReconnect, attempt = 0) {
    if (disposed) return;
    closeSocket();
    connState = isReconnect ? 'reconnecting' : 'connecting';

    const ws = new WebSocket(api.termWsUrl(termId));
    ws.binaryType = 'arraybuffer';
    wsRef = ws;

    ws.onopen = () => {
      if (disposed || wsRef !== ws) return;
      if (isReconnect && termRef) {
        // Server replays the scrollback buffer at the start of every attachment;
        // resetting here avoids duplicating lines that were already on screen.
        termRef.reset();
      }
      connState = 'live';
      errorMsg = '';
      lastCols = 0;
      lastRows = 0;
      fitTerminal();
      if (termRef && (lastCols === 0 || lastRows === 0)) {
        sendResize(termRef.cols, termRef.rows);
      }
    };

    ws.onmessage = (ev) => {
      if (disposed || wsRef !== ws || !termRef) return;
      if (ev.data instanceof ArrayBuffer) {
        termRef.write(new Uint8Array(ev.data));
      } else if (typeof ev.data === 'string') {
        termRef.write(ev.data);
      }
    };

    ws.onclose = async () => {
      if (disposed || wsRef !== ws) return;
      wsRef = null;
      if (intentionalEnd) {
        connState = 'ended';
        onchanged();
        return;
      }
      try {
        const list = await api.terms();
        if (disposed) return;
        const found = list.find((t) => t.id === termId);
        if (found) summary = found;
        onchanged();
        if (found?.alive) {
          connState = 'reconnecting';
          const delay = Math.min(2000, 350 * Math.pow(1.6, attempt));
          reconnectTimer = setTimeout(() => connectSocket(termId, true, attempt + 1), delay);
        } else if (found) {
          connState = 'exited';
        } else {
          connState = 'ended';
        }
      } catch {
        if (disposed) return;
        connState = 'reconnecting';
        const delay = Math.min(2500, 500 * Math.pow(1.6, attempt));
        reconnectTimer = setTimeout(() => connectSocket(termId, true, attempt + 1), delay);
      }
    };
  }

  async function startSession(projectId, resetDisplay = false) {
    if (disposed) return;
    intentionalEnd = false;
    closeSocket();
    errorMsg = '';
    connState = 'connecting';
    if (resetDisplay && termRef) {
      termRef.reset();
    }
    try {
      fitTerminal();
      const cols = termRef?.cols ?? 80;
      const rows = termRef?.rows ?? 24;
      const started = await api.startTerm(projectId, 'project', { cols, rows });
      if (disposed) return;
      summary = started;
      onchanged();
      connectSocket(started.id, false, 0);
      termRef?.focus();
    } catch (e) {
      if (disposed) return;
      connState = 'error';
      errorMsg = e.message || 'Could not start terminal';
    }
  }

  async function endSession() {
    if (!summary) return;
    intentionalEnd = true;
    const id = summary.id;
    closeSocket();
    try {
      await api.endTerm(id);
    } catch {}
    if (!disposed) {
      connState = 'ended';
      summary = summary ? { ...summary, alive: false, waffle: false } : null;
      onchanged();
    }
  }

  onMount(() => {
    const el = mountEl;
    const projectId = project.id;
    if (!el || !projectId) return;

    disposed = false;
    intentionalEnd = false;
    summary = null;

    const narrowScreen = window.innerWidth <= 760;
    const term = new Terminal({
      cursorBlink: true,
      fontSize: narrowScreen ? 12 : 13,
      fontFamily: "ui-monospace, 'SF Mono', Menlo, Consolas, monospace",
      scrollback: 5000,
      theme: xtermPalette(isDark),
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(el);

    termRef = term;
    fitRef = fit;

    const enc = new TextEncoder();
    const sendBin = (bytes) => {
      if (wsRef?.readyState === WebSocket.OPEN && bytes.byteLength > 0) {
        wsRef.send(bytes);
      }
    };
    const dataSub = term.onData((data) => sendBin(enc.encode(data)));
    const binSub = term.onBinary((bin) => {
      const out = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i) & 0xff;
      sendBin(out);
    });
    const titleSub = term.onTitleChange((title) => {
      if (title && summary) summary = { ...summary, title };
    });

    let raf = 0;
    const ro = new ResizeObserver(() => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => fitTerminal());
    });
    ro.observe(el);

    fitTerminal();
    term.focus();
    startSession(projectId, false);

    return () => {
      disposed = true;
      cancelAnimationFrame(raf);
      ro.disconnect();
      dataSub.dispose();
      binSub.dispose();
      titleSub.dispose();
      closeSocket();
      term.dispose();
      termRef = null;
      fitRef = null;
    };
  });

  function focusHost() {
    if (termRef && !termRef.hasSelection()) {
      termRef.focus();
    }
  }

  const statusLabel = $derived(
    connState === 'live'
      ? 'Live'
      : connState === 'reconnecting'
        ? 'Reconnecting…'
        : connState === 'connecting'
          ? 'Connecting…'
          : connState === 'exited'
            ? 'Exited'
            : connState === 'ended'
              ? 'Ended'
              : 'Error',
  );
</script>

<div class="term-wrap" data-term-state={connState}>
  <div class="term-bar">
    <span class="term-status" class:live={connState === 'live'} class:warn={connState === 'reconnecting'}>
      <span class="term-pip" aria-hidden="true"></span>
      <span>{statusLabel}</span>
    </span>
    <span class="term-path mono" title={project.path}>{summary?.title ? `${summary.title} · ` : ''}{project.path}</span>
    {#if summary?.waffle}
      <span class="term-chip" title="Waffle Iron MCP relay attached">Waffle MCP</span>
    {/if}
    <div class="term-actions">
      {#if connState === 'exited' || connState === 'ended' || connState === 'error'}
        <button type="button" class="btn primary" onclick={() => startSession(project.id, true)}>Restart</button>
      {:else if summary}
        <button type="button" class="btn" onclick={endSession}>End</button>
      {/if}
    </div>
  </div>

  {#if errorMsg}
    <p class="term-err">{errorMsg}</p>
  {/if}

  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="term-host" class:dark={isDark} onclick={focusHost}>
    <div class="term-mount" bind:this={mountEl}></div>
  </div>
</div>

<style>
  .term-wrap {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 380px;
    min-width: 0;
    width: 100%;
    overflow: hidden;
    background: var(--panel);
  }

  .term-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 14px;
    border-bottom: 1px solid var(--line);
    background: var(--panel);
    flex-wrap: wrap;
    min-width: 0;
    flex-shrink: 0;
  }

  .term-status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
    flex-shrink: 0;
  }

  .term-status.live {
    color: var(--accent);
  }

  .term-status.warn {
    color: var(--warn);
  }

  .term-pip {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: currentColor;
  }

  .term-path {
    color: var(--muted);
    font-size: 12px;
    min-width: 0;
    flex: 1 1 120px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .term-chip {
    font-size: 11px;
    font-weight: 600;
    padding: 1px 8px;
    border-radius: 10px;
    background: var(--accent-soft);
    color: var(--accent);
    flex-shrink: 0;
  }

  .term-actions {
    margin-left: auto;
    display: flex;
    gap: 6px;
    flex-shrink: 0;
  }

  .term-actions .btn {
    padding: 3px 10px;
    font-size: 12.5px;
  }

  .term-err {
    margin: 0;
    padding: 8px 14px;
    color: var(--bad);
    background: var(--bad-soft);
    border-bottom: 1px solid var(--line);
    font-size: 13px;
    flex-shrink: 0;
  }

  .term-host {
    flex: 1;
    min-height: 0;
    min-width: 0;
    width: 100%;
    padding: 6px 8px;
    overflow: hidden;
    background: #ffffff;
  }

  .term-host.dark {
    background: #0e1921;
  }

  .term-mount {
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .term-mount :global(.xterm) {
    height: 100%;
    max-width: 100%;
  }

  .term-mount :global(.xterm-viewport) {
    max-width: 100%;
  }

  @media (max-width: 760px) {
    .term-wrap {
      min-height: 0;
    }

    .term-bar {
      padding: 5px 10px;
      gap: 8px;
    }

    .term-host {
      padding: 4px 6px;
    }
  }
</style>
