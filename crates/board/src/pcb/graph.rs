//! The path current takes through a net's copper between two pads: shortest, over track
//! centrelines, via barrels, and straight lines across a zone fill between the points where
//! other copper meets it. A fast proxy for a current path, not a field solution: a straight line
//! across a fill may cut a corner the fill does not have.
//!
//! What joins what is the checker's ([`crate::joined`], KiCad's way): a track joins where its
//! round end reaches into other copper, a pad no zone joins (zone connection "none") is not joined
//! by the fill it lies in, and copper that merely overlaps a pad's corner does not join it.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::pcb::Board;
use crate::{ItemId, Kind, Point, Shape, DRAWN};

#[derive(Debug, Clone)]
pub struct Path {
    /// Points from the first pad's centre to the second's, mm.
    pub points: Vec<[f64; 2]>,
    /// The layer of each point.
    pub layers: Vec<String>,
    /// mm.
    pub length: f64,
}

#[derive(PartialEq)]
struct Item(f64, usize);
impl Eq for Item {}
impl PartialOrd for Item {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Item {
    fn cmp(&self, o: &Self) -> Ordering {
        o.0.total_cmp(&self.0)
    }
}

/// The shortest copper path on `net` from pad `from` to pad `to` (`REF.PIN`), or `None` when
/// the copper does not join them (or the board was not drawn from a file).
pub fn copper_path(board: &Board, net: &str, from: &str, to: &str) -> Option<Path> {
    let src = board.source.as_deref()?;
    let net = src.nets.iter().position(|n| n == net).filter(|&n| n != 0)? as u32;
    let unfilled: std::collections::HashSet<usize> = src.unfilled.iter().map(|u| u.0).collect();
    // The pad each pad item belongs to: a pad of several polygons is one terminal.
    let pad_of: HashMap<ItemId, usize> = src.pads.iter().enumerate().flat_map(|(k, p)| p.items.iter().map(move |&id| (id, k))).collect();

    // Nodes: a point on a layer. A pad has one per layer at its centre, a via one per layer, a
    // track one at each end.
    let mut nodes: Vec<(Point, usize)> = Vec::new();
    let mut adj: Vec<Vec<(usize, f64)>> = Vec::new();
    let mut add = |nodes: &mut Vec<(Point, usize)>, p: Point, l: usize| {
        nodes.push((p, l));
        adj.push(Vec::new());
        nodes.len() - 1
    };
    let mut own: HashMap<ItemId, Vec<usize>> = HashMap::new();
    let mut pad_nodes: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut pours: Vec<ItemId> = Vec::new();
    let mut copper: Vec<ItemId> = Vec::new();
    let layers_of = |mask: u64| (0..src.layers.len()).filter(move |l| mask & (1 << l) != 0);
    for (id, it) in src.items().filter(|(_, it)| it.net == net && it.is_copper()) {
        match (&it.kind, &it.shape) {
            (Kind::Pour { zone, .. }, _) => {
                if *zone == DRAWN || !unfilled.contains(zone) {
                    pours.push(id);
                }
                continue;
            }
            (Kind::Pad { .. }, _) => {
                let Some(&k) = pad_of.get(&id) else { continue };
                let ids = pad_nodes.entry(k).or_insert_with(|| layers_of(it.layers).map(|l| add(&mut nodes, src.pads[k].centre, l)).collect()).clone();
                own.insert(id, ids);
            }
            (Kind::Track, Shape::Capsule { a, b, .. }) => {
                let l = it.layers.trailing_zeros() as usize;
                own.insert(id, vec![add(&mut nodes, *a, l), add(&mut nodes, *b, l)]);
            }
            (Kind::Via, Shape::Capsule { a, .. }) => {
                own.insert(id, layers_of(it.layers).map(|l| add(&mut nodes, *a, l)).collect());
            }
            _ => continue,
        }
        copper.push(id);
    }
    let link = |adj: &mut Vec<Vec<(usize, f64)>>, a: usize, b: usize, w: f64| {
        adj[a].push((b, w));
        adj[b].push((a, w));
    };
    // Along a track; through a pad or a via's barrel at no cost.
    for &id in &copper {
        let ns = &own[&id];
        let track = matches!(src.item(id).map(|it| &it.kind), Some(Kind::Track));
        for w in ns.windows(2) {
            let d = if track { nodes[w[0]].0.distance(nodes[w[1]].0) } else { 0.0 };
            link(&mut adj, w[0], w[1], d);
        }
    }
    // How far a node of `id` reaches: a track end or a via its radius, a pad's centre nothing.
    let reach = |id: ItemId| match src.item(id).map(|it| (&it.kind, &it.shape)) {
        Some((Kind::Track | Kind::Via, Shape::Capsule { r, .. })) => *r as f64,
        _ => 0.0,
    };
    let touches = |p: Point, r: f64, shape: &Shape| shape.gap_within(&Shape::Capsule { a: p, b: p, r: 0 }, r) <= r;
    // Joined copper: each node of one that reaches into the other meets every node of the other
    // on that layer (a track's end meets a track it lands on in its middle at both its ends;
    // a track running through a pad meets the pad's centre); joined where no node reaches, the
    // nearest two meet.
    let mut seen: std::collections::HashSet<(ItemId, ItemId)> = Default::default();
    for &x in &copper {
        let xi = src.item(x).expect("listed");
        for l in layers_of(xi.layers) {
            for y in src.near(l, xi.shape.bbox()) {
                if y == x || !own.contains_key(&y) || own[&x] == own[&y] || !seen.insert((x.min(y), x.max(y))) {
                    continue;
                }
                let yi = src.item(y).expect("listed");
                if !yi.on(l) || !crate::joined(src, x, y) {
                    continue;
                }
                let on = |id: ItemId| own[&id].iter().copied().filter(|&n| nodes[n].1 == l).collect::<Vec<_>>();
                let (xs, ys) = (on(x), on(y));
                let mut met = false;
                for (a, b, bi, ra) in [(&xs, &ys, yi, reach(x)), (&ys, &xs, xi, reach(y))] {
                    for &n in a.iter().filter(|&&n| touches(nodes[n].0, ra, &bi.shape)) {
                        for &m in b {
                            link(&mut adj, n, m, nodes[n].0.distance(nodes[m].0));
                            met = true;
                        }
                    }
                }
                if !met {
                    if let Some((n, m)) = xs.iter().flat_map(|&n| ys.iter().map(move |&m| (n, m))).min_by(|p, q| nodes[p.0].0.distance(nodes[p.1].0).total_cmp(&nodes[q.0].0.distance(nodes[q.1].0))) {
                        link(&mut adj, n, m, nodes[n].0.distance(nodes[m].0));
                    }
                }
            }
        }
    }
    // A fill: the nodes of the copper it joins, every one to every other in a straight line. A
    // pad joins at its centre, a track or via at the nodes that reach into the fill (all of them
    // if none does).
    for &f in &pours {
        let fi = src.item(f).expect("listed");
        let l = fi.layers.trailing_zeros() as usize;
        let mut members: Vec<usize> = Vec::new();
        for y in src.near(l, fi.shape.bbox()) {
            if !own.contains_key(&y) || !crate::joined(src, f, y) {
                continue;
            }
            let ns: Vec<usize> = own[&y].iter().copied().filter(|&n| nodes[n].1 == l).collect();
            let pad = matches!(src.item(y).map(|it| &it.kind), Some(Kind::Pad { .. }));
            let within: Vec<usize> = ns.iter().copied().filter(|&n| touches(nodes[n].0, reach(y), &fi.shape)).collect();
            members.extend(if pad || within.is_empty() { ns } else { within });
        }
        members.sort_unstable();
        members.dedup();
        for (k, &i) in members.iter().enumerate() {
            for &j in &members[k + 1..] {
                link(&mut adj, i, j, nodes[i].0.distance(nodes[j].0));
            }
        }
    }

    // A pin with several pads (a MOSFET's three source leads, an exposed pad in pieces) is one
    // terminal: the path may start or end at any of them.
    board.pad(from)?;
    board.pad(to)?;
    let find = |ref_pin: &str| -> Vec<usize> {
        let (r, pin) = ref_pin.split_once('.').unwrap_or_default();
        pad_nodes.iter().filter(|(k, _)| src.pads[**k].number == pin && src.footprints[src.pads[**k].footprint].reference == r).flat_map(|(_, ns)| ns.iter().copied()).collect()
    };
    let (starts, ends) = (find(from), find(to));

    let mut best = vec![f64::INFINITY; nodes.len()];
    let mut prev = vec![usize::MAX; nodes.len()];
    let mut heap = BinaryHeap::new();
    for &s in &starts {
        best[s] = 0.0;
        heap.push(Item(0.0, s));
    }
    while let Some(Item(d, u)) = heap.pop() {
        if d > best[u] {
            continue;
        }
        for &(v, w) in &adj[u] {
            if d + w < best[v] {
                best[v] = d + w;
                prev[v] = u;
                heap.push(Item(d + w, v));
            }
        }
    }
    let &end = ends.iter().min_by(|a, b| best[**a].total_cmp(&best[**b]))?;
    if !best[end].is_finite() {
        return None;
    }
    let mut chain = vec![end];
    while prev[*chain.last().unwrap()] != usize::MAX {
        chain.push(prev[*chain.last().unwrap()]);
    }
    chain.reverse();
    let mm = |p: Point| [p.x as f64 / 1e6, p.y as f64 / 1e6];
    let mut points: Vec<[f64; 2]> = Vec::new();
    let mut layers = Vec::new();
    for i in chain {
        let p = mm(nodes[i].0);
        if points.last().is_some_and(|q| crate::pcb::geom::dist(*q, p) < 1e-9) {
            continue;
        }
        points.push(p);
        layers.push(src.layers[nodes[i].1].clone());
    }
    Some(Path { points, layers, length: best[end] / 1e6 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pcb::geom::P;
    use crate::pcb::{Fill, Pad, Track, Via};

    fn pad(part: &str, at: P) -> Pad {
        let sq = vec![[at[0] - 0.3, at[1] - 0.3], [at[0] + 0.3, at[1] - 0.3], [at[0] + 0.3, at[1] + 0.3], [at[0] - 0.3, at[1] + 0.3]];
        Pad { part: part.into(), pin: "1".into(), net: "N".into(), layers: vec!["F.Cu".into()], centre: at, shape: vec![sq] }
    }

    fn track(a: P, b: P, width: f64, layer: &str) -> Track {
        Track { a, b, width, layer: layer.into(), net: "N".into(), locked: false }
    }

    fn via(at: P, layers: [&str; 2]) -> Via {
        Via { at, size: 0.4, drill: 0.2, layers: layers.iter().map(|l| l.to_string()).collect(), net: "N".into() }
    }

    const TWO: [&str; 2] = ["F.Cu", "B.Cu"];

    #[test]
    fn track_through_a_pad_joins_it() {
        // One track from A to B runs straight over P's pad; P has no track end of its own.
        let pads = [pad("A", [0.0, 0.0]), pad("P", [4.0, 0.0]), pad("B", [10.0, 0.0])];
        let board = Board::sketch(&TWO, &pads, &[track([0.0, 0.0], [10.0, 0.0], 0.2, "F.Cu")], &[], &[]).unwrap();
        let p = copper_path(&board, "N", "P.1", "B.1").expect("P joins the track");
        assert!((p.length - 6.0).abs() < 1e-6, "{}", p.length);
        let p = copper_path(&board, "N", "A.1", "P.1").unwrap();
        assert!((p.length - 4.0).abs() < 1e-6, "{}", p.length);
    }

    #[test]
    fn overlapping_tracks_join() {
        // Two 0.1 mm tracks whose ends are 0.09 mm apart: an end reaches into the other, so they join.
        let t = |y: f64| [track([0.0, 0.0], [1.0, 0.0], 0.1, "F.Cu"), track([1.0, y], [2.0, y], 0.1, "F.Cu")];
        let board = Board::sketch(&["F.Cu"], &[pad("A", [0.0, 0.0]), pad("B", [2.0, 0.09])], &t(0.09), &[], &[]).unwrap();
        assert!(copper_path(&board, "N", "A.1", "B.1").is_some());
        // 0.101 mm apart they do not touch.
        let board = Board::sketch(&["F.Cu"], &[pad("A", [0.0, 0.0]), pad("B", [2.0, 0.101])], &t(0.101), &[], &[]).unwrap();
        assert!(copper_path(&board, "N", "A.1", "B.1").is_none());
    }

    #[test]
    fn via_in_pad_joins_it() {
        // A's pad holds a via; the track leaves it on B.Cu and comes up through a second via
        // inside B's pad. No track end touches either pad on F.Cu.
        let board = Board::sketch(
            &TWO,
            &[pad("A", [0.0, 0.0]), pad("B", [5.0, 0.0])],
            &[track([0.0, 0.1], [5.0, 0.1], 0.2, "B.Cu")],
            &[via([0.0, 0.1], TWO), via([5.0, 0.1], TWO)],
            &[],
        )
        .unwrap();
        let p = copper_path(&board, "N", "A.1", "B.1").expect("the vias join the pads");
        assert!((p.length - 5.2).abs() < 1e-6, "{}", p.length);
    }

    #[test]
    fn a_pin_with_several_pads_is_one_terminal() {
        // Q's pin 1 is two pads; the track reaches only the second.
        let pads = [pad("Q", [0.0, 0.0]), pad("Q", [0.0, 2.0]), pad("B", [5.0, 2.0])];
        let board = Board::sketch(&["F.Cu"], &pads, &[track([0.0, 2.0], [5.0, 2.0], 0.2, "F.Cu")], &[], &[]).unwrap();
        let p = copper_path(&board, "N", "Q.1", "B.1").expect("the second pad of Q.1 is joined");
        assert!((p.length - 5.0).abs() < 1e-6, "{}", p.length);
    }

    #[test]
    fn stacked_hdi_vias_join_across_layers() {
        // B.Cu pad -> microvia (B.Cu..In2.Cu) -> buried via (In2.Cu..In1.Cu) -> microvia (In1.Cu..F.Cu) -> F.Cu track -> F.Cu pad.
        let mut j = pad("J", [0.0, 0.0]);
        j.layers = vec!["B.Cu".into()];
        let board = Board::sketch(
            &["F.Cu", "In1.Cu", "In2.Cu", "B.Cu"],
            &[j, pad("D", [2.0, 0.0])],
            &[track([0.0, 0.0], [2.0, 0.0], 0.1, "F.Cu")],
            &[via([0.0, 0.0], ["F.Cu", "In1.Cu"]), via([0.0, 0.0], ["In1.Cu", "In2.Cu"]), via([0.0, 0.0], ["In2.Cu", "B.Cu"])],
            &[],
        )
        .unwrap();
        let p = copper_path(&board, "N", "J.1", "D.1").expect("stacked HDI vias connect B.Cu to F.Cu");
        assert!((p.length - 2.0).abs() < 1e-6, "{}", p.length);
    }

    #[test]
    fn a_fill_joins_the_copper_in_it_straight_across() {
        let fill = Fill { net: "N".into(), layer: "F.Cu".into(), polygon: vec![[-1.0, -1.0], [6.0, -1.0], [6.0, 1.0], [-1.0, 1.0]] };
        let board = Board::sketch(&["F.Cu"], &[pad("A", [0.0, 0.0]), pad("B", [5.0, 0.0])], &[], &[], &[fill]).unwrap();
        let p = copper_path(&board, "N", "A.1", "B.1").expect("the fill joins them");
        assert!((p.length - 5.0).abs() < 1e-6, "{}", p.length);
    }

    #[test]
    fn a_track_over_a_pads_edge_does_not_join_it() {
        // The track runs past P's pad, its side over the pad's edge, neither end nor the pad's
        // centre in the other: KiCad joins at anchors, so they are not joined.
        let board = Board::sketch(&["F.Cu"], &[pad("A", [0.0, 0.0]), pad("P", [2.0, 0.35])], &[track([0.0, 0.0], [4.0, 0.0], 0.2, "F.Cu")], &[], &[]).unwrap();
        assert!(copper_path(&board, "N", "A.1", "P.1").is_none());
    }
}
