//! A KiCad board as copper per net: what the fast verifier checks measure and what the block
//! generators place against. Millimetres, KiCad's frame (Y down).
//!
//! It is drawn from [`crate::Board`], the board the router lays copper on and the checker judges
//! ([`Board::of`]), so the judge measures the copper as they see it: arcs as the same chords,
//! pads joined on the layers KiCad joins them on, a routed board as routed.

pub mod geom;
pub mod graph;

use std::collections::BTreeMap;
use std::path::Path;

use geom::P;
use serde::Serialize;

use crate::{Kind, Point, Shape, DRAWN};

#[derive(Debug, Clone, Serialize)]
pub struct Board {
    /// Copper layer names, front to back.
    pub layers: Vec<String>,
    pub parts: BTreeMap<String, Part>,
    pub pads: Vec<Pad>,
    pub tracks: Vec<Track>,
    pub vias: Vec<Via>,
    /// Zone fills as KiCad last computed them.
    pub fills: Vec<Fill>,
    /// The board's outer Edge.Cuts loop, arcs as chords within 5 um; none when it has no
    /// closed outline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline: Option<Vec<P>>,
    /// The board this was drawn from ([`Board::of`]): which copper joins which is asked of it
    /// ([`graph`]), as the checker asks. None for a board put together by hand, which joins
    /// nothing.
    #[serde(skip)]
    pub source: Option<std::sync::Arc<crate::Board>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Part {
    pub reference: String,
    pub value: String,
    pub footprint: String,
    pub at: P,
    pub rotation: f64,
    pub back: bool,
    pub locked: bool,
    /// Convex hull of the courtyard, board frame.
    pub courtyard: Option<Vec<P>>,
    /// Indices into `Board::pads`.
    pub pads: Vec<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Pad {
    pub part: String,
    pub pin: String,
    pub net: String,
    pub layers: Vec<String>,
    pub centre: P,
    /// Copper outline, one or more polygons (a union).
    pub shape: Vec<Vec<P>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Track {
    pub a: P,
    pub b: P,
    pub width: f64,
    pub layer: String,
    pub net: String,
    pub locked: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Via {
    pub at: P,
    pub size: f64,
    pub drill: f64,
    /// Every copper layer the barrel joins.
    pub layers: Vec<String>,
    pub net: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Fill {
    pub net: String,
    pub layer: String,
    pub polygon: Vec<P>,
}

/// Nanometres to millimetres.
fn mm(p: Point) -> P {
    [p.x as f64 / 1e6, p.y as f64 / 1e6]
}

/// Chord error allowed when a round pad becomes a polygon, mm: as the main model's arcs.
const ROUND_ERROR: f64 = 0.005;

/// A shape as one polygon, mm: a round pad as a polygon within [`ROUND_ERROR`] of its circle,
/// with its corners on it.
fn polygon(shape: &Shape) -> Vec<P> {
    match shape {
        Shape::Polygon(p) => p.iter().copied().map(mm).collect(),
        Shape::Capsule { a, b, r } => {
            let r = *r as f64 / 1e6;
            let step = 2.0 * (1.0 - (ROUND_ERROR / r.max(ROUND_ERROR)).min(1.0)).acos();
            let n = ((std::f64::consts::TAU / step.max(1e-3)).ceil() as usize).clamp(24, 1024);
            if a == b {
                geom::circle(mm(*a), r, n)
            } else {
                geom::capsule(mm(*a), mm(*b), 2.0 * r)
            }
        }
    }
}

impl Board {
    pub fn load(path: &Path) -> Result<Board, String> {
        crate::Board::load(path).map(|(b, _)| Board::of(std::sync::Arc::new(b))).map_err(|e| e.to_string())
    }

    /// A board file's text, with KiCad's default rules.
    pub fn parse(text: &str) -> Result<Board, String> {
        let (pcb, copper) = kicad::parse_board(text).map_err(|e| e.to_string())?;
        let rules = kicad::Rules::parse("{}").map_err(|e| e.to_string())?;
        Ok(Board::of(std::sync::Arc::new(crate::Board::new(&pcb, &copper, rules))))
    }

    /// The copper of `board` per net, as it stands (a routed board's routes included).
    pub fn of(source: std::sync::Arc<crate::Board>) -> Board {
        let board = source.as_ref();
        let layers = board.layers.clone();
        let names = |mask: u64| -> Vec<String> { (0..layers.len()).filter(|l| mask & (1 << l) != 0).map(|l| layers[l].clone()).collect() };
        let mut parts: BTreeMap<String, Part> = BTreeMap::new();
        for f in &board.footprints {
            parts.insert(
                f.reference.clone(),
                Part {
                    reference: f.reference.clone(),
                    value: f.value.clone(),
                    footprint: f.library_id.clone(),
                    at: mm(f.at),
                    rotation: f.rotation_deg,
                    back: f.back,
                    locked: f.locked,
                    courtyard: f.courtyard.as_ref().map(|c| c.iter().copied().map(mm).collect()),
                    pads: Vec::new(),
                },
            );
        }
        let mut pads = Vec::new();
        for info in &board.pads {
            let items: Vec<_> = info.items.iter().filter_map(|&id| board.item(id)).filter(|it| it.layers != 0).collect();
            let Some(first) = items.first() else { continue };
            let reference = &board.footprints[info.footprint].reference;
            if let Some(p) = parts.get_mut(reference) {
                p.pads.push(pads.len());
            }
            pads.push(Pad {
                part: reference.clone(),
                pin: info.number.clone(),
                net: board.net_name(first.net).to_string(),
                layers: names(first.layers),
                centre: mm(info.centre),
                shape: items.iter().map(|it| polygon(&it.shape)).collect(),
            });
        }
        let (mut tracks, mut vias, mut fills) = (Vec::new(), Vec::new(), Vec::new());
        let unfilled: std::collections::HashSet<usize> = board.unfilled.iter().map(|u| u.0).collect();
        for (_, it) in board.items() {
            let net = board.net_name(it.net).to_string();
            match (&it.kind, &it.shape) {
                (Kind::Track, Shape::Capsule { a, b, r }) => {
                    let layer = names(it.layers).into_iter().next().unwrap_or_default();
                    tracks.push(Track { a: mm(*a), b: mm(*b), width: 2.0 * *r as f64 / 1e6, layer, net, locked: it.locked });
                }
                (Kind::Via, Shape::Capsule { a, r, .. }) => {
                    let drill = it.hole.map_or(0.0, |(_, h)| 2.0 * h as f64 / 1e6);
                    vias.push(Via { at: mm(*a), size: 2.0 * *r as f64 / 1e6, drill, layers: names(it.layers), net });
                }
                // A zone's fill; an unfilled zone's outline standing in for it is not KiCad's fill.
                (Kind::Pour { zone, .. }, Shape::Polygon(_)) if *zone != DRAWN && !unfilled.contains(zone) => {
                    for layer in names(it.layers) {
                        fills.push(Fill { net: net.clone(), layer, polygon: polygon(&it.shape) });
                    }
                }
                _ => {}
            }
        }
        let outline = board.outline.first().filter(|l| l.len() >= 3).map(|l| l.iter().copied().map(mm).collect());
        Board { layers, parts, pads, tracks, vias, fills, outline, source: Some(source.clone()) }
    }

    /// A board of the copper given, as KiCad would read it: written out as a board file and read
    /// back, so that it is drawn from the main model like any other. Each pad is a rectangle the
    /// size of its first shape's box, in a footprint at the origin named for its part; each fill
    /// a zone of its net filled as given. For tests and sketches.
    pub fn sketch(layers: &[&str], pads: &[Pad], tracks: &[Track], vias: &[Via], fills: &[Fill]) -> Result<Board, String> {
        use std::fmt::Write;
        let mut nets: Vec<&str> = pads.iter().map(|p| p.net.as_str()).chain(tracks.iter().map(|t| t.net.as_str())).chain(vias.iter().map(|v| v.net.as_str())).chain(fills.iter().map(|f| f.net.as_str())).filter(|n| !n.is_empty()).collect();
        nets.sort_unstable();
        nets.dedup();
        let num = |n: &str| nets.iter().position(|m| *m == n).map_or(0, |k| k + 1);
        let id = |l: &str| match l {
            "F.Cu" => 0,
            "B.Cu" => 31,
            _ => l.trim_start_matches("In").trim_end_matches(".Cu").parse::<usize>().unwrap_or(30),
        };
        let pts = |ps: &[P]| ps.iter().map(|q| format!("(xy {} {})", q[0], q[1])).collect::<Vec<_>>().join(" ");
        let quoted = |ls: &[String]| ls.iter().map(|l| format!("\"{l}\"")).collect::<Vec<_>>().join(" ");
        let mut t = String::from("(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))\n(layers");
        for l in layers {
            let _ = write!(t, " ({} \"{l}\" signal)", id(l));
        }
        t.push_str(" (44 \"Edge.Cuts\" user))\n(net 0 \"\")");
        for (k, n) in nets.iter().enumerate() {
            let _ = write!(t, " (net {} \"{n}\")", k + 1);
        }
        let mut refs: Vec<&str> = pads.iter().map(|p| p.part.as_str()).collect();
        refs.sort_unstable();
        refs.dedup();
        for r in refs {
            let _ = write!(t, "\n(footprint \"sketch:{r}\" (layer \"F.Cu\") (at 0 0) (property \"Reference\" \"{r}\") (property \"Value\" \"\")");
            for p in pads.iter().filter(|p| p.part == r) {
                let Some((lo, hi)) = p.shape.first().and_then(|s| geom::bbox(s.iter().copied())) else { continue };
                let _ = write!(t, " (pad \"{}\" smd rect (at {} {}) (size {} {}) (layers {}) (net {} \"{}\"))", p.pin, p.centre[0], p.centre[1], hi[0] - lo[0], hi[1] - lo[1], quoted(&p.layers), num(&p.net), p.net);
            }
            t.push(')');
        }
        for k in tracks {
            let _ = write!(t, "\n(segment (start {} {}) (end {} {}) (width {}) (layer \"{}\") (net {}))", k.a[0], k.a[1], k.b[0], k.b[1], k.width, k.layer, num(&k.net));
        }
        for v in vias {
            let _ = write!(t, "\n(via (at {} {}) (size {}) (drill {}) (layers {}) (net {}))", v.at[0], v.at[1], v.size, v.drill, quoted(&v.layers), num(&v.net));
        }
        for f in fills {
            let _ = write!(
                t,
                "\n(zone (net {}) (net_name \"{}\") (layer \"{}\") (hatch edge 0.5) (connect_pads (clearance 0.2)) (min_thickness 0.1) (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5)) (polygon (pts {})) (filled_polygon (layer \"{}\") (pts {})))",
                num(&f.net), f.net, f.layer, pts(&f.polygon), f.layer, pts(&f.polygon)
            );
        }
        t.push_str("\n)\n");
        Board::parse(&t)
    }

    /// The pad `REF.PIN`. Footprints with several pads of one number (exposed pads, connector
    /// shells) give the first.
    pub fn pad(&self, ref_pin: &str) -> Option<&Pad> {
        let (r, pin) = ref_pin.split_once('.')?;
        let part = self.parts.get(r)?;
        part.pads.iter().map(|&i| &self.pads[i]).find(|p| p.pin == pin)
    }

    /// Every copper shape of a net, per layer, as polygons.
    pub fn net_copper(&self, net: &str) -> Vec<(String, Vec<P>)> {
        let mut out = Vec::new();
        for p in self.pads.iter().filter(|p| p.net == net) {
            for l in &p.layers {
                for s in &p.shape {
                    out.push((l.clone(), s.clone()));
                }
            }
        }
        for t in self.tracks.iter().filter(|t| t.net == net) {
            out.push((t.layer.clone(), geom::capsule(t.a, t.b, t.width)));
        }
        for v in self.vias.iter().filter(|v| v.net == net) {
            for l in &v.layers {
                out.push((l.clone(), geom::circle(v.at, v.size / 2.0, 16)));
            }
        }
        for f in self.fills.iter().filter(|f| f.net == net) {
            out.push((f.layer.clone(), f.polygon.clone()));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposed_mask_segment_layers_parse() {
        let src = r#"(kicad_pcb (version 20241229) (generator "pcbnew")
          (general (thickness 1.6))
          (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (36 "B.Mask" user))
          (net 0 "")
          (net 1 "GND")
          (footprint "R:R_0402" (layer "F.Cu") (at 0 0)
            (property "Reference" "R1")
            (property "Value" "10k")
            (pad "1" smd rect (at 0 0) (size 0.5 0.5) (layers "F.Cu" "F.Mask") (net 1 "GND")))
          (segment (start 0 0) (end 5 0) (width 0.25) (layers "B.Cu" "B.Mask") (solder_mask_margin 0) (net 1))
        )"#;
        let b = Board::parse(src).expect("parse exposed-mask segment");
        assert_eq!(b.tracks.len(), 1);
        assert_eq!(b.tracks[0].layer, "B.Cu");
        assert_eq!(b.tracks[0].net, "GND");
        assert_eq!(b.pads.len(), 1);
    }

    #[test]
    fn paste_only_pads_are_ignored() {
        // KiCad 10 0201 footprints carry unnamed `(layers "F.Paste")` stencil apertures alongside
        // the two copper pads; they are not electrical pads.
        let src = r#"(kicad_pcb (version 20260206) (generator "pcbnew")
          (general (thickness 1.6))
          (layers (0 "F.Cu" signal) (2 "B.Cu" signal) (13 "F.Paste" user) (1 "F.Mask" user))
          (footprint "Capacitor_SMD:C_0201_0603Metric" (layer "F.Cu") (at 0 0)
            (property "Reference" "C1")
            (property "Value" "100n")
            (pad "" smd roundrect (at -0.345 0) (size 0.318 0.36) (layers "F.Paste") (roundrect_rratio 0.25))
            (pad "" smd roundrect (at 0.345 0) (size 0.318 0.36) (layers "F.Paste") (roundrect_rratio 0.25))
            (pad "1" smd roundrect (at -0.32 0) (size 0.46 0.4) (layers "F.Cu" "F.Mask") (roundrect_rratio 0.25) (net "+3.3V"))
            (pad "2" smd roundrect (at 0.32 0) (size 0.46 0.4) (layers "F.Cu" "F.Mask") (roundrect_rratio 0.25) (net "GND")))
        )"#;
        let b = Board::parse(src).expect("parse KiCad 10 0201 footprint");
        assert_eq!(b.pads.len(), 2);
        assert_eq!(b.parts["C1"].pads.len(), 2);
    }

    #[test]
    fn kicad10_zone_fills_preserve_net_name() {
        // KiCad 10 omits `(net_name "...")` on zones, writing `(net "GND")` directly.
        let src = r#"(kicad_pcb (version 20260206) (generator "pcbnew")
          (general (thickness 1.6))
          (layers (0 "F.Cu" signal) (1 "In1.Cu" signal) (2 "B.Cu" signal))
          (zone (net "GND") (layer "In1.Cu")
            (polygon (pts (xy 0 0) (xy 10 0) (xy 10 10) (xy 0 10)))
            (filled_polygon (layer "In1.Cu") (pts (xy 0 0) (xy 10 0) (xy 10 10) (xy 0 10))))
        )"#;
        let b = Board::parse(src).expect("parse KiCad 10 zone");
        assert_eq!(b.fills.len(), 1);
        assert_eq!(b.fills[0].net, "GND");
        assert_eq!(b.fills[0].layer, "In1.Cu");
    }
}
