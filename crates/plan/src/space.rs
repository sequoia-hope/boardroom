//! The room on one copper layer: the obstacles there (pads, fixed copper, holes, keepouts, the
//! board's edge) as polygons, and a constrained triangulation of their corners. A triangle edge
//! between two obstacles is a gap; its room is what the rules leave between them.

use std::collections::HashMap;
use std::sync::Arc;

use board::{Board, ItemId, Kind, Nm, Point, Shape};
use serde::Serialize;
use spade::{ConstrainedDelaunayTriangulation, Point2, Triangulation};

/// A map whose keys are a few small numbers: hashed with a multiply and a turn each, which
/// is all such keys need. A search looks its places up millions of times, and the standard
/// hash, made to withstand keys chosen to collide, was a good part of what a search took.
pub(crate) type Map<K, V> = HashMap<K, V, std::hash::BuildHasherDefault<Quick>>;

#[derive(Default)]
pub(crate) struct Quick(u64);

impl std::hash::Hasher for Quick {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(b as u64);
        }
    }
    fn write_u64(&mut self, x: u64) {
        self.0 = (self.0.rotate_left(5) ^ x).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
    fn write_u32(&mut self, x: u32) {
        self.write_u64(x as u64);
    }
    fn write_u8(&mut self, x: u8) {
        self.write_u64(x as u64);
    }
    fn write_usize(&mut self, x: usize) {
        self.write_u64(x as u64);
    }
    fn write_isize(&mut self, x: isize) {
        self.write_u64(x as u64);
    }
}

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
    /// What a track of another net keeps from a via's copper for the sake of its hole, mm
    /// ([`Obstacle::hole`]).
    pub hole: f64,
    /// The way straight out of each pad of a fine-pitch part: from the pad's outer end, a
    /// little way on, with the pad's net. No via of another net stands where a track along it
    /// could not pass ([`Spots::shuts`]): a pin between two others has no other way out, and
    /// a via, unlike a strand, is not lifted to let it through.
    pub ways_out: Vec<(P, P, u32)>,
    /// How near the middle of a via of another net may be to a way out, mm.
    pub out_keep: f64,
    /// A via of the plan nearer a spot than this, mm, may have a say in whether a via may
    /// stand there: two vias' copper and the farthest any rule of the board reaches (a net
    /// class may ask more clearance than the least two vias stand apart at).
    pub beside: f64,
    /// What the exact checker says of a via of each net at each spot on the bare board
    /// ([`Spots::alone`]), for each asked about so far.
    pub checked: Checked,
    /// The board's rule areas that bar the tracks of some nets and let others through, worked
    /// out once for the plan: the triangulation is made again whenever a via goes in or out.
    pub bars: Vec<Bar>,
    /// The spots that stand in a pour that must stay joined ([`Board::kept_joined`]), each with
    /// those pours by their numbers there: a via of another net there costs [`KEPT_VIA`] more.
    pub kept: std::collections::HashMap<usize, Vec<usize>>,
    /// The sides the triangulation could not take ([`Space::new`]), found so far.
    pub left_out: LeftOut,
    /// Pins with a hole, each with a layer on which its spokes are no obstacles: they shut
    /// another pad in there, and the pin has spokes enough on another layer (KiCad leaves a
    /// pad no spoke reaches on a layer be, and so does the checker while another layer joins
    /// it). A wire may run over them all.
    pub spared: Vec<(ItemId, usize)>,
    /// Pads with a hole in them, each with the place inside it a via of its net stands, and the
    /// net ([`crate::hole`]): the plan's first vias, in this order. A
    /// connection of the pad may start or end at its hole on any other layer; one no strand
    /// ends at is not drilled.
    pub holes: Vec<(ItemId, P, u32)>,
    /// The pads a hole has been tried for ([`crate::hole`]), whether or not one could stand.
    pub wanted: Vec<ItemId>,
    /// The pads whose own middle is no place for a track to end, each with the place its
    /// tracks end at instead ([`middles`]): a pad half past the board's edge.
    pub middles: std::collections::HashMap<ItemId, P>,
}

/// For each pad whose middle lies off the board, or nearer its edge than the end of a track
/// may: the middle of the part of its copper that is on the board and that far in. A
/// castellated pad is half past the edge, its middle on the edge or beyond it (EnvOpenPico's
/// two rows of header pins, test set, 2026-11-22: their middles 0.05 mm outside the board, and
/// 17 of the first plan's 88 strands could not be drawn for starting there). A pad with no
/// such part is left out, and keeps its middle.
pub fn middles(board: &Board) -> std::collections::HashMap<ItemId, P> {
    let edge: Vec<Vec<P>> = bounds(board).iter().map(|l| l.iter().map(|&q| mm(q)).collect()).collect();
    let mut out = std::collections::HashMap::new();
    let Some(outer) = edge.first() else { return out };
    let (lo, hi) = outer.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
    for (id, item) in board.items() {
        if !matches!(item.kind, Kind::Pad { .. }) || item.net == 0 {
            continue;
        }
        let keep = board.rules.min_copper_edge_clearance + crate::width(board, item.net) / 2.0 + 0.02;
        let poly = outline(&item.shape);
        if poly.len() < 3 {
            continue;
        }
        let c = centre(&poly);
        let deep = |p: P| p[0] - lo[0] > keep && hi[0] - p[0] > keep && p[1] - lo[1] > keep && hi[1] - p[1] > keep;
        let from_edge = |p: P| edge.iter().flat_map(|l| (0..l.len()).map(move |i| point_segment(p, l[i], l[(i + 1) % l.len()]))).fold(f64::MAX, f64::min);
        let on = |p: P| inside(p, outer) && !edge[1..].iter().any(|h| inside(p, h)) && from_edge(p) >= keep;
        let (plo, phi) = poly.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
        // (A pad's middle well inside the box round the board is tried against the outline
        // itself only where its own box is not: the look at every side of the outline is the
        // dear part, and most pads are nowhere near the edge.)
        if (deep(plo) && deep(phi) && inside(c, outer)) || on(c) {
            continue;
        }
        const N: usize = 12;
        let good: Vec<P> = (0..=N).flat_map(|i| (0..=N).map(move |j| (i, j))).map(|(i, j)| [plo[0] + (phi[0] - plo[0]) * i as f64 / N as f64, plo[1] + (phi[1] - plo[1]) * j as f64 / N as f64]).filter(|&p| inside(p, &poly) && on(p)).collect();
        if good.is_empty() {
            continue;
        }
        let mid = centre(&good);
        let nearest = good.iter().copied().min_by(|a, b| dist(*a, mid).total_cmp(&dist(*b, mid))).unwrap_or(mid);
        out.insert(id, nearest);
    }
    out
}

/// For each layer, the sides of the board's own obstacles and of its rule areas that the
/// triangulation could not take, each as whether it is a rule area's, which obstacle or area,
/// and which side: found once, and left out of every map of the layer made after.
/// A copy of the spots starts with none of it.
#[derive(Debug, Default)]
pub struct LeftOut(std::sync::Mutex<Map<usize, Vec<Side>>>);

/// A side by where it lasts from one map to the next: whether it is a rule area's, which
/// obstacle or area it is of, and which of its sides.
type Side = (bool, usize, usize);

impl Clone for LeftOut {
    fn clone(&self) -> LeftOut {
        LeftOut::default()
    }
}

/// What a via of a net not let cut it costs in a pour that must stay joined, beyond any other
/// via, as length, mm: a long way round, taken only where nothing else is (the checker still
/// says whether the pour stays joined with it there).
pub const KEPT_VIA: f64 = 30.0;

/// A rule area that bars the tracks of some nets ([`Board::nets_barred`]): its outline, mm, and
/// for each copper layer the nets it bars there (sorted; empty on a layer it is not on).
#[derive(Debug, Clone)]
pub struct Bar {
    pub poly: Vec<P>,
    pub nets: Vec<Vec<u32>>,
}

/// The rule areas of `board` that bar some net's tracks on some layer.
pub fn bars(board: &Board) -> Vec<Bar> {
    if !board.custom.has("disallow") {
        return Vec::new();
    }
    board
        .areas
        .iter()
        .enumerate()
        .filter_map(|(k, (_, shape, layers))| {
            let Shape::Polygon(outline) = shape else { return None };
            let nets: Vec<Vec<u32>> = (0..board.layers.len()).map(|l| if layers & (1 << l) != 0 { board.nets_barred("track", k, l) } else { Vec::new() }).collect();
            nets.iter().any(|n| !n.is_empty()).then(|| Bar { poly: outline.iter().map(|&p| mm(p)).collect(), nets })
        })
        .collect()
}

/// For a spot and a net: whether the exact checker lets a via of that net stand there on the
/// bare board, and whether a thermal spoke is near it. And the bare board to try them on.
/// A copy of the spots starts with none of it: its places may be others.
#[derive(Debug, Default)]
pub struct Checked(std::sync::Mutex<Alone>);

/// What [`Checked`] holds: the answers so far, and the bare board to try a via on.
#[derive(Debug, Default)]
struct Alone {
    known: Map<(usize, u32), (bool, bool)>,
    bare: Option<Board>,
}

impl Clone for Checked {
    fn clone(&self) -> Checked {
        Checked::default()
    }
}

/// How far from a via a thermal spoke is looked for, nm: farther than a via's copper and any
/// clearance reach.
const SPOKE_NEAR: Nm = 1_000_000;

impl Spots {
    /// What a via of `net` at spot `c` costs beyond any via's: [`KEPT_VIA`] in a pour that must
    /// stay joined and that the net may not cut.
    pub fn dearer(&self, board: &Board, c: usize, net: u32) -> f64 {
        let Some(kept) = self.kept.get(&c) else { return 0.0 };
        if kept.iter().any(|&k| board.kept_joined.get(k).is_some_and(|j| j.net != net && !j.cut_by.contains(&net))) { KEPT_VIA } else { 0.0 }
    }

    /// The spots of `at` in each pour of the board's that must stay joined.
    pub fn kept_of(board: &Board, at: &[P]) -> std::collections::HashMap<usize, Vec<usize>> {
        let mut out: std::collections::HashMap<usize, Vec<usize>> = Default::default();
        for (k, j) in board.kept_joined.iter().enumerate() {
            for (c, p) in at.iter().enumerate() {
                let q = board::Point::new((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64);
                if j.outline.contains(q) {
                    out.entry(c).or_default().push(k);
                }
            }
        }
        out
    }

    /// Whether the exact checker lets `via`, a via of `net` at spot `c`, stand on the bare
    /// board, and whether what it says could be otherwise with the plan's other vias on the
    /// board wherever they stand: where a pad's thermal spoke is near, of which two vias can
    /// together cut more than one may, where an opening drawn in the mask is near, which
    /// takes copper of one net only, or where vias are bare in the mask, and keep more from
    /// each other than their copper asks. (A via of the plan that stands within
    /// [`Spots::beside`] has a say besides: that is for the asker to see to.) Worked out
    /// once for each spot and net: a search asks it of every spot it comes by, and a plan
    /// is thousands of searches.
    pub fn alone(&self, board: &Board, c: usize, net: u32, via: &dyn Fn() -> board::Item) -> (bool, bool) {
        let Ok(mut alone) = self.checked.0.lock() else { return (true, true) };
        if let Some(&was) = alone.known.get(&(c, net)) {
            return was;
        }
        let bare = alone.bare.get_or_insert_with(|| board.clone());
        let item = via();
        let about = item.shape.bbox().grow(SPOKE_NEAR);
        let spoked = board.via_openings.0 != 0
            || board.mask_areas.iter().any(|(shape, ..)| about.overlaps(shape.bbox()))
            || (0..bare.layers.len()).any(|l| bare.near(l, about).into_iter().any(|id| bare.item(id).is_some_and(|i| matches!(i.kind, Kind::Spoke { .. }))));
        let id = bare.insert(item);
        let fits = board::check_item(bare, id).is_empty();
        bare.discard(id);
        alone.known.insert((c, net), (fits, spoked));
        (fits, spoked)
    }

    /// Whether a via of `net` at `p` would shut the way out of a pad of another net.
    pub fn shuts(&self, p: P, net: u32) -> bool {
        self.ways_out.iter().any(|&(a, b, of)| of != net && point_segment(p, a, b) < self.out_keep)
    }
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
    /// How far its copper stands out past the polygon, mm: a shape of many corners (a pad
    /// with round corners) is held as a simpler one, and the corners left out lie outside it.
    pub past: f64,
    /// What a track of another net keeps from its copper so as to keep the rules' distance
    /// from the hole drilled in it, mm: what the copper round the hole is narrower than that
    /// distance by, and nothing where it has no hole. A small via's ring is narrower than the
    /// hole clearance, and a track that keeps only its clearance from the ring is too near
    /// the hole.
    pub hole: f64,
}

/// [`Obstacle::hole`] for copper `shape` with a hole of radius `r` between `a` and `b` in it,
/// mm.
pub fn hole_keep(board: &Board, shape: &Shape, (a, b, r): (Point, Point, Nm)) -> f64 {
    // The least copper from the hole's edge to the shape's.
    let ring = [a, b]
        .iter()
        .map(|&h| match shape {
            Shape::Capsule { a, b, r } => *r as f64 / 1e6 - point_segment(mm(h), mm(*a), mm(*b)),
            Shape::Polygon(p) if shape.contains(h) => (0..p.len()).map(|i| point_segment(mm(h), mm(p[i]), mm(p[(i + 1) % p.len()]))).fold(f64::MAX, f64::min),
            Shape::Polygon(_) => 0.0,
        })
        .fold(f64::MAX, f64::min)
        - r as f64 / 1e6;
    (board.rules.min_hole_clearance * 1e3 - ring.max(0.0)).max(0.0)
}

#[derive(Debug, Clone, Serialize)]
pub struct Tri {
    pub v: [usize; 3],
    /// `e[i]` is the edge opposite `v[i]`.
    pub e: [usize; 3],
    /// The obstacle this triangle lies in, if any.
    pub inside: Option<usize>,
    /// The rule areas ([`Spots::bars`], by bit, the first 64) this triangle lies in.
    pub bars: u64,
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
    /// For each edge, the most room it has whatever its length: how far one of its corners is
    /// from a side of an obstacle that starts at its other corner and leans over it
    /// ([`Space::leant_on`]).
    #[serde(skip)]
    cap: Vec<f64>,
    /// The same for each side that leans over the edge, with the obstacle the side is of: a
    /// strand of that obstacle's own net is not kept off it ([`Space::width_for`]).
    #[serde(skip)]
    caps: Vec<Vec<(f64, usize)>>,
    /// For each corner, the far corners of the obstacle sides that meet at it.
    #[serde(skip)]
    sides_at: Vec<Vec<usize>>,
    /// For each corner, the rule areas ([`Spots::bars`], by bit) it is a corner of.
    #[serde(skip)]
    bar_of: Vec<u64>,
}

/// What a track keeps from a corner of a rule area that bars its net, mm.
const AREA_KEEP: f64 = 0.01;

/// How far out from everything on it a board with no edge drawn at all is taken to reach, nm.
const FRAME: i64 = 2_000_000;

/// The loops that bound the room on a board: its outline with its cut-outs, or, where its
/// edge is not drawn as loops the board model could take, what of it does close
/// ([`closed_loops`]) within the least convex shape round every line of the edge and
/// everything on the board (a box round it all, and a little more, where no edge is drawn at
/// all). Without it the room would be what lies between the outermost pads of each layer, and
/// a layer with pads in one row would have none.
pub fn bounds(board: &Board) -> Vec<Vec<Point>> {
    if !board.outline.is_empty() {
        return board.outline.clone();
    }
    let hull = hull(board);
    // The edge is drawn, but not as loops the board model could take: an end a little short
    // of the next line's, or a place where more than two lines meet, and it has none at all.
    // What does close is taken all the same: an outline, where one closes round most of the
    // board, and the cut-outs inside it (pinci's seventeen switch cut-outs, 2026-11-20: the
    // outline branches at one corner, and with no loop the plan ran its strands across
    // every cut-out for the checker to refuse).
    let area = |l: &[Point]| (0..l.len()).map(|i| l[i].x as f64 * l[(i + 1) % l.len()].y as f64 - l[(i + 1) % l.len()].x as f64 * l[i].y as f64).sum::<f64>().abs() / 2.0;
    let Some(around) = hull.first() else { return hull };
    // An edge drawn as thousands of short lines is as many corners in the map of every layer
    // (ULK, 2026-11-20: 5,962 of them, and the board no longer routed in its time).
    let fewer = |l: Vec<Point>| -> Vec<Point> { simpler(&l.iter().map(|&p| mm(p)).collect::<Vec<P>>(), FINE).iter().map(|p| Point::new((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64)).collect() };
    let mut closed: Vec<(f64, Vec<Point>)> = closed_loops(board).into_iter().map(fewer).map(|l| (area(&l), l)).collect();
    closed.sort_by(|a, b| b.0.total_cmp(&a.0));
    let whole = area(around);
    let outer = match closed.first() {
        Some((a, _)) if *a >= OUTLINE_SHARE * whole => closed.remove(0).1,
        _ => around.clone(),
    };
    // A cut-out has no pad in it: a loop with pads in it is a board of its own, one of a
    // panel's or a part of this one drawn round, and its pads are to be reached.
    let pads: Vec<Point> = board.items().filter(|(_, i)| matches!(i.kind, Kind::Pad { .. })).filter_map(|(id, _)| board.bbox(id)).map(|b| Point::new((b.min.x + b.max.x) / 2, (b.min.y + b.max.y) / 2)).collect();
    let cut_out = |l: &[Point]| l.iter().all(|&p| board::inside(p, &outer)) && !pads.iter().any(|&p| board::inside(p, l));
    let cuts: Vec<Vec<Point>> = closed.into_iter().map(|(_, l)| l).filter(|l| cut_out(l)).collect();
    std::iter::once(outer).chain(cuts).collect()
}

/// How much of the area round everything on the board a closed loop of its edge must take up
/// to be the board's outline.
const OUTLINE_SHARE: f64 = 0.5;

/// How near each other two loose ends of edge lines are taken for one place, nm: nearer than
/// any copper could pass between.
const SAME_PLACE: i64 = 100_000;

/// The loops the lines of the board's edge close into, each as its corners: a set of lines
/// joined end to end, two at every end, a loose end (one no other line ends at) taken as one
/// place with every end within [`SAME_PLACE`] of it. Lines that do not close, or branch, are
/// in none.
pub fn closed_loops(board: &Board) -> Vec<Vec<Point>> {
    let mut places: Vec<Point> = Vec::new();
    let mut known: HashMap<(i64, i64), usize> = HashMap::new();
    let mut place = |p: Point| {
        *known.entry((p.x, p.y)).or_insert_with(|| {
            places.push(p);
            places.len() - 1
        })
    };
    let tidy = |mut lines: Vec<(usize, usize)>| {
        lines.retain(|(a, b)| a != b);
        for l in &mut lines {
            *l = (l.0.min(l.1), l.0.max(l.1));
        }
        lines.sort_unstable();
        lines.dedup();
        lines
    };
    let mut lines = tidy(board.outline_runs.iter().flat_map(|run| run.windows(2)).map(|w| (place(w[0]), place(w[1]))).collect());
    // A loose end is one place with every end within reach of it: an arc's end computed a
    // hair off the line it meets, a sliver of a line between the two, a line run a hair past
    // a corner. Ends that are not loose stay apart from each other however near: a curve
    // drawn as many short lines keeps its corners.
    let mut ends = vec![0usize; places.len()];
    for &(a, b) in &lines {
        ends[a] += 1;
        ends[b] += 1;
    }
    let cell = |p: Point| (p.x.div_euclid(SAME_PLACE), p.y.div_euclid(SAME_PLACE));
    let mut near: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (k, &p) in places.iter().enumerate() {
        near.entry(cell(p)).or_default().push(k);
    }
    let mut as_one: Vec<usize> = (0..places.len()).collect();
    fn first(as_one: &mut [usize], mut k: usize) -> usize {
        while as_one[k] != k {
            as_one[k] = as_one[as_one[k]];
            k = as_one[k];
        }
        k
    }
    for a in (0..places.len()).filter(|&k| ends[k] == 1) {
        let (cx, cy) = cell(places[a]);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for &b in near.get(&(cx + dx, cy + dy)).into_iter().flatten() {
                    if b != a && places[a].distance(places[b]) <= SAME_PLACE as f64 {
                        let (x, y) = (first(&mut as_one, a), first(&mut as_one, b));
                        as_one[x.max(y)] = x.min(y);
                    }
                }
            }
        }
    }
    let as_one: Vec<usize> = (0..places.len()).map(|k| first(&mut as_one, k)).collect();
    lines = tidy(lines.into_iter().map(|(a, b)| (as_one[a], as_one[b])).collect());
    let mut at: Vec<Vec<usize>> = vec![Vec::new(); places.len()];
    for &(a, b) in &lines {
        at[a].push(b);
        at[b].push(a);
    }
    let mut seen = vec![false; places.len()];
    let mut loops = Vec::new();
    for start in 0..places.len() {
        if seen[start] || at[start].len() != 2 {
            continue;
        }
        // Round from one place to the next, while each has two lines, back to the first.
        let (mut walk, mut from, mut here) = (vec![start], start, at[start][0]);
        seen[start] = true;
        let closes = loop {
            if here == start {
                break true;
            }
            if seen[here] || at[here].len() != 2 {
                break false;
            }
            seen[here] = true;
            walk.push(here);
            let next = if at[here][0] == from { at[here][1] } else { at[here][0] };
            (from, here) = (here, next);
        };
        if closes && walk.len() >= 3 {
            loops.push(walk.into_iter().map(|k| places[k]).collect());
        }
    }
    loops
}

/// The least convex shape round every line of the board's edge and everything on the board (a
/// box round it all, and a little more, where no edge is drawn at all).
fn hull(board: &Board) -> Vec<Vec<Point>> {
    let corners = |b: board::BBox| [b.min, Point::new(b.max.x, b.min.y), b.max, Point::new(b.min.x, b.max.y)];
    let things = board.items().filter_map(|(id, _)| board.bbox(id));
    if board.outline_edges.is_empty() {
        return things.reduce(|a, b| a.union(b)).map(|b| corners(b.grow(FRAME)).to_vec()).into_iter().collect();
    }
    let mut points: Vec<Point> = board.outline_edges.iter().flat_map(|&(a, b, _, _)| [a, b]).chain(things.flat_map(corners)).collect();
    points.sort_by_key(|p| (p.x, p.y));
    points.dedup();
    if points.len() < 3 {
        return Vec::new();
    }
    // The hull, one chain along the bottom and one back along the top.
    let turn = |o: Point, a: Point, b: Point| (a.x - o.x) as i128 * (b.y - o.y) as i128 - (a.y - o.y) as i128 * (b.x - o.x) as i128;
    let mut hull: Vec<Point> = Vec::new();
    for half in [points.clone(), points.iter().rev().copied().collect()] {
        let start = hull.len();
        for p in half {
            while hull.len() >= start + 2 && turn(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0 {
                hull.pop();
            }
            hull.push(p);
        }
        hull.pop();
    }
    vec![hull]
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

/// How many sides at most are left out of a layer's map before its fault is taken for another.
const LEFT_OUT: usize = 32;

/// `work`, and what it panicked with if it did, without the panic's message printed: for work
/// whose panic is expected and answered. Other threads' panics are printed as ever.
fn quietly<T>(work: impl FnOnce() -> T) -> std::thread::Result<T> {
    thread_local! {
        static QUIET: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    static HOOK: std::sync::Once = std::sync::Once::new();
    HOOK.call_once(|| {
        let before = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if !QUIET.with(|q| q.get()) {
                before(info);
            }
        }));
    });
    QUIET.with(|q| q.set(true));
    let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work));
    QUIET.with(|q| q.set(false));
    out
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
                // Copper with a hole in it: a pad's, a via's.
                let drilled = item.filter(|_| what != What::Hole).and_then(|id| match board.hole_shape(id, board.item(id)?)? {
                    Shape::Capsule { a, b, r } => Some((a, b, r)),
                    Shape::Polygon(_) => None,
                });
                let hole = drilled.map_or(0.0, |h| hole_keep(board, shape, h));
                // The corners its simpler outline leaves out, and how far the farthest is.
                let past = match shape {
                    Shape::Polygon(all) if reach == 0.0 && all.len() > poly.len() => all
                        .iter()
                        .map(|&q| mm(q))
                        .filter(|&q| !inside(q, &poly))
                        .map(|q| (0..poly.len()).map(|i| point_segment(q, poly[i], poly[(i + 1) % poly.len()])).fold(f64::MAX, f64::min))
                        .fold(0.0, f64::max),
                    _ => 0.0,
                };
                let middle = item.filter(|_| what == What::Pad).and_then(|id| spots.middles.get(&id).copied()).unwrap_or_else(|| centre(&poly));
                obstacles.push(Obstacle { what, net, item, centre: middle, poly, reach, mask, hole, past });
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
        // out, and the checker counts what is cut. A pin with a hole whose spokes on this layer
        // shut another pad in is given none here ([`Spots::spared`]): another layer joins it.
        let mut spokes: HashMap<ItemId, Vec<(ItemId, &board::Item)>> = HashMap::new();
        for (id, item) in board.items() {
            if let Kind::Spoke { pad, .. } = item.kind
                && item.on(layer)
                && !spots.spared.contains(&(pad, layer))
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
        let loops: Vec<Vec<P>> = bounds(board).iter().map(|l| l.iter().map(|&p| mm(p)).collect()).collect();
        let first_edge = obstacles.len();
        for l in &loops {
            obstacles.push(Obstacle { what: What::Edge, net: 0, item: None, centre: centre(l), poly: l.clone(), reach: 0.0, mask: 0.0, hole: 0.0, past: 0.0 });
        }
        let first_site = obstacles.len();
        for (k, &(at, net)) in vias.iter().enumerate() {
            ends.insert(End::Site(k), obstacles.len());
            let poly = (0..3).map(|i| {
                let a = std::f64::consts::PI * (0.5 + 2.0 * i as f64 / 3.0);
                [at[0] + VIA_CORNER * a.cos(), at[1] + VIA_CORNER * a.sin()]
            });
            obstacles.push(Obstacle { what: What::Pad, net, item: None, centre: at, poly: poly.collect(), reach: crate::via_size(board, net).0 as f64 / 1e6 + VIA_CORNER, mask: opened(Some(board.via_openings)), hole: spots.hole, past: 0.0 });
        }

        // Corners, one vertex per place (to a nanometre), and each polygon's sides as constraints.
        // The triangulation cannot take every side: it stops (a panic of its own) on some that
        // run along or end on another's, as where a pad is drawn as shapes that overlap with
        // sides in common, or an outline runs back along itself (eternal-keypad, urchin and
        // KLOR, of the 631 gate boards, 2026-11-20). The map is then made again without that
        // side, which is no wall in it: the checker still refuses a wire across it.
        let key = |p: P| ((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64);
        let bars_here: Vec<usize> = (0..spots.bars.len().min(64)).filter(|&k| !spots.bars[k].nets[layer].is_empty()).collect();
        type Made = (ConstrainedDelaunayTriangulation<Point2<f64>>, HashMap<(i64, i64), usize>, HashMap<(i64, i64), u64>);
        // A side of the board's own keeps its place from one map to the next, and is left out
        // from the first: a via's does not, and is found again.
        let lasting = |(k, i): (usize, usize)| if k < first_site { Some((false, k, i)) } else { k.checked_sub(obstacles.len()).map(|b| (true, b, i)) };
        let known: Vec<Side> = spots.left_out.0.lock().unwrap_or_else(|e| e.into_inner()).get(&layer).cloned().unwrap_or_default();
        let mut left_out: Vec<(usize, usize)> = known.iter().map(|&(bar, k, i)| (if bar { obstacles.len() + k } else { k }, i)).collect();
        let (cdt, owner, bar_at) = loop {
            // The side being added: an obstacle's (or, past the obstacles, a rule area's) and
            // which of its sides.
            let adding = std::cell::Cell::new((usize::MAX, 0));
            let made = quietly(|| -> Option<Made> {
                let mut cdt = ConstrainedDelaunayTriangulation::<Point2<f64>>::new();
                let mut at: HashMap<(i64, i64), spade::handles::FixedVertexHandle> = HashMap::new();
                let mut owner: HashMap<(i64, i64), usize> = HashMap::new();
                let mut bar_at: HashMap<(i64, i64), u64> = HashMap::new();
                // The outlines of rule areas that bar some nets here come after the obstacles,
                // as sides of triangles, so that each triangle is wholly in or out of each.
                // Their corners belong to no obstacle and nothing keeps a distance from them.
                let polys = obstacles.iter().map(|o| &o.poly).chain(bars_here.iter().map(|&k| &spots.bars[k].poly));
                for (k, poly) in polys.enumerate() {
                    let mut hs = Vec::with_capacity(poly.len());
                    for &p in poly.iter() {
                        if k < obstacles.len() {
                            owner.entry(key(p)).or_insert(k);
                        } else {
                            *bar_at.entry(key(p)).or_default() |= 1 << bars_here[k - obstacles.len()];
                        }
                        let h = match at.get(&key(p)) {
                            Some(&h) => Some(h),
                            None => cdt.insert(Point2::new(p[0], p[1])).ok().inspect(|&h| _ = at.insert(key(p), h)),
                        };
                        hs.extend(h);
                    }
                    for i in 0..hs.len() {
                        let (a, b) = (hs[i], hs[(i + 1) % hs.len()]);
                        if a != b && !left_out.contains(&(k, i)) {
                            adding.set((k, i));
                            cdt.add_constraint_and_split(a, b, |p| p);
                        }
                    }
                }
                Some((cdt, owner, bar_at))
            });
            match made {
                Ok(Some(map)) => break map,
                Ok(None) => unreachable!(),
                Err(panic) => {
                    // The same side twice, or too many: not this fault.
                    if adding.get().0 == usize::MAX || left_out.contains(&adding.get()) || left_out.len() >= LEFT_OUT {
                        std::panic::resume_unwind(panic);
                    }
                    left_out.push(adding.get());
                }
            }
        };
        let found: Vec<Side> = left_out.iter().filter_map(|&side| lasting(side)).filter(|side| !known.contains(side)).collect();
        if !found.is_empty() {
            let mut all = spots.left_out.0.lock().unwrap_or_else(|e| e.into_inner());
            let of_layer = all.entry(layer).or_default();
            let new: Vec<_> = found.into_iter().filter(|side| !of_layer.contains(side)).collect();
            if !new.is_empty() {
                // Said once for a layer, however many plans of the board are made.
                static SAID: std::sync::Mutex<Vec<(String, usize)>> = std::sync::Mutex::new(Vec::new());
                let mut said = SAID.lock().unwrap_or_else(|e| e.into_inner());
                if !said.contains(&(board.layers[layer].clone(), new.len())) {
                    said.push((board.layers[layer].clone(), new.len()));
                    eprintln!("plan: {} left out of the map of {}: the triangulation could not take {}", if new.len() == 1 { "a side".to_string() } else { format!("{} sides", new.len()) }, board.layers[layer], if new.len() == 1 { "it" } else { "them" });
                }
                of_layer.extend(new);
            }
        }

        let verts: Vec<P> = cdt.vertices().map(|v| [v.position().x, v.position().y]).collect();
        let vert_of = verts.iter().map(|&p| owner.get(&key(p)).copied()).collect();
        let bar_of = verts.iter().map(|&p| bar_at.get(&key(p)).copied().unwrap_or(0)).collect();
        let mut edges: Vec<Edge> = cdt
            .undirected_edges()
            .map(|e| {
                let [a, b] = e.vertices();
                Edge { v: [a.index(), b.index()], t: [None, None], len: dist(verts[a.index()], verts[b.index()]) }
            })
            .collect();
        // The obstacles whose boxes reach into each square millimetre, in their order: a
        // triangle is inside the first obstacle that holds its middle, and there are thousands
        // of each.
        let cell = |p: P| ((p[0]).floor() as i64, (p[1]).floor() as i64);
        let mut held: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (k, o) in obstacles.iter().enumerate() {
            if o.what == What::Edge || o.poly.is_empty() {
                continue;
            }
            let lo = cell([o.poly.iter().map(|p| p[0]).fold(f64::MAX, f64::min), o.poly.iter().map(|p| p[1]).fold(f64::MAX, f64::min)]);
            let hi = cell([o.poly.iter().map(|p| p[0]).fold(f64::MIN, f64::max), o.poly.iter().map(|p| p[1]).fold(f64::MIN, f64::max)]);
            for x in lo.0..=hi.0 {
                for y in lo.1..=hi.1 {
                    held.entry((x, y)).or_default().push(k);
                }
            }
        }
        let mut tris = Vec::new();
        // The triangle each face of the triangulation is.
        let mut tri_of: HashMap<usize, usize> = HashMap::new();
        for f in cdt.inner_faces() {
            tri_of.insert(f.fix().index(), tris.len());
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
                held.get(&cell(c)).and_then(|of| of.iter().copied().find(|&k| inside(c, &obstacles[k].poly)))
            };
            for &x in &e {
                let slot = if edges[x].t[0].is_none() { 0 } else { 1 };
                edges[x].t[slot] = Some(tris.len());
            }
            let bars = bars_here.iter().filter(|&&k| self::inside(c, &spots.bars[k].poly)).fold(0u64, |m, &k| m | 1 << k);
            tris.push(Tri { v, e, inside, bars });
        }
        let sites: Vec<Site> = (0..vias.len()).map(|k| Site { obstacle: first_site + k }).collect();
        // Each spot in its triangle, found by walking the triangulation: there are many of both.
        let mut spot_tri = vec![None; spots.at.len()];
        let mut tri_spots = vec![Vec::new(); tris.len()];
        for (k, &p) in spots.at.iter().enumerate() {
            if vias.iter().any(|&(at, _)| dist(at, p) < spots.apart) {
                continue;
            }
            let spade::PositionInTriangulation::OnFace(face) = cdt.locate(Point2::new(p[0], p[1])) else { continue };
            let Some(&t) = tri_of.get(&face.index()) else { continue };
            if tris[t].inside.is_some() {
                continue;
            }
            let c = tris[t].v.map(|v| verts[v]);
            let clear = (0..3).all(|i| {
                let (a, b, o) = (c[i], c[(i + 1) % 3], c[(i + 2) % 3]);
                let side = |q: P| (b[0] - a[0]) * (q[1] - a[1]) - (b[1] - a[1]) * (q[0] - a[0]);
                side(p) * side(o) > 0.0 && side(p).abs() >= SPOT_MARGIN * dist(a, b)
            });
            if clear {
                spot_tri[k] = Some(t);
                tri_spots[t].push(k);
            }
        }
        let mut space = Space { layer, obstacles, verts, vert_of, tris, edges, ends, sites, spots: spots.clone(), spot_tri, tri_spots, cap: Vec::new(), caps: Vec::new(), sides_at: Vec::new(), bar_of };
        space.caps = (0..space.edges.len()).map(|e| space.leant_on(e)).collect();
        space.cap = space.caps.iter().map(|c| c.iter().map(|c| c.0).fold(f64::MAX, f64::min)).collect();
        let mut sides_at = vec![Vec::new(); space.verts.len()];
        for (e, edge) in space.edges.iter().enumerate() {
            if !space.gap(e) {
                sides_at[edge.v[0]].push(edge.v[1]);
                sides_at[edge.v[1]].push(edge.v[0]);
            }
        }
        space.sides_at = sides_at;
        space
    }

    /// The room of gap `e` where a side of an obstacle leans over it, mm. A gap is as long as
    /// its two corners are apart; but where one of them is the end of a side that runs back
    /// towards the other at less than a right angle, and nothing stands between, everything
    /// that crosses the gap passes between that other corner and the side, and those are
    /// nearer: a row of pads beside a long edge has a gap from each pad's corner to the far
    /// end of the edge, each many times as long as the strip between the row and the edge is
    /// wide. For each such side: the room it leaves, and the obstacle it is a side of.
    fn leant_on(&self, e: usize) -> Vec<(f64, usize)> {
        let edge = &self.edges[e];
        let mut room = Vec::new();
        if !self.gap(e) {
            return room;
        }
        let cross = |o: P, a: P, b: P| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
        for (a, b) in [(edge.v[0], edge.v[1]), (edge.v[1], edge.v[0])] {
            let (pa, pb) = (self.verts[a], self.verts[b]);
            for first in edge.t.iter().flatten().copied() {
                // Round `b` from the gap, through the free triangles, as far as a side of an
                // obstacle: the corners passed on the way.
                let (mut t, mut from) = (first, a);
                let mut passed: Vec<P> = Vec::new();
                for _ in 0..self.tris.len() {
                    if !self.free(t) {
                        break;
                    }
                    let tri = &self.tris[t];
                    let Some(i) = tri.v.iter().position(|&v| v == from) else { break };
                    let Some(&x) = tri.v.iter().find(|&&v| v != from && v != b) else { break };
                    let px = self.verts[x];
                    // Turned a right angle from the gap, or more: nothing leans over it.
                    if (px[0] - pb[0]) * (pa[0] - pb[0]) + (px[1] - pb[1]) * (pa[1] - pb[1]) <= 0.0 {
                        break;
                    }
                    let side = tri.e[i];
                    if !self.gap(side) {
                        // The foot of the gap's other corner on this side, if it is on it.
                        let l2 = dist(pb, px).powi(2);
                        let along = ((pa[0] - pb[0]) * (px[0] - pb[0]) + (pa[1] - pb[1]) * (px[1] - pb[1])) / l2.max(1e-18);
                        if along <= 1.0 {
                            let foot = lerp(pb, px, along);
                            // Nothing between: every corner passed is beyond the line from
                            // the corner to its foot.
                            let near = cross(pa, foot, pb);
                            if let (true, Some(of)) = (passed.iter().all(|&q| cross(pa, foot, q) * near <= 1e-12), self.vert_of[b]) {
                                room.push((dist(pa, foot), of));
                            }
                        }
                        break;
                    }
                    passed.push(px);
                    let Some(next) = self.edges[side].t.iter().flatten().copied().find(|&y| y != t) else { break };
                    (t, from) = (next, x);
                }
            }
        }
        room
    }

    /// The room between corner `v` and a point `p` that is to be a corner too (a via's middle):
    /// how far apart they are, or less where a side of `v`'s obstacle leans over the line
    /// between them, as [`Space::leant_on`] has it for a gap that is there: what passes
    /// between the two passes between `p` and that side.
    pub fn room_to(&self, v: usize, p: P) -> f64 {
        let x = self.verts[v];
        let mut room = dist(x, p);
        for &far in self.sides_at.get(v).map_or(&[][..], |s| s.as_slice()) {
            let y = self.verts[far];
            let l2 = dist(x, y).powi(2);
            let along = ((p[0] - x[0]) * (y[0] - x[0]) + (p[1] - x[1]) * (y[1] - x[1])) / l2.max(1e-18);
            if along > 0.0 && along <= 1.0 {
                room = room.min(dist(p, lerp(x, y, along)));
            }
        }
        room
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
    /// And no more than a side that leans over it leaves ([`Space::leant_on`]).
    pub fn width(&self, e: usize) -> f64 {
        let edge = &self.edges[e];
        let (a, b) = (self.verts[edge.v[0]], self.verts[edge.v[1]]);
        let (x, y) = ((b[0] - a[0]).abs(), (b[1] - a[1]).abs());
        x.max(y).max((x + y) * std::f64::consts::FRAC_1_SQRT_2).min(self.cap.get(e).copied().unwrap_or(f64::MAX))
    }

    /// [`Space::width`] for strands that are all of `net`: a side that leans over the gap
    /// takes no room from it where it is a side of that net's own copper. Such a strand is
    /// not passing between the side and the gap's other corner: it may lie on its own pad, or
    /// has come out of that very side (the heater actuator board of the gate tier, 2026-11-21:
    /// a capacitor's pad 0.5 mm from a hole of another net beside the middle of its side, and
    /// the track its designer runs straight out of that side past the hole).
    pub fn width_for(&self, e: usize, net: u32) -> f64 {
        let caps = self.caps.get(e).map_or(&[][..], |c| c.as_slice());
        if net == 0 || caps.is_empty() {
            return self.width(e);
        }
        let edge = &self.edges[e];
        let (a, b) = (self.verts[edge.v[0]], self.verts[edge.v[1]]);
        let (x, y) = ((b[0] - a[0]).abs(), (b[1] - a[1]).abs());
        let own = |o: usize| matches!(self.obstacles[o].what, What::Pad | What::Copper) && self.obstacles[o].net == net;
        caps.iter().filter(|c| !own(c.1)).map(|c| c.0).fold(x.max(y).max((x + y) * std::f64::consts::FRAC_1_SQRT_2), f64::min)
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

    /// Whether `v` is a corner of a rule area that bars tracks of `net` on this layer.
    fn corner_bars(&self, v: usize, net: u32) -> bool {
        let mut bits = self.bar_of.get(v).copied().unwrap_or(0);
        while bits != 0 {
            let k = bits.trailing_zeros() as usize;
            if self.spots.bars[k].nets[self.layer].binary_search(&net).is_ok() {
                return true;
            }
            bits &= bits - 1;
        }
        false
    }

    /// Whether a rule area this triangle lies in bars tracks of `net` on this layer.
    pub fn barred(&self, t: usize, net: u32) -> bool {
        let mut bits = self.tris[t].bars;
        while bits != 0 {
            let k = bits.trailing_zeros() as usize;
            if self.spots.bars[k].nets[self.layer].binary_search(&net).is_ok() {
                return true;
            }
            bits &= bits - 1;
        }
        false
    }

    /// What copper of `net` must keep from the corner `v`, edge to edge, mm.
    pub fn keep(&self, board: &Board, v: usize, net: u32) -> f64 {
        // A corner where two obstacles' sides were found to cross is no object; a corner of
        // a rule area that bars the net is kept a hair off, so that a track pulled tight round
        // it does not lie along the area's edge (the checker counts that as in it).
        let Some(k) = self.vert_of[v] else {
            return if self.corner_bars(v, net) { AREA_KEEP } else { 0.0 };
        };
        let o = &self.obstacles[k];
        match o.what {
            What::Edge => board.rules.min_copper_edge_clearance * 1e3,
            What::Hole => board.rules.min_hole_clearance * 1e3 + o.reach,
            What::Keepout => 0.0,
            What::Pad | What::Copper if o.net == net && net != 0 => 0.0,
            // A pad that asks for more than its net does is given it, as the checker gives it.
            What::Pad | What::Copper => {
                let own = o.item.and_then(|i| board.pad_clearances.get(&i).copied()).unwrap_or(0);
                (board.clearance(net, o.net).max(own) as f64 / 1e6).max(o.mask).max(o.hole) + o.reach + o.past
            }
        }
    }
}
