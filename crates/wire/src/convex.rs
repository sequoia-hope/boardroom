//! Points and the handful of things done with them.

pub type P = [f64; 2];

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
