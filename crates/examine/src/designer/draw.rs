//! Pictures of what was read, drawn over the board's copper: SVG, millimetres, KiCad's frame.

use std::fmt::Write;

use board::pcb::geom::{self, P};
use board::pcb::Board;

use super::frames::{AGREES, ON_GRID};
use super::Extraction;

/// Colours for things told apart by number.
const PALETTE: [&str; 10] = [
    "#e6194b", "#3cb44b", "#4363d8", "#f58231", "#911eb4", "#008080", "#9a6324", "#f032e6",
    "#808000", "#000075",
];

fn path(poly: &[P]) -> String {
    let mut d = String::new();
    for (i, p) in poly.iter().enumerate() {
        let _ = write!(
            d,
            "{}{:.3} {:.3}",
            if i == 0 { "M" } else { "L" },
            p[0],
            p[1]
        );
    }
    d.push('Z');
    d
}

fn open(board: &Board) -> (String, f64) {
    let (lo, hi) = super::extent(board);
    let m = 1.5;
    let (w, h) = (hi[0] - lo[0] + 2.0 * m, hi[1] - lo[1] + 2.0 * m);
    let mut s = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='{:.3} {:.3} {w:.3} {h:.3}' stroke-linecap='round' stroke-linejoin='round' fill='none'>\n<rect x='{:.3}' y='{:.3}' width='{w:.3}' height='{h:.3}' fill='#fff'/>\n",
        lo[0] - m,
        lo[1] - m,
        lo[0] - m,
        lo[1] - m
    );
    if let Some(o) = &board.outline {
        let _ = writeln!(
            s,
            "<path d='{}' stroke='#000' stroke-width='0.12'/>",
            path(o)
        );
    }
    (s, w.max(h))
}

fn pads(s: &mut String, board: &Board, layer: Option<&str>, colour: &str) {
    let mut d = String::new();
    for p in board
        .pads
        .iter()
        .filter(|p| layer.is_none_or(|l| p.layers.iter().any(|x| x == l)))
    {
        for poly in &p.shape {
            d.push_str(&path(poly));
        }
    }
    let _ = writeln!(s, "<path d='{d}' fill='{colour}'/>");
}

fn tracks(s: &mut String, board: &Board, layer: Option<&str>, colour: &str, opacity: f64) {
    let _ = writeln!(s, "<g stroke='{colour}' stroke-opacity='{opacity}'>");
    for t in board
        .tracks
        .iter()
        .filter(|t| layer.is_none_or(|l| t.layer == l))
    {
        let _ = writeln!(
            s,
            "<path d='M{:.3} {:.3}L{:.3} {:.3}' stroke-width='{:.3}'/>",
            t.a[0], t.a[1], t.b[0], t.b[1], t.width
        );
    }
    s.push_str("</g>\n");
}

/// One layer: its pours pale, its tracks dark, its pads grey.
pub fn layer(board: &Board, layer: &str) -> String {
    let (mut s, _) = open(board);
    let mut d = String::new();
    for f in board.fills.iter().filter(|f| f.layer == layer) {
        d.push_str(&path(&f.polygon));
    }
    let _ = writeln!(s, "<path d='{d}' fill='#f0b46e' fill-rule='evenodd'/>");
    pads(&mut s, board, Some(layer), "#9a9a9a");
    tracks(&mut s, board, Some(layer), "#1a2a6c", 1.0);
    s.push_str("</svg>\n");
    s
}

/// Every neighbourhood's frame as a cross: grey on the board's axes, red when turned, a hollow
/// ring where the tracks agree on no frame. The diagonals of the frame are the thin arms.
pub fn frames(board: &Board, x: &Extraction) -> String {
    let (mut s, _) = open(board);
    pads(&mut s, board, None, "#dddddd");
    tracks(&mut s, board, None, "#6b7a99", 0.55);
    let r = x.frames.cell_mm * 0.42;
    for c in &x.frames.cells {
        if c.track_mm < x.frames.cell_mm {
            continue;
        }
        if c.agreement < AGREES {
            let _ = writeln!(s, "<circle cx='{:.3}' cy='{:.3}' r='{:.3}' stroke='#b08900' stroke-width='0.12' stroke-dasharray='0.4 0.3'/>", c.centre[0], c.centre[1], r * 0.5);
            continue;
        }
        let colour = if c.frame_deg.abs() > ON_GRID {
            "#d81e1e"
        } else {
            "#444444"
        };
        for (k, width) in [(0.0, 0.2), (90.0, 0.2), (45.0, 0.07), (135.0, 0.07)] {
            let a = (c.frame_deg + k).to_radians();
            let v = [a.cos() * r, a.sin() * r];
            let (p, q) = (geom::sub(c.centre, v), geom::add(c.centre, v));
            let _ = writeln!(
                s,
                "<path d='M{:.3} {:.3}L{:.3} {:.3}' stroke='{colour}' stroke-width='{width}'/>",
                p[0], p[1], q[0], q[1]
            );
        }
        if c.frame_deg.abs() > ON_GRID {
            let _ = writeln!(
                s,
                "<text x='{:.3}' y='{:.3}' font-size='{:.2}' fill='{colour}' font-family='sans-serif'>{:.0}</text>",
                c.centre[0] + r * 0.2,
                c.centre[1] - r * 0.25,
                x.frames.cell_mm * 0.28,
                c.frame_deg
            );
        }
    }
    s.push_str("</svg>\n");
    s
}

/// One layer's bundles, each its own colour with its number of nets; other track grey.
pub fn bundles(board: &Board, x: &Extraction, layer: &str) -> String {
    let (mut s, size) = open(board);
    pads(&mut s, board, Some(layer), "#dddddd");
    tracks(&mut s, board, Some(layer), "#9aa0aa", 0.8);
    let font = (size / 60.0).clamp(0.6, 2.5);
    for (k, b) in x
        .bundles
        .bundles
        .iter()
        .enumerate()
        .filter(|(_, b)| b.layer == layer)
    {
        let colour = PALETTE[k % PALETTE.len()];
        let _ = writeln!(s, "<g stroke='{colour}'>");
        for &i in &b.tracks {
            let t = &board.tracks[i];
            let _ = writeln!(
                s,
                "<path d='M{:.3} {:.3}L{:.3} {:.3}' stroke-width='{:.3}'/>",
                t.a[0], t.a[1], t.b[0], t.b[1], t.width
            );
        }
        s.push_str("</g>\n");
        // Twice, a white edge under black letters: not every renderer honours paint-order.
        for (fill, edge) in [("#fff", font / 4.0), ("#000", 0.0)] {
            let _ = writeln!(
                s,
                "<text x='{:.3}' y='{:.3}' font-size='{font:.2}' font-family='sans-serif' font-weight='bold' fill='{fill}' stroke='#fff' stroke-width='{edge:.2}' text-anchor='middle'>{}</text>",
                b.centre[0],
                b.centre[1] + font * 0.35,
                b.nets.len()
            );
        }
    }
    s.push_str("</svg>\n");
    s
}

/// Parts routed alike, boxed in one colour per repeat.
pub fn repeats(board: &Board, x: &Extraction) -> String {
    let (mut s, size) = open(board);
    pads(&mut s, board, None, "#cfcfcf");
    tracks(&mut s, board, None, "#6b7a99", 0.6);
    let width = (size / 300.0).clamp(0.12, 0.6);
    for (k, r) in x.repeats.repeats.iter().enumerate() {
        let colour = PALETTE[k % PALETTE.len()];
        for m in &r.members {
            let part = &board.parts[m];
            let pts = part
                .pads
                .iter()
                .flat_map(|&i| board.pads[i].shape.iter().flatten().copied());
            if let Some((lo, hi)) = geom::bbox(pts) {
                let g = 0.3;
                let _ = writeln!(
                    s,
                    "<rect x='{:.3}' y='{:.3}' width='{:.3}' height='{:.3}' stroke='{colour}' stroke-width='{width:.2}' fill='{colour}' fill-opacity='0.15'/>",
                    lo[0] - g,
                    lo[1] - g,
                    hi[0] - lo[0] + 2.0 * g,
                    hi[1] - lo[1] + 2.0 * g
                );
            }
        }
    }
    s.push_str("</svg>\n");
    s
}
