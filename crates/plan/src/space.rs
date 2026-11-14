//! The room on one copper layer: the obstacles there (pads, fixed copper, holes, keepouts, the
//! board's edge) as polygons, and a constrained triangulation of their corners. A triangle edge
//! between two obstacles is a gap; its room is what the rules leave between them.

use std::collections::HashMap;
use std::sync::Arc;

use board::{Board, ItemId, Kind, Nm, Point, Shape};
use serde::Serialize;
use spade::{ConstrainedDelaunayTriangulation, Point2, Triangulation};

/// Millimetres, KiCad's frame.
pub type P = [f64; 2];

pub fn mm(p: Point) -> P {
    [p.x as f64 / 1e6, p.y as f64 / 1e6]
}

pub fn dist(a: P, b: P) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

pub fn lerp(a: P, b: P, t: f64) -> P {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

pub fn point_segment(p: P, a: P, b: P) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    dist(p, [a[0] + dx * t, a[1] + dy * t])
}

pub fn inside(p: P, poly: &[P]) -> bool {
    let mut odd = false;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
            odd = !odd;
        }
    }
    odd
}

/// Where a strand may end: a pad or via of the board, or a via of the plan, by its number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum End {
    Item(ItemId),
    Site(usize),
}

/// A via of the plan, the same on every layer: an obstacle like a pad.
#[derive(Debug, Clone, Serialize)]
pub struct Site {
    pub obstacle: usize,
}

/// The places a via could stand: none of them is in the triangulation. A via is an obstacle
/// only once a connection has put one in, and then every layer is triangulated again with it.
#[derive(Debug, Clone, Default)]
pub struct Spots {
    pub at: Vec<P>,
    /// A via's radius, mm.
    pub radius: f64,
    /// How far apart the middles of two vias must be, mm.
    pub apart: f64,
}

/// A gap by the places of its two corners, to a nanometre, the lesser first: the same gap in
/// every triangulation that has it.
pub type GapKey = ((i64, i64), (i64, i64));

pub fn gap_key(a: P, b: P) -> GapKey {
    let k = |p: P| ((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64);
    (k(a).min(k(b)), k(a).max(k(b)))
}

/// A via of the plan is a triangle this far from its middle to its corners, mm, with the via's
/// radius as its obstacle's reach: all but a point, so that it stands inside one triangle of
/// the triangulation it is put into, clear of the strands that pass it.
pub const VIA_CORNER: f64 = 0.001;

/// A spot this near a side of the triangle it is in, mm, is not used while that side is there.
const SPOT_MARGIN: f64 = 0.005;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum What {
    /// A pad, or a via: a strand of its net may end on it.
    Pad,
    /// Copper that stays: locked tracks and vias, a block's pours, copper drawn on the layer.
    Copper,
    Hole,
    Keepout,
    /// The board's edge and its cutouts.
    Edge,
}

#[derive(Debug, Clone, Serialize)]
pub struct Obstacle {
    pub what: What,
    /// 0 for none.
    pub net: u32,
    pub item: Option<ItemId>,
    pub poly: Vec<P>,
    /// A middle point: where a strand that ends on this obstacle is taken to end.
    pub centre: P,
    /// How far its copper reaches beyond the polygon, mm: a via's radius, nothing for the rest.
    pub reach: f64,
    /// What a track of another net keeps from its copper so as not to be bared in its solder
    /// mask opening, mm: nothing where it has none on this layer.
    pub mask: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Tri {
    pub v: [usize; 3],
    /// `e[i]` is the edge opposite `v[i]`.
    pub e: [usize; 3],
    /// The obstacle this triangle lies in, if any.
    pub inside: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Edge {
    pub v: [usize; 2],
    /// The triangle on each side; `None` beyond the triangulation's hull.
    pub t: [Option<usize>; 2],
    pub len: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Space {
    pub layer: usize,
    pub obstacles: Vec<Obstacle>,
    pub verts: Vec<P>,
    /// The obstacle each corner belongs to (the first, where several share it).
    pub vert_of: Vec<Option<usize>>,
    pub tris: Vec<Tri>,
    pub edges: Vec<Edge>,
    /// The obstacle of each thing a strand may end on.
    #[serde(skip)]
    pub ends: HashMap<End, usize>,
    /// The plan's vias, in its order: the same on every layer.
    pub sites: Vec<Site>,
    /// The places a via could stand.
    #[serde(skip)]
    pub spots: Arc<Spots>,
    /// The free triangle each spot is in, if a via could be put there on this layer as it is:
    /// none for a spot too near a via of the plan, or on a side of its triangle.
    #[serde(skip)]
    pub spot_tri: Vec<Option<usize>>,
    /// The spots in each triangle.
    #[serde(skip)]
    pub tri_spots: Vec<Vec<usize>>,
}

/// The outline of a shape as a polygon that holds all of it: a circle's is a 12-gon round it.
pub fn outline(shape: &Shape) -> Vec<P> {
    match shape {
        Shape::Polygon(p) => simpler(&p.iter().map(|&q| mm(q)).collect::<Vec<P>>(), FINE),
        Shape::Capsule { a, b, r } => {
            let (a, b) = (mm(*a), mm(*b));
            const N: usize = 6; // corners per half turn
            let r = *r as f64 / 1e6 / (std::f64::consts::PI / (2 * N) as f64).cos();
            let along = if dist(a, b) > 0.0 { (b[1] - a[1]).atan2(b[0] - a[0]) } else { 0.0 };
            let cap = |c: P, from: f64| (0..N + (dist(a, b) > 0.0) as usize).map(move |k| {
                let t = from + std::f64::consts::PI * k as f64 / N as f64;
                [c[0] + r * t.cos(), c[1] + r * t.sin()]
            });
            let quarter = std::f64::consts::FRAC_PI_2;
            cap(b, along - quarter).chain(cap(a, along + quarter)).collect()
        }
    }
}

/// A round shape as what it is: its middle (a triangle [`VIA_CORNER`] across) or its middle
/// line (a sliver that wide), and how far it reaches from there. None for a shape that is not
/// round. A circle taken as a polygon has corners that stand out past the circle or sides
/// that cut inside it, and one track between two header pins fits by less than either.
pub fn slim(shape: &Shape) -> Option<(Vec<P>, f64)> {
    let (a, b, r) = match shape {
        Shape::Capsule { a, b, r } => (mm(*a), mm(*b), *r as f64 / 1e6),
        Shape::Polygon(p) => oval(&p.iter().map(|&q| mm(q)).collect::<Vec<P>>())?,
    };
    let long = dist(a, b);
    if long < 4.0 * VIA_CORNER {
        let poly = (0..3).map(|i| {
            let t = std::f64::consts::PI * (0.5 + 2.0 * i as f64 / 3.0);
            [a[0] + VIA_CORNER * t.cos(), a[1] + VIA_CORNER * t.sin()]
        });
        return Some((poly.collect(), r + VIA_CORNER));
    }
    let n = [-(b[1] - a[1]) / long * VIA_CORNER, (b[0] - a[0]) / long * VIA_CORNER];
    Some((vec![[a[0] + n[0], a[1] + n[1]], [b[0] + n[0], b[1] + n[1]], [b[0] - n[0], b[1] - n[1]], [a[0] - n[0], a[1] - n[1]]], r + VIA_CORNER))
}

/// How far a polygon's corners may be from an oval's edge for the polygon to be that oval, mm.
const ROUND: f64 = 0.004;

/// The oval a polygon was made from, if it is one: the two ends of its middle line and its
/// radius, every corner within [`ROUND`] inside its edge and none outside. An oval or round pad
/// comes from the file as a polygon of many corners.
fn oval(poly: &[P]) -> Option<(P, P, f64)> {
    if poly.len() < 12 {
        return None;
    }
    // Its length is between the two corners farthest apart; its width is twice the farthest
    // any corner is from the line between them.
    let first = (0..poly.len()).max_by(|&x, &y| dist(poly[0], poly[x]).total_cmp(&dist(poly[0], poly[y])))?;
    let other = (0..poly.len()).max_by(|&x, &y| dist(poly[first], poly[x]).total_cmp(&dist(poly[first], poly[y])))?;
    let (p, q) = (poly[first], poly[other]);
    let long = dist(p, q);
    if long <= 0.0 {
        return None;
    }
    let u = [(q[0] - p[0]) / long, (q[1] - p[1]) / long];
    let r = poly.iter().map(|c| ((c[0] - p[0]) * u[1] - (c[1] - p[1]) * u[0]).abs()).fold(0.0, f64::max);
    // A circle's corners are all as far apart as its two farthest: its radius is half that.
    let r = r.min(long / 2.0);
    let (a, b) = ([p[0] + u[0] * r, p[1] + u[1] * r], [q[0] - u[0] * r, q[1] - u[1] * r]);
    let (a, b) = if long - 2.0 * r < 4.0 * VIA_CORNER { ([(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0], [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0]) } else { (a, b) };
    let from = |c: P| if dist(a, b) > 0.0 { point_segment(c, a, b) } else { dist(c, a) };
    let far = poly.iter().map(|&c| from(c)).fold(0.0, f64::max);
    let near = poly.iter().map(|&c| from(c)).fold(f64::MAX, f64::min);
    // And no side cuts far inside: the middle of each is on the edge too.
    let sides = (0..poly.len()).map(|i| from([(poly[i][0] + poly[(i + 1) % poly.len()][0]) / 2.0, (poly[i][1] + poly[(i + 1) % poly.len()][1]) / 2.0])).fold(f64::MAX, f64::min);
    (far - near <= ROUND && far - sides <= 4.0 * ROUND).then_some((a, b, far))
}

/// A polygon's corners may move this far when it is made simpler, mm: a round pad comes as
/// hundreds of corners, and each is a vertex of the triangulation.
const FINE: f64 = 0.01;

/// The polygon with the corners left out that lie within `by` of the line between those kept.
fn simpler(poly: &[P], by: f64) -> Vec<P> {
    if poly.len() <= 8 {
        return poly.to_vec();
    }
    fn between(poly: &[P], i: usize, j: usize, by: f64, keep: &mut Vec<usize>) {
        let far = (i + 1..j).map(|k| (point_segment(poly[k], poly[i], poly[j % poly.len()]), k)).max_by(|x, y| x.0.total_cmp(&y.0));
        if let Some((d, k)) = far
            && d > by
        {
            between(poly, i, k, by, keep);
            keep.push(k);
            between(poly, k, j, by, keep);
        }
    }
    // Start from the corner farthest from the first, so the two halves are well apart.
    let far = (0..poly.len()).max_by(|&x, &y| dist(poly[0], poly[x]).total_cmp(&dist(poly[0], poly[y]))).unwrap();
    let mut keep = vec![0];
    between(poly, 0, far, by, &mut keep);
    keep.push(far);
    between(poly, far, poly.len(), by, &mut keep);
    keep.iter().map(|&k| poly[k]).collect()
}

fn centre(poly: &[P]) -> P {
    let n = poly.len().max(1) as f64;
    [poly.iter().map(|p| p[0]).sum::<f64>() / n, poly.iter().map(|p| p[1]).sum::<f64>() / n]
}

impl Space {
    /// The room on `layer` of `board`, tracks and vias that are not locked left out (strip the
    /// board first: see [`crate::strip`]), with the plan's `vias` (each a middle and a net) as
    /// obstacles, and each of `spots` found in its triangle.
    pub fn new(board: &Board, layer: usize, vias: &[(P, u32)], spots: &Arc<Spots>) -> Space {
        let mut obstacles = Vec::new();
        let mut ends = HashMap::new();
        let opened = |opening: Option<(u64, Nm)>| opening.filter(|o| o.0 & (1 << layer) != 0).map_or(0.0, |o| (o.1 + board.mask_margin()) as f64 / 1e6);
        let add = |what, net, item: Option<ItemId>, shape: &Shape, obstacles: &mut Vec<Obstacle>| {
            let (poly, reach) = slim(shape).unwrap_or_else(|| (outline(shape), 0.0));
            if poly.len() >= 3 {
                let mask = if what == What::Pad { opened(item.and_then(|id| board.mask_openings.get(&id).copied())) } else { 0.0 };
                obstacles.push(Obstacle { what, net, item, centre: centre(&poly), poly, reach, mask });
            }
        };
        for (id, item) in board.items() {
            match &item.kind {
                Kind::Pad { .. } | Kind::Via if item.on(layer) => {
                    ends.insert(End::Item(id), obstacles.len());
                    add(What::Pad, item.net, Some(id), &item.shape, &mut obstacles);
                }
                Kind::Track if item.on(layer) => add(What::Copper, item.net, Some(id), &item.shape, &mut obstacles),
                Kind::Pour { .. } if item.on(layer) && !item.refilled() => add(What::Copper, item.net, Some(id), &item.shape, &mut obstacles),
                Kind::Keepout { forbids, .. } if forbids.tracks && item.on(layer) => add(What::Keepout, 0, Some(id), &item.shape, &mut obstacles),
                _ => {}
            }
            if item.hole.is_some()
                && !item.on(layer)
                && let Some(h) = board.hole_shape(id, item)
            {
                add(What::Hole, 0, Some(id), &h, &mut obstacles);
            }
        }
        // A pad joined to its pour by spokes must keep some of them whole. As many as must stay
        // are obstacles of the pour's net: those with the most room about them, so that the
        // ones a wire may cut are the ones in the narrow places wires need. The rest are left
        // out, and the checker counts what is cut.
        let mut spokes: HashMap<ItemId, Vec<(ItemId, &board::Item)>> = HashMap::new();
        for (id, item) in board.items() {
            if let Kind::Spoke { pad, .. } = item.kind
                && item.on(layer)
            {
                spokes.entry(pad).or_default().push((id, item));
            }
        }
        let mut pads: Vec<ItemId> = spokes.keys().copied().collect();
        pads.sort();
        let others: Vec<(ItemId, u32, board::BBox)> = board.items().filter(|(_, i)| matches!(i.kind, Kind::Pad { .. }) && i.on(layer)).filter_map(|(id, i)| Some((id, i.net, board.bbox(id)?))).collect();
        for pad in pads {
            let mut of = spokes.remove(&pad).unwrap_or_default();
            let Some(&(_, first)) = of.first() else { continue };
            let Kind::Spoke { need, clearance, .. } = first.kind else { continue };
            let room = |id: ItemId, net: u32| {
                let Some(b) = board.bbox(id) else { return 0.0 };
                let (x, y) = ((b.min.x + b.max.x) / 2, (b.min.y + b.max.y) / 2);
                others
                    .iter()
                    .filter(|o| o.0 != pad && o.1 != net)
                    .map(|o| {
                        let (dx, dy) = ((o.2.min.x - x).max(x - o.2.max.x).max(0) as f64, (o.2.min.y - y).max(y - o.2.max.y).max(0) as f64);
                        dx.hypot(dy)
                    })
                    .fold(f64::MAX, f64::min)
            };
            of.sort_by(|a, b| room(b.0, b.1.net).total_cmp(&room(a.0, a.1.net)).then(a.0.cmp(&b.0)));
            // A round pad is held as its middle line: a spoke starts at the pad's middle, and
            // only the part of it past the pad's edge is there to keep from.
            let round = board.item(pad).and_then(|p| slim(&p.shape)).map(|(line, reach)| (line[0], line[line.len() - 2], reach));
            for (id, item) in of.into_iter().take(need as usize) {
                let shape = match (&item.shape, round) {
                    (Shape::Polygon(c), Some((a, b, reach))) if c.len() == 4 => {
                        // Its corners run near, far, far, near: each near one is moved out
                        // along the spoke to the pad's edge.
                        let c: Vec<P> = c.iter().map(|&q| mm(q)).collect();
                        let out = |near: P, far: P| -> Option<P> {
                            let long = dist(near, far);
                            let at = |t: f64| [near[0] + (far[0] - near[0]) * t / long, near[1] + (far[1] - near[1]) * t / long];
                            let (mut lo, mut hi) = (0.0, long);
                            if point_segment(far, a, b) <= reach {
                                return None;
                            }
                            if point_segment(near, a, b) >= reach {
                                return Some(near);
                            }
                            for _ in 0..30 {
                                let mid = (lo + hi) / 2.0;
                                if point_segment(at(mid), a, b) < reach { lo = mid } else { hi = mid }
                            }
                            Some(at(hi))
                        };
                        let (Some(n0), Some(n3)) = (out(c[0], c[1]), out(c[3], c[2])) else { continue };
                        Shape::Polygon([n0, c[1], c[2], n3].iter().map(|q| board::Point::new((q[0] * 1e6).round() as i64, (q[1] * 1e6).round() as i64)).collect())
                    }
                    _ => item.shape.clone(),
                };
                let before = obstacles.len();
                add(What::Copper, item.net, Some(id), &shape, &mut obstacles);
                if let Some(o) = obstacles.get_mut(before) {
                    o.mask = clearance as f64 / 1e6;
                }
            }
        }
        let loops: Vec<Vec<P>> = board.outline.iter().map(|l| l.iter().map(|&p| mm(p)).collect()).collect();
        let first_edge = obstacles.len();
        for l in &loops {
            obstacles.push(Obstacle { what: What::Edge, net: 0, item: None, centre: centre(l), poly: l.clone(), reach: 0.0, mask: 0.0 });
        }
        let first_site = obstacles.len();
        for (k, &(at, net)) in vias.iter().enumerate() {
            ends.insert(End::Site(k), obstacles.len());
            let poly = (0..3).map(|i| {
                let a = std::f64::consts::PI * (0.5 + 2.0 * i as f64 / 3.0);
                [at[0] + VIA_CORNER * a.cos(), at[1] + VIA_CORNER * a.sin()]
            });
            obstacles.push(Obstacle { what: What::Pad, net, item: None, centre: at, poly: poly.collect(), reach: spots.radius + VIA_CORNER, mask: opened(Some(board.via_openings)) });
        }

        // Corners, one vertex per place (to a nanometre), and each polygon's sides as constraints.
        let mut cdt = ConstrainedDelaunayTriangulation::<Point2<f64>>::new();
        let mut at: HashMap<(i64, i64), spade::handles::FixedVertexHandle> = HashMap::new();
        let mut owner: HashMap<(i64, i64), usize> = HashMap::new();
        let key = |p: P| ((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64);
        for (k, o) in obstacles.iter().enumerate() {
            let hs: Vec<_> = o
                .poly
                .iter()
                .filter_map(|&p| {
                    owner.entry(key(p)).or_insert(k);
                    match at.get(&key(p)) {
                        Some(&h) => Some(h),
                        None => {
                            let h = cdt.insert(Point2::new(p[0], p[1])).ok()?;
                            at.insert(key(p), h);
                            Some(h)
                        }
                    }
                })
                .collect();
            for i in 0..hs.len() {
                let (a, b) = (hs[i], hs[(i + 1) % hs.len()]);
                if a != b {
                    cdt.add_constraint_and_split(a, b, |p| p);
                }
            }
        }

        let verts: Vec<P> = cdt.vertices().map(|v| [v.position().x, v.position().y]).collect();
        let vert_of = verts.iter().map(|&p| owner.get(&key(p)).copied()).collect();
        let mut edges: Vec<Edge> = cdt
            .undirected_edges()
            .map(|e| {
                let [a, b] = e.vertices();
                Edge { v: [a.index(), b.index()], t: [None, None], len: dist(verts[a.index()], verts[b.index()]) }
            })
            .collect();
        let mut tris = Vec::new();
        for f in cdt.inner_faces() {
            let v = f.vertices().map(|v| v.index());
            // adjacent_edges()[i] runs from v[i] to v[i+1]: it is opposite v[i+2].
            let ae = f.adjacent_edges().map(|e| e.as_undirected().index());
            let e = [ae[1], ae[2], ae[0]];
            let c = [(verts[v[0]][0] + verts[v[1]][0] + verts[v[2]][0]) / 3.0, (verts[v[0]][1] + verts[v[1]][1] + verts[v[2]][1]) / 3.0];
            // Off the board counts as inside its edge.
            let off = loops.first().is_some_and(|l| !inside(c, l)) || loops.iter().skip(1).any(|l| inside(c, l));
            let inside = if off {
                Some(first_edge)
            } else {
                obstacles.iter().position(|o| o.what != What::Edge && inside(c, &o.poly))
            };
            for &x in &e {
                let slot = if edges[x].t[0].is_none() { 0 } else { 1 };
                edges[x].t[slot] = Some(tris.len());
            }
            tris.push(Tri { v, e, inside });
        }
        let sites: Vec<Site> = (0..vias.len()).map(|k| Site { obstacle: first_site + k }).collect();
        // Each spot in its triangle: by the triangles' boxes first, as there are many of both.
        let mut spot_tri = vec![None; spots.at.len()];
        let mut tri_spots = vec![Vec::new(); tris.len()];
        let free: Vec<(usize, [P; 3])> = tris.iter().enumerate().filter(|(_, t)| t.inside.is_none()).map(|(k, t)| (k, t.v.map(|v| verts[v]))).collect();
        for (k, &p) in spots.at.iter().enumerate() {
            if vias.iter().any(|&(at, _)| dist(at, p) < spots.apart) {
                continue;
            }
            let within = free.iter().find(|(_, c)| {
                (0..2).all(|i| p[i] >= c[0][i].min(c[1][i]).min(c[2][i]) && p[i] <= c[0][i].max(c[1][i]).max(c[2][i]))
                    && (0..3).all(|i| {
                        let (a, b, o) = (c[i], c[(i + 1) % 3], c[(i + 2) % 3]);
                        let side = |q: P| (b[0] - a[0]) * (q[1] - a[1]) - (b[1] - a[1]) * (q[0] - a[0]);
                        side(p) * side(o) > 0.0 && side(p).abs() >= SPOT_MARGIN * dist(a, b)
                    })
            });
            if let Some(&(t, _)) = within {
                spot_tri[k] = Some(t);
                tri_spots[t].push(k);
            }
        }
        Space { layer, obstacles, verts, vert_of, tris, edges, ends, sites, spots: spots.clone(), spot_tri, tri_spots }
    }

    /// Whether an edge is a gap: not a side of an obstacle, which no strand passes along.
    pub fn gap(&self, e: usize) -> bool {
        let [a, b] = self.edges[e].v.map(|v| self.vert_of[v]);
        a.is_none() || a != b
    }

    /// Whether there is room on both sides of an edge: a strand that crosses it is passing
    /// between two things, not coming onto the one it ends on.
    pub fn passed(&self, e: usize) -> bool {
        self.edges[e].t.iter().all(|t| t.is_some_and(|t| self.free(t)))
    }

    /// The room across a gap for wires in the eight directions, mm. Wires that cross a gap
    /// square on are as near each other along it as they may be; wires at a slant to it take
    /// more of it each. So a gap that lies along none of the eight directions holds less than
    /// its length: its length as seen along the direction that crosses it most nearly square.
    pub fn width(&self, e: usize) -> f64 {
        let edge = &self.edges[e];
        let (a, b) = (self.verts[edge.v[0]], self.verts[edge.v[1]]);
        let (x, y) = ((b[0] - a[0]).abs(), (b[1] - a[1]).abs());
        x.max(y).max((x + y) * std::f64::consts::FRAC_1_SQRT_2)
    }

    /// The plan's vias here: the middle and the net of each.
    pub fn vias(&self) -> Vec<(P, u32)> {
        self.sites.iter().map(|s| (self.obstacles[s.obstacle].centre, self.obstacles[s.obstacle].net)).collect()
    }

    /// The middle of a triangle.
    pub fn middle(&self, t: usize) -> P {
        let [a, b, c] = self.tris[t].v.map(|v| self.verts[v]);
        [(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0]
    }

    /// Whether a strand may be in this triangle at all.
    pub fn free(&self, t: usize) -> bool {
        self.tris[t].inside.is_none()
    }

    /// What copper of `net` must keep from the corner `v`, edge to edge, mm.
    pub fn keep(&self, board: &Board, v: usize, net: u32) -> f64 {
        // A corner where two obstacles' sides were found to cross is no object.
        let Some(k) = self.vert_of[v] else { return 0.0 };
        let o = &self.obstacles[k];
        match o.what {
            What::Edge => board.rules.min_copper_edge_clearance * 1e3,
            What::Hole => board.rules.min_hole_clearance * 1e3 + o.reach,
            What::Keepout => 0.0,
            What::Pad | What::Copper if o.net == net && net != 0 => 0.0,
            // A pad that asks for more than its net does is given it, as the checker gives it.
            What::Pad | What::Copper => {
                let own = o.item.and_then(|i| board.pad_clearances.get(&i).copied()).unwrap_or(0);
                (board.clearance(net, o.net).max(own) as f64 / 1e6).max(o.mask) + o.reach
            }
        }
    }
}
