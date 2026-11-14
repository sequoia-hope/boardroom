//! The plan as a picture, one per layer: the board as a rubber-band sketch before any copper.
//! Obstacles in grey, each gap that is at least half full a line from amber to red at full, each strand a
//! line through its crossings in its net's colour, what is left over as dashed red.

use std::fmt::Write;

use board::Board;

use crate::space::{What, P};
use crate::Plan;

/// A gap is drawn when its strands need at least this share of its room.
const SHOWN: f64 = 0.5;

const PALETTE: [&str; 10] = ["#e6194b", "#3cb44b", "#4363d8", "#f58231", "#911eb4", "#008080", "#9a6324", "#f032e6", "#808000", "#000075"];

fn path(pts: &[P], close: bool) -> String {
    let mut d = String::new();
    for (i, p) in pts.iter().enumerate() {
        let _ = write!(d, "{}{:.3} {:.3}", if i == 0 { "M" } else { "L" }, p[0], p[1]);
    }
    if close {
        d.push('Z');
    }
    d
}

/// One layer of the plan as SVG, millimetres, KiCad's frame.
pub fn layer(board: &Board, plan: &Plan, l: usize) -> String {
    let sheet = &plan.sheets[l];
    let sp = &sheet.space;
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for p in &sp.verts {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let m = 1.0;
    let (w, h) = (hi[0] - lo[0] + 2.0 * m, hi[1] - lo[1] + 2.0 * m);
    let mut s = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='{:.3} {:.3} {w:.3} {h:.3}' stroke-linecap='round' stroke-linejoin='round' fill='none'>\n<rect x='{:.3}' y='{:.3}' width='{w:.3}' height='{h:.3}' fill='#fff'/>\n",
        lo[0] - m, lo[1] - m, lo[0] - m, lo[1] - m
    );
    let thin = (w.max(h) / 1500.0).max(0.02);
    for o in &sp.obstacles {
        let (fill, stroke) = match o.what {
            What::Edge => ("none", "#000"),
            What::Pad => ("#c9c9c9", "#888"),
            What::Copper => ("#e3d6b8", "#a89868"),
            What::Hole => ("#fff", "#555"),
            What::Keepout => ("#f6dede", "#c88"),
        };
        if o.reach > 0.0 {
            // A round thing, held as its middle or middle line: drawn as far out as it reaches.
            let _ = writeln!(s, "<path d='{}' stroke='{fill}' stroke-width='{:.3}'/>", path(&o.poly, true), 2.0 * o.reach);
            continue;
        }
        let _ = writeln!(s, "<path d='{}' fill='{fill}' stroke='{stroke}' stroke-width='{:.3}'/>", path(&o.poly, true), thin * 2.0);
    }
    // Gaps: only edges with a free triangle on a side.
    for (e, edge) in sp.edges.iter().enumerate() {
        let sides: Vec<usize> = edge.t.iter().flatten().copied().collect();
        if sides.len() < 2 || !sides.iter().all(|&t| sp.free(t)) {
            continue;
        }
        // Only a gap that is filling up is drawn: on an open board most triangle sides are
        // long lines to the board's corners with a strand or two across them, and drawn they
        // bury the strands.
        let f = sheet.fill(board, e);
        if f < SHOWN {
            continue;
        }
        // Amber at [`SHOWN`], to red at full.
        let t = ((f - SHOWN) / (1.0 - SHOWN)).clamp(0.0, 1.0);
        let (colour, width) = (format!("rgb(230,{:.0},40)", 170.0 * (1.0 - t)), thin * 2.5);
        let (a, b) = (sp.verts[edge.v[0]], sp.verts[edge.v[1]]);
        let _ = writeln!(s, "<path d='M{:.3} {:.3}L{:.3} {:.3}' stroke='{colour}' stroke-width='{width:.3}'/>", a[0], a[1], b[0], b[1]);
    }
    for k in sheet.laid() {
        let st = &sheet.strands[k];
        let _ = writeln!(
            s,
            "<path d='{}' stroke='{}' stroke-width='{:.3}' opacity='0.85'><title>{}</title></path>",
            path(&sheet.sketch(board, k), false),
            PALETTE[st.net as usize % PALETTE.len()],
            (st.width * 0.6).max(thin * 3.0),
            board.net_name(st.net)
        );
    }
    for left in &plan.left {
        let (Some(a), Some(b)) = (sp.ends.get(&crate::End::Item(left.from)), sp.ends.get(&crate::End::Item(left.to))) else { continue };
        let (a, b) = (sp.obstacles[*a].centre, sp.obstacles[*b].centre);
        let _ = writeln!(s, "<path d='M{:.3} {:.3}L{:.3} {:.3}' stroke='#d00' stroke-width='{:.3}' stroke-dasharray='{:.3} {:.3}'><title>{}: {}</title></path>", a[0], a[1], b[0], b[1], thin * 4.0, thin * 12.0, thin * 8.0, board.net_name(left.net), left.why);
    }
    s.push_str("</svg>\n");
    s
}
