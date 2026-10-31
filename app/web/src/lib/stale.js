// Whether Boardroom's drawn tabs still show the board. They are drawn from pcbview's data and
// from KiCad's renders, both made before publishing; after Composer or Route writes the
// .kicad_pcb they show the board as it was (docs/demo-spec.md, "What goes stale").

import { fileUrl } from './api.js';

/**
 * A board's KiCad files: named after its `.kicad_pro` when it has one (servodrive's ethernet board
 * is `servodrive_E.kicad_pcb`), else `hw/<board>/<variant>/<board>.kicad_*`.
 */
export function kicadFiles(b) {
  const stem = b.kicad_pro ? b.kicad_pro.replace(/\.kicad_pro$/, '') : `hw/${b.board}/${b.variant}/${b.board}`;
  return { pcb: `${stem}.kicad_pcb`, pro: `${stem}.kicad_pro`, dru: `${stem}.kicad_dru` };
}

/** The project's board files. */
export const boardFiles = (project) => project.boards.map((b) => kicadFiles(b).pcb);

/** When a project file last changed, from the server's Last-Modified; null if it doesn't say. */
export async function modified(id, path) {
  const res = await fetch(fileUrl(id, path), { method: 'HEAD', cache: 'no-cache' });
  const lm = res.ok ? res.headers.get('last-modified') : null;
  return lm ? new Date(lm) : null;
}

/**
 * The newest change to one of the project's board files, `{path, app, time, message}`, or null.
 * In the browser alone: a change this tab made to the published board. On this machine: when the
 * file last changed on disk, by whichever app (`app` null).
 */
export async function boardChange(project, store) {
  const files = boardFiles(project);
  if (store.kind === 'browser') {
    const changed = (await store.changes()).filter((c) => files.includes(c.path));
    return changed.sort((a, b) => b.time.localeCompare(a.time))[0] ?? null;
  }
  const times = await Promise.all(files.map(async (path) => ({ path, time: await modified(project.id, path) })));
  const newest = times.filter((t) => t.time).sort((a, b) => b.time - a.time)[0];
  return newest ? { path: newest.path, app: null, time: newest.time.toISOString(), message: '' } : null;
}
