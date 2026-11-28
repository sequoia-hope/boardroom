//! Another router's copper drawn again by this one (owner, 2026-11-26: "have the new router
//! redraw what the old one closes"). The nets asked for are taken off the board, their copper
//! read back as strands of a plan of what is left ([`plan::read`]), and the strands drawn as
//! any are, in 0/45/90 at our widths, each wire put to the checker with everything else on the
//! board as copper that stays.
//!
//! Copper that cannot be read, or whose wire cannot be drawn, is left as it was laid, and so
//! is a net's that drawn again joins less than it did; the rest is read and drawn again round
//! it, and may end on it. What comes back says what was drawn again and what was left, with
//! why.

use std::collections::{HashMap, HashSet};

use board::{Board, ItemId};
use plan::Plan;

use crate::{draw, onto, Drawing};

/// What [`redraw`] made of a board.
pub struct Redrawn {
    /// The board with the copper that stays: all but [`Redrawn::lifted`] (a track of it that
    /// two wires end on at two places as two tracks, end to end).
    pub base: Board,
    /// The plan read from the copper lifted, and its drawing.
    pub plan: Plan,
    pub drawing: Drawing,
    /// The tracks and vias drawn again, as they are numbered on the board given.
    pub lifted: Vec<ItemId>,
    /// What was left as it was laid: how many tracks and vias, of which net, and why.
    pub as_laid: Vec<(usize, u32, String)>,
}

/// How many connections each net has open, pours aside.
fn open(board: &Board) -> HashMap<u32, usize> {
    let mut out = HashMap::new();
    for line in board::connectivity_without_pours(board).airlines {
        *out.entry(line.net).or_insert(0) += 1;
    }
    out
}

/// The tracks and vias `which` of `board` drawn again by this router, as far as they can be.
pub fn redraw(board: &Board, which: &[ItemId]) -> Redrawn {
    let was = open(board);
    let net_of = |id: ItemId| board.item(id).map(|i| i.net);
    let mut stay: HashSet<ItemId> = HashSet::new();
    let mut as_laid: Vec<(usize, u32, String)> = Vec::new();
    loop {
        let lifted: Vec<ItemId> = which.iter().copied().filter(|id| !stay.contains(id)).collect();
        // What is left as laid this round: the copper named, or, where none of it is still
        // off the board, all of its net that is (so that every round leaves something more).
        let mut leave = |stay: &mut HashSet<ItemId>, items: &[ItemId], net: u32, why: String| {
            let mut more: Vec<ItemId> = items.iter().copied().filter(|id| lifted.contains(id) && !stay.contains(id)).collect();
            if more.is_empty() {
                more = lifted.iter().copied().filter(|&id| net_of(id) == Some(net) && !stay.contains(&id)).collect();
            }
            more.sort_unstable();
            more.dedup();
            if !more.is_empty() {
                as_laid.push((more.len(), net, why));
            }
            stay.extend(more);
        };
        let mut base = board.clone();
        let (pieces, vias) = plan::read::lift(&mut base, &lifted);
        let read = match plan::read::read(&mut base, &pieces, &vias) {
            Ok(read) => read,
            Err(unread) => {
                let before = stay.len();
                for u in unread {
                    leave(&mut stay, &u.items, u.net, u.why);
                }
                if stay.len() == before {
                    // Nothing more to leave: as it was laid, all of it.
                    return Redrawn { base: board.clone(), plan: plan::start(board), drawing: Drawing::default(), lifted: Vec::new(), as_laid };
                }
                continue;
            }
        };
        let plan = read.plan;
        let drawing = draw(&base, &plan);
        // REDRAW_WATCH=DIR (for looking into one): the course of each wire that could not
        // be drawn, as DIR/NET-LAYER-STRAND.svg.
        if let Ok(dir) = std::env::var("REDRAW_WATCH") {
            for u in &drawing.undrawn {
                if let Some(svg) = crate::draw_watching(&base, &plan, Some((u.layer, u.strand))).1 {
                    let name = format!("{}-{}-{}.svg", base.net_name(u.net).replace('/', "_"), base.layers[u.layer], u.strand);
                    let _ = std::fs::write(std::path::Path::new(&dir).join(name), svg);
                }
            }
        }
        let before = stay.len();
        for u in &drawing.undrawn {
            let connection = plan.sheets[u.layer].strands[u.strand].connection;
            leave(&mut stay, &read.from[connection], u.net, format!("a track of it could not be drawn: {}", u.why));
        }
        if stay.len() == before {
            let mut nets: Vec<u32> = lifted.iter().filter_map(|&id| net_of(id)).collect();
            nets.sort_unstable();
            nets.dedup();
            let after = onto(&base, &plan, &drawing).0;
            let now_open = open(&after);
            for net in nets.into_iter().filter(|n| now_open.get(n).copied().unwrap_or(0) > was.get(n).copied().unwrap_or(0)) {
                // (REDRAW_WATCH: what is open of it before and after, and its wires.)
                if let Ok(dir) = std::env::var("REDRAW_WATCH") {
                    let lines = |b: &Board| board::connectivity_without_pours(b).airlines.iter().filter(|l| l.net == net).map(|l| format!("  {:.3}, {:.3} to {:.3}, {:.3}\n", l.from.x as f64 / 1e6, l.from.y as f64 / 1e6, l.to.x as f64 / 1e6, l.to.y as f64 / 1e6)).collect::<String>();
                    let wires: String = drawing.wires.iter().filter(|w| w.net == net).map(|w| format!("  {} {:?} to {:?}: {:?}\n", base.layers[w.layer], plan.sheets[w.layer].strands[w.strand].from, plan.sheets[w.layer].strands[w.strand].to, w.pts)).collect();
                    let _ = std::fs::write(std::path::Path::new(&dir).join(format!("joins-less-{}.txt", base.net_name(net).replace('/', "_"))), format!("open before:\n{}open after:\n{}wires:\n{wires}", lines(board), lines(&after)));
                }
                leave(&mut stay, &[], net, "drawn again it joins less than it did".to_string());
            }
        }
        if stay.len() == before {
            return Redrawn { base, plan, drawing, lifted, as_laid };
        }
    }
}
