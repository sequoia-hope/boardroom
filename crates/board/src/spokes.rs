//! Room for the thermal spokes a pour reaches its pads by. Where a zone joins a pad with thermal
//! reliefs, KiCad's filler leaves a gap round the pad and bridges it with spokes, one every
//! quarter turn; DRC reports the pad (`starved_thermal`) when fewer than the rules' minimum
//! (`min_resolved_spokes`, the project's, 2 by default, or a custom rule's) reach the pour, unless none do. A spoke reaches it when
//! the pour still lies past the gap at its tip, at least the zone's minimum width deep. New
//! copper of another net laid there, across the gap or past it, takes the spoke away.
//!
//! So before routing, each such pad keeps room for as many spokes as the minimum asks, chosen
//! among those that fit (inside the zone, clear of the copper already on the board), those the
//! pour in the file already has first: an item of kind [`Kind::Spoke`] the spoke's length and
//! the land past its gap, which other nets' new copper keeps its clearance from (and the zone's,
//! where wider) and which joins nothing. Probed with kicad-cli 9.0.8 (2026-10-31): a spoke
//! survives a track whose clearance leaves the pour past the gap as deep as `min_thickness` and
//! no less; the spokes of a rectangular pad run along its own axes, a round pad's at 45°; a pad
//! with fewer spokes than the minimum is reported whether the pour's edge, a neighbour pad or a
//! track took the others.
//!
//! A spoke is the zone's copper: KiCad cuts every other net's clearance out of it, the zone's
//! own where that is the wider, so a pad between near neighbours of other nets can be left with
//! no spoke and is then not joined to the pour at all (CAL430FR's 0.5 mm pitch U2 in a zone with
//! 0.508 mm clearance, 2026-11-13). And where two pads of the net stand too near for pour
//! between them, the spokes they send towards each other are kept if they meet, as a bridge
//! from pad to pad (Shift-in 32's row of ground pins: a track between two pins cuts it).
//!
//! From KiCad 9.0.8's filler (`zone_filler.cpp`, read 2026-11-14 and again 2026-11-20): a
//! spoke is kept where the middle of its far end, past the gap by the arc error and half the
//! zone's minimum width, is in the pour as it stands less every other net's clearance, pruned
//! to the minimum width; or where it and another pad's spoke have their ends in each other.
//! The spoke is then added and those clearances cut out of it, and the whole pruned again, so
//! one whose side a neighbour's clearance takes is kept narrower, down to the minimum width.
//! A pad narrower than the zone's minimum width gets no spokes. Every pad the zone joins by
//! reliefs has its own gap, pads of one number too. And its check
//! (`drc_test_provider_zone_connections.cpp`) counts a pad's spokes as the places the fill's
//! outline crosses the pad's outline grown by half the gap: copper at the middle of the gap,
//! whether or not it comes in to the pad (a spoke cut off short of its pad is counted, and
//! joins nothing). What it then makes of them is in [`crate::Pours::starved`].
//!
//! So which spokes a board has before routing is asked the same way ([`Board::reserve_spokes`]):
//! the end of each in our own fill of the board ([`crate::Pours`]), then a strip of it as wide
//! as the minimum width clear across the gap, from the pad out: a spoke here is one that joins
//! its pad. Each spoke a pad could have and what was made of it is kept ([`SpokeTrial`]);
//! `bench/spokes.py` sets them beside the fills KiCad wrote. On the 631 gate boards
//! (2026-11-20) most of what the two differ on is on a few boards with fields of through pins
//! on plane layers, and strips of pour within a raster step of the minimum width.

use std::collections::HashSet;

use kicad::{Copper, Pcb};

use crate::geom::{inside, nm, Nm, Point, Shape};
use crate::model::{Board, Item, ItemId, Kind};

/// KiCad's defaults for a zone's thermal gap and spoke width, where the zone sets neither.
const DEFAULT_GAP: f64 = 0.5e-3;
const DEFAULT_WIDTH: f64 = 0.5e-3;
/// The shallowest land kept past a spoke's gap, where a zone sets no minimum width.
const MIN_LAND: Nm = 25_000;
/// How much farther than its clearance KiCad's filler keeps a pour from other nets' copper
/// (`zone_filler.cpp`, 9.0.8: every knockout is the clearance and `m_ExtraClearance`, half a
/// micron). It decides a spoke whose pad stands from its neighbour exactly the zone's clearance
/// past its own edge, pins 0.5 mm apart in a zone that keeps 0.5 mm: the neighbour's knockout
/// takes the spoke to half a micron past the pad's edge, and the pour that is left touches
/// nothing (pmw3360's J1, 2026-11-22: its ground pin is unconnected to KiCad on the designer's
/// board too but for the track the designer ran from it).
const FILL_MARGIN: Nm = 500;

/// How far past its gap KiCad draws a spoke, to test what its tip lands in, over half the
/// zone's minimum width (its filler's `spokesBox`: the gap, its arc error and half that width).
const PAST: Nm = 5_000;

/// How far off a pad's edge a spoke is looked for to see that it joins the pad.
const BY_PAD: Nm = 20_000;

/// A spoke with a pad of its net past its gap: the pad it is of, its layer, the middle of its
/// far end, and the spoke as far as that.
struct Facing {
    pad: ItemId,
    layer: usize,
    trial: usize,
    tip: Point,
    stub: Shape,
    /// What made it: the pad's gap and how far past it the spoke reaches, the pad's centre,
    /// the spoke's turn, how far past the pad it goes, and half its width and half the least
    /// width.
    gap: Nm,
    reach: Nm,
    centre: [f64; 2],
    angle: f64,
    past: Nm,
    half: Nm,
    least: Nm,
}

/// A spoke to be asked of the pour once every pad's gap is known: which of the pads found it is for, the pad's centre and item, the spoke's turn,
/// and the gap, land and half width it is made with.
struct Asked {
    found: usize,
    /// Which of the board's [`SpokeTrial`]s it is.
    trial: usize,
    centre: [f64; 2],
    angle: f64,
    item: Item,
    gap: Nm,
    land: Nm,
    half: Nm,
    /// The spoke's own width, which may be less than the land's.
    width: Nm,
}

/// How a zone joins a pad of its net: the pad's (or its footprint's) zone connection, else the
/// zone's; `true` where by thermal reliefs.
fn by_reliefs(pad: &kicad::PadCopper, zone: &kicad::Zone, through_hole: bool) -> bool {
    let connect = pad.zone_connect.unwrap_or(if zone.no_pads {
        0
    } else if zone.solid_pads {
        2
    } else if zone.reliefs_tht_only {
        3
    } else {
        1
    });
    connect == 1 || (connect == 3 && through_hole)
}

/// The spoke of a pad centred at `c` (metres) running at `angle` degrees: from the centre past
/// the pad's edge (as far as `pad` reaches that way), across `gap`, and `land` beyond, from
/// `lo` to `hi` across its axis; and the land alone. In nanometres.
fn spoke_shape(c: [f64; 2], angle: f64, pad: &Shape, gap: Nm, land: Nm, (lo, hi): (Nm, Nm)) -> (Shape, Shape) {
    let (s, co) = angle.to_radians().sin_cos();
    // KiCad's frame: y down, positive angles counter-clockwise on screen (`kicad::place`).
    let along = |p: Point| (p.x as f64 - c[0] * 1e9) * co - (p.y as f64 - c[1] * 1e9) * s;
    let reach = match pad {
        Shape::Polygon(points) => points.iter().map(|p| along(*p)).fold(0.0, f64::max),
        Shape::Capsule { a, b, r } => along(*a).max(along(*b)) + *r as f64,
    };
    let rect = |from: f64, to: f64| {
        let (from, to, lo, hi) = (from * 1e-9, to * 1e-9, lo as f64 * 1e-9, hi as f64 * 1e-9);
        let corners = [[from, lo], [to, lo], [to, hi], [from, hi]];
        Shape::Polygon(corners.iter().map(|&q| Point::from_m(kicad::place(c, angle, q))).collect())
    };
    let past = reach + gap as f64;
    (rect(0.0, past + land as f64), rect(past, past + land as f64))
}

/// The part of such a spoke that crosses the gap: from the pad's edge that way to `past` beyond
/// it, from `lo` to `hi` across its axis. In nanometres.
fn across_gap(c: [f64; 2], angle: f64, pad: &Shape, past: Nm, lo: Nm, hi: Nm) -> Shape {
    let (s, co) = angle.to_radians().sin_cos();
    let along = |p: Point| (p.x as f64 - c[0] * 1e9) * co - (p.y as f64 - c[1] * 1e9) * s;
    let reach = match pad {
        Shape::Polygon(points) => points.iter().map(|p| along(*p)).fold(0.0, f64::max),
        Shape::Capsule { a, b, r } => along(*a).max(along(*b)) + *r as f64,
    };
    let (from, to, lo, hi) = (reach * 1e-9, (reach + past as f64) * 1e-9, lo as f64 * 1e-9, hi as f64 * 1e-9);
    Shape::Polygon([[from, lo], [to, lo], [to, hi], [from, hi]].iter().map(|&q| Point::from_m(kicad::place(c, angle, q))).collect())
}

/// One of the four spokes a pad could have to a zone on a layer, and what was made of it on
/// the board as it was read: kept for setting beside KiCad's own fill (`bench/spokes.py`).
#[derive(Debug, Clone)]
pub struct SpokeTrial {
    pub pad: ItemId,
    pub zone: usize,
    pub layer: usize,
    /// The way it runs, degrees, as KiCad turns.
    pub angle: f64,
    /// The middle of the pad's gap on the spoke's axis: where KiCad's check looks for it.
    pub mid: Point,
    /// On its axis just off the pad's edge: copper there and at `mid` is a spoke that joins
    /// the pad, copper at `mid` alone one cut off short of it.
    pub by_pad: Point,
    pub made: SpokeMade,
}

/// What was made of a spoke a pad could have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpokeMade {
    Kept,
    /// The pad is narrower than the zone's least width: KiCad gives it no spokes.
    PadTooNarrow,
    /// Its far end lies in no pour, and it meets no other pad's spoke.
    NoPour,
    /// Other nets' clearances, a rule area or the zone's edge leave no strip of it as wide as
    /// the zone's least width from the middle of the gap out.
    Cut,
}

impl Board {
    /// Keeps room for each thermal pad's spokes; `pads` pairs each of `copper.pads` (by index)
    /// with its item. Run once the board's copper, keepouts and edges are in.
    pub(crate) fn reserve_spokes(&mut self, pcb: &Pcb, copper: &Copper, pads: &[(usize, ItemId)]) {
        let kept: HashSet<usize> = self.items().filter_map(|(_, it)| match it.kind {
            Kind::Pour { zone, kept: true } => Some(zone),
            _ => None,
        }).collect();
        let mut wanted = 0;
        let mut short = 0;
        for (z, zone) in copper.zones.iter().enumerate() {
            if zone.keepout.is_some() || zone.net == 0 || zone.outline.len() < 3 || kept.contains(&z) {
                continue;
            }
            let outline: Vec<Point> = zone.outline.iter().copied().map(Point::from_m).collect();
            let cutouts: Vec<Vec<Point>> = zone.cutouts.iter().map(|c| c.iter().copied().map(Point::from_m).collect()).collect();
            let within = |p: Point| inside(p, &outline) && !cutouts.iter().any(|c| inside(p, c));
            let layers: Vec<usize> = copper.expand(&zone.layers).iter().filter_map(|n| self.layers.iter().position(|l| l == n)).collect();
            // Each pad's spokes that are kept, by pad and layer, with its footprint and how
            // many must stay; every spoke such a pad could have; and those whose end is in no
            // pour.
            let mut found: Vec<(ItemId, usize, usize, usize, Vec<Shape>)> = Vec::new();
            let mut asked: Vec<Asked> = Vec::new();
            let mut facing: Vec<Facing> = Vec::new();
            for &(k, id) in pads {
                let pad = &copper.pads[k];
                let Some(item) = self.item(id).cloned() else { continue };
                if item.net != zone.net || !by_reliefs(pad, zone, item.plated_hole()) {
                    continue;
                }
                let centre = Point::from_m(pad.spoke_centre);
                if !within(centre) {
                    continue;
                }
                let size = pcb.footprints[pad.footprint].pads[pad.pad].size;
                for &layer in layers.iter().filter(|&&l| item.on(l)) {
                    let pour = self.items().find(|(_, it)| matches!(it.kind, Kind::Pour { zone: q, .. } if q == z) && it.on(layer)).map(|(p, _)| p);
                    let me = crate::custom::Thing::Item(self, id, &item);
                    let pour_thing = pour.and_then(|p| self.item(p).map(|it| crate::custom::Thing::Item(self, p, it)));
                    let rule = |kind: &str, own: Option<f64>, zone_value: f64, default: f64| -> Nm {
                        let implicit = nm(if zone_value > 0.0 { zone_value } else { default });
                        own.map(nm).unwrap_or_else(|| self.rule_min(kind, &me, pour_thing.as_ref(), layer, implicit))
                    };
                    let gap = rule("thermal_relief_gap", pad.thermal_gap, zone.thermal_gap, DEFAULT_GAP);
                    // KiCad narrows a spoke to the pad's own width.
                    let width = rule("thermal_spoke_width", pad.thermal_width, zone.thermal_bridge_width, DEFAULT_WIDTH).min(nm(size[0].min(size[1])));
                    let count = match self.custom.has("min_resolved_spokes") {
                        true => {
                            let name = self.layers[layer].clone();
                            let r = self.custom.resolve("min_resolved_spokes", &me, pour_thing.as_ref().map(|t| t as &dyn kicad::dru::Object), &name, self.layers.len());
                            r.strictest_min(self.rules.min_resolved_spokes)
                        }
                        false => self.rules.min_resolved_spokes,
                    }
                    .round()
                    .clamp(0.0, 4.0) as usize;
                    if count == 0 {
                        continue;
                    }
                    self.relief_pads.insert((id, z), gap);
                    let land = nm(zone.min_thickness).max(MIN_LAND);
                    // KiCad makes no spoke narrower than the zone's least width: a pad narrower
                    // than that has its gap and no spokes, and the pour does not join it.
                    let narrow = width < nm(zone.min_thickness);
                    let half = width.max(land) / 2;
                    for turn in 0..4 {
                        let angle = pad.spoke_angle + 90.0 * turn as f64;
                        let on_axis = |past: Nm| match across_gap(pad.spoke_centre, angle, &item.shape, past, -1, 1) {
                            Shape::Polygon(c) => Point::new((c[1].x + c[2].x) / 2, (c[1].y + c[2].y) / 2),
                            Shape::Capsule { a, .. } => a,
                        };
                        let trial = self.spoke_trials.len();
                        self.spoke_trials.push(SpokeTrial { pad: id, zone: z, layer, angle, mid: on_axis(gap / 2), by_pad: on_axis(BY_PAD), made: if narrow { SpokeMade::PadTooNarrow } else { SpokeMade::NoPour } });
                        if !narrow {
                            asked.push(Asked { found: found.len(), trial, centre: pad.spoke_centre, angle, item: item.clone(), gap, land, half, width });
                        }
                    }
                    found.push((id, layer, pad.footprint, count, Vec::new()));
                }
            }
            // What KiCad's filler asks of a spoke (`zone_filler.cpp`, 9.0.8): the middle of its
            // far end, past the gap by its arc error and half the zone's least width, lies in
            // the pour as the pads' gaps and other nets' clearances leave it, pruned to that
            // width. The spoke is then added whole, those clearances and what lies outside
            // the zone cut off it, and what is left narrower than the least width goes. A
            // neighbour's clearance may take a corner of the land past the gap and the spoke
            // is there all the same (BB-PWR-3608's P1, 2026-11-20: the square pad beside a
            // round one 0.3 mm from the land of its spoke, in a zone that keeps 0.508 mm).
            // One whose end is in no pour may still meet another pad's coming the other way.
            if !asked.is_empty() {
                let pours = crate::Pours::before_routing(self, z);
                for Asked { found: f, trial, centre, angle, item, gap, land, half, width } in asked {
                    let (id, layer) = (found[f].0, found[f].1);
                    let (stub, _) = spoke_shape(centre, angle, &item.shape, gap, PAST + land / 2, (-width / 2, width / 2));
                    let Shape::Polygon(c) = &stub else { continue };
                    let tip = Point::new((c[1].x + c[2].x) / 2, (c[1].y + c[2].y) / 2);
                    if !pours.poured(z, layer, tip) {
                        facing.push(Facing { pad: id, layer, trial, tip, stub, gap, reach: PAST + land / 2, centre, angle, past: gap + PAST + land / 2, half: width / 2, least: land / 2 });
                        continue;
                    }
                    let strip = self.crosses(centre, angle, &item, z, layer, gap + PAST, half, land / 2, nm(zone.clearance), &within);
                    self.spoke_trials[trial].made = if strip.is_some() { SpokeMade::Kept } else { SpokeMade::Cut };
                    if let Some(strip) = strip {
                        found[f].4.push(spoke_shape(centre, angle, &item.shape, gap, land, strip).0);
                    }
                }
            }
            // Spokes that meet: each one's tip in the other. KiCad keeps both, as a bridge from
            // pad to pad (zone_filler: a spoke whose end is in no pour is kept where it and
            // another's end are in each other).
            for k in 0..facing.len() {
                let me = &facing[k];
                let met = facing.iter().any(|o| o.pad != me.pad && o.layer == me.layer && o.stub.contains(me.tip) && me.stub.contains(o.tip));
                if !met {
                    continue;
                }
                let Some(item) = self.item(me.pad).cloned() else { continue };
                let (pad_id, layer, trial, gap, reach) = (me.pad, me.layer, me.trial, me.gap, me.reach);
                let (centre, angle, past, half, least) = (me.centre, me.angle, me.past, me.half, me.least);
                let strip = self.crosses(centre, angle, &item, z, layer, past, half, least, nm(zone.clearance), &within);
                self.spoke_trials[trial].made = if strip.is_some() { SpokeMade::Kept } else { SpokeMade::Cut };
                if let (Some(strip), Some(entry)) = (strip, found.iter_mut().find(|f| f.0 == pad_id && f.1 == layer)) {
                    entry.4.push(spoke_shape(centre, angle, &item.shape, gap, reach, strip).0);
                }
            }
            for (id, layer, footprint, count, fits) in found {
                wanted += 1;
                short += usize::from(fits.len() < count);
                let spare = fits.len().saturating_sub(count) as u8;
                let net = zone.net;
                for shape in fits {
                    self.insert(Item {
                        kind: Kind::Spoke { zone: z, footprint, pad: id, need: count as u8, spare, clearance: nm(zone.clearance) },
                        net,
                        layers: 1 << layer,
                        shape,
                        hole: None,
                        locked: true,
                        span: None,
                    });
                }
            }
        }
        // The checker looks this far round copper for what it must keep clear of.
        let widest = self.items().filter_map(|(_, it)| match it.kind {
            Kind::Spoke { clearance, .. } => Some(clearance),
            _ => None,
        }).max().unwrap_or(0);
        self.limits.max_clearance = self.limits.max_clearance.max(widest);
        if short > 0 {
            self.warnings.push(format!("thermal spokes: {short} of {wanted} relief pads have room for fewer spokes than the rules ask, before routing"));
        }
    }

    /// What of a spoke of `pad` at `angle`, `half` wide either side of its axis, crosses the
    /// pad's gap, from the pad's edge to `past` it, as from where to where across the axis:
    /// the whole of it where that has room there ([`Board::room_for`]), else a strip of it as
    /// wide as the zone's least width (twice `least`) that has, None where there is none. The
    /// strip found is the room kept for the spoke, not the spoke's middle whichever it was: a
    /// spoke left only its side away from a neighbour is gone once a track takes that side,
    /// though the track keeps clear of its middle (esp8266_32x32panel's RN1, 2026-11-21: pad 5
    /// counted as joined to the ground pour, which KiCad's fill stops 0.08 mm short of).
    ///
    /// Across the gap only: under the pad the pad's own copper is there whatever is cut out of
    /// the spoke. And any such strip will do, its middle or either side: KiCad cuts other nets'
    /// clearances and rule areas out of the spoke and keeps what is left as wide as the least
    /// width, so a neighbour takes the side it is on (AS5043's C3, 2026-11-20: the spokes
    /// between two ground pads, a rule area 4 um into their side). The strip must lie in the
    /// zone (`within`).
    ///
    /// All the way from the pad: KiCad's check is easier, and counts a spoke cut off short of
    /// its pad so long as copper is left at the middle of the gap (the DS8500 board's U2:
    /// spokes cut to a point between the pins beside them). But such a stub joins the pad to
    /// nothing. Taken for spokes, they made pads joined that KiCad has unconnected, and the
    /// router left them so (ErgoTravel 3 unconnected to 28, Biscay's sipm-comp 0 to 24,
    /// 2026-11-20): a spoke here is one that joins.
    #[allow(clippy::too_many_arguments)]
    fn crosses(&mut self, centre: [f64; 2], angle: f64, pad: &Item, zone: usize, layer: usize, past: Nm, half: Nm, least: Nm, own: Nm, within: &dyn Fn(Point) -> bool) -> Option<(Nm, Nm)> {
        let across = |lo: Nm, hi: Nm| across_gap(centre, angle, &pad.shape, past, lo, hi);
        // The strip a hair in from each side must be clear by the filler's margin too
        // ([`FILL_MARGIN`]): a neighbour that takes half a micron off the spoke's end at the
        // pad leaves a spoke that touches nothing. Not the strip whole: that also takes the
        // spokes a neighbour stands exactly its clearance beside, and of those KiCad keeps some
        // and drops others (tried 2026-11-22: 16 more spokes agreed on the test set's
        // GameTiger and sra_dev_board, 17 fewer on the gate tier's boards).
        let mut fits = |lo: Nm, hi: Nm| {
            let strip = across(lo, hi);
            matches!(&strip, Shape::Polygon(c) if c.iter().all(|&p| within(p)))
                && self.room_for(&strip, pad, zone, layer, own)
                && !self.crowded(&across(lo + 2 * FILL_MARGIN, hi - 2 * FILL_MARGIN), pad, layer, own, FILL_MARGIN)
        };
        if fits(-half, half) {
            return Some((-half, half));
        }
        if least >= half {
            return None;
        }
        [(-least, least), (-half, 2 * least - half), (half - 2 * least, half)].into_iter().find(|&(lo, hi)| fits(lo, hi))
    }

    /// Whether other nets' copper that stays (pours refilled round it and unlocked tracks and
    /// vias aside) is nearer `shape`, a piece of a spoke of `pad` on `layer`, than the pour may
    /// come to it, and `margin` more. A spoke is the zone's copper, so it keeps the zone's own
    /// clearance `own` from other nets where that is the wider: KiCad pours a spoke and then
    /// cuts every other net's clearance out of it, and a pad between two near neighbours of
    /// other nets is left with none (kicad-cli 9.0.8, 2026-11-13: CAL430FR's U2, 0.5 mm pitch
    /// in a zone with 0.508 mm clearance, has no ground pad joined to the pour).
    fn crowded(&self, shape: &Shape, pad: &Item, layer: usize, own: Nm, margin: Nm) -> bool {
        let near = shape.bbox().grow(own + margin);
        self.near(layer, near).into_iter().filter_map(|o| self.item(o).map(|it| (o, it))).any(|(o_id, o)| {
            let stays = match o.kind {
                Kind::Pad { .. } => true,
                Kind::Track | Kind::Via => o.locked,
                Kind::Pour { kept, .. } => kept,
                Kind::Keepout { .. } | Kind::Spoke { .. } => false,
            };
            // A pad with a clearance of its own is kept from by that, whatever the zone's.
            let own = margin
                + match self.pad_clearances.get(&o_id) {
                    Some(&c) if matches!(o.kind, Kind::Pad { .. }) => c.max(self.min_clearance_nm),
                    _ => own,
                };
            stays && o.net != pad.net && o.on(layer) && o.layers != 0 && shape.gap_within(&o.shape, own as f64) < own as f64
        })
    }

    /// Whether a spoke of `pad`'s zone `zone` fits on `layer` as `shape`: as the zone's own copper
    /// would, clear of other nets' copper ([`Board::crowded`]), the board's edge, holes and
    /// keepouts that forbid pours.
    fn room_for(&mut self, shape: &Shape, pad: &Item, zone: usize, layer: usize, own: Nm) -> bool {
        if self.crowded(shape, pad, layer, own, 0) {
            return false;
        }
        let trial = self.insert(Item { kind: Kind::Pour { zone, kept: true }, net: pad.net, layers: 1 << layer, shape: shape.clone(), hole: None, locked: true, span: None });
        // Unlocked tracks and vias are the router's to take up (`--strip-all` takes them all,
        // after loading): they leave a spoke its room.
        let clear = crate::check::check_item(self, trial).iter().all(|v| v.b.and_then(|b| self.item(b)).is_some_and(|o| matches!(o.kind, Kind::Track | Kind::Via) && !o.locked));
        self.discard(trial);
        clear
    }
}
