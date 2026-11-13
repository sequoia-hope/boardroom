//! One board's routing beside the designer's routing of the same placement: the raw sums of the
//! "like the designers" scoreboard (`docs/one-system-spec.md`).
//!
//! Every angle is measured in a neighbourhood's frame, never in the board's x and y: a board
//! routed on a turned grid is on its grid. Ours is measured twice, against its own frames (is it
//! disciplined at all) and against the designer's (does it follow the grid the designer chose
//! there). Sums, not shares, so a set of boards adds up.

use board::pcb::geom;
use board::pcb::Board;
use serde::Serialize;

use super::layers::Role;
use super::{extract, frames, Extraction};

#[derive(Debug, Clone, Serialize)]
pub struct Side {
    pub track_mm: f64,
    pub vias: usize,
    /// Track on the layers the designer kept as planes, mm.
    pub on_planes_mm: f64,
    /// Track off the 45-degree grid of its own neighbourhood's frame, mm.
    pub off_own_frame_mm: f64,
    /// Track off the grid of the designer's frame there, mm (for the designer, the same).
    pub off_designer_frame_mm: f64,
    /// Track off the grid of the board's x and y, mm: what the old score measured.
    pub off_board_axes_mm: f64,
    /// Track in a bundle (beside another net's), mm.
    pub bundled_mm: f64,
    /// Over the layers with track: length along each layer's own axis, and along either axis.
    pub along_axis_mm: f64,
    pub on_axes_mm: f64,
    /// Parts of three or more pads, and those routed like another of the same footprint.
    pub parts: usize,
    pub repeated: usize,
    /// (layer, track mm), in the board's layer order.
    pub by_layer: Vec<(String, f64)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Like {
    /// The layers the designer kept as planes: half poured and carrying little track.
    pub plane_layers: Vec<String>,
    pub designer: Side,
    pub ours: Side,
}

fn side(board: &Board, x: &Extraction, planes: &[String], designer: &frames::Frames) -> Side {
    let on = |layers: &[String]| -> f64 {
        board
            .tracks
            .iter()
            .filter(|t| layers.contains(&t.layer))
            .map(|t| geom::dist(t.a, t.b))
            .fold(0.0, |a, l| a + l)
    };
    Side {
        track_mm: x.track_mm,
        vias: board.vias.len(),
        on_planes_mm: on(planes),
        off_own_frame_mm: x.frames.off_local_frame * x.track_mm,
        off_designer_frame_mm: frames::off_frames_mm(board, designer),
        off_board_axes_mm: x.frames.off_board_axes * x.track_mm,
        bundled_mm: x.bundles.bundled * x.track_mm,
        along_axis_mm: x
            .frames
            .runs
            .iter()
            .map(|r| r.axis_mm[0].max(r.axis_mm[1]))
            .sum(),
        on_axes_mm: x
            .frames
            .runs
            .iter()
            .map(|r| r.axis_mm[0] + r.axis_mm[1])
            .sum(),
        parts: x.repeats.parts,
        repeated: x.repeats.repeated,
        by_layer: x
            .layers
            .iter()
            .map(|l| (l.layer.clone(), l.track_mm))
            .collect(),
    }
}

/// `ours` beside `designer`: two routings of one placement.
pub fn like(designer: &Board, ours: &Board) -> Like {
    let (theirs, mine) = (extract(designer), extract(ours));
    let plane_layers: Vec<String> = theirs
        .layers
        .iter()
        .filter(|l| l.role == Role::Plane)
        .map(|l| l.layer.clone())
        .collect();
    Like {
        designer: side(designer, &theirs, &plane_layers, &theirs.frames),
        ours: side(ours, &mine, &plane_layers, &theirs.frames),
        plane_layers,
    }
}
