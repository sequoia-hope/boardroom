//! Convex regions as lists of corners, in millimetres. A region may be a single point or a
//! segment: the place a track starts is a point, and what a line leaves of a region is a segment.

pub type P = [f64; 2];

/// Lengths this small are nothing, mm.
pub const TOL: f64 = 1e-7;

pub fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

pub fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1]]
}

pub fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1]]
}

pub fn scale(a: P, s: f64) -> P {
    [a[0] * s, a[1] * s]
}

pub fn cross(a: P, b: P) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

pub fn len(a: P) -> f64 {
    dot(a, a).sqrt()
}

/// The normal on the left of a direction.
pub fn left(d: P) -> P {
    [-d[1], d[0]]
}

fn push(out: &mut Vec<P>, p: P) {
    if out.last().is_none_or(|&q| len(sub(p, q)) > 1e-9) {
        out.push(p);
    }
}

/// The part of a region where `n·p <= c`.
pub fn clip(poly: &[P], n: P, c: f64) -> Vec<P> {
    let m = poly.len();
    if m == 0 {
        return Vec::new();
    }
    if m == 1 {
        return if dot(n, poly[0]) <= c + TOL { poly.to_vec() } else { Vec::new() };
    }
    let mut out = Vec::new();
    // A segment has one side; a polygon as many as corners.
    for i in 0..if m > 2 { m } else { 1 } {
        let (a, b) = (poly[i], poly[(i + 1) % m]);
        let (da, db) = (dot(n, a) - c, dot(n, b) - c);
        if da <= TOL {
            push(&mut out, a);
        }
        if (da > TOL && db < -TOL) || (da < -TOL && db > TOL) {
            let t = da / (da - db);
            push(&mut out, add(a, scale(sub(b, a), t)));
        }
    }
    if m == 2 && dot(n, poly[1]) - c <= TOL {
        push(&mut out, poly[1]);
    }
    if out.len() > 1 && len(sub(out[0], out[out.len() - 1])) <= 1e-9 {
        out.pop();
    }
    out
}

/// The part of a region between two lines of direction `d`: where the offset `left(d)·p` is
/// between `lo` and `hi`.
pub fn strip(poly: &[P], d: P, (lo, hi): (f64, f64)) -> Vec<P> {
    let n = left(d);
    clip(&clip(poly, n, hi), scale(n, -1.0), -lo)
}

/// The least and greatest of `n·p` over a region.
pub fn extent(poly: &[P], n: P) -> (f64, f64) {
    poly.iter().fold((f64::MAX, f64::MIN), |(lo, hi), &p| (lo.min(dot(n, p)), hi.max(dot(n, p))))
}

/// The convex hull, anticlockwise with y up; a point or a segment where that is all there is.
pub fn hull(points: &[P]) -> Vec<P> {
    let mut p = points.to_vec();
    p.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    p.dedup_by(|a, b| len(sub(*a, *b)) <= 1e-9);
    if p.len() < 3 {
        return p;
    }
    let mut h: Vec<P> = Vec::new();
    for pass in 0..2 {
        let start = h.len();
        let it: Box<dyn Iterator<Item = &P>> = if pass == 0 { Box::new(p.iter()) } else { Box::new(p.iter().rev()) };
        for &q in it {
            while h.len() >= start + 2 && cross(sub(h[h.len() - 1], h[h.len() - 2]), sub(q, h[h.len() - 1])) <= 1e-12 {
                h.pop();
            }
            h.push(q);
        }
        h.pop();
    }
    h
}

/// Everything a region reaches going along `d`.
pub fn sweep(poly: &[P], d: P) -> Vec<P> {
    const FAR: f64 = 1e4;
    let mut all = poly.to_vec();
    all.extend(poly.iter().map(|&p| add(p, scale(d, FAR))));
    hull(&all)
}

fn point_segment(p: P, a: P, b: P) -> f64 {
    let ab = sub(b, a);
    let l2 = dot(ab, ab);
    let t = if l2 > 0.0 { (dot(sub(p, a), ab) / l2).clamp(0.0, 1.0) } else { 0.0 };
    len(sub(p, add(a, scale(ab, t))))
}

/// Whether `p` is in the region, or within `tol` of it.
pub fn contains(poly: &[P], p: P, tol: f64) -> bool {
    match poly.len() {
        0 => false,
        1 => len(sub(p, poly[0])) <= tol,
        2 => point_segment(p, poly[0], poly[1]) <= tol,
        m => {
            let turn: f64 = (0..m).map(|i| cross(poly[i], poly[(i + 1) % m])).sum();
            let s = if turn >= 0.0 { 1.0 } else { -1.0 };
            (0..m).all(|i| {
                let (a, b) = (poly[i], poly[(i + 1) % m]);
                s * cross(sub(b, a), sub(p, a)) >= -tol * len(sub(b, a))
            })
        }
    }
}

/// Whether every corner of `inner` is in `outer`.
pub fn within(inner: &[P], outer: &[P]) -> bool {
    inner.iter().all(|&p| contains(outer, p, 1e-6))
}
