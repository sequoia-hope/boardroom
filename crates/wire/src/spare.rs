//! Copper a net has no need of. Every connection is planned from pad to pad and every wire is
//! drawn alone, so a wire knows nothing of the copper its net has already: it runs past a via
//! of its net, or beside another wire of it, and joins that copper a second time. The two
//! ways between the same two places shut in a patch of board, a ring, and one of them is
//! spare.
//!
//! [`spare`] takes the drawing as places where a net's copper is joined and the stretches of
//! track between them, and while some stretch is one of two ways round takes out the worst
//! such: the narrowest, and of those the longest. What is left joins everything that was
//! joined. Copper is taken as joined only where the board's own connectivity
//! ([`board::joined`]) is sure to join it: the end of a piece of track lying inside other
//! copper, the middle of a via or of a round pad lying within a track's width of its middle
//! line. A pour joins nothing here, so a track is never spare for lying in its net's pour.

use std::collections::HashMap;

use board::{Board, ItemId, Kind, Point, Shape};
use plan::space::P;
use plan::Plan;

use crate::{onto, Drawing, Wire};

/// One straight piece of a wire, as it is on the board.
struct Piece {
    wire: usize,
    /// Its place in the wire: from point `seg` to the next.
    seg: usize,
    id: ItemId,
    a: Point,
    b: Point,
    r: f64,
    /// Places along it where other copper is joined to it, each as the share of the way from
    /// `a` to `b` and the place's number: in order once all are found.
    cuts: Vec<(f64, usize)>,
}

/// A stretch of a piece between two places next to each other on it.
struct Stretch {
    /// The places it runs between.
    ends: [usize; 2],
    piece: usize,
    /// The shares of the piece it runs from and to.
    t: [f64; 2],
    width: f64,
    length: f64,
    net: u32,
    layer: usize,
    /// Still on the board.
    there: bool,
}

/// Places that are one place: copper joined there.
struct Places(Vec<usize>);

impl Places {
    fn new_one(&mut self) -> usize {
        self.0.push(self.0.len());
        self.0.len() - 1
    }

    fn of(&mut self, mut i: usize) -> usize {
        while self.0[i] != i {
            self.0[i] = self.0[self.0[i]];
            i = self.0[i];
        }
        i
    }

    fn join(&mut self, a: usize, b: usize) {
        let (a, b) = (self.of(a), self.of(b));
        // The lower number stays: the same answer whatever order they were joined in.
        self.0[a.max(b)] = a.min(b);
    }
}

/// How near an end of a piece a place must be to be that end, nm.
const AT_AN_END: f64 = 1000.0;
/// Two pieces of one wire are joined only where they are this many times the wire's width
/// apart along it: every piece lies on the one before it at the bend between them.
const ALONG: f64 = 2.0;

/// Where on the piece from `a` to `b` the point nearest `p` is, as a share of the way, and how
/// far from `p` that is.
fn nearest(p: Point, a: Point, b: Point) -> (f64, f64) {
    let (dx, dy) = ((b.x - a.x) as f64, (b.y - a.y) as f64);
    let (px, py) = ((p.x - a.x) as f64, (p.y - a.y) as f64);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 { 0.0 } else { ((px * dx + py * dy) / len2).clamp(0.0, 1.0) };
    (t, (px - t * dx).hypot(py - t * dy))
}

/// Which stretches are the only way between the two parts of the board they join: `ends` of
/// each stretch that is `there`, among `n` places.
fn only_ways(n: usize, ends: &[[usize; 2]], there: &[bool]) -> Vec<bool> {
    let mut at: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n];
    for (e, [u, v]) in ends.iter().enumerate().filter(|(e, _)| there[*e]) {
        at[*u].push((*v, e));
        at[*v].push((*u, e));
    }
    let mut only = vec![false; ends.len()];
    let (mut seen, mut low) = (vec![usize::MAX; n], vec![0; n]);
    let mut count = 0;
    for root in 0..n {
        if seen[root] != usize::MAX || at[root].is_empty() {
            continue;
        }
        // Each place on the way down: the place, the stretch it was come to by, and how many
        // of its stretches have been followed.
        let mut down: Vec<(usize, usize, usize)> = vec![(root, usize::MAX, 0)];
        seen[root] = count;
        low[root] = count;
        count += 1;
        while let Some(&(x, by, k)) = down.last() {
            if let Some(&(y, e)) = at[x].get(k) {
                if let Some(top) = down.last_mut() {
                    top.2 += 1;
                }
                if e == by {
                    continue;
                }
                if seen[y] == usize::MAX {
                    seen[y] = count;
                    low[y] = count;
                    count += 1;
                    down.push((y, e, 0));
                } else {
                    low[x] = low[x].min(seen[y]);
                }
                continue;
            }
            down.pop();
            if let Some(&(parent, _, _)) = down.last() {
                low[parent] = low[parent].min(low[x]);
                if low[x] > seen[parent] {
                    only[by] = true;
                }
            }
        }
    }
    only
}

/// `drawing` without the copper its nets have no need of, and how many runs of track were
/// taken out: none of a net in `leave`. The lengths taken out are in the drawing's `spare`.
/// A via left with track that touches it and does not end at its middle is given a short
/// piece that does, after the drawing's other wires.
pub fn spare(board: &Board, plan: &Plan, drawing: &Drawing, leave: &[u32]) -> (Drawing, usize) {
    let (with, new) = onto(board, plan, drawing);
    let mut places = Places(Vec::new());
    // Each end of each piece is a place, the same for every piece that ends at that point of
    // that layer.
    let mut at_point: HashMap<(usize, i64, i64), usize> = HashMap::new();
    let mut pieces: Vec<Piece> = Vec::new();
    let mut piece_of: HashMap<ItemId, usize> = HashMap::new();
    let mut ids = new.iter();
    for (w, wire) in drawing.wires.iter().enumerate() {
        for seg in 0..wire.pts.len().saturating_sub(1) {
            let Some(&id) = ids.next() else { continue };
            let Some(Shape::Capsule { a, b, r }) = with.item(id).map(|i| &i.shape) else { continue };
            piece_of.insert(id, pieces.len());
            let mut cuts = Vec::new();
            for (t, p) in [(0.0, a), (1.0, b)] {
                let place = *at_point.entry((wire.layer, p.x, p.y)).or_insert_with(|| places.new_one());
                cuts.push((t, place));
            }
            pieces.push(Piece { wire: w, seg, id, a: *a, b: *b, r: *r as f64, cuts });
        }
    }
    // Everything else of a net is a place of its own: a pad, a via, a spoke drawn by hand,
    // copper that was on the board already. What the bare board joins is one place.
    let bare = board::connectivity_without_pours(board).cluster;
    let mut of_cluster: HashMap<usize, usize> = HashMap::new();
    let mut of_item: HashMap<ItemId, usize> = HashMap::new();
    // Places that are copper the wires must reach: a stretch ends at each.
    let mut fixed: Vec<usize> = Vec::new();
    // Places that are a via of the plan.
    let mut vias: Vec<usize> = Vec::new();
    let other = |id: ItemId| with.item(id).filter(|i| i.is_copper() && i.net != 0 && !matches!(i.kind, Kind::Pour { .. }) && !piece_of.contains_key(&id));
    let mut place_of = |id: ItemId, places: &mut Places| -> usize {
        *of_item.entry(id).or_insert_with(|| {
            let place = match bare.get(&id) {
                Some(&c) => *of_cluster.entry(c).or_insert_with(|| places.new_one()),
                None => places.new_one(),
            };
            if board.item(id).is_some() {
                fixed.push(place);
            } else if with.item(id).is_some_and(|i| matches!(i.kind, Kind::Via)) {
                vias.push(place);
            } else {
                fixed.push(place);
            }
            place
        })
    };
    // The place on piece `k` nearest `p`: an end of it, or a place of its own along it.
    let on_piece = |pieces: &mut Vec<Piece>, places: &mut Places, k: usize, p: Point| -> usize {
        let piece = &mut pieces[k];
        let (t, _) = nearest(p, piece.a, piece.b);
        let len = piece.a.distance(piece.b);
        if let Some(&(_, place)) = piece.cuts.iter().find(|c| (c.0 - t).abs() * len < AT_AN_END) {
            return place;
        }
        let place = places.new_one();
        piece.cuts.push((t, place));
        place
    };
    // How far along its wire a share of a piece is, nm.
    let along = |pieces: &[Piece], k: usize, t: f64| -> f64 {
        let piece = &pieces[k];
        let wire = &drawing.wires[piece.wire];
        let before: f64 = wire.pts[..=piece.seg].windows(2).map(|s| (s[1][0] - s[0][0]).hypot(s[1][1] - s[0][1])).sum();
        before * 1e6 + t * piece.a.distance(piece.b)
    };
    for k in 0..pieces.len() {
        let (id, a, b, r) = (pieces[k].id, pieces[k].a, pieces[k].b, pieces[k].r);
        let Some(item) = with.item(id) else { continue };
        let (net, layer) = (item.net, drawing.wires[pieces[k].wire].layer);
        let mut near = with.near(layer, item.shape.bbox());
        near.sort_unstable();
        near.dedup();
        for o in near {
            if o == id {
                continue;
            }
            if let Some(&j) = piece_of.get(&o) {
                // Another piece of track: joined where an end of this one lies in its copper.
                let it = &pieces[j];
                if with.item(o).is_none_or(|i| i.net != net || !i.on(layer)) {
                    continue;
                }
                let (ia, ib, ir, same) = (it.a, it.b, it.r, it.wire == pieces[k].wire);
                for (t, p) in [(0.0, a), (1.0, b)] {
                    let (u, d) = nearest(p, ia, ib);
                    if d >= ir || (same && (along(&pieces, k, t) - along(&pieces, j, u)).abs() < ALONG * 2.0 * r.max(ir)) {
                        continue;
                    }
                    let here = pieces[k].cuts[t as usize].1;
                    let there = on_piece(&mut pieces, &mut places, j, p);
                    places.join(here, there);
                }
                continue;
            }
            let Some(it) = other(o).filter(|i| i.net == net && i.on(layer)) else { continue };
            // An end of the piece lying in it.
            for (t, p) in [(0, a), (1, b)] {
                if it.shape.contains(p) {
                    let (here, there) = (pieces[k].cuts[t].1, place_of(o, &mut places));
                    places.join(here, there);
                }
            }
            // Its own middle, or an end of it, lying in the piece's copper; a via or a round
            // pad is joined wherever the piece's middle line comes into it.
            let (pts, n) = board::anchors(&with, o);
            for &p in &pts[..n] {
                let within = match &it.shape {
                    Shape::Capsule { a: c, b: d, r: s } if c == d => (*s as f64).max(r),
                    _ => r,
                };
                if nearest(p, a, b).1 < within {
                    let here = on_piece(&mut pieces, &mut places, k, p);
                    let there = place_of(o, &mut places);
                    places.join(here, there);
                }
            }
        }
    }
    // The stretches: each piece from one of its places to the next.
    let mut stretches: Vec<Stretch> = Vec::new();
    for (k, piece) in pieces.iter_mut().enumerate() {
        piece.cuts.sort_by(|x, y| x.0.total_cmp(&y.0));
        let wire = &drawing.wires[piece.wire];
        let length = piece.a.distance(piece.b) / 1e6;
        for c in piece.cuts.windows(2) {
            stretches.push(Stretch { ends: [places.of(c[0].1), places.of(c[1].1)], piece: k, t: [c[0].0, c[1].0], width: wire.width, length: length * (c[1].0 - c[0].0), net: wire.net, layer: wire.layer, there: true });
        }
    }
    let n = places.0.len();
    let mut is_fixed = vec![false; n];
    for &f in &fixed {
        is_fixed[places.of(f)] = true;
    }
    let mut is_via = vec![false; n];
    for &v in &vias {
        is_via[places.of(v)] = true;
    }
    // A stretch from a place to itself lies inside the copper it is joined to (the last of a
    // wire inside its via): it shuts nothing in and stays.
    let ends: Vec<[usize; 2]> = stretches.iter().map(|s| s.ends).collect();
    let mut there: Vec<bool> = stretches.iter().map(|s| s.ends[0] != s.ends[1]).collect();
    let mut taken = 0;
    // The places the runs taken out ended at.
    let mut touched: Vec<usize> = Vec::new();
    loop {
        let only = only_ways(n, &ends, &there);
        let mut at: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (e, s) in stretches.iter().enumerate().filter(|(e, _)| there[*e]) {
            at[s.ends[0]].push(e);
            at[s.ends[1]].push(e);
        }
        // A place a run of stretches goes on through: two stretches meet there and nothing
        // else does.
        let through = |x: usize| at[x].len() == 2 && !is_fixed[x] && !is_via[x];
        // The runs that are one of two ways round, each with what is to be said against it.
        let mut seen = vec![false; stretches.len()];
        let mut worst: Option<((f64, f64), Vec<usize>)> = None;
        for first in 0..stretches.len() {
            if !there[first] || only[first] || seen[first] || leave.contains(&stretches[first].net) {
                continue;
            }
            let mut run = vec![first];
            seen[first] = true;
            let mut tips = stretches[first].ends;
            for tip in tips.iter_mut() {
                let mut last = first;
                while through(*tip) {
                    let Some(&next) = at[*tip].iter().find(|&&e| e != last) else { break };
                    if seen[next] {
                        break;
                    }
                    seen[next] = true;
                    run.push(next);
                    let e = stretches[next].ends;
                    *tip = if e[0] == *tip { e[1] } else { e[0] };
                    last = next;
                }
            }
            // A via must join copper on two layers or more, or KiCad calls it dangling (three
            // tracks on one inner layer and nothing on the others is a via to nowhere). A run
            // whose going would leave a via so stays, ring and all.
            let layers = |x: usize, without: &[usize]| {
                let mut l: Vec<usize> = at[x].iter().filter(|e| !without.contains(e)).map(|&e| stretches[e].layer).collect();
                l.sort_unstable();
                l.dedup();
                l.len()
            };
            if tips.iter().any(|&x| is_via[x] && layers(x, &[]) >= 2 && layers(x, &run) < 2) {
                continue;
            }
            let narrow = run.iter().map(|&e| stretches[e].width).fold(f64::MAX, f64::min);
            let length: f64 = run.iter().map(|&e| stretches[e].length).sum();
            let against = (-narrow, length);
            if worst.as_ref().is_none_or(|w| against > w.0) {
                worst = Some((against, run));
            }
        }
        let Some((_, run)) = worst else { break };
        for e in run {
            there[e] = false;
            touched.extend(stretches[e].ends);
        }
        taken += 1;
    }
    // What a run taken out leaves hanging: a stretch whose end, where the run joined it, now
    // joins nothing (a sliver from a bend to the place along it where it meets other copper).
    // Each is taken out too, back to where something else joins, but never up to a via.
    while let Some(x) = touched.pop() {
        if is_fixed[x] || is_via[x] {
            continue;
        }
        let mut left = (0..stretches.len()).filter(|&e| there[e] && stretches[e].ends.contains(&x));
        let (Some(e), None) = (left.next(), left.next()) else { continue };
        let y = if stretches[e].ends[0] == x { stretches[e].ends[1] } else { stretches[e].ends[0] };
        if is_via[y] {
            continue;
        }
        there[e] = false;
        touched.push(y);
    }
    for (s, t) in stretches.iter_mut().zip(&there) {
        s.there = *t || s.ends[0] == s.ends[1];
    }
    // A via that a wire of its net is joined to, with no piece of that wire ending at the
    // via's middle: the wire comes into the via's copper and stops short, or runs on through
    // it to one side (what ended at the middle was one of two ways round, and went).
    // The board takes the two as joined, but a hand that moves the via in KiCad moves it
    // alone. A piece from where the track is nearest to the via's middle joins them; it lies
    // in the via's own copper and the track's.
    let mut plan_vias: Vec<(ItemId, usize)> = of_item.iter().filter(|(id, _)| board.item(**id).is_none() && with.item(**id).is_some_and(|i| matches!(i.kind, Kind::Via))).map(|(&id, &place)| (id, places.of(place))).collect();
    plan_vias.sort_unstable();
    let mut joints: Vec<Wire> = Vec::new();
    for (id, place) in plan_vias {
        let Some(Shape::Capsule { a: middle, r, .. }) = with.item(id).map(|i| &i.shape) else { continue };
        // Each end of a stretch that is at the via: of which wire, how far from the via's
        // middle, and where. (Wire by wire, not layer by layer: one wire that ends at the
        // middle does not join another of its layer that stops short of it.)
        let mut at: Vec<(usize, f64, usize, usize, [f64; 2])> = Vec::new();
        // The far ends of the pieces that do end at the middle, each with its layer: a wire
        // one of those starts from is joined already (by the piece put in the last time).
        let mut joined: Vec<(usize, [f64; 2])> = Vec::new();
        for (e, stretch) in stretches.iter().enumerate().filter(|(_, s)| s.there) {
            for end in 0..2 {
                if stretch.ends[end] == place {
                    let piece = &pieces[stretch.piece];
                    let t = stretch.t[end];
                    let here = [piece.a.x as f64 + t * (piece.b.x - piece.a.x) as f64, piece.a.y as f64 + t * (piece.b.y - piece.a.y) as f64];
                    let away = (here[0] - middle.x as f64).hypot(here[1] - middle.y as f64);
                    if away < AT_AN_END {
                        let far = if piece.a.distance(*middle) < piece.b.distance(*middle) { piece.b } else { piece.a };
                        joined.push((stretch.layer, [far.x as f64, far.y as f64]));
                    }
                    at.push((piece.wire, away, e, end, here));
                }
            }
        }
        at.sort_by(|x, y| x.0.cmp(&y.0).then(x.1.total_cmp(&y.1)).then((x.2, x.3).cmp(&(y.2, y.3))));
        at.dedup_by_key(|x| x.0);
        for (_, away, e, end, here) in at {
            let (layer, reach) = (stretches[e].layer, stretches[e].width * 1e6);
            if away < AT_AN_END || away > *r as f64 + reach || joined.iter().any(|&(l, far)| l == layer && (far[0] - here[0]).hypot(far[1] - here[1]) < reach) {
                continue;
            }
            let piece = &pieces[stretches[e].piece];
            let wire = &drawing.wires[piece.wire];
            let (a, b, t) = (wire.pts[piece.seg], wire.pts[piece.seg + 1], stretches[e].t[end]);
            let from = if t <= 0.0 { a } else if t >= 1.0 { b } else { [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t] };
            joints.push(Wire { pts: vec![from, [middle.x as f64 / 1e6, middle.y as f64 / 1e6]], as_planned: false, ..wire.clone() });
        }
    }
    if taken == 0 && joints.is_empty() {
        return (drawing.clone(), 0);
    }
    // The wires again: each as what is left of it, in as many lengths as it is cut into.
    let mut out = Drawing { wires: Vec::new(), undrawn: drawing.undrawn.clone(), ties: drawing.ties.clone(), stitches: drawing.stitches.clone(), spare: drawing.spare.clone() };
    let mut s = 0;
    for (w, wire) in drawing.wires.iter().enumerate() {
        let (from, spare_from) = (out.wires.len(), out.spare.len());
        let mut run: Vec<P> = Vec::new();
        let mut kept = true;
        let close = |run: &mut Vec<P>, kept: bool, out: &mut Drawing| {
            if run.len() >= 2 {
                let piece = Wire { pts: std::mem::take(run), ..wire.clone() };
                if kept { out.wires.push(piece) } else { out.spare.push(piece) }
            }
            run.clear();
        };
        while s < stretches.len() && pieces[stretches[s].piece].wire == w {
            let stretch = &stretches[s];
            let seg = pieces[stretch.piece].seg;
            let (a, b) = (wire.pts[seg], wire.pts[seg + 1]);
            let point = |t: f64| if t <= 0.0 { a } else if t >= 1.0 { b } else { [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t] };
            if stretch.there != kept {
                close(&mut run, kept, &mut out);
                kept = stretch.there;
            }
            let (from, to) = (point(stretch.t[0]), point(stretch.t[1]));
            if run.is_empty() {
                run.push(from);
            }
            // A place along a piece is no bend.
            if stretch.t[0] > 0.0 && run.len() >= 2 {
                run.pop();
            }
            run.push(to);
            s += 1;
        }
        close(&mut run, kept, &mut out);
        // What is kept of a wire beside a length taken out and is shorter than the wire is
        // wide lies in the copper it ends in: KiCad calls such a sliver dangling, and it
        // joins nothing the copper round it does not.
        let mut k = from;
        while k < out.wires.len() {
            let pts = &out.wires[k].pts;
            let len: f64 = pts.windows(2).map(|s| (s[1][0] - s[0][0]).hypot(s[1][1] - s[0][1])).sum();
            let ends = [pts[0], pts[pts.len() - 1]];
            let beside = out.spare[spare_from..].iter().any(|p| [p.pts[0], p.pts[p.pts.len() - 1]].iter().any(|q| ends.contains(q)));
            if len < wire.width && beside {
                out.spare.push(out.wires.remove(k));
            } else {
                k += 1;
            }
        }
    }
    out.wires.extend(joints);
    (out, taken)
}
