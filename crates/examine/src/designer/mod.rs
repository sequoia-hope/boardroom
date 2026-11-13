//! What a designer did on a routed board, read from the copper alone: which layers are planes,
//! which way the routing runs in each neighbourhood (its frame), which nets run side by side
//! (bundles), and which parts carry the same local routing (repeats).
//!
//! These are the measures the "like the designers" scoreboard is built from
//! (`docs/one-system-spec.md`). Each has a picture in [`draw`], because every one of them is a
//! guess about intent and is to be looked at before it is counted.

pub mod bundles;
pub mod draw;
pub mod frames;
pub mod layers;
pub mod like;
pub mod repeats;

use board::pcb::geom::{self, P};
use board::pcb::Board;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Extraction {
    /// Board area used for shares, mm2: the outline's, or without one the box of all the copper.
    pub area_mm2: f64,
    pub track_mm: f64,
    pub layers: Vec<layers::LayerUse>,
    pub frames: frames::Frames,
    pub bundles: bundles::Bundles,
    pub repeats: repeats::Repeats,
}

pub fn extract(board: &Board) -> Extraction {
    Extraction {
        area_mm2: area(board),
        track_mm: board.tracks.iter().map(|t| geom::dist(t.a, t.b)).sum(),
        layers: layers::layers(board),
        frames: frames::frames(board),
        bundles: bundles::bundles(board),
        repeats: repeats::repeats(board),
    }
}

/// The box of everything placed, drawn or poured, mm.
pub fn extent(board: &Board) -> (P, P) {
    let pts = board
        .outline
        .iter()
        .flatten()
        .copied()
        .chain(board.pads.iter().map(|p| p.centre))
        .chain(board.tracks.iter().flat_map(|t| [t.a, t.b]))
        .chain(board.vias.iter().map(|v| v.at))
        .chain(board.fills.iter().flat_map(|f| f.polygon.iter().copied()));
    geom::bbox(pts).unwrap_or(([0.0, 0.0], [1.0, 1.0]))
}

pub fn area(board: &Board) -> f64 {
    match &board.outline {
        Some(o) if geom::area(o) > 0.0 => geom::area(o),
        _ => {
            let (lo, hi) = extent(board);
            ((hi[0] - lo[0]) * (hi[1] - lo[1])).max(1e-9)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use board::pcb::geom::{self, P};
    use board::pcb::{Board, Fill, Pad, Part, Track};

    use super::layers::Role;

    fn board(tracks: Vec<Track>) -> Board {
        Board {
            layers: vec!["F.Cu".into(), "B.Cu".into()],
            parts: BTreeMap::new(),
            pads: vec![],
            tracks,
            vias: vec![],
            fills: vec![],
            rules: None,
            outline: Some(vec![[0.0, 0.0], [40.0, 0.0], [40.0, 40.0], [0.0, 40.0]]),
        }
    }

    fn track(a: P, b: P, net: &str) -> Track {
        Track {
            a,
            b,
            width: 0.2,
            layer: "F.Cu".into(),
            net: net.into(),
            locked: false,
        }
    }

    /// `n` parallel tracks `pitch` apart, 20 mm long, turned by `deg` about the board's middle.
    fn bus(n: usize, pitch: f64, deg: f64) -> Vec<Track> {
        let turn = |p: P| geom::add(geom::rotate(geom::sub(p, [20.0, 20.0]), deg), [20.0, 20.0]);
        (0..n)
            .map(|k| {
                track(
                    turn([10.0, 15.0 + k as f64 * pitch]),
                    turn([30.0, 15.0 + k as f64 * pitch]),
                    &format!("D{k}"),
                )
            })
            .collect()
    }

    #[test]
    fn a_turned_bus_is_off_the_boards_axes_and_on_its_own() {
        let x = super::extract(&board(bus(4, 0.4, 20.0)));
        assert!(
            x.frames.off_board_axes > 0.99,
            "{}",
            x.frames.off_board_axes
        );
        assert!(
            x.frames.off_local_frame < 0.01,
            "{}",
            x.frames.off_local_frame
        );
        assert!(x.frames.turned > 0.99);
        // KiCad's positive rotation is counter-clockwise on screen, a negative heading with Y down.
        assert!(
            (x.frames.board_deg + 20.0).abs() < 0.1,
            "{}",
            x.frames.board_deg
        );
        let straight = super::extract(&board(bus(4, 0.4, 0.0)));
        assert!(straight.frames.off_board_axes < 0.01 && straight.frames.turned < 0.01);
    }

    #[test]
    fn a_turned_layer_runs_along_its_own_axis() {
        // A bus turned 20 degrees on the front, the same turned a further 90 on the back.
        let mut tracks = bus(4, 0.4, 20.0);
        tracks.extend(bus(4, 0.4, 110.0).into_iter().map(|t| Track {
            layer: "B.Cu".into(),
            ..t
        }));
        let f = super::extract(&board(tracks)).frames;
        assert_eq!(f.runs.len(), 2);
        let (front, back) = (&f.runs[0], &f.runs[1]);
        assert_eq!(
            (front.layer.as_str(), back.layer.as_str()),
            ("F.Cu", "B.Cu")
        );
        assert!(
            front.axis_mm[0] > 79.0 && front.axis_mm[1] < 0.1,
            "{front:?}"
        );
        assert!(back.axis_mm[1] > 79.0 && back.axis_mm[0] < 0.1, "{back:?}");
        assert!(f.along_axis() > 0.99);
        // Both directions on one layer: no layer keeps to an axis.
        let mut mixed = bus(4, 0.4, 20.0);
        mixed.extend(bus(4, 0.4, 110.0));
        assert!(super::extract(&board(mixed)).frames.along_axis() < 0.51);
    }

    #[test]
    fn ours_is_measured_in_the_designers_frame() {
        let designer = board(bus(4, 0.4, 20.0));
        // Ours on the board's axes: disciplined in its own frame, off the designer's.
        let ours = board(bus(4, 0.4, 0.0));
        let l = super::like::like(&designer, &ours);
        assert!(l.designer.off_designer_frame_mm < 0.1);
        assert!(l.designer.off_board_axes_mm > 79.0);
        assert!(l.ours.off_own_frame_mm < 0.1);
        assert!(l.ours.off_designer_frame_mm > 79.0, "{:?}", l.ours);
        // Ours on the designer's grid, on the back: on it, though off the board's axes.
        let same: Vec<Track> = bus(4, 0.4, 20.0)
            .into_iter()
            .map(|t| Track {
                layer: "B.Cu".into(),
                ..t
            })
            .collect();
        let l = super::like::like(&designer, &board(same));
        assert!(l.ours.off_designer_frame_mm < 0.1);
    }

    #[test]
    fn track_on_the_designers_plane_is_counted() {
        let mut designer = board(bus(4, 0.4, 0.0));
        designer.fills.push(Fill {
            net: "GND".into(),
            layer: "B.Cu".into(),
            polygon: vec![[1.0, 1.0], [39.0, 1.0], [39.0, 39.0], [0.0, 39.0]],
        });
        let mut ours = board(bus(4, 0.4, 0.0));
        ours.tracks[3].layer = "B.Cu".into();
        let l = super::like::like(&designer, &ours);
        assert_eq!(l.plane_layers, ["B.Cu"]);
        assert!(l.designer.on_planes_mm == 0.0 && (l.ours.on_planes_mm - 20.0).abs() < 1e-9);
    }

    #[test]
    fn tracks_at_every_angle_agree_on_no_frame() {
        let fan = (0..36)
            .map(|k| {
                track(
                    [20.0, 20.0],
                    geom::add([20.0, 20.0], geom::rotate([15.0, 0.0], k as f64 * 5.0)),
                    &format!("N{k}"),
                )
            })
            .collect();
        let x = super::extract(&board(fan));
        assert!(
            x.frames.board_agreement < 0.1,
            "{}",
            x.frames.board_agreement
        );
        assert!(x.frames.turned < 0.01);
    }

    #[test]
    fn packed_tracks_are_one_bundle_at_any_angle() {
        for deg in [0.0, 20.0, 45.0] {
            let x = super::extract(&board(bus(5, 0.4, deg)));
            assert_eq!(x.bundles.bundles.len(), 1, "{deg}");
            assert_eq!(x.bundles.bundles[0].nets.len(), 5);
            assert!(x.bundles.bundled > 0.99);
        }
    }

    #[test]
    fn a_bus_at_a_pitch_is_a_bundle_unless_something_is_between() {
        // 2 mm apart, 20 mm long: at a pitch.
        let x = super::extract(&board(bus(3, 2.0, 0.0)));
        assert_eq!(
            x.bundles
                .bundles
                .iter()
                .map(|b| b.nets.len())
                .collect::<Vec<_>>(),
            [3]
        );
        // The outer two are too far apart to be at a pitch, and the middle one, cut short, runs
        // beside neither for long enough to join them.
        let mut t = bus(3, 1.5, 0.0);
        t[1].b = [12.0, t[1].a[1]];
        let x = super::extract(&board(t));
        assert!(x.bundles.bundles.is_empty(), "{:?}", x.bundles.bundles);
        // Two nets a long way apart are not a bundle.
        let x = super::extract(&board(bus(2, 3.0, 0.0)));
        assert!(x.bundles.bundles.is_empty());
    }

    #[test]
    fn a_bundle_follows_its_nets_round_a_bend() {
        let mut t = Vec::new();
        for k in 0..3 {
            let d = k as f64 * 0.4;
            t.push(track(
                [5.0, 10.0 + d],
                [20.0 - d, 10.0 + d],
                &format!("D{k}"),
            ));
            t.push(track(
                [20.0 - d, 10.0 + d],
                [20.0 - d, 30.0],
                &format!("D{k}"),
            ));
        }
        let x = super::extract(&board(t));
        assert_eq!(x.bundles.bundles.len(), 1);
        assert_eq!(x.bundles.bundles[0].tracks.len(), 6);
    }

    #[test]
    fn a_poured_layer_with_little_track_is_a_plane() {
        let mut b = board(bus(4, 0.4, 0.0));
        b.fills.push(Fill {
            net: "GND".into(),
            layer: "B.Cu".into(),
            polygon: vec![[1.0, 1.0], [39.0, 1.0], [39.0, 39.0], [1.0, 39.0]],
        });
        let l = super::layers::layers(&b);
        assert_eq!((l[0].role, l[1].role), (Role::Signal, Role::Plane));
        assert_eq!(l[1].fill_net.as_deref(), Some("GND"));
        // Routed on as well, it is poured and routed.
        let more: Vec<Track> = bus(4, 0.4, 0.0)
            .into_iter()
            .map(|t| Track {
                layer: "B.Cu".into(),
                ..t
            })
            .collect();
        b.tracks.extend(more);
        assert_eq!(super::layers::layers(&b)[1].role, Role::PouredSignal);
    }

    /// A three-pad part at `at`, turned by `deg`, each pad routed straight out 3 mm; the third
    /// pad's track is bent when `odd`.
    fn cell(b: &mut Board, reference: &str, at: P, deg: f64, odd: bool) {
        let place = |p: P| geom::add(geom::rotate(p, deg), at);
        let mut pads = Vec::new();
        for (k, x) in [-1.0, 0.0, 1.0].into_iter().enumerate() {
            let net = format!("{reference}_{k}");
            pads.push(b.pads.len());
            b.pads.push(Pad {
                part: reference.into(),
                pin: (k + 1).to_string(),
                net: net.clone(),
                layers: vec!["F.Cu".into()],
                centre: place([x, 0.0]),
                shape: vec![],
            });
            let end = if odd && k == 2 {
                [x + 3.0, 0.0]
            } else {
                [x, 3.0]
            };
            b.tracks.push(track(place([x, 0.0]), place(end), &net));
        }
        b.parts.insert(
            reference.into(),
            Part {
                reference: reference.into(),
                value: String::new(),
                footprint: "SOT-23".into(),
                at,
                rotation: deg,
                back: false,
                locked: false,
                courtyard: None,
                pads,
            },
        );
    }

    #[test]
    fn parts_routed_alike_repeat_whatever_their_rotation() {
        let mut b = board(vec![]);
        cell(&mut b, "Q1", [8.0, 8.0], 0.0, false);
        cell(&mut b, "Q2", [20.0, 8.0], 90.0, false);
        cell(&mut b, "Q3", [30.0, 30.0], 215.0, false);
        cell(&mut b, "Q4", [10.0, 30.0], 0.0, true);
        let r = super::repeats::repeats(&b);
        assert_eq!(r.repeats.len(), 1, "{:?}", r.repeats);
        assert_eq!(r.repeats[0].members, ["Q1", "Q2", "Q3"]);
        assert_eq!((r.parts, r.repeated), (4, 3));
    }
}
