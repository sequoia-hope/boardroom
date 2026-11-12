//! What each copper layer is used for.

use std::collections::BTreeMap;

use board::pcb::geom;
use board::pcb::Board;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// At least half poured and carrying little of the board's track.
    Plane,
    /// At least half poured and routed on as well.
    PouredSignal,
    Signal,
    Empty,
}

#[derive(Debug, Clone, Serialize)]
pub struct LayerUse {
    pub layer: String,
    pub tracks: usize,
    pub track_mm: f64,
    /// This layer's share of the board's track length.
    pub track_share: f64,
    pub fill_mm2: f64,
    /// Filled copper over the board's area.
    pub fill_share: f64,
    /// The net with the most filled copper here.
    pub fill_net: Option<String>,
    pub role: Role,
}

/// A layer is a plane when a pour covers this share of the board.
pub const POURED: f64 = 0.5;
/// A poured layer is still a plane with up to this share of the track an even split would give
/// it (an escape or a jumper on a plane layer does not make it a signal layer).
pub const PLANE_TRACK: f64 = 0.25;

pub fn layers(board: &Board) -> Vec<LayerUse> {
    let area = super::area(board);
    let total: f64 = board.tracks.iter().map(|t| geom::dist(t.a, t.b)).sum();
    let even = total / board.layers.len().max(1) as f64;
    board
        .layers
        .iter()
        .map(|layer| {
            let on: Vec<_> = board.tracks.iter().filter(|t| &t.layer == layer).collect();
            let track_mm: f64 = on.iter().map(|t| geom::dist(t.a, t.b)).sum();
            let mut by_net: BTreeMap<&str, f64> = BTreeMap::new();
            for f in board.fills.iter().filter(|f| &f.layer == layer) {
                *by_net.entry(f.net.as_str()).or_default() += geom::area(&f.polygon);
            }
            let fill_mm2: f64 = by_net.values().sum();
            let fill_share = fill_mm2 / area;
            let role = if fill_share >= POURED {
                if track_mm <= PLANE_TRACK * even {
                    Role::Plane
                } else {
                    Role::PouredSignal
                }
            } else if on.is_empty() && fill_mm2 == 0.0 {
                Role::Empty
            } else {
                Role::Signal
            };
            LayerUse {
                layer: layer.clone(),
                tracks: on.len(),
                track_mm,
                track_share: if total > 0.0 { track_mm / total } else { 0.0 },
                fill_mm2,
                fill_share,
                fill_net: by_net
                    .iter()
                    .max_by(|a, b| a.1.total_cmp(b.1))
                    .map(|(n, _)| n.to_string()),
                role,
            }
        })
        .collect()
}
