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

/// One layer as SVG, millimetres, KiCad's frame. `faults` are places to ring; `pours` is the
/// layer's poured copper as rectangles in nanometres, each with its net, drawn pale under all.
pub fn layer(board: &Board, plan: &Plan, drawing: &Drawing, l: usize, faults: &[P], pours: &[(u32, [board::Nm; 4])]) -> String {
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
    let mut pour = String::new();
    for (net, r) in pours {
        let (x, y) = (r[0] as f64 / 1e6, r[1] as f64 / 1e6);
        let _ = write!(pour, "<rect x='{x:.3}' y='{y:.3}' width='{:.3}' height='{:.3}' fill='{}'/>", r[2] as f64 / 1e6 - x, r[3] as f64 / 1e6 - y, PALETTE[*net as usize % PALETTE.len()]);
    }
    if !pour.is_empty() {
        let _ = writeln!(s, "<g opacity='0.16' shape-rendering='crispEdges'>{pour}</g>");
    }
    for o in &sp.obstacles {
        let (fill, stroke) = match o.what {
            What::Edge => ("none", "#000"),
            What::Pad => ("#c9c9c9", "#888"),
            What::Copper => ("#e3d6b8", "#a89868"),
            What::Hole => ("#fff", "#555"),
            What::Keepout => ("#f6dede", "#c88"),
            What::Opening => ("#fdf3d0", "#ca6"),
        };
        if o.reach > 0.0 {
            // A round thing, held as its middle or middle line: drawn as far out as it reaches.
            if o.what != What::Pad || o.item.is_some() {
                let _ = writeln!(s, "<path d='{}' stroke='{fill}' stroke-width='{:.3}'/>", path(&o.poly, true), 2.0 * o.reach);
            }
            continue;
        }
        let _ = writeln!(s, "<path d='{}' fill='{fill}' stroke='{stroke}' stroke-width='{:.3}'/>", path(&o.poly, true), thin * 2.0);
    }
    for wire in drawing.wires.iter().filter(|w| w.layer == l) {
        let _ = writeln!(s, "<path d='{}' stroke='{}' stroke-width='{:.4}'><title>{}</title></path>", path(&wire.pts, false), PALETTE[wire.net as usize % PALETTE.len()], wire.width, board.net_name(wire.net));
    }
    for tie in drawing.ties.iter().filter(|t| t.layer == l) {
        let _ = writeln!(s, "<path d='{}' stroke='{}' stroke-width='{:.4}'><title>{}, a spoke drawn by hand</title></path>", path(&tie.pts, false), PALETTE[tie.net as usize % PALETTE.len()], tie.width, board.net_name(tie.net));
    }
    for (at, net) in plan.vias.iter().map(|v| (v.at, v.net)).chain(drawing.stitches.iter().map(|v| (v.at, v.net))) {
        let (r, drill) = plan::via_size(board, net);
        let _ = writeln!(s, "<circle cx='{:.4}' cy='{:.4}' r='{:.4}' fill='{}' stroke='none'/><circle cx='{:.4}' cy='{:.4}' r='{:.4}' fill='#fff'/>", at[0], at[1], r as f64 / 1e6, PALETTE[net as usize % PALETTE.len()], at[0], at[1], drill as f64 / 1e6);
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

/// One strand's course as SVG: obstacles (the room kept for a pad's spoke pale orange with a
/// broken outline, the rest grey), where it may bend (thin blue), each corner it keeps
/// from as a circle of that distance with a cross at the corner (green where the corner is on
/// the wire's left, purple on its right), the sides it keeps from (orange), and the wire found
/// (black). Also the wires already drawn on the layer (grey-blue, at their widths), the strand
/// as it lies in the plan
/// (`line`, dashed red), the part of each gap the wire may cross in (thick orange), and the
/// room in front of each corner that it keeps out of (the half of a pink circle its tick is
/// on). Under the picture, what each mark stands for.
pub fn course(board: &Board, sheet: &plan::Sheet, course: &crate::chain::Course, found: Option<&crate::chain::Path>, drawn: &[&crate::Wire], line: &[P]) -> String {
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
    let keys = keys(found.is_some());
    // The key's letters: a line under the picture for each kind of mark, a sentence long.
    let letter = w / 70.0;
    let below = (keys.len() as f64 * KEY_ROW + 1.0) * letter;
    let mut s = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='{:.3} {:.3} {w:.3} {:.3}' stroke-linecap='round' stroke-linejoin='round' fill='none'>\n<rect x='{:.3}' y='{:.3}' width='{w:.3}' height='{:.3}' fill='#fff'/>\n",
        lo[0] - m, lo[1] - m, h + below, lo[0] - m, lo[1] - m, h + below
    );
    for o in &sheet.space.obstacles {
        if o.what == What::Edge {
            continue;
        }
        if o.reach > 0.0 {
            let _ = writeln!(s, "<path d='{}' stroke='#ddd' stroke-width='{:.4}'/>", path(&o.poly, true), 2.0 * o.reach);
            continue;
        }
        if kept_room(board, o) {
            continue;
        }
        let _ = writeln!(s, "<path d='{}' fill='#ddd' stroke='#999' stroke-width='{:.4}'/>", path(&o.poly, true), thin);
    }
    // Room kept for a pad's bridge to its pour is no copper, and lies half on its pad: over
    // the pads, pale and with a broken outline, so that it is not taken for a second pad.
    for o in sheet.space.obstacles.iter().filter(|o| kept_room(board, o)) {
        let _ = writeln!(s, "<path d='{}' fill='#fb6' fill-opacity='0.35' stroke='#c60' stroke-width='{:.4}' stroke-dasharray='{:.4} {:.4}'/>", path(&o.poly, true), thin * 1.5, thin * 5.0, thin * 4.0);
    }
    for t in &course.within {
        let _ = writeln!(s, "<path d='{}' stroke='#8ad' stroke-width='{:.4}'/>", path(t, true), thin);
    }
    for w in drawn {
        let _ = writeln!(s, "<path d='{}' stroke='#9ab' stroke-opacity='0.7' stroke-width='{:.4}'><title>strand {} net {}</title></path>", path(&w.pts, false), w.width, w.strand, w.net);
    }
    for g in &course.gaps {
        let at = |u: f64| [g.ends[0][0] + (g.ends[1][0] - g.ends[0][0]) * u, g.ends[0][1] + (g.ends[1][1] - g.ends[0][1]) * u];
        let _ = writeln!(s, "<path d='{}' stroke='#fa0' stroke-width='{:.4}'/>", path(&g.ends, false), thin);
        let _ = writeln!(s, "<path d='{}' stroke='#fa0' stroke-width='{:.4}'/>", path(&[at(g.lo), at(g.hi)], false), thin * 4.0);
    }
    for f in &course.fronts {
        let _ = writeln!(s, "<circle cx='{:.4}' cy='{:.4}' r='{:.4}' stroke='#f6a' stroke-width='{:.4}'/>", f.at[0], f.at[1], f.r, thin);
        let _ = writeln!(s, "<path d='{}' stroke='#f6a' stroke-width='{:.4}'/>", path(&[f.at, [f.at[0] + f.along[0] * f.r, f.at[1] + f.along[1] * f.r]], false), thin);
    }
    if line.len() >= 2 {
        let _ = writeln!(s, "<path d='{}' stroke='#d00' stroke-width='{:.4}' stroke-dasharray='{:.4} {:.4}'/>", path(line, false), thin * 2.0, thin * 6.0, thin * 4.0);
    }
    for c in course.corners.iter().chain(course.stages.iter().flat_map(|s| &s.corners)) {
        let colour = if c.left { "#2a2" } else { "#a2a" };
        let _ = writeln!(s, "<circle cx='{:.4}' cy='{:.4}' r='{:.4}' stroke='{colour}' stroke-width='{:.4}'/>", c.at[0], c.at[1], c.r, thin * 1.5);
        // The corner itself, a cross: the circle is a distance from it and no copper.
        let _ = writeln!(s, "{}", cross(c.at, thin * 8.0, colour, thin * 1.5));
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
    // The key, on white under a line across the picture's foot.
    let (left, top) = (lo[0] - m, lo[1] - m + h);
    let _ = writeln!(s, "<rect x='{left:.4}' y='{top:.4}' width='{w:.4}' height='{below:.4}' fill='#fff'/>");
    let _ = writeln!(s, "<path d='M{left:.4} {top:.4}h{w:.4}' stroke='#999' stroke-width='{thin:.4}'/>");
    for (k, (mark, colour, name, says)) in keys.iter().enumerate() {
        let x = left + letter;
        let y = top + (k as f64 * KEY_ROW + 1.4) * letter;
        let (mid, end) = (x + 1.25 * letter, x + 2.5 * letter);
        let line = |width: f64, more: &str| format!("<path d='M{x:.4} {y:.4}L{end:.4} {y:.4}' stroke='{colour}' stroke-width='{width:.4}'{more}/>");
        let ring = format!("<circle cx='{mid:.4}' cy='{y:.4}' r='{:.4}' stroke='{colour}' stroke-width='{:.4}'/>", 0.6 * letter, thin * 1.5);
        let centred = format!("{ring}{}", cross([mid, y], 0.2 * letter, colour, thin * 1.5));
        let drawn = match mark {
            Mark::Copper => format!("<rect x='{x:.4}' y='{:.4}' width='{:.4}' height='{:.4}' fill='{colour}' stroke='#999' stroke-width='{thin:.4}'/>", y - 0.5 * letter, 2.5 * letter, letter),
            Mark::Kept => format!("<rect x='{x:.4}' y='{:.4}' width='{:.4}' height='{:.4}' fill='{colour}' fill-opacity='0.35' stroke='#c60' stroke-width='{:.4}' stroke-dasharray='{:.4} {:.4}'/>", y - 0.5 * letter, 2.5 * letter, letter, thin * 1.5, thin * 5.0, thin * 4.0),
            Mark::Wire => line(0.6 * letter, " stroke-opacity='0.7'"),
            Mark::Planned => line(thin * 2.0, &format!(" stroke-dasharray='{:.4} {:.4}'", thin * 6.0, thin * 4.0)),
            Mark::Line(by) => line(thin * by, ""),
            Mark::End => format!("<circle cx='{mid:.4}' cy='{y:.4}' r='{:.4}' fill='{colour}'/>", thin * 4.0),
            Mark::Ring => centred,
            Mark::Gap => format!("{}<path d='M{:.4} {y:.4}L{:.4} {y:.4}' stroke='{colour}' stroke-width='{:.4}'/>", line(thin, ""), x + 0.8 * letter, x + 1.7 * letter, thin * 4.0),
            Mark::Front => format!("{ring}<path d='M{mid:.4} {y:.4}L{:.4} {y:.4}' stroke='{colour}' stroke-width='{thin:.4}'/>", mid + 0.6 * letter),
        };
        let _ = writeln!(s, "{drawn}\n<text x='{:.4}' y='{:.4}' font-family='sans-serif' font-size='{letter:.4}' fill='#222' stroke='none'><tspan font-weight='bold'>{name}:</tspan> {says}</text>", x + 3.2 * letter, y + 0.35 * letter);
    }
    s.push_str("</svg>\n");
    s
}

/// Whether an obstacle is the room kept for a pad's spoke to its pour, and no copper.
fn kept_room(board: &Board, o: &plan::space::Obstacle) -> bool {
    o.item.and_then(|id| board.item(id)).is_some_and(|i| matches!(i.kind, board::Kind::Spoke { .. }))
}

/// A cross at a point, its arms this long.
fn cross(at: P, arm: f64, colour: &str, width: f64) -> String {
    format!("<path d='M{:.4} {:.4}h{:.4}M{:.4} {:.4}v{:.4}' stroke='{colour}' stroke-width='{width:.4}'/>", at[0] - arm, at[1], 2.0 * arm, at[0], at[1] - arm, 2.0 * arm)
}

/// A key's line under [`course`]'s picture, in letter heights.
const KEY_ROW: f64 = 1.8;

/// How a key shows the mark it tells of.
enum Mark {
    Copper,
    Kept,
    Wire,
    Planned,
    /// A plain line, this many times the thinnest.
    Line(f64),
    End,
    Ring,
    Gap,
    Front,
}

/// What each mark in [`course`]'s picture stands for: the mark, its colour, what it looks
/// like and what it means, for a reader who has not seen the code. The connection's own marks
/// first, then what is round it, then what holds the track.
fn keys(found: bool) -> [(Mark, &'static str, &'static str, &'static str); 12] {
    [
        (Mark::Planned, "#d00", "Dashed red line", "the route planned for this connection: which way it goes, not yet a track."),
        (Mark::Line(3.0), "#000", "Black line", if found { "the track drawn along that route." } else { "the track drawn along that route. None here: no track could be fitted." }),
        (Mark::End, "#000", "Black dots", "the two points the connection joins."),
        (Mark::Copper, "#ddd", "Grey shapes", "copper the track must keep clear of: pads and vias."),
        (Mark::Kept, "#fb6", "Pale orange box, broken outline", "not copper. Room kept free for a pad's bridge to its pour; the track keeps clear of it too."),
        (Mark::Wire, "#9ab", "Thick grey-blue lines", "tracks already drawn for other connections."),
        (Mark::Ring, "#2a2", "Green circle", "not copper. The track's centre line must stay outside it. Its centre (+) is a corner passed on the track's left."),
        (Mark::Ring, "#a2a", "Purple circle", "the same, round a corner passed on the track's right."),
        (Mark::Line(2.0), "#f80", "Orange line", "a side of something the track runs past and must keep its clearance from."),
        (Mark::Gap, "#fa0", "Yellow line", "a gap the route goes through. The track's centre must cross it in the thick part."),
        (Mark::Front, "#f6a", "Pink circle with a tick", "room kept at a corner for other tracks that pass between it and this one."),
        (Mark::Line(1.0), "#8ad", "Thin blue outline", "the track may bend only inside this area."),
    ]
}
