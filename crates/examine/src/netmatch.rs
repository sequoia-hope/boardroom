//! Does a netlist describe this board? A schematic's netlist is only worth reading beside a
//! board (for pin functions, part values, sheets) if it is the netlist the board was laid out
//! from. Net names are no test: old boards and their netlists name unnamed nets differently.
//! What must agree is the grouping: each net as a set of pads, `REF.PIN`.

use std::collections::{BTreeMap, BTreeSet};

use board::pcb::Board;
use intent::netlist::Netlist;
use serde::Serialize;

/// At least this share of the netlist's nets must be the same set of pads on the board for the
/// netlist to count as the board's.
pub const MATCH: f64 = 0.95;

#[derive(Debug, Clone, Serialize)]
pub struct NetMatch {
    /// Parts by reference: in both, only on the board, only in the netlist.
    pub parts_both: usize,
    pub parts_only_on_board: Vec<String>,
    pub parts_only_in_netlist: Vec<String>,
    /// The netlist's nets of two or more pads, and how many are exactly one net's pads on the
    /// board.
    pub nets: usize,
    pub nets_same: usize,
    /// Of the others, the first few by name, with the pads the board lacks from the net or
    /// joins to it.
    pub nets_apart: Vec<NetApart>,
    /// `nets_same / nets`, and whether it reaches [`MATCH`].
    pub share: f64,
    pub matches: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetApart {
    pub net: String,
    pub pads: usize,
    /// Pads of the netlist's net that the board has on another net or does not have at all.
    pub missing: Vec<String>,
    /// Pads the board joins to this net's pads that the netlist does not.
    pub extra: Vec<String>,
}

/// Compares the netlist's nets with the board's, pad for pad.
pub fn compare(netlist: &Netlist, board: &Board) -> NetMatch {
    let on_board: BTreeSet<&str> = board.parts.keys().map(String::as_str).collect();
    let in_netlist: BTreeSet<&str> = netlist.components.keys().map(String::as_str).collect();
    // The board's nets as pad sets, and each pad's net.
    let mut board_nets: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    let mut net_of: BTreeMap<String, &str> = BTreeMap::new();
    for p in board.pads.iter().filter(|p| !p.net.is_empty()) {
        let pad = format!("{}.{}", p.part, p.pin);
        board_nets.entry(&p.net).or_default().insert(pad.clone());
        net_of.insert(pad, &p.net);
    }
    let (mut nets, mut same, mut apart) = (0, 0, Vec::new());
    for (name, net) in &netlist.nets {
        let pads: BTreeSet<String> = net
            .nodes
            .iter()
            .map(|n| format!("{}.{}", n.reference, n.pin))
            .collect();
        if pads.len() < 2 {
            continue;
        }
        nets += 1;
        // The board's net that holds most of this net's pads.
        let mut votes: BTreeMap<&str, usize> = BTreeMap::new();
        for p in &pads {
            if let Some(n) = net_of.get(p) {
                *votes.entry(n).or_default() += 1;
            }
        }
        let best = votes.iter().max_by_key(|(_, c)| **c).map(|(n, _)| *n);
        let there = best.map(|n| &board_nets[n]);
        if there == Some(&pads) {
            same += 1;
        } else if apart.len() < 12 {
            let empty = BTreeSet::new();
            let there = there.unwrap_or(&empty);
            apart.push(NetApart {
                net: name.clone(),
                pads: pads.len(),
                missing: pads.difference(there).take(8).cloned().collect(),
                extra: there.difference(&pads).take(8).cloned().collect(),
            });
        }
    }
    let share = if nets == 0 {
        0.0
    } else {
        same as f64 / nets as f64
    };
    NetMatch {
        parts_both: on_board.intersection(&in_netlist).count(),
        parts_only_on_board: on_board
            .difference(&in_netlist)
            .map(|s| s.to_string())
            .collect(),
        parts_only_in_netlist: in_netlist
            .difference(&on_board)
            .map(|s| s.to_string())
            .collect(),
        nets,
        nets_same: same,
        nets_apart: apart,
        share,
        matches: nets > 0 && share >= MATCH,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOARD: &str = r#"(kicad_pcb (version 20241229) (generator "pcbnew")
      (general (thickness 1.6))
      (layers (0 "F.Cu" signal) (31 "B.Cu" signal))
      (net 0 "") (net 1 "GND") (net 2 "Net-(R1-Pad2)")
      (footprint "R:R" (layer "F.Cu") (at 0 0) (property "Reference" "R1")
        (pad "1" smd rect (at 0 0) (size 0.5 0.5) (layers "F.Cu") (net 1 "GND"))
        (pad "2" smd rect (at 1 0) (size 0.5 0.5) (layers "F.Cu") (net 2 "Net-(R1-Pad2)")))
      (footprint "R:R" (layer "F.Cu") (at 5 0) (property "Reference" "R2")
        (pad "1" smd rect (at 0 0) (size 0.5 0.5) (layers "F.Cu") (net 2 "Net-(R1-Pad2)"))
        (pad "2" smd rect (at 1 0) (size 0.5 0.5) (layers "F.Cu") (net 1 "GND")))
    )"#;

    fn netlist(second: &str) -> Netlist {
        let text = format!(
            r#"(export (version D) (components (comp (ref R1) (value 1k)) (comp (ref R2) (value 1k)))
              (nets (net (code 1) (name GND) (node (ref R1) (pin 1)) (node (ref R2) (pin 2)))
                    (net (code 2) (name "N-000001") {second})))"#
        );
        Netlist::parse(&text).unwrap()
    }

    #[test]
    fn the_same_grouping_matches_whatever_the_nets_are_called() {
        let m = compare(
            &netlist("(node (ref R1) (pin 2)) (node (ref R2) (pin 1))"),
            &Board::parse(BOARD).unwrap(),
        );
        assert_eq!((m.nets, m.nets_same, m.matches), (2, 2, true));
        assert!(m.parts_only_on_board.is_empty() && m.parts_only_in_netlist.is_empty());
    }

    #[test]
    fn a_net_wired_otherwise_is_reported_with_its_pads() {
        let m = compare(
            &netlist("(node (ref R1) (pin 2)) (node (ref R3) (pin 1))"),
            &Board::parse(BOARD).unwrap(),
        );
        assert_eq!((m.nets, m.nets_same, m.matches), (2, 1, false));
        assert_eq!(m.nets_apart[0].missing, ["R3.1"]);
        assert_eq!(m.nets_apart[0].extra, ["R2.1"]);
    }
}
