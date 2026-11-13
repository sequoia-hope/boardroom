//! The drawing as a picture, one per layer: obstacles in grey, each wire at its width in its
//! net's colour, the plan's vias as rings, a strand that was not drawn as a dashed red sketch
//! (with the path the checker refused, if one was found, as a thin magenta line),
//! and a red ring round each place the checker faults.

use std::fmt::Write;

use board::Board;
use plan::space::{What, P};
use plan::Plan;

use crate::Drawing;

const PALETTE: [&str; 10] = ["#e6194b", "#3cb44b", "#4363d8", "#f58231", "#911eb4", "#008080", "#9a6324", "#f032e6", "#808000", "#000075"];

fn path(pts: &[P], close: bool) -> String {
    let mut d = String::new();
    for (i, p) in pts.iter().enumerate() {
        let _ = write!(d, "{}{:.4} {:.4}", if i == 0 { "M" } else { "L" }, p[0], p[1]);
    }
    if close {
        d.push('Z');
    }
    d
}

/// One layer as SVG, millimetres, KiCad's frame. `faults` are places to ring.
pub fn layer(board: &Board, plan: &Plan, drawing: &Drawing, l: usize, faults: &[P]) -> String {
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
        let _ = writeln!(s, "<path d='{}' fill='{fill}' stroke='{stroke}' stroke-width='{:.3}'/>", path(&o.poly, true), thin * 2.0);
    }
    for wire in drawing.wires.iter().filter(|w| w.layer == l) {
        let _ = writeln!(s, "<path d='{}' stroke='{}' stroke-width='{:.4}'><title>{}</title></path>", path(&wire.pts, false), PALETTE[wire.net as usize % PALETTE.len()], wire.width, board.net_name(wire.net));
    }
    let (r, drill) = plan::via_size(board);
    for v in &plan.vias {
        let _ = writeln!(s, "<circle cx='{:.4}' cy='{:.4}' r='{:.4}' fill='{}' stroke='none'/><circle cx='{:.4}' cy='{:.4}' r='{:.4}' fill='#fff'/>", v.at[0], v.at[1], r as f64 / 1e6, PALETTE[v.net as usize % PALETTE.len()], v.at[0], v.at[1], drill as f64 / 1e6);
    }
    for u in drawing.undrawn.iter().filter(|u| u.layer == l) {
        let _ = writeln!(s, "<path d='{}' stroke='#d00' stroke-width='{:.3}' stroke-dasharray='{:.3} {:.3}'><title>{}: {}</title></path>", path(&sheet.sketch(board, u.strand), false), thin * 4.0, thin * 12.0, thin * 8.0, board.net_name(u.net), u.why);
    }
    for u in drawing.undrawn.iter().filter(|u| u.layer == l && !u.refused.is_empty()) {
        let _ = writeln!(s, "<path d='{}' stroke='#f0f' stroke-width='{:.3}'><title>{}: {}</title></path>", path(&u.refused, false), thin * 2.0, board.net_name(u.net), u.why);
    }
    for left in &plan.left {
        let (Some(a), Some(b)) = (sp.ends.get(&plan::End::Item(left.from)), sp.ends.get(&plan::End::Item(left.to))) else { continue };
        let (a, b) = (sp.obstacles[*a].centre, sp.obstacles[*b].centre);
        let _ = writeln!(s, "<path d='M{:.3} {:.3}L{:.3} {:.3}' stroke='#d0a000' stroke-width='{:.3}' stroke-dasharray='{:.3} {:.3}'><title>{}: {}</title></path>", a[0], a[1], b[0], b[1], thin * 3.0, thin * 6.0, thin * 6.0, board.net_name(left.net), left.why);
    }
    for f in faults {
        let _ = writeln!(s, "<circle cx='{:.4}' cy='{:.4}' r='{:.3}' stroke='#f00' stroke-width='{:.3}'/>", f[0], f[1], thin * 14.0, thin * 3.0);
    }
    s.push_str("</svg>\n");
    s
}

/// One strand's course as SVG: obstacles, where it may bend (thin blue), each corner it keeps
/// from as a circle of that distance (green where the corner is on the wire's left, purple on
/// its right), the sides it keeps from (orange), and the wire found (black).
pub fn course(sheet: &plan::Sheet, course: &crate::chain::Course, found: Option<&crate::chain::Path>) -> String {
    let all: Vec<P> = course.within.iter().flatten().copied().chain([course.a, course.b]).collect();
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for p in &all {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let m = 1.0;
    let (w, h) = (hi[0] - lo[0] + 2.0 * m, hi[1] - lo[1] + 2.0 * m);
    let thin = (w.max(h) / 1500.0).max(0.005);
    let mut s = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='{:.3} {:.3} {w:.3} {h:.3}' stroke-linecap='round' stroke-linejoin='round' fill='none'>\n<rect x='{:.3}' y='{:.3}' width='{w:.3}' height='{h:.3}' fill='#fff'/>\n",
        lo[0] - m, lo[1] - m, lo[0] - m, lo[1] - m
    );
    for o in &sheet.space.obstacles {
        if o.what == What::Edge {
            continue;
        }
        let _ = writeln!(s, "<path d='{}' fill='#ddd' stroke='#999' stroke-width='{:.4}'/>", path(&o.poly, true), thin);
    }
    for t in &course.within {
        let _ = writeln!(s, "<path d='{}' stroke='#8ad' stroke-width='{:.4}'/>", path(t, true), thin);
    }
    for g in &course.gaps {
        let _ = writeln!(s, "<path d='{}' stroke='#fa0' stroke-width='{:.4}'/>", path(&g.ends, false), thin);
    }
    for c in course.corners.iter().chain(course.stages.iter().flat_map(|s| &s.corners)) {
        let _ = writeln!(s, "<circle cx='{:.4}' cy='{:.4}' r='{:.4}' stroke='{}' stroke-width='{:.4}'/>", c.at[0], c.at[1], c.r, if c.left { "#2a2" } else { "#a2a" }, thin * 1.5);
    }
    for wall in course.walls.iter().chain(course.stages.iter().flat_map(|s| &s.walls)) {
        let _ = writeln!(s, "<path d='{}' stroke='#f80' stroke-width='{:.4}'/>", path(&[wall.a, wall.b], false), thin * 2.0);
    }
    if let Some(found) = found {
        let _ = writeln!(s, "<path d='{}' stroke='#000' stroke-width='{:.4}'/>", path(&found.pts, false), thin * 3.0);
    }
    for p in [course.a, course.b] {
        let _ = writeln!(s, "<circle cx='{:.4}' cy='{:.4}' r='{:.4}' fill='#000'/>", p[0], p[1], thin * 4.0);
    }
    s.push_str("</svg>\n");
    s
}
