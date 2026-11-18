//! What the fab can make: the narrowest track it builds on a layer of a given copper weight in a
//! stackup of a given number of layers.
//!
//! JLCPCB, standard rigid FR-4, from <https://jlcpcb.com/capabilities/pcb-capabilities>
//! ("Traces": "Min. track width and spacing"), read 2026-11-18. The page gives 1 oz and 2 oz for
//! 1–2 layers and multilayer, and 2.5, 3.5 and 4.5 oz for 2 layers only; it gives nothing by
//! inner copper weight, so inner layers (0.5 oz by default) take the 1 oz figure. Check it again
//! before relying on a new board: the fab's limits change.

use crate::geom::Nm;

/// One ounce of copper per square foot, as thick as KiCad's stackups write it.
const OZ: f64 = 0.035e-3;

/// The narrowest track the fab builds on a copper layer `copper_m` thick in a stackup of
/// `layers` copper layers; a layer of unknown thickness is taken as 1 oz.
pub fn min_track(layers: u32, copper_m: Option<f64>) -> Nm {
    let oz = copper_m.map_or(1.0, |t| t / OZ);
    let multilayer = layers > 2;
    let mm = match oz {
        o if o <= 1.25 => if multilayer { 0.09 } else { 0.10 },
        o if o <= 2.25 => if multilayer { 0.15 } else { 0.16 },
        o if o <= 3.0 => 0.20,
        o if o <= 4.0 => 0.25,
        _ => 0.30,
    };
    (mm * 1e6) as Nm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fab_minimum_follows_the_stackup() {
        assert_eq!(min_track(2, Some(0.035e-3)), 100_000);
        assert_eq!(min_track(4, Some(0.035e-3)), 90_000);
        assert_eq!(min_track(8, Some(0.0175e-3)), 90_000);
        assert_eq!(min_track(6, Some(0.07e-3)), 150_000);
        assert_eq!(min_track(2, Some(0.07e-3)), 160_000);
        assert_eq!(min_track(4, None), 90_000);
    }
}
