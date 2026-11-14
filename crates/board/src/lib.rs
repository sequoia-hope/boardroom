//! The routing board model in integer nanometres, built from a KiCad board: copper shapes per
//! layer in a spatial index, connectivity, and the design-rule checks a router must keep.

pub mod model;
/// The board as Composer's checks and generators see it: parts, pads with nets, copper per net,
/// in millimetres (from Circuit Composer's `pcb` crate, API kept).
pub mod pcb;
pub mod check;
pub mod custom;
pub mod connect;
pub mod geom;
pub mod fill;
pub mod spokes;

pub use model::{arc_chords, Block, Board, ZoneArea, FootprintInfo, Item, ItemId, Kind, LoadError, ANY_ONE_NET, DRAWN};
pub use custom::Thing;
pub use check::{check, check_item, check_item_with_pours, intrudes, mask_gap, mask_pair_gap, within, Rule, Violation, MASK_MARGIN};
pub use connect::{anchors, connectivity, connectivity_poured, connectivity_without_pours, joined, net_connectivity, Airline, Connectivity};
pub use fill::{fill_zones, fill_zones_until, Fill, Pours, Starved};
pub use geom::{inside, nm, point_segment, BBox, Nm, Point, Shape};
