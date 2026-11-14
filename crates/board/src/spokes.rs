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

use kicad::{Copper, Pcb};

use crate::geom::{inside, nm, Nm, Point, Shape};
use crate::model::{Board, Item, ItemId, Kind};

/// KiCad's defaults for a zone's thermal gap and spoke width, where the zone sets neither.
const DEFAULT_GAP: f64 = 0.5e-3;
const DEFAULT_WIDTH: f64 = 0.5e-3;
/// The shallowest land kept past a spoke's gap, where a zone sets no minimum width.
const MIN_LAND: Nm = 25_000;

/// How far past its gap KiCad draws a spoke, to test what its tip lands in.
const PAST: Nm = 40_000;

/// A spoke with a pad of its net past its gap: the pad it is of, its layer, the middle of its
/// far end, and the spoke as far as that.
struct Facing {
    pad: ItemId,
    layer: usize,
    tip: Point,
    stub: Shape,
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
/// the pad's edge (as far as `pad` reaches that way), across `gap`, and `land` beyond, `half`
/// either side of its axis; and the land alone. In nanometres.
fn spoke_shape(c: [f64; 2], angle: f64, pad: &Shape, gap: Nm, land: Nm, half: Nm) -> (Shape, Shape) {
    let (s, co) = angle.to_radians().sin_cos();
    // KiCad's frame: y down, positive angles counter-clockwise on screen (`kicad::place`).
    let along = |p: Point| (p.x as f64 - c[0] * 1e9) * co - (p.y as f64 - c[1] * 1e9) * s;
    let reach = match pad {
        Shape::Polygon(points) => points.iter().map(|p| along(*p)).fold(0.0, f64::max),
        Shape::Capsule { a, b, r } => along(*a).max(along(*b)) + *r as f64,
    };
    let rect = |from: f64, to: f64| {
        let (from, to, half) = (from * 1e-9, to * 1e-9, half as f64 * 1e-9);
        let corners = [[from, -half], [to, -half], [to, half], [from, half]];
        Shape::Polygon(corners.iter().map(|&q| Point::from_m(kicad::place(c, angle, q))).collect())
    };
    let past = reach + gap as f64;
    (rect(0.0, past + land as f64), rect(past, past + land as f64))
}

impl Board {
    /// Keeps room for each thermal pad's spokes; `pads` pairs each of `copper.pads` (by index)
    /// with its item. Run once the board's copper, keepouts and edges are in.
    pub(crate) fn reserve_spokes(&mut self, pcb: &Pcb, copper: &Copper, pads: &[(usize, ItemId)]) {
        let kept: std::collections::HashSet<usize> = self.items().filter_map(|(_, it)| match it.kind {
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
            // Each pad's spokes that fit, by pad and layer, with its footprint and how many
            // must stay; and the spokes with a pad of the net past their gap.
            let mut found: Vec<(ItemId, usize, usize, usize, Vec<Shape>)> = Vec::new();
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
                    let half = width.max(land) / 2;
                    let mut fits: Vec<Shape> = Vec::new();
                    for turn in 0..4 {
                        let angle = pad.spoke_angle + 90.0 * turn as f64;
                        let (shape, past) = spoke_shape(pad.spoke_centre, angle, &item.shape, gap, land, half);
                        let Shape::Polygon(corners) = &shape else { continue };
                        if !corners.iter().all(|&p| within(p)) {
                            continue;
                        }
                        if !self.in_a_relief(&past, id, &item, layer, gap) {
                            if self.room_for(&shape, &item, z, layer, nm(zone.clearance)) {
                                fits.push(shape);
                            }
                            continue;
                        }
                        // No pour past the gap, for a pad of the net stands there: the spoke
                        // may still meet that pad's spoke coming the other way.
                        let (stub, _) = spoke_shape(pad.spoke_centre, angle, &item.shape, gap, PAST, width / 2);
                        if let Shape::Polygon(c) = &stub {
                            let tip = Point::new((c[1].x + c[2].x) / 2, (c[1].y + c[2].y) / 2);
                            facing.push(Facing { pad: id, layer, tip, stub });
                        }
                    }
                    found.push((id, layer, pad.footprint, count, fits));
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
                let (pad_id, layer, stub) = (me.pad, me.layer, me.stub.clone());
                if self.room_for(&stub, &item, z, layer, nm(zone.clearance)) {
                    if let Some(entry) = found.iter_mut().find(|f| f.0 == pad_id && f.1 == layer) {
                        entry.4.push(stub);
                    }
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

    /// Whether a spoke's land `past` falls in the gap the pour leaves round another pad of the
    /// pad's net (that of `id`, a pad on `layer`): no pour is there for the spoke to reach (two
    /// overlapping pads of a USB-C connector's ground, say). Each pad's gap is taken as `gap`.
    fn in_a_relief(&self, past: &Shape, id: ItemId, pad: &Item, layer: usize, gap: Nm) -> bool {
        let Kind::Pad { footprint, number, .. } = &pad.kind else { return false };
        let reach = past.bbox().grow(gap);
        self.near(layer, reach).into_iter().filter(|&o| o != id).filter_map(|o| self.item(o)).any(|o| {
            let own = matches!(&o.kind, Kind::Pad { footprint: f, number: n, .. } if f == footprint && n == number);
            matches!(o.kind, Kind::Pad { .. }) && o.net == pad.net && o.on(layer) && !own && past.gap_within(&o.shape, gap as f64) < gap as f64
        })
    }

    /// Whether a spoke of `pad`'s zone `zone` fits on `layer` as `shape`: as the zone's own copper
    /// would, clear of other nets' copper (pours refilled round it and unlocked tracks and vias
    /// aside), the board's edge, holes and keepouts that forbid pours. A spoke is the zone's
    /// copper, so it keeps the zone's own clearance `own` from other nets where that is the
    /// wider: KiCad pours a spoke and then cuts every other net's clearance out of it, and a pad
    /// between two near neighbours of other nets is left with none (kicad-cli 9.0.8,
    /// 2026-11-13: CAL430FR's U2, 0.5 mm pitch in a zone with 0.508 mm clearance, has no
    /// ground pad joined to the pour).
    fn room_for(&mut self, shape: &Shape, pad: &Item, zone: usize, layer: usize, own: Nm) -> bool {
        let near = shape.bbox().grow(own);
        let crowded = self.near(layer, near).into_iter().filter_map(|o| self.item(o)).any(|o| {
            let stays = match o.kind {
                Kind::Pad { .. } => true,
                Kind::Track | Kind::Via => o.locked,
                Kind::Pour { kept, .. } => kept,
                Kind::Keepout { .. } | Kind::Spoke { .. } => false,
            };
            stays && o.net != pad.net && o.on(layer) && o.layers != 0 && shape.gap_within(&o.shape, own as f64) < own as f64
        });
        if crowded {
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
