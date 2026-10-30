// Talks to the Boardroom server, or, in the static build (GitHub Pages), reads
// a snapshot written by `boardroom snapshot`. The static build can look but
// not act: nothing on a web page can open your folders or start KiCad.

export const STATIC = !!import.meta.env.VITE_STATIC;

async function json(url, init) {
  const res = await fetch(url, init);
  if (!res.ok) throw new Error((await res.text()) || `${res.status} ${res.statusText}`);
  return res.status === 204 ? null : res.json();
}

/** Circuit Composer's page: through Boardroom's proxy, or its own Pages site. */
export const COMPOSER = STATIC ? 'https://sequoia-hope.github.io/circuit-composer/' : '/apps/composer/';

export function examples() {
  return STATIC ? Promise.resolve([]) : json('/api/examples');
}

export function projects() {
  return json(STATIC ? './snapshot/projects.json' : '/api/projects');
}

/** URL of a file inside a project. */
export function fileUrl(id, path) {
  const rel = path.split('/').map(encodeURIComponent).join('/');
  return STATIC ? `./snapshot/${id}/${rel}` : `/api/projects/${id}/files/${rel}`;
}

export async function docs(project) {
  if (STATIC) return project.docs ?? [];
  return json(`/api/projects/${project.id}/docs`);
}

export async function text(id, path) {
  const res = await fetch(fileUrl(id, path));
  if (!res.ok) throw new Error(`${path}: ${res.status}`);
  return res.text();
}

/** A directory listing: [{name, dir}]. */
export async function listing(project, path) {
  if (STATIC) return project.listings?.[path] ?? [];
  return json(fileUrl(project.id, path));
}

export function open(id, what) {
  return json(`/api/projects/${id}/open/${what}`, { method: 'POST' });
}

export function addProject(path) {
  return json('/api/projects', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ path }),
  });
}

export function removeProject(id) {
  return json(`/api/projects/${id}`, { method: 'DELETE' });
}

export function pickFolder() {
  return json('/api/pick-folder', { method: 'POST' });
}

/** Calls onMessage for every bus message and returns {send, close}; reconnects if the server restarts. */
export function bus(onMessage) {
  if (STATIC) return { send() {}, close() {} };
  let socket;
  let closed = false;
  const connect = () => {
    socket = new WebSocket(`${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/api/bus`);
    socket.onmessage = (e) => {
      try {
        onMessage(JSON.parse(e.data));
      } catch {}
    };
    socket.onclose = () => {
      if (!closed) setTimeout(connect, 2000);
    };
  };
  connect();
  return {
    send(msg) {
      if (socket?.readyState === WebSocket.OPEN) socket.send(JSON.stringify(msg));
    },
    close() {
      closed = true;
      socket?.close();
    },
  };
}
