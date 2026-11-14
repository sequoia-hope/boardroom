//! Zone fills as KiCad would pour them after routing, on a fine raster: each zone's outline
//! inside the board, less the clearance around other nets' copper, holes, the board edge, rule
//! areas and higher-priority pours, opened to the zone's minimum width, with islands that touch
//! none of the zone's own copper taken out. Edges are good to a raster step: the refill itself
//! is KiCad's.
//!
//! [`fill_zones`] is for showing the board: pads join their pour solidly, with no thermal
//! spokes. [`Pours`] is for telling what a pour joins once copper has been laid on the board
//! ([`crate::connectivity_poured`]): there a pad joined by thermal reliefs has its gap round it
//! and is joined only by the spokes kept for it whose land the pour still reaches.

use crate::geom::{nm, point_segment, BBox, Nm, Point, Shape};
use crate::model::{Board, Kind, ZoneArea};

/// Side of a raster cell: 25 µm, a sixth of the narrowest pour the boards here allow.
pub const STEP: Nm = 25_000;
/// A raster larger than this many cells a side is not filled, so a stray coordinate cannot ask
/// for gigabytes.
const SIDE_MAX: i64 = 20_000;

/// One zone's copper on one layer, as rectangles `[x0, y0, x1, y1]` in nanometres.
#[derive(Debug, Clone, PartialEq)]
pub struct Fill {
    pub zone: usize,
    pub net: u32,
    pub layer: usize,
    pub rects: Vec<[Nm; 4]>,
    /// Its separate pieces of copper: a plane a route cut in two has two.
    pub pieces: usize,
}

impl Fill {
    /// Its copper's area in square nanometres.
    pub fn area(&self) -> f64 {
        self.rects.iter().map(|r| (r[2] - r[0]) as f64 * (r[3] - r[1]) as f64).sum()
    }
}

/// Cells on a grid shared by every raster, so two rasters line up cell for cell.
#[derive(Clone)]
struct Raster {
    /// The first cell's column and row on the shared grid.
    i0: i64,
    j0: i64,
    w: usize,
    h: usize,
    cells: Vec<bool>,
}

impl Raster {
    fn over(b: BBox) -> Option<Raster> {
        let (i0, j0) = (b.min.x.div_euclid(STEP), b.min.y.div_euclid(STEP));
        let (i1, j1) = (b.max.x.div_euclid(STEP) + 1, b.max.y.div_euclid(STEP) + 1);
        if i1 - i0 > SIDE_MAX || j1 - j0 > SIDE_MAX || i1 <= i0 || j1 <= j0 {
            return None;
        }
        let (w, h) = ((i1 - i0) as usize, (j1 - j0) as usize);
        Some(Raster { i0, j0, w, h, cells: vec![false; w * h] })
    }

    fn center(&self, i: usize, j: usize) -> Point {
        Point::new((self.i0 + i as i64) * STEP + STEP / 2, (self.j0 + j as i64) * STEP + STEP / 2)
    }

    fn get(&self, i: i64, j: i64) -> bool {
        i >= 0 && j >= 0 && (i as usize) < self.w && (j as usize) < self.h && self.cells[j as usize * self.w + i as usize]
    }

    /// The cells whose centres may lie in `b`, as column and row ranges.
    fn span(&self, b: BBox) -> (std::ops::Range<usize>, std::ops::Range<usize>) {
        let clamp = |v: i64, n: usize| v.clamp(0, n as i64) as usize;
        let cols = clamp(b.min.x.div_euclid(STEP) - self.i0, self.w)..clamp(b.max.x.div_euclid(STEP) + 1 - self.i0, self.w);
        let rows = clamp(b.min.y.div_euclid(STEP) - self.j0, self.h)..clamp(b.max.y.div_euclid(STEP) + 1 - self.j0, self.h);
        (cols, rows)
    }

    /// Sets every cell whose centre is inside `polys`, taken together even-odd, to `value`.
    fn scan(&mut self, polys: &[&[Point]], value: bool) {
        let mut xs = Vec::new();
        for j in 0..self.h {
            let y = self.center(0, j).y;
            xs.clear();
            for poly in polys {
                for k in 0..poly.len() {
                    let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
                    if (a.y > y) != (b.y > y) {
                        xs.push(a.x as f64 + (y - a.y) as f64 * (b.x - a.x) as f64 / (b.y - a.y) as f64);
                    }
                }
            }
            xs.sort_by(f64::total_cmp);
            for pair in xs.chunks_exact(2) {
                let from = ((pair[0] - STEP as f64 / 2.0) / STEP as f64).ceil() as i64 - self.i0;
                let to = ((pair[1] - STEP as f64 / 2.0) / STEP as f64).ceil() as i64 - self.i0;
                for i in from.max(0)..to.min(self.w as i64) {
                    self.cells[j * self.w + i as usize] = value;
                }
            }
        }
    }

    /// Clears the cells within `b` whose centres `near` picks out.
    fn clear(&mut self, b: BBox, near: impl Fn(Point) -> bool) {
        let (cols, rows) = self.span(b);
        for j in rows {
            for i in cols.clone() {
                let k = j * self.w + i;
                if self.cells[k] && near(self.center(i, j)) {
                    self.cells[k] = false;
                }
            }
        }
    }

    /// The cells of row `j` whose centres may lie within `reach` of the segment `a`–`b`: none
    /// if the row is further than that from all of it.
    fn row_near(&self, j: usize, a: Point, b: Point, reach: f64) -> std::ops::Range<usize> {
        let y = self.center(0, j).y as f64;
        let (ay, by) = (a.y as f64, b.y as f64);
        // The part of the segment within `reach` of the row, as a span of its length.
        let (t0, t1) = if ay == by {
            if (ay - y).abs() > reach {
                return 0..0;
            }
            (0.0, 1.0)
        } else {
            let (u, v) = ((y - reach - ay) / (by - ay), (y + reach - ay) / (by - ay));
            (u.min(v).max(0.0), u.max(v).min(1.0))
        };
        if t0 > t1 {
            return 0..0;
        }
        let x = |t: f64| a.x as f64 + t * (b.x - a.x) as f64;
        let (lo, hi) = (x(t0).min(x(t1)) - reach, x(t0).max(x(t1)) + reach);
        let col = |v: f64| ((v / STEP as f64).floor() as i64 - self.i0).clamp(0, self.w as i64) as usize;
        col(lo)..(col(hi) + 1).min(self.w)
    }

    /// Clears every cell whose centre is nearer than `reach` to the segment `a`–`b`.
    fn clear_near(&mut self, a: Point, b: Point, reach: f64) {
        if reach <= 0.0 {
            return;
        }
        let (_, rows) = self.span(BBox::of([a, b]).grow(reach.ceil() as Nm));
        for j in rows {
            for i in self.row_near(j, a, b, reach) {
                let k = j * self.w + i;
                if self.cells[k] && point_segment(self.center(i, j), a, b) < reach {
                    self.cells[k] = false;
                }
            }
        }
    }

    /// Clears every cell whose centre is in `shape` or nearer than `by` to it.
    fn clear_round(&mut self, shape: &Shape, by: f64) {
        if by <= 0.0 {
            return;
        }
        match shape {
            Shape::Capsule { a, b, r } => self.clear_near(*a, *b, *r as f64 + by),
            Shape::Polygon(p) => {
                self.scan(&[p], false);
                for k in 0..p.len() {
                    self.clear_near(p[k], p[(k + 1) % p.len()], by);
                }
            }
        }
    }

    /// The cells whose centres are in `shape`.
    fn cells_in(&self, shape: &Shape) -> Vec<usize> {
        let mut out = Vec::new();
        let (_, rows) = self.span(shape.bbox());
        match shape {
            Shape::Capsule { a, b, r } => {
                for j in rows {
                    for i in self.row_near(j, *a, *b, *r as f64) {
                        if point_segment(self.center(i, j), *a, *b) <= *r as f64 {
                            out.push(j * self.w + i);
                        }
                    }
                }
            }
            Shape::Polygon(poly) => {
                let mut xs = Vec::new();
                for j in rows {
                    let y = self.center(0, j).y;
                    xs.clear();
                    for k in 0..poly.len() {
                        let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
                        if (a.y > y) != (b.y > y) {
                            xs.push(a.x as f64 + (y - a.y) as f64 * (b.x - a.x) as f64 / (b.y - a.y) as f64);
                        }
                    }
                    xs.sort_by(f64::total_cmp);
                    for pair in xs.chunks_exact(2) {
                        let from = ((pair[0] - STEP as f64 / 2.0) / STEP as f64).ceil() as i64 - self.i0;
                        let to = ((pair[1] - STEP as f64 / 2.0) / STEP as f64).ceil() as i64 - self.i0;
                        out.extend((from.max(0)..to.min(self.w as i64)).map(|i| j * self.w + i as usize));
                    }
                }
            }
        }
        out
    }

    /// Whether a cell differs from one of its four neighbours, the raster's edge counting as empty.
    fn edge(&self, i: usize, j: usize) -> bool {
        let v = self.cells[j * self.w + i];
        let (i, j) = (i as i64, j as i64);
        [(i - 1, j), (i + 1, j), (i, j - 1), (i, j + 1)].iter().any(|&(x, y)| self.get(x, y) != v)
    }

    /// Opens the copper by `r`: what a disc of radius `r` cannot reach by sliding inside it goes.
    fn open(&mut self, r: Nm) {
        let reach = r / STEP;
        if reach == 0 {
            return;
        }
        let disc: Vec<(i64, i64)> = (-reach..=reach)
            .flat_map(|di| (-reach..=reach).map(move |dj| (di, dj)))
            .filter(|(di, dj)| (di * di + dj * dj) * STEP * STEP <= r * r)
            .collect();
        let before = self.cells.clone();
        let mut eroded = self.cells.clone();
        for j in 0..self.h {
            for i in 0..self.w {
                if !before[j * self.w + i] && self.edge(i, j) {
                    for &(di, dj) in &disc {
                        let (x, y) = (i as i64 + di, j as i64 + dj);
                        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
                            eroded[y as usize * self.w + x as usize] = false;
                        }
                    }
                }
            }
        }
        // Copper on the raster's own edge borders nothing filled beyond it.
        for j in 0..self.h {
            for i in 0..self.w {
                if before[j * self.w + i] && (i == 0 || j == 0 || i + 1 == self.w || j + 1 == self.h) {
                    for &(di, dj) in &disc {
                        let (x, y) = (i as i64 + di, j as i64 + dj);
                        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
                            eroded[y as usize * self.w + x as usize] = false;
                        }
                    }
                }
            }
        }
        self.cells = eroded;
        let eroded = self.clone();
        // Growing back from the edge of what is left reaches everything growing from all of it would.
        for j in 0..self.h {
            for i in 0..self.w {
                if eroded.cells[j * self.w + i] && eroded.edge(i, j) {
                    for &(di, dj) in &disc {
                        let (x, y) = (i as i64 + di, j as i64 + dj);
                        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
                            let k = y as usize * self.w + x as usize;
                            self.cells[k] |= before[k];
                        }
                    }
                }
            }
        }
    }

    /// Keeps only the copper joined, side by side, to a seed cell.
    fn keep_seeded(&mut self, seeds: &[usize]) {
        let mut kept = vec![false; self.cells.len()];
        let mut stack: Vec<usize> = seeds.iter().copied().filter(|&k| self.cells[k]).collect();
        while let Some(k) = stack.pop() {
            if kept[k] || !self.cells[k] {
                continue;
            }
            kept[k] = true;
            let (i, j) = (k % self.w, k / self.w);
            if i > 0 {
                stack.push(k - 1);
            }
            if i + 1 < self.w {
                stack.push(k + 1);
            }
            if j > 0 {
                stack.push(k - self.w);
            }
            if j + 1 < self.h {
                stack.push(k + self.w);
            }
        }
        self.cells = kept;
    }

    /// How many pieces the copper is in, cells joined side to side.
    fn pieces(&self) -> usize {
        let mut seen = vec![false; self.cells.len()];
        let mut pieces = 0;
        for start in 0..self.cells.len() {
            if seen[start] || !self.cells[start] {
                continue;
            }
            pieces += 1;
            let mut stack = vec![start];
            while let Some(k) = stack.pop() {
                if seen[k] || !self.cells[k] {
                    continue;
                }
                seen[k] = true;
                let (i, j) = (k % self.w, k / self.w);
                if i > 0 {
                    stack.push(k - 1);
                }
                if i + 1 < self.w {
                    stack.push(k + 1);
                }
                if j > 0 {
                    stack.push(k - self.w);
                }
                if j + 1 < self.h {
                    stack.push(k + self.w);
                }
            }
        }
        pieces
    }

    /// The copper as rectangles: runs along each row, runs alike in successive rows merged.
    fn rects(&self) -> Vec<[Nm; 4]> {
        let mut done = Vec::new();
        // Open rectangles by (first column, end column): the row they started on.
        let mut open: Vec<(usize, usize, usize)> = Vec::new();
        for j in 0..=self.h {
            let mut runs = Vec::new();
            if j < self.h {
                let mut i = 0;
                while i < self.w {
                    if self.cells[j * self.w + i] {
                        let from = i;
                        while i < self.w && self.cells[j * self.w + i] {
                            i += 1;
                        }
                        runs.push((from, i));
                    }
                    i += 1;
                }
            }
            let mut still = Vec::with_capacity(runs.len());
            for (from, to, top) in open.drain(..) {
                if let Some(pos) = runs.iter().position(|&r| r == (from, to)) {
                    runs.swap_remove(pos);
                    still.push((from, to, top));
                } else {
                    let x = |i: usize| (self.i0 + i as i64) * STEP;
                    let y = |j: usize| (self.j0 + j as i64) * STEP;
                    done.push([x(from), y(top), x(to), y(j)]);
                }
            }
            still.extend(runs.into_iter().map(|(from, to)| (from, to, j)));
            open = still;
        }
        done
    }
}

/// Every zone that pours copper, filled, highest priority first.
pub fn fill_zones(board: &Board) -> Vec<Fill> {
    fill_zones_until(board, &|| false).unwrap_or_default()
}

/// The zones in the order they are poured, highest priority first.
fn order(board: &Board) -> Vec<&ZoneArea> {
    let mut order: Vec<&ZoneArea> = board.zone_areas.iter().collect();
    order.sort_by_key(|z| std::cmp::Reverse(z.priority));
    order
}

/// The fills of [`fill_zones`], or None once `stop` says so: a large board's pours take a
/// second or more, which a run's deadline must bound. `stop` is asked between zones and every
/// [`STOP_EVERY`] items or rows within one.
pub fn fill_zones_until(board: &Board, stop: &dyn Fn() -> bool) -> Option<Vec<Fill>> {
    let mut poured: Vec<(u32, usize, Raster)> = Vec::new();
    let mut fills = Vec::new();
    for zone in order(board) {
        for layer in (0..board.layers.len()).filter(|l| zone.layers & (1 << l) != 0) {
            if stop() {
                return None;
            }
            if let Some(r) = fill_one(board, zone, layer, &poured, false, stop)? {
                fills.push(Fill { zone: zone.zone, net: zone.net, layer, rects: r.rects(), pieces: r.pieces() });
                poured.push((zone.net, layer, r));
            }
        }
    }
    Some(fills)
}

/// How many items or raster rows a fill works through between asking whether to stop.
const STOP_EVERY: usize = 64;

/// One zone's fill on one layer: Some(None) when it pours nothing there, None when `stop` said so.
/// With `as_joined`, the pour is what joins: a block's kept pour is copper like any other, of
/// its net, and a pad joined by thermal spokes has its gap round it, the pour beyond held by
/// the lands of its spokes. Without, no pour on the board is looked at and every pad of the
/// net is poured over.
fn fill_one(board: &Board, zone: &ZoneArea, layer: usize, poured: &[(u32, usize, Raster)], as_joined: bool, stop: &dyn Fn() -> bool) -> Option<Option<Raster>> {
    let Shape::Polygon(outline) = &zone.outline else { return Some(None) };
    if outline.len() < 3 {
        return Some(None);
    }
    let mut b = BBox::of(outline.iter().copied());
    if !board.outline.is_empty() {
        let edge = BBox::of(board.outline.iter().flatten().copied());
        b = BBox { min: Point::new(b.min.x.max(edge.min.x), b.min.y.max(edge.min.y)), max: Point::new(b.max.x.min(edge.max.x), b.max.y.min(edge.max.y)) };
    }
    let Some(mut r) = Raster::over(b) else { return Some(None) };
    r.scan(&[outline], true);
    for cutout in &zone.cutouts {
        if let Shape::Polygon(c) = cutout {
            r.scan(&[c], false);
        }
    }
    if !board.outline.is_empty() {
        let mut board_area = r.clone();
        board_area.cells.fill(false);
        board_area.scan(&board.outline.iter().map(Vec::as_slice).collect::<Vec<_>>(), true);
        for (c, inside) in r.cells.iter_mut().zip(&board_area.cells) {
            *c &= inside;
        }
        for &(a, e, eb, edge) in &board.outline_edges {
            if eb.grow(edge).overlaps(b) {
                r.clear_near(a, e, edge as f64);
            }
        }
    }
    let own = zone.clearance;
    let net = zone.net;
    let mut seeds = Vec::new();
    for (k, id) in board.near(layer, b).into_iter().enumerate() {
        if k % STOP_EVERY == 0 && stop() {
            return None;
        }
        let Some(item) = board.item(id) else { continue };
        if !item.on(layer) {
            continue;
        }
        match &item.kind {
            Kind::Keepout { forbids, .. } => {
                if let (true, Shape::Polygon(p)) = (forbids.copperpour, &item.shape) {
                    r.scan(&[p], false);
                }
            }
            // Room kept for a spoke is no copper; its land holds the pour it reaches.
            Kind::Spoke { zone: z, .. } => {
                if let (true, Some(p)) = (as_joined && *z == zone.zone, land(&item.shape, zone.min_thickness.max(STEP))) {
                    let (i, j) = (p.x.div_euclid(STEP) - r.i0, p.y.div_euclid(STEP) - r.j0);
                    if i >= 0 && j >= 0 && (i as usize) < r.w && (j as usize) < r.h {
                        seeds.push(j as usize * r.w + i as usize);
                    }
                }
            }
            Kind::Pour { kept, .. } if !(as_joined && *kept) => {}
            Kind::Pad { .. } if as_joined && item.net == net && board.relief_pads.contains_key(&(id, zone.zone)) => {
                r.clear_round(&item.shape, board.relief_pads[&(id, zone.zone)] as f64);
            }
            _ if net != 0 && item.net == net => seeds.extend(r.cells_in(&item.shape)),
            _ => r.clear_round(&item.shape, own.max(board.clearance(net, item.net)) as f64),
        }
    }
    let holes = board.layers.len();
    let hole_gap = nm(board.rules.min_hole_clearance);
    for (k, id) in board.near(holes, b).into_iter().enumerate() {
        if k % STOP_EVERY == 0 && stop() {
            return None;
        }
        let Some(item) = board.item(id) else { continue };
        let Some(Shape::Capsule { a, b: end, r: radius }) = board.hole_shape(id, item) else { continue };
        if net != 0 && item.net == net && item.on(layer) {
            continue;
        }
        let reach = radius + hole_gap.max(own).max(board.clearance(net, item.net));
        // A slot: every point within reach of its centre line.
        r.clear_near(a, end, reach as f64);
    }
    for (other, l, done) in poured {
        if *l != layer || *other == net {
            continue;
        }
        let c = own.max(board.clearance(net, *other)) as f64;
        for j in 0..done.h {
            if j % STOP_EVERY == 0 && stop() {
                return None;
            }
            for i in 0..done.w {
                if !done.cells[j * done.w + i] {
                    continue;
                }
                let (x, y) = (done.i0 + i as i64 - r.i0, done.j0 + j as i64 - r.j0);
                if x >= 0 && y >= 0 && (x as usize) < r.w && (y as usize) < r.h {
                    r.cells[y as usize * r.w + x as usize] = false;
                }
                if done.edge(i, j) {
                    let p = done.center(i, j);
                    r.clear(BBox::of([p]).grow(c as Nm + STEP), |q| q.distance(p) < c + STEP as f64 / 2.0);
                }
            }
        }
    }
    if stop() {
        return None;
    }
    r.open(zone.min_thickness / 2);
    if net != 0 {
        r.keep_seeded(&seeds);
    }
    Some(r.cells.iter().any(|&c| c).then_some(r))
}

/// The middle of a spoke's land: half the land's depth in from the spoke's far end, on its
/// axis.
fn land(spoke: &Shape, depth: Nm) -> Option<Point> {
    let Shape::Polygon(c) = spoke else { return None };
    if c.len() != 4 {
        return None;
    }
    let (far, back) = (Point::new((c[1].x + c[2].x) / 2, (c[1].y + c[2].y) / 2), Point::new((c[0].x + c[3].x) / 2, (c[0].y + c[3].y) / 2));
    let t = (depth as f64 / 2.0) / far.distance(back).max(1.0);
    Some(Point::new(far.x + ((back.x - far.x) as f64 * t) as Nm, far.y + ((back.y - far.y) as f64 * t) as Nm))
}

/// One zone's pour on one layer, each cell with the piece of copper it is in.
struct Part {
    zone: usize,
    net: u32,
    layer: usize,
    /// How deep a spoke's land must be: the zone's minimum width.
    land: Nm,
    raster: Raster,
    /// Per cell, the piece it is in, counted from 1 over every part; 0 for none.
    piece: Vec<u32>,
}

/// The pours as a refill would leave them round the copper on the board as it stands, to tell
/// what each piece of pour joins. A block's kept pours are copper already and are not poured.
/// Good to a raster step, as [`fill_zones`] is: the refill itself is KiCad's.
pub struct Pours {
    parts: Vec<Part>,
    pieces: usize,
}

impl Pours {
    pub fn of(board: &Board) -> Pours {
        let mut poured: Vec<(u32, usize, Raster)> = Vec::new();
        let mut parts = Vec::new();
        let mut pieces = 0u32;
        for zone in order(board).into_iter().filter(|z| !z.kept) {
            for layer in (0..board.layers.len()).filter(|l| zone.layers & (1 << l) != 0) {
                let Some(Some(r)) = fill_one(board, zone, layer, &poured, true, &|| false) else { continue };
                poured.push((zone.net, layer, r.clone()));
                if zone.net == 0 {
                    continue;
                }
                let mut piece = vec![0u32; r.cells.len()];
                for start in 0..r.cells.len() {
                    if piece[start] != 0 || !r.cells[start] {
                        continue;
                    }
                    pieces += 1;
                    let mut stack = vec![start];
                    while let Some(k) = stack.pop() {
                        if piece[k] != 0 || !r.cells[k] {
                            continue;
                        }
                        piece[k] = pieces;
                        let (i, j) = (k % r.w, k / r.w);
                        if i > 0 {
                            stack.push(k - 1);
                        }
                        if i + 1 < r.w {
                            stack.push(k + 1);
                        }
                        if j > 0 {
                            stack.push(k - r.w);
                        }
                        if j + 1 < r.h {
                            stack.push(k + r.w);
                        }
                    }
                }
                parts.push(Part { zone: zone.zone, net: zone.net, layer, land: zone.min_thickness.max(STEP), raster: r, piece });
            }
        }
        Pours { parts, pieces: pieces as usize }
    }

    /// How many pieces of pour there are; each is numbered below this.
    pub fn pieces(&self) -> usize {
        self.pieces
    }

    /// Whether any net is poured.
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// The nets that are poured.
    pub fn nets(&self) -> Vec<u32> {
        let mut nets: Vec<u32> = self.parts.iter().map(|p| p.net).collect();
        nets.sort_unstable();
        nets.dedup();
        nets
    }

    fn at(part: &Part, p: Point) -> Option<usize> {
        let r = &part.raster;
        let (i, j) = (p.x.div_euclid(STEP) - r.i0, p.y.div_euclid(STEP) - r.j0);
        if i < 0 || j < 0 || i as usize >= r.w || j as usize >= r.h {
            return None;
        }
        match part.piece[j as usize * r.w + i as usize] {
            0 => None,
            k => Some(k as usize - 1),
        }
    }

    /// The piece of `net`'s pour on `layer` that covers everything within `r` of `p`: where a
    /// via of that radius stands wholly in the pour.
    pub fn piece(&self, net: u32, layer: usize, p: Point, r: Nm) -> Option<usize> {
        self.parts.iter().filter(|part| part.net == net && part.layer == layer).find_map(|part| {
            let piece = Pours::at(part, p)?;
            let n = r / STEP + 1;
            for dj in -n..=n {
                for di in -n..=n {
                    let q = Point::new(p.x + di * STEP, p.y + dj * STEP);
                    if (di * di + dj * dj) * STEP * STEP <= (r + STEP) * (r + STEP) && Pours::at(part, q) != Some(piece) {
                        return None;
                    }
                }
            }
            Some(piece)
        })
    }

    /// The pieces of pour that copper item `id` is joined to on `layer`. A track is joined where
    /// an end of it lies in the pour, a via where its middle does, a pad the pour joins solidly
    /// where any of it does. A pad joined by thermal spokes is joined by each spoke whose land,
    /// past the gap round the pad, is still pour.
    pub fn joined(&self, board: &Board, id: crate::model::ItemId, layer: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let Some(item) = board.item(id) else { return out };
        if !item.on(layer) || !item.is_copper() {
            return out;
        }
        for part in self.parts.iter().filter(|part| part.net == item.net && part.layer == layer) {
            let mut add = |piece: Option<usize>| {
                if let Some(k) = piece.filter(|k| !out.contains(k)) {
                    out.push(k);
                }
            };
            match item.kind {
                Kind::Pad { .. } => {
                    let joins = match board.pad_zone_connect.get(&id) {
                        Some(&c) => c != 0,
                        None => !board.zones_without_pads.contains(&part.zone),
                    };
                    if !joins {
                        continue;
                    }
                    if board.relief_pads.contains_key(&(id, part.zone)) {
                        let near = item.shape.bbox().grow(STEP);
                        for s in board.near(layer, near) {
                            let Some(spoke) = board.item(s) else { continue };
                            if matches!(spoke.kind, Kind::Spoke { pad, zone, .. } if pad == id && zone == part.zone) && spoke.on(layer) {
                                add(land(&spoke.shape, part.land).and_then(|p| Pours::at(part, p)));
                            }
                        }
                    } else {
                        for cell in part.raster.cells_in(&item.shape) {
                            add((part.piece[cell] != 0).then(|| part.piece[cell] as usize - 1));
                        }
                    }
                }
                Kind::Pour { .. } => {
                    for cell in part.raster.cells_in(&item.shape) {
                        add((part.piece[cell] != 0).then(|| part.piece[cell] as usize - 1));
                    }
                }
                _ => {
                    let (pts, len) = crate::connect::anchors(board, id);
                    for p in &pts[..len] {
                        add(Pours::at(part, *p));
                    }
                }
            }
        }
        out
    }

    /// The pour of every net on `layer` as rectangles `[x0, y0, x1, y1]`, with each one's net.
    pub fn rects(&self, layer: usize) -> Vec<(u32, [Nm; 4])> {
        self.parts.iter().filter(|p| p.layer == layer).flat_map(|p| p.raster.rects().into_iter().map(move |r| (p.net, r))).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Item;

    fn board(text: &str) -> Board {
        let (pcb, copper) = kicad::parse_board(text).unwrap();
        Board::new(&pcb, &copper, kicad::Rules::parse("{}").unwrap())
    }

    const ZONE: &str = r#"(kicad_pcb (version 20240101) (general (thickness 1.6))
        (layers (0 "F.Cu" signal) (31 "B.Cu" signal)) (net 0 "") (net 1 "GND") (net 2 "SIG")
        (zone (net 1) (net_name "GND") (layer "F.Cu") (uuid "z") (connect_pads yes (clearance 0.2))
          (min_thickness 0.2) (polygon (pts (xy 0 0) (xy 10 0) (xy 10 10) (xy 0 10)))))"#;

    fn track(net: u32, a: Point, b: Point) -> Item {
        Item { kind: Kind::Track, net, layers: 1, shape: Shape::Capsule { a, b, r: 100_000 }, hole: None, locked: false, span: None }
    }

    fn covered(fills: &[Fill], p: Point) -> bool {
        fills.iter().any(|f| f.rects.iter().any(|r| p.x >= r[0] && p.x < r[2] && p.y >= r[1] && p.y < r[3]))
    }

    #[test]
    fn a_pour_keeps_clear_of_other_nets_and_drops_islands_without_its_own_copper() {
        let mut b = board(ZONE);
        assert!(fill_zones(&b).is_empty(), "a pour touching none of its net's copper is one island, removed");
        b.insert(track(1, Point::new(1_000_000, 1_000_000), Point::new(2_000_000, 1_000_000)));
        // A wall of another net from top to bottom cuts off the right side.
        b.insert(track(2, Point::new(5_000_000, -1_000_000), Point::new(5_000_000, 11_000_000)));
        let fills = fill_zones(&b);
        assert_eq!(fills.len(), 1);
        assert!(covered(&fills, Point::new(3_000_000, 5_000_000)));
        assert!(!covered(&fills, Point::new(8_000_000, 5_000_000)), "the island right of the wall has no GND copper");
        // 0.1 mm half-width plus 0.2 mm clearance: nothing within 0.3 mm of the wall's centre.
        assert!(!covered(&fills, Point::new(4_750_000, 5_000_000)));
        assert!(covered(&fills, Point::new(4_650_000, 5_000_000)));
        assert_eq!(fills[0].pieces, 1);
    }

    #[test]
    fn a_pour_cut_in_two_with_its_own_copper_on_both_sides_is_two_pieces() {
        let mut b = board(ZONE);
        b.insert(track(1, Point::new(1_000_000, 1_000_000), Point::new(2_000_000, 1_000_000)));
        let whole = fill_zones(&b);
        assert_eq!(whole[0].pieces, 1);
        b.insert(track(1, Point::new(8_000_000, 1_000_000), Point::new(9_000_000, 1_000_000)));
        b.insert(track(2, Point::new(5_000_000, -1_000_000), Point::new(5_000_000, 11_000_000)));
        let cut = fill_zones(&b);
        assert_eq!(cut[0].pieces, 2);
        // The wall takes its own width and a clearance each side, 0.6 mm by 10 mm.
        let lost = (whole[0].area() - cut[0].area()) / 1e12;
        assert!((lost - 6.0).abs() < 0.5, "{lost} mm²");
    }
}
