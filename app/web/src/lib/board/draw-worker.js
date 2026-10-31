// Turns a .kicad_pcb into UltraRoute Studio's lines (ultraroute's view(): the board, one frame of
// everything on it, its poured zones), off the page's thread. In: {id, pcb, pro, dru}. Out:
// {id, lines} or {id, error}.
import init, { view, last_panic } from '../../draw/router/router.js';

const ready = init();

self.onmessage = async ({ data: { id, pcb, pro, dru } }) => {
  try {
    await ready;
    const lines = [];
    view(pcb, pro, dru, (line) => lines.push(line));
    self.postMessage({ id, lines });
  } catch (e) {
    self.postMessage({ id, error: String(e?.message ?? e) || last_panic() });
  }
};
