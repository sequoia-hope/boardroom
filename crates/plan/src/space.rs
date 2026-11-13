//! The room on one copper layer: the obstacles there (pads, fixed copper, holes, keepouts, the
//! board's edge) as polygons, and a constrained triangulation of their corners. A triangle edge
//! between two obstacles is a gap; its room is what the rules leave between them.

use std::collections::HashMap;

use board::{Board, ItemId, Kind, Point, Shape};
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

/// Where a strand may end: a pad or via of the board, or a via site the plan has taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum End {
    Item(ItemId),
    Site(usize),
}

/// A place a via could stand, the same on every layer: a small polygon in the triangulation
/// that strands may cross until a via takes it.
#[derive(Debug, Clone, Serialize)]
pub struct Site {
    pub obstacle: usize,
    /// The triangles inside it, and the edges those have (its sides and what joins them).
    pub tris: Vec<usize>,
    pub edges: Vec<usize>,
    /// The edges outside it that end on one of its corners: a via here narrows them.
    pub around: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum What {
    /// A pad, or a via: a strand of its net may end on it.
    Pad,
    /// A via site no via has taken: no obstacle yet.
    Site,
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
}

#[derive(Debug, Clone, Serialize)]
pub struct Tri {
    pub v: [usize; 3],
    /// `e[i]` is the edge opposite `v[i]`.
    pub e: [usize; 3],
    /// The obstacle this triangle lies in, if any (a via site only once a via has taken it).
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
    /// The via sites, in the plan's order: the same sites on every layer.
    pub sites: Vec<Site>,
    /// The site each triangle lies in, if any.
    #[serde(skip)]
    pub tri_site: Vec<Option<usize>>,
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
    /// board first: see [`crate::strip`]).
    pub fn new(board: &Board, layer: usize, sites: &[Vec<P>]) -> Space {
        let mut obstacles = Vec::new();
        let mut ends = HashMap::new();
        let add = |what, net, item: Option<ItemId>, shape: &Shape, obstacles: &mut Vec<Obstacle>| {
            let poly = outline(shape);
            if poly.len() >= 3 {
                obstacles.push(Obstacle { what, net, item, centre: centre(&poly), poly });
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
        let loops: Vec<Vec<P>> = board.outline.iter().map(|l| l.iter().map(|&p| mm(p)).collect()).collect();
        let first_edge = obstacles.len();
        for l in &loops {
            obstacles.push(Obstacle { what: What::Edge, net: 0, item: None, centre: centre(l), poly: l.clone() });
        }
        let first_site = obstacles.len();
        for (k, poly) in sites.iter().enumerate() {
            ends.insert(End::Site(k), obstacles.len());
            obstacles.push(Obstacle { what: What::Site, net: 0, item: None, centre: centre(poly), poly: poly.clone() });
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
        let mut in_site: Vec<(usize, usize)> = Vec::new();
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
                obstacles[..first_edge].iter().position(|o| inside(c, &o.poly))
            };
            if inside.is_none()
                && let Some(k) = sites.iter().position(|poly| crate::space::inside(c, poly))
            {
                in_site.push((tris.len(), k));
            }
            for &x in &e {
                let slot = if edges[x].t[0].is_none() { 0 } else { 1 };
                edges[x].t[slot] = Some(tris.len());
            }
            tris.push(Tri { v, e, inside });
        }
        let vert_of: Vec<Option<usize>> = vert_of;
        let mut site_list: Vec<Site> = (0..sites.len()).map(|k| Site { obstacle: first_site + k, tris: Vec::new(), edges: Vec::new(), around: Vec::new() }).collect();
        for &(t, k) in &in_site {
            site_list[k].tris.push(t);
            for e in tris[t].e {
                if !site_list[k].edges.contains(&e) {
                    site_list[k].edges.push(e);
                }
            }
        }
        for (e, edge) in edges.iter().enumerate() {
            for v in edge.v {
                if let Some(o) = vert_of[v].filter(|&o| o >= first_site)
                    && !site_list[o - first_site].edges.contains(&e)
                    && !site_list[o - first_site].around.contains(&e)
                {
                    site_list[o - first_site].around.push(e);
                }
            }
        }
        let mut tri_site = vec![None; tris.len()];
        for &(t, k) in &in_site {
            tri_site[t] = Some(k);
        }
        Space { layer, obstacles, verts, vert_of, tris, edges, ends, sites: site_list, tri_site }
    }

    /// The site's triangles become a via of `net`, or (`None`) room again.
    pub fn take_site(&mut self, site: usize, net: Option<u32>) {
        let o = self.sites[site].obstacle;
        self.obstacles[o].what = if net.is_some() { What::Pad } else { What::Site };
        self.obstacles[o].net = net.unwrap_or(0);
        for t in self.sites[site].tris.clone() {
            self.tris[t].inside = net.map(|_| o);
        }
    }

    /// Whether an edge is only scaffolding: both its corners on one via site that no via has
    /// taken. It is no gap, and holds any number of strands.
    pub fn scaffold(&self, e: usize) -> bool {
        let [a, b] = self.edges[e].v.map(|v| self.vert_of[v]);
        a == b && a.is_some_and(|o| self.obstacles[o].what == What::Site)
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

    /// What copper of `net` must keep from the corner `v`, edge to edge, mm. `via` is a site
    /// taken to hold a via of a net, to try it before it does.
    pub fn keep(&self, board: &Board, v: usize, net: u32, via: Option<(usize, u32)>) -> f64 {
        // A corner where two obstacles' sides were found to cross is no object.
        let Some(k) = self.vert_of[v] else { return 0.0 };
        let o = &self.obstacles[k];
        if let Some((site, of)) = via
            && self.sites[site].obstacle == k
        {
            return if of == net && net != 0 { 0.0 } else { board.clearance(net, of) as f64 / 1e6 };
        }
        match o.what {
            What::Site => 0.0,
            What::Edge => board.rules.min_copper_edge_clearance * 1e3,
            What::Hole => board.rules.min_hole_clearance * 1e3,
            What::Keepout => 0.0,
            What::Pad | What::Copper if o.net == net && net != 0 => 0.0,
            What::Pad | What::Copper => board.clearance(net, o.net) as f64 / 1e6,
        }
    }
}
